<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox, ElTable } from "element-plus";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import type {
  PipelineProductDetailSku,
  PipelineProductDetailView,
  PipelineProductView,
} from "../../types/app";
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
  recollectProduct,
  republishProduct,
  recollectProducts,
  republishProducts,
  confirmWithCategory,
  searchCategories,
  categoryOptions,
  categorySearching,
  importExcel,
  addPublishTargets,
  loadProductDetail,
} = usePipeline(command);

// 采集明细抽屉：点商品标题按需拉取 collected_data 展示（主图/SKU/详情图）
const detailDrawerVisible = ref(false);
const detailProduct = ref<PipelineProductDetailView | null>(null);
const detailLoading = ref(false);
const detailImagesExpanded = ref(false);

async function openDetailDrawer(row: PipelineProductView) {
  detailDrawerVisible.value = true;
  detailProduct.value = null;
  detailImagesExpanded.value = false;
  detailLoading.value = true;
  detailProduct.value = await loadProductDetail(row.id);
  detailLoading.value = false;
}

/** 把 SKU 规格对象拼成可读文本（尺码: xxx / 身高: yyy），无规格回退 external_sku_id。 */
function skuSpecText(sku: PipelineProductDetailSku): string {
  if (sku.specs && typeof sku.specs === "object") {
    const parts = Object.entries(sku.specs).map(
      ([key, value]) => `${key}: ${String(value)}`,
    );
    if (parts.length > 0) return parts.join(" / ");
  }
  return sku.external_sku_id || "—";
}

/** 淘宝商品参数（taobao_item_params）键值对，供抽屉展示。 */
const itemParamEntries = computed<[string, unknown][]>(() => {
  const params = detailProduct.value?.item_params;
  return params && typeof params === "object" ? Object.entries(params) : [];
});

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

/** 可重新采集：已采集过且非进行中/非全上架的稳定态（待铺货/待确认/失败）。 */
function canRecollect(row: PipelineProductView) {
  return (
    row.status === "collected" ||
    row.status === "need_confirm" ||
    row.status === "error"
  );
}

/** 可重新铺货：有店铺货失败（blocked）且非「待确认」。need_confirm 的失败店要走「确认」补
 *  类目/属性，直接重推类目没解决会再次失败、形成无效循环，故不在此开放（只对真失败开放）。 */
function canRepublish(row: PipelineProductView) {
  return row.failed_shops > 0 && !row.can_confirm;
}

/** 单个重新采集（破坏性，二次确认）。 */
async function onRecollect(productId: string) {
  try {
    await ElMessageBox.confirm(
      "重新采集会清空已采集/审查结果并重走流水线，已上架的店保持不动。确认重新采集？",
      "重新采集",
      { type: "warning", confirmButtonText: "重新采集", cancelButtonText: "取消" },
    );
  } catch {
    return; // 用户取消
  }
  await recollectProduct(productId);
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

// 表格多选：可铺货/可重采/可重铺的商品都能勾选，供批量操作。
// el-table 配 row-key + reserve-selection，3 秒轮询整表替换数据时按 id 保留勾选，
// 不再因对象引用失效而几秒后自动取消全选；选中集由 selection-change 单一维护。
const tableRef = ref<InstanceType<typeof ElTable>>();
const selectedRows = ref<PipelineProductView[]>([]);
function onSelectionChange(rows: PipelineProductView[]) {
  selectedRows.value = rows;
}
function isRowSelectable(row: PipelineProductView) {
  return canAddTargets(row.status) || canRecollect(row) || canRepublish(row);
}

// 勾选集按可执行动作分组，顶部据此显示对应批量按钮（同一批勾选可含不同状态商品）
const selectedAddable = computed(() =>
  selectedRows.value.filter((r) => canAddTargets(r.status)),
);
const selectedRecollectable = computed(() =>
  selectedRows.value.filter((r) => canRecollect(r)),
);
const selectedRepublishable = computed(() =>
  selectedRows.value.filter((r) => canRepublish(r)),
);

/** 批量重新铺货：重推所有勾选中铺货失败的店。 */
async function onBatchRepublish() {
  await republishProducts(selectedRepublishable.value.map((r) => r.id));
  tableRef.value?.clearSelection();
}

/** 批量重新采集（破坏性，二次确认）。 */
async function onBatchRecollect() {
  const ids = selectedRecollectable.value.map((r) => r.id);
  try {
    await ElMessageBox.confirm(
      `确认对 ${ids.length} 个商品重新采集？将清空已采集/审查结果重走流水线，已上架的店保持不动。`,
      "批量重新采集",
      { type: "warning", confirmButtonText: "重新采集", cancelButtonText: "取消" },
    );
  } catch {
    return;
  }
  await recollectProducts(ids);
  tableRef.value?.clearSelection();
}

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
    tableRef.value?.clearSelection();
  }
}

// 确认抽屉：need_confirm 商品在此查看详情、可改标题/选类目后确认进入铺货；
// 没店的「只采集」商品在此补选店，选店→搜该店类目→确认即建店铺货。
const confirmDrawerVisible = ref(false);
const confirmProduct = ref<PipelineProductView | null>(null);
const confirmTitle = ref("");
const confirmCategoryKeyword = ref("");
const confirmCategoryKey = ref("");
const confirmPickedShopId = ref<string>("");
const confirming = ref(false);

// 没有目标店的商品（只采集没选店）需在确认抽屉里补选店后才能确认
const needShopPicker = computed(
  () => (confirmProduct.value?.shops.length ?? 0) === 0,
);
// 类目搜索/校验所用的店：已有店用第一个 target 店，没店则用抽屉里补选的店。
// 补选店刻意单选——微信类目按店确定，多选会出现「类目只在首店有权限、其余店校验不过」必然报错。
const confirmShopId = computed(
  () => confirmProduct.value?.shops[0]?.shop_id ?? confirmPickedShopId.value,
);

/** 淘宝类目路径（形如「童装/婴儿装/亲子装>T恤」）取最末一级作微信类目搜索词。 */
function lastCategorySegment(path: string): string {
  const parts = path
    .split(/[>/]/)
    .map((s) => s.trim())
    .filter(Boolean);
  return parts[parts.length - 1] ?? "";
}

function openConfirmDrawer(product: PipelineProductView) {
  confirmProduct.value = product;
  confirmTitle.value = product.title;
  // 预填类目候选：用淘宝原始类目末级作搜索词，少打字
  confirmCategoryKeyword.value = lastCategorySegment(product.category_path);
  confirmCategoryKey.value = "";
  confirmPickedShopId.value = "";
  categoryOptions.value = [];
  confirmDrawerVisible.value = true;
  // 已有目标店的商品立即按预填词搜一次类目候选；没店的等补选店后由 watch 触发
  if (confirmShopId.value && confirmCategoryKeyword.value) {
    void doSearchCategories();
  }
}

async function doSearchCategories() {
  if (!confirmShopId.value) {
    ElMessage.warning(
      needShopPicker.value ? "请先在上方选择目标小店" : "该商品没有目标店铺",
    );
    return;
  }
  await searchCategories(confirmShopId.value, confirmCategoryKeyword.value);
}

// 没店商品在抽屉里补选店后，自动按预填词搜该店类目候选（换店亦重搜）
watch(confirmShopId, (shopId) => {
  if (!confirmDrawerVisible.value || !needShopPicker.value || !shopId) return;
  confirmCategoryKey.value = "";
  if (confirmCategoryKeyword.value) void doSearchCategories();
});

async function onConfirm() {
  if (!confirmProduct.value) return;
  // 没店的商品确认时必须补选店（类目须按真实目标店确定）
  if (needShopPicker.value && !confirmPickedShopId.value) {
    ElMessage.warning("请先选择要铺货到的微信小店");
    return;
  }
  const selected = categoryOptions.value.find(
    (o) => o.category_ids.join("/") === confirmCategoryKey.value,
  );
  // 没店商品的 reviewed_data 还没类目，必须在此选定叶子类目，否则后端校验会拦截
  if (needShopPicker.value && !selected) {
    ElMessage.warning("请搜索并选择一个微信叶子类目");
    return;
  }
  confirming.value = true;
  const ok = await confirmWithCategory(confirmProduct.value.id, {
    title: confirmTitle.value.trim() || undefined,
    categoryIds: selected?.category_ids,
    categoryPath: selected?.category_path,
    targetShopIds: needShopPicker.value ? [confirmPickedShopId.value] : undefined,
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
            v-if="selectedAddable.length > 0"
            type="success"
            @click="openShopPickerDrawer(selectedAddable.map((r) => r.id))"
          >
            批量铺货到…（{{ selectedAddable.length }}）
          </el-button>
          <el-button
            v-if="selectedRepublishable.length > 0"
            type="warning"
            @click="onBatchRepublish"
          >
            批量重新铺货（{{ selectedRepublishable.length }}）
          </el-button>
          <el-button
            v-if="selectedRecollectable.length > 0"
            @click="onBatchRecollect"
          >
            批量重新采集（{{ selectedRecollectable.length }}）
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
        ref="tableRef"
        :data="filteredProducts"
        class="dense-table"
        row-key="id"
        @selection-change="onSelectionChange"
      >
        <el-table-column
          type="selection"
          width="48"
          :selectable="isRowSelectable"
          reserve-selection
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
              <a class="product-title-link" @click="openDetailDrawer(row)">
                {{ row.title }}
              </a>
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

        <el-table-column label="操作" width="340" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="row.can_confirm"
              size="small"
              type="primary"
              @click="openConfirmDrawer(row)"
            >
              确认
            </el-button>
            <el-button
              v-if="canRepublish(row)"
              size="small"
              type="warning"
              @click="republishProduct(row.id)"
            >
              重新铺货
            </el-button>
            <el-button
              v-if="canAddTargets(row.status)"
              size="small"
              type="success"
              @click="openShopPickerDrawer([row.id])"
            >
              选店铺货
            </el-button>
            <el-button
              v-if="canRecollect(row)"
              size="small"
              @click="onRecollect(row.id)"
            >
              重新采集
            </el-button>
            <span
              v-if="
                !row.can_confirm &&
                !canRepublish(row) &&
                !canAddTargets(row.status) &&
                !canRecollect(row)
              "
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

        <div v-if="needShopPicker" class="confirm-section">
          <span class="step-title">目标小店（必选 · 单店）</span>
          <el-select
            v-model="confirmPickedShopId"
            placeholder="该商品当时只采集没选店，请选择要铺货到的微信小店"
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
            类目按所选店确定，故此处单选；确认后直接建店铺货。要一次铺到多个店，请用列表的「选店铺货」。
          </p>
        </div>

        <div class="confirm-section">
          <span class="step-title">
            微信类目{{ needShopPicker ? "（必选）" : "（不选则沿用 AI 已选）" }}
          </span>
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

    <el-drawer
      v-model="detailDrawerVisible"
      title="采集明细"
      size="600px"
      append-to-body
    >
      <div v-loading="detailLoading" class="detail-drawer">
        <template v-if="detailProduct">
          <div v-if="detailProduct.images.length" class="detail-images">
            <el-image
              v-for="(img, i) in detailProduct.images"
              :key="i"
              :src="img"
              :preview-src-list="detailProduct.images"
              :initial-index="i"
              fit="cover"
              class="detail-thumb"
            />
          </div>

          <h3 class="detail-title">{{ detailProduct.title }}</h3>
          <a
            :href="detailProduct.source_url"
            target="_blank"
            class="muted product-link"
          >
            {{ detailProduct.source_url }}
          </a>

          <el-descriptions :column="2" size="small" border class="detail-desc">
            <el-descriptions-item label="供应商">
              {{ detailProduct.supplier_name || "—" }}
            </el-descriptions-item>
            <el-descriptions-item label="品牌">
              {{ detailProduct.brand_hint || "—" }}
            </el-descriptions-item>
            <el-descriptions-item label="重量">
              {{ detailProduct.weight_gram ? `${detailProduct.weight_gram}g` : "—" }}
            </el-descriptions-item>
            <el-descriptions-item label="SKU 数">
              {{ detailProduct.skus.length }}
            </el-descriptions-item>
            <el-descriptions-item label="类目" :span="2">
              {{ detailProduct.category_path || detailProduct.category_hint || "—" }}
            </el-descriptions-item>
          </el-descriptions>

          <template v-if="itemParamEntries.length">
            <div class="detail-section-title">
              商品参数（{{ itemParamEntries.length }}）
            </div>
            <el-descriptions :column="2" size="small" border class="detail-desc">
              <el-descriptions-item
                v-for="[key, value] in itemParamEntries"
                :key="key"
                :label="key"
              >
                {{ String(value) }}
              </el-descriptions-item>
            </el-descriptions>
          </template>

          <div class="detail-section-title">SKU（{{ detailProduct.skus.length }}）</div>
          <el-table
            :data="detailProduct.skus"
            size="small"
            border
            max-height="320"
            class="detail-sku-table"
          >
            <el-table-column label="规格" min-width="200">
              <template #default="{ row }">{{ skuSpecText(row) }}</template>
            </el-table-column>
            <el-table-column label="成本价" width="90" align="right">
              <template #default="{ row }">¥{{ row.cost_price }}</template>
            </el-table-column>
            <el-table-column label="库存" width="70" align="right" prop="stock" />
          </el-table>

          <div
            v-if="detailProduct.detail_images.length"
            class="detail-section-title"
          >
            <el-button
              text
              type="primary"
              @click="detailImagesExpanded = !detailImagesExpanded"
            >
              {{ detailImagesExpanded ? "收起" : "查看" }}详情图（{{
                detailProduct.detail_images.length
              }}）
            </el-button>
          </div>
          <div v-if="detailImagesExpanded" class="detail-long-images">
            <el-image
              v-for="(img, i) in detailProduct.detail_images"
              :key="i"
              :src="img"
              :preview-src-list="detailProduct.detail_images"
              :initial-index="i"
              fit="contain"
              loading="lazy"
              class="detail-long-img"
            />
          </div>
        </template>
        <el-empty v-else-if="!detailLoading" description="暂无采集明细" />
      </div>
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
.product-title-link {
  font-weight: 600;
  color: #2563eb;
  cursor: pointer;
  text-decoration: none;
}
.product-title-link:hover {
  text-decoration: underline;
}
.detail-drawer {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 200px;
}
.detail-images {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.detail-thumb {
  width: 96px;
  height: 96px;
  border-radius: 6px;
  border: 1px solid #ebe8e0;
}
.detail-title {
  margin: 4px 0 0;
  font-size: 15px;
  line-height: 1.4;
}
.detail-desc {
  margin-top: 4px;
}
.detail-section-title {
  margin-top: 4px;
  font-weight: 600;
  font-size: 13px;
  color: #6c685e;
}
.detail-long-images {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.detail-long-img {
  width: 100%;
  border-radius: 4px;
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
