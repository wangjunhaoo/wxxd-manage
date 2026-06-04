<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import type { PipelineProductView } from "../../types/app";
import { usePipeline } from "../../composables/usePipeline";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  command,
  shops,
  collectionFilePath,
  collectionFileName,
  selectCollectionExcelFile,
  clearCollectionExcelFile,
  triggerTaobaoLogin,
  checkTaobaoLoginState,
  formatCents,
  publishPricingDialogVisible,
  publishPricingForm,
  publishPricingSummary,
  publishPricingSaving,
  openPublishPricingDialog,
  savePublishPricingStrategyFromForm,
  computeSalePriceCents,
  UploadFilled,
  Refresh,
} = props.ctx;

const {
  pipelineProducts,
  pipelineLoading,
  refreshPipeline,
  startPipelinePolling,
  retryProduct,
  confirmWithCategory,
  searchCategories,
  categoryOptions,
  categorySearching,
  importExcel,
  addPublishTargets,
} = usePipeline(command);

const importDialogVisible = ref(false);
const importTargetShopIds = ref<string[]>([]);
const importing = ref(false);
const statusFilter = ref<string>("all");

const STATUS_META: Record<
  string,
  { label: string; tone: "info" | "warning" | "primary" | "success" | "danger" }
> = {
  pending_collect: { label: "待采集", tone: "info" },
  collecting: { label: "采集/审查中", tone: "info" },
  collected: { label: "待铺货", tone: "warning" },
  need_confirm: { label: "待确认", tone: "warning" },
  publishing: { label: "铺货中", tone: "primary" },
  listed: { label: "已上架", tone: "success" },
  error: { label: "异常", tone: "danger" },
};

/** 可补选店铺货的状态：待铺货（已采集审查完成）或仍在采集/审查中（提前补店）。 */
function canAddTargets(status: string) {
  return (
    status === "collected" ||
    status === "pending_collect" ||
    status === "collecting"
  );
}

function statusLabel(s: string) {
  return STATUS_META[s]?.label ?? s;
}
function statusTone(s: string) {
  return STATUS_META[s]?.tone ?? "info";
}

const stats = computed(() => {
  const c: Record<string, number> = {
    all: pipelineProducts.value.length,
    collecting: 0,
    collected: 0,
    need_confirm: 0,
    publishing: 0,
    listed: 0,
    error: 0,
  };
  for (const p of pipelineProducts.value) {
    if (p.status === "pending_collect" || p.status === "collecting") c.collecting += 1;
    else if (c[p.status] !== undefined) c[p.status] += 1;
  }
  return c;
});

const overviewItems = computed(() => [
  { key: "all", label: "全部", value: stats.value.all },
  { key: "collecting", label: "采集中", value: stats.value.collecting },
  { key: "collected", label: "待铺货", value: stats.value.collected },
  { key: "need_confirm", label: "待确认", value: stats.value.need_confirm },
  { key: "publishing", label: "铺货中", value: stats.value.publishing },
  { key: "listed", label: "已上架", value: stats.value.listed },
  { key: "error", label: "异常", value: stats.value.error },
]);

const filteredProducts = computed<PipelineProductView[]>(() => {
  if (statusFilter.value === "all") return pipelineProducts.value;
  if (statusFilter.value === "collecting") {
    return pipelineProducts.value.filter(
      (p) => p.status === "pending_collect" || p.status === "collecting",
    );
  }
  return pipelineProducts.value.filter((p) => p.status === statusFilter.value);
});

function progressPercent(row: PipelineProductView) {
  if (row.total_shops <= 0) return 0;
  return Math.round((row.listed_shops / row.total_shops) * 100);
}

function openImport() {
  importDialogVisible.value = true;
}

async function onImport() {
  if (!collectionFilePath.value.trim()) {
    ElMessage.warning("请先选择 Excel 文件");
    return;
  }
  // 目标店可选：不选店则只采集不铺货，采集完成后可在列表中补选店铺货
  importing.value = true;
  const ok = await importExcel(
    collectionFilePath.value.trim(),
    importTargetShopIds.value,
  );
  importing.value = false;
  if (ok) {
    importDialogVisible.value = false;
    importTargetShopIds.value = [];
    clearCollectionExcelFile();
  }
}

// 表格多选：仅「待铺货 / 采集审查中」的商品可勾选，供批量补铺货
const selectedRows = ref<PipelineProductView[]>([]);
function onSelectionChange(rows: PipelineProductView[]) {
  selectedRows.value = rows;
}
function isRowSelectable(row: PipelineProductView) {
  return canAddTargets(row.status);
}

// 轮询每隔几秒整表替换 pipelineProducts：用最新数据重建选中集，剔除已被推进到
// 不可补货状态（如审查通过转铺货中/已上架）的陈旧勾选，避免批量铺货误操作过期商品。
watch(pipelineProducts, (latest) => {
  if (selectedRows.value.length === 0) return;
  const selectedIds = new Set(selectedRows.value.map((r) => r.id));
  selectedRows.value = latest.filter(
    (p) => selectedIds.has(p.id) && canAddTargets(p.status),
  );
});

// 选店铺货对话框：单个（行内按钮）或批量（勾选后顶部按钮）复用同一对话框
const shopPickerVisible = ref(false);
const shopPickerProductIds = ref<string[]>([]);
const shopPickerShopIds = ref<string[]>([]);
const shopPickerSubmitting = ref(false);

function openShopPickerDrawer(productIds: string[]) {
  if (productIds.length === 0) {
    ElMessage.warning("请先选择要铺货的商品");
    return;
  }
  shopPickerProductIds.value = productIds;
  shopPickerShopIds.value = [];
  shopPickerVisible.value = true;
}

async function onShopPickerConfirm() {
  if (shopPickerShopIds.value.length === 0) {
    ElMessage.warning("请先选择要铺货到的微信小店");
    return;
  }
  shopPickerSubmitting.value = true;
  const ok = await addPublishTargets(
    shopPickerProductIds.value,
    shopPickerShopIds.value,
  );
  shopPickerSubmitting.value = false;
  if (ok) {
    shopPickerVisible.value = false;
    selectedRows.value = [];
  }
}

// 确认抽屉：need_confirm 商品在此查看详情、可改标题/选类目后确认进入铺货
const confirmDrawerVisible = ref(false);
const confirmProduct = ref<PipelineProductView | null>(null);
const confirmTitle = ref("");
const confirmCategoryKeyword = ref("");
const confirmCategoryKey = ref("");
const confirming = ref(false);

const confirmShopId = computed(() => confirmProduct.value?.shops[0]?.shop_id ?? "");

function openConfirmDrawer(product: PipelineProductView) {
  confirmProduct.value = product;
  confirmTitle.value = product.title;
  confirmCategoryKeyword.value = "";
  confirmCategoryKey.value = "";
  categoryOptions.value = [];
  confirmDrawerVisible.value = true;
}

async function doSearchCategories() {
  if (!confirmShopId.value) {
    ElMessage.warning("该商品没有目标店铺");
    return;
  }
  await searchCategories(confirmShopId.value, confirmCategoryKeyword.value);
}

async function onConfirm() {
  if (!confirmProduct.value) return;
  const selected = categoryOptions.value.find(
    (o) => o.category_ids.join("/") === confirmCategoryKey.value,
  );
  confirming.value = true;
  const ok = await confirmWithCategory(confirmProduct.value.id, {
    title: confirmTitle.value.trim() || undefined,
    categoryIds: selected?.category_ids,
    categoryPath: selected?.category_path,
  });
  confirming.value = false;
  if (ok) confirmDrawerVisible.value = false;
}

// ===== 价格策略：全局加价规则（加价倍率 / 固定加价 / 最低售价）=====
// 固定加价、最低售价以「元」交互、底层以「分」存储；加价倍率为直接倍率。
const fixedMarkupYuan = computed({
  get: () => publishPricingForm.value.sale_price_fixed_cents / 100,
  set: (value: number) => {
    publishPricingForm.value.sale_price_fixed_cents = Math.round(
      Number(value || 0) * 100,
    );
  },
});
const floorPriceYuan = computed({
  get: () => publishPricingForm.value.sale_price_floor_cents / 100,
  set: (value: number) => {
    publishPricingForm.value.sale_price_floor_cents = Math.round(
      Number(value || 0) * 100,
    );
  },
});

// 流水线视图不含 SKU 成本，故以可调的「示例成本价」实时预演策略定价效果
const previewCostYuan = ref(10);
const previewSaleCents = computed(() =>
  computeSalePriceCents(previewCostYuan.value || 0, publishPricingForm.value),
);

onMounted(() => startPipelinePolling());
</script>

<template>
  <section class="content-stack publish-workbench">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>商品工作台</h2>
          <p class="muted">
            导入即自动采集 → AI 审查 → 铺货上架；只在「待确认」或「异常」时介入。
          </p>
        </div>
        <div class="button-group">
          <el-button type="primary" :icon="UploadFilled" @click="openImport">
            导入铺货表
          </el-button>
          <el-button
            v-if="selectedRows.length > 0"
            type="success"
            @click="openShopPickerDrawer(selectedRows.map((r) => r.id))"
          >
            批量铺货到…（已选 {{ selectedRows.length }}）
          </el-button>
          <el-dropdown trigger="click">
            <el-button>淘宝采集</el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="triggerTaobaoLogin">
                  淘宝登录（保持采集登录态）
                </el-dropdown-item>
                <el-dropdown-item @click="checkTaobaoLoginState">
                  检测登录状态
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
          <el-tooltip :content="publishPricingSummary" placement="bottom">
            <el-button @click="openPublishPricingDialog">价格策略</el-button>
          </el-tooltip>
          <el-button :icon="Refresh" :loading="pipelineLoading" @click="refreshPipeline">
            刷新
          </el-button>
        </div>
      </div>

      <div class="workbench-overview">
        <button
          v-for="item in overviewItems"
          :key="item.key"
          type="button"
          class="overview-chip"
          :class="{ active: statusFilter === item.key }"
          @click="statusFilter = item.key"
        >
          <span class="overview-label">{{ item.label }}</span>
          <span class="overview-value">{{ item.value }}</span>
        </button>
      </div>
    </div>

    <div class="panel">
      <el-table
        :data="filteredProducts"
        class="dense-table"
        row-key="id"
        @selection-change="onSelectionChange"
      >
        <el-table-column
          type="selection"
          width="48"
          :selectable="isRowSelectable"
        />
        <el-table-column type="expand">
          <template #default="{ row }">
            <div class="target-rows">
              <div v-for="t in row.shops" :key="t.id" class="target-row">
                <strong>{{ t.shop_name }}</strong>
                <el-tag size="small" :type="t.error_code ? 'danger' : 'info'">
                  {{ t.status_text }}
                </el-tag>
                <span v-if="t.error_reason" class="muted">{{ t.error_reason }}</span>
              </div>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="商品" min-width="300" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="product-cell">
              <strong>{{ row.title }}</strong>
              <a :href="row.source_url" target="_blank" class="muted product-link">
                {{ row.source_url }}
              </a>
              <small v-if="row.category_path" class="muted">{{ row.category_path }}</small>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="statusTone(row.status)">{{ statusLabel(row.status) }}</el-tag>
          </template>
        </el-table-column>

        <el-table-column label="进度" width="170">
          <template #default="{ row }">
            <el-progress
              v-if="row.status === 'publishing' || row.status === 'listed'"
              :percentage="progressPercent(row)"
              :status="row.status === 'listed' ? 'success' : undefined"
            />
            <small class="muted">{{ row.progress_text || "—" }}</small>
          </template>
        </el-table-column>

        <el-table-column label="目标店" width="150">
          <template #default="{ row }">
            <span>{{ row.listed_shops }}/{{ row.total_shops }} 已上架</span>
            <span v-if="row.failed_shops > 0" class="danger-text">
              · {{ row.failed_shops }} 异常
            </span>
          </template>
        </el-table-column>

        <el-table-column label="异常 / 原因" min-width="220" show-overflow-tooltip>
          <template #default="{ row }">
            <span v-if="row.error_reason" class="danger-text">{{ row.error_reason }}</span>
            <span v-else class="muted">—</span>
          </template>
        </el-table-column>

        <el-table-column label="操作" width="220" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="row.can_confirm"
              size="small"
              type="primary"
              @click="openConfirmDrawer(row)"
            >
              确认
            </el-button>
            <el-button v-if="row.can_retry" size="small" @click="retryProduct(row.id)">
              重试
            </el-button>
            <el-button
              v-if="canAddTargets(row.status)"
              size="small"
              type="success"
              @click="openShopPickerDrawer([row.id])"
            >
              选店铺货
            </el-button>
            <span
              v-if="!row.can_confirm && !row.can_retry && !canAddTargets(row.status)"
              class="muted"
              >—</span
            >
          </template>
        </el-table-column>

        <template #empty>
          <div class="muted">
            还没有商品。点「导入铺货表」选 Excel 和目标店，导入后自动采集铺货。
          </div>
        </template>
      </el-table>
    </div>

    <el-dialog
      v-model="publishPricingDialogVisible"
      title="铺货价格策略"
      width="560px"
      append-to-body
    >
      <p class="muted small pricing-intro">
        全局生效：所有商品铺货时由采集成本价按此策略计算上架售价；已显式指定售价的 SKU 不受影响。
      </p>
      <el-form label-width="96px" class="pricing-form">
        <el-form-item label="加价倍率">
          <el-input-number
            v-model="publishPricingForm.sale_price_markup_rate"
            :min="0.1"
            :max="100"
            :precision="2"
            :step="0.1"
          />
          <span class="form-suffix">× 成本价</span>
        </el-form-item>
        <el-form-item label="固定加价">
          <el-input-number
            v-model="fixedMarkupYuan"
            :min="0"
            :max="100000"
            :precision="2"
            :step="1"
          />
          <span class="form-suffix">元</span>
        </el-form-item>
        <el-form-item label="最低售价">
          <el-input-number
            v-model="floorPriceYuan"
            :min="0.01"
            :max="100000"
            :precision="2"
            :step="1"
          />
          <span class="form-suffix">元</span>
        </el-form-item>
      </el-form>

      <div class="pricing-preview">
        <div class="pricing-preview-head">
          <strong>定价试算</strong>
          <span class="muted small">{{ publishPricingSummary }}</span>
        </div>
        <div class="pricing-preview-body">
          <span class="muted">示例成本价</span>
          <el-input-number
            v-model="previewCostYuan"
            :min="0"
            :max="100000"
            :precision="2"
            :step="1"
            size="small"
          />
          <span class="muted">元</span>
          <span class="pricing-arrow">→ 上架售价</span>
          <strong class="pricing-result">¥{{ formatCents(previewSaleCents) }}</strong>
        </div>
      </div>

      <template #footer>
        <el-button @click="publishPricingDialogVisible = false">取消</el-button>
        <el-button
          type="primary"
          :loading="publishPricingSaving"
          @click="savePublishPricingStrategyFromForm"
        >
          保存策略
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="importDialogVisible"
      title="导入铺货表"
      width="600px"
      append-to-body
    >
      <div class="import-form">
        <div class="import-step">
          <span class="step-title">1. 选择 Excel 文件</span>
          <div class="excel-picker">
            <el-button type="primary" :icon="UploadFilled" @click="selectCollectionExcelFile">
              选择文件
            </el-button>
            <el-button :disabled="!collectionFilePath" @click="clearCollectionExcelFile">
              清除
            </el-button>
            <span class="muted">{{ collectionFileName || "尚未选择文件" }}</span>
          </div>
          <p class="muted small">
            Excel 无表头，三列：商品名 · 淘宝链接 · 微信类目路径（用 &gt; 连接）。
          </p>
        </div>
        <div class="import-step">
          <span class="step-title">2. 选择本批目标小店（可选）</span>
          <el-select
            v-model="importTargetShopIds"
            multiple
            collapse-tags
            placeholder="不选则仅采集，采集完成后可在列表中补选店铺货"
            style="width: 100%"
          >
            <el-option
              v-for="shop in shops"
              :key="shop.id"
              :label="`${shop.name} (${shop.group_name})`"
              :value="shop.id"
            />
          </el-select>
          <p class="muted small">
            不选店则仅采集不铺货；采集审查完成后状态为「待铺货」，可在列表中补选店铺货。
          </p>
        </div>
      </div>
      <template #footer>
        <el-button @click="importDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="importing" @click="onImport">
          开始导入并采集
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="shopPickerVisible"
      title="选店铺货"
      width="520px"
      append-to-body
    >
      <div class="import-step">
        <span class="step-title">
          为 {{ shopPickerProductIds.length }} 个商品选择目标小店
        </span>
        <el-select
          v-model="shopPickerShopIds"
          multiple
          collapse-tags
          placeholder="选择要铺货到的微信小店"
          style="width: 100%"
        >
          <el-option
            v-for="shop in shops"
            :key="shop.id"
            :label="`${shop.name} (${shop.group_name})`"
            :value="shop.id"
          />
        </el-select>
        <p class="muted small">
          已采集完成的商品选店后直接进入铺货；仍在采集/审查中的商品会在审查通过后自动铺货。
        </p>
      </div>
      <template #footer>
        <el-button @click="shopPickerVisible = false">取消</el-button>
        <el-button
          type="primary"
          :loading="shopPickerSubmitting"
          @click="onShopPickerConfirm"
        >
          确认铺货
        </el-button>
      </template>
    </el-dialog>

    <el-drawer
      v-model="confirmDrawerVisible"
      title="确认审查并进入铺货"
      size="480px"
      append-to-body
    >
      <div v-if="confirmProduct" class="confirm-drawer">
        <div class="confirm-section">
          <span class="step-title">商品</span>
          <strong>{{ confirmProduct.title }}</strong>
          <span v-if="confirmProduct.error_reason" class="danger-text">
            {{ confirmProduct.error_reason }}
          </span>
        </div>

        <div class="confirm-section">
          <span class="step-title">标题（可修改）</span>
          <el-input v-model="confirmTitle" placeholder="商品标题" />
        </div>

        <div class="confirm-section">
          <span class="step-title">微信类目（不选则沿用 AI 已选）</span>
          <div class="category-search">
            <el-input
              v-model="confirmCategoryKeyword"
              placeholder="输入类目关键词，如「连衣裙」"
              @keyup.enter="doSearchCategories"
            />
            <el-button :loading="categorySearching" @click="doSearchCategories">
              搜索
            </el-button>
          </div>
          <el-select
            v-if="categoryOptions.length > 0"
            v-model="confirmCategoryKey"
            placeholder="选择叶子类目"
            style="width: 100%; margin-top: 8px"
          >
            <el-option
              v-for="opt in categoryOptions"
              :key="opt.category_ids.join('/')"
              :label="opt.category_path"
              :value="opt.category_ids.join('/')"
            />
          </el-select>
        </div>

        <div class="confirm-section">
          <span class="step-title">各店进度</span>
          <div v-for="t in confirmProduct.shops" :key="t.id" class="target-row">
            <strong>{{ t.shop_name }}</strong>
            <span class="muted">{{ t.status_text }}</span>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="confirmDrawerVisible = false">取消</el-button>
        <el-button type="primary" :loading="confirming" @click="onConfirm">
          确认并铺货
        </el-button>
      </template>
    </el-drawer>
  </section>
</template>

<style scoped>
.workbench-overview {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 12px;
}
.overview-chip {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 14px;
  border: 1px solid var(--el-border-color, #dcdfe6);
  border-radius: 999px;
  background: transparent;
  cursor: pointer;
  transition: all 0.15s;
}
.overview-chip:hover {
  border-color: var(--el-color-primary, #c38a21);
}
.overview-chip.active {
  border-color: var(--el-color-primary, #c38a21);
  background: rgba(195, 138, 33, 0.1);
}
.overview-label {
  font-size: 13px;
  color: #6c685e;
}
.overview-value {
  font-weight: 600;
  font-size: 15px;
}
.product-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.product-link {
  font-size: 12px;
  text-decoration: none;
}
.target-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 16px;
}
.target-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.muted {
  color: #8c887e;
}
.muted.small {
  font-size: 12px;
}
.danger-text {
  color: #bd4c2f;
}
.import-form {
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.import-step {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.step-title {
  font-weight: 600;
}
.excel-picker {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.button-group {
  display: flex;
  gap: 10px;
}
.confirm-drawer {
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.confirm-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.category-search {
  display: flex;
  gap: 8px;
}
.pricing-intro {
  margin: 0 0 16px;
  line-height: 1.6;
}
.pricing-form {
  margin-bottom: 4px;
}
.form-suffix {
  margin-left: 8px;
  color: #8c887e;
}
.pricing-preview {
  margin-top: 12px;
  padding: 14px 16px;
  background: rgba(195, 138, 33, 0.06);
  border: 1px solid rgba(195, 138, 33, 0.18);
  border-radius: 10px;
}
.pricing-preview-head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 12px;
}
.pricing-preview-body {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.pricing-arrow {
  margin-left: 6px;
  color: #6c685e;
}
.pricing-result {
  color: var(--el-color-primary, #c38a21);
  font-size: 17px;
}
</style>
