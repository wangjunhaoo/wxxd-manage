<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import type {
  WechatShopProductSkuView,
  WechatShopProductView,
} from "../../types/app";
import {
  useShopProducts,
  shopProductStatusLabel,
  shopProductStatusTone,
  isListed,
  canListing,
  isAuditing,
} from "../../composables/useShopProducts";

const props = defineProps<{ ctx: WxXdAppContext }>();
const { command, shops, formatCents, formatDateTime, Refresh, Search } = props.ctx;

const {
  shopProducts,
  total,
  loading,
  syncing,
  refreshingStock,
  selectedShopId,
  statusFilter,
  keyword,
  detailSkus,
  refreshList,
  syncProducts,
  refreshStock,
  loadDetail,
  listingProduct,
  delistingProduct,
  deleteProduct,
  updateStock,
} = useShopProducts(command);

const shopOptions = computed(() =>
  shops.value.map((shop) => ({ value: shop.id, label: shop.name })),
);

/** 状态筛选选项：基于全量商品统计各状态数量，保证切换时选项稳定。 */
const statusOptions = computed(() => {
  const counts = new Map<number, number>();
  for (const product of shopProducts.value) {
    counts.set(product.status, (counts.get(product.status) ?? 0) + 1);
  }
  const options: Array<{ value: number | "all"; label: string }> = [
    { value: "all", label: `全部 (${shopProducts.value.length})` },
  ];
  for (const [status, count] of [...counts.entries()].sort((a, b) => a[0] - b[0])) {
    options.push({ value: status, label: `${shopProductStatusLabel(status)} (${count})` });
  }
  return options;
});

const summary = computed(() => ({
  total: shopProducts.value.length,
  listed: shopProducts.value.filter((product) => isListed(product.status)).length,
  delisted: shopProducts.value.filter((product) => [11, 12, 13, 14, 15].includes(product.status))
    .length,
  auditing: shopProducts.value.filter((product) => isAuditing(product.status)).length,
  zeroStock: shopProducts.value.filter((product) => product.total_stock <= 0).length,
}));

/** 前端过滤：状态 + 关键词（标题/微信商品ID/外部商品ID）。 */
const displayedProducts = computed(() => {
  let list = shopProducts.value;
  if (statusFilter.value !== "all") {
    list = list.filter((product) => product.status === statusFilter.value);
  }
  const kw = keyword.value.trim().toLowerCase();
  if (kw) {
    list = list.filter(
      (product) =>
        product.title.toLowerCase().includes(kw) ||
        product.wechat_product_id.toLowerCase().includes(kw) ||
        (product.out_product_id ?? "").toLowerCase().includes(kw),
    );
  }
  return list;
});

function onShopChange() {
  statusFilter.value = "all";
  keyword.value = "";
  void refreshList();
}

async function onExpandChange(row: WechatShopProductView, expandedRows: WechatShopProductView[]) {
  const expanded = expandedRows.some((item) => item.id === row.id);
  if (expanded && !detailSkus.value[row.id]) {
    await loadDetail(row.id);
  }
}

async function onDelete(product: WechatShopProductView) {
  try {
    await ElMessageBox.confirm(
      `确定删除商品「${product.title}」？此操作会从微信小店彻底删除，无法恢复。`,
      "删除商品",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
    );
  } catch {
    return;
  }
  await deleteProduct(product);
}

// ===== 库存调整对话框 =====
const stockDialogVisible = ref(false);
const stockForm = ref<{
  product: WechatShopProductView | null;
  skuId: string;
  skuLabel: string;
  diffType: number;
  num: number;
  currentStock: number;
}>({ product: null, skuId: "", skuLabel: "", diffType: 1, num: 0, currentStock: 0 });
const stockSubmitting = ref(false);

function skuLabelOf(sku: WechatShopProductSkuView): string {
  if (sku.sku_attrs && sku.sku_attrs !== "null") {
    return `${sku.sku_code ?? sku.sku_id}（${sku.sku_attrs}）`;
  }
  return sku.sku_code ?? sku.sku_id;
}

function openStockDialog(product: WechatShopProductView, sku: WechatShopProductSkuView) {
  stockForm.value = {
    product,
    skuId: sku.sku_id,
    skuLabel: skuLabelOf(sku),
    diffType: 1,
    num: 0,
    currentStock: sku.stock_num ?? 0,
  };
  stockDialogVisible.value = true;
}

const stockPreview = computed(() => {
  const form = stockForm.value;
  if (form.diffType === 1) return form.currentStock + form.num;
  if (form.diffType === 2) return Math.max(0, form.currentStock - form.num);
  return form.num;
});

async function submitStock() {
  const form = stockForm.value;
  if (!form.product) return;
  if (form.num < 0) {
    ElMessage.warning("数量不能为负");
    return;
  }
  stockSubmitting.value = true;
  const ok = await updateStock(form.product, form.skuId, form.diffType, form.num);
  stockSubmitting.value = false;
  if (ok) stockDialogVisible.value = false;
}
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>商品管理</h2>
          <p>按店铺直连微信小店 API，管理真实在售/在审商品：同步、上下架、删除、库存调整。</p>
        </div>
        <div class="button-group">
          <el-button
            type="primary"
            :loading="syncing"
            :disabled="!selectedShopId"
            @click="syncProducts"
          >
            同步商品
          </el-button>
          <el-button
            :loading="refreshingStock"
            :disabled="!selectedShopId"
            @click="refreshStock"
          >
            刷新库存
          </el-button>
          <el-button :icon="Refresh" :disabled="!selectedShopId" @click="refreshList">刷新</el-button>
        </div>
      </div>

      <div class="form-grid">
        <el-select
          v-model="selectedShopId"
          placeholder="选择店铺（必选）"
          filterable
          @change="onShopChange"
        >
          <el-option
            v-for="option in shopOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-select v-model="statusFilter" placeholder="商品状态" :disabled="!selectedShopId">
          <el-option
            v-for="option in statusOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-input
          v-model="keyword"
          placeholder="搜索标题、微信商品 ID 或外部商品 ID"
          clearable
          :prefix-icon="Search"
          :disabled="!selectedShopId"
        />
      </div>

      <dl class="status-list compact">
        <div>
          <dt>商品总数</dt>
          <dd>{{ summary.total }}</dd>
        </div>
        <div>
          <dt>已上架</dt>
          <dd>{{ summary.listed }}</dd>
        </div>
        <div>
          <dt>已下架</dt>
          <dd>{{ summary.delisted }}</dd>
        </div>
        <div>
          <dt>审核中</dt>
          <dd>{{ summary.auditing }}</dd>
        </div>
        <div>
          <dt>零库存</dt>
          <dd>{{ summary.zeroStock }}</dd>
        </div>
      </dl>

      <el-table
        v-loading="loading"
        :data="displayedProducts"
        class="dense-table"
        row-key="id"
        @expand-change="onExpandChange"
      >
        <el-table-column type="expand">
          <template #default="{ row }">
            <div v-if="detailSkus[row.id] && detailSkus[row.id].length > 0" class="sub-panel">
              <el-table :data="detailSkus[row.id]" class="dense-table">
                <el-table-column label="SKU" min-width="200">
                  <template #default="{ row: sku }">
                    <div class="operator-main-cell">
                      <strong>{{ sku.sku_code || sku.sku_id }}</strong>
                      <span v-if="sku.sku_attrs && sku.sku_attrs !== 'null'">{{ sku.sku_attrs }}</span>
                    </div>
                  </template>
                </el-table-column>
                <el-table-column label="售价" width="120">
                  <template #default="{ row: sku }">{{ formatCents(sku.sale_price_cents) }}</template>
                </el-table-column>
                <el-table-column label="库存" width="100">
                  <template #default="{ row: sku }">{{ sku.stock_num ?? "—" }}</template>
                </el-table-column>
                <el-table-column prop="out_sku_id" label="外部 SKU" min-width="140" show-overflow-tooltip />
                <el-table-column label="操作" width="120" fixed="right">
                  <template #default="{ row: sku }">
                    <el-button size="small" link type="primary" @click="openStockDialog(row, sku)">
                      改库存
                    </el-button>
                  </template>
                </el-table-column>
              </el-table>
            </div>
            <div v-else class="empty-state">这个商品没有 SKU 记录。</div>
          </template>
        </el-table-column>

        <el-table-column label="主图" width="70">
          <template #default="{ row }">
            <el-image
              v-if="row.head_img"
              :src="row.head_img"
              fit="cover"
              style="width: 44px; height: 44px; border-radius: 6px"
              lazy
            />
            <span v-else>—</span>
          </template>
        </el-table-column>
        <el-table-column label="商品" min-width="280" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.title }}</strong>
              <span>微信 ID：{{ row.wechat_product_id }}</span>
              <span v-if="row.out_product_id">外部：{{ row.out_product_id }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="shopProductStatusTone(row.status)">
              {{ shopProductStatusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="最低价" width="110">
          <template #default="{ row }">{{ formatCents(row.min_price_cents) }}</template>
        </el-table-column>
        <el-table-column prop="total_stock" label="总库存" width="90" />
        <el-table-column prop="sku_count" label="SKU 数" width="80" />
        <el-table-column label="同步时间" min-width="170">
          <template #default="{ row }">{{ formatDateTime(row.synced_at) }}</template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <div class="button-group compact">
              <el-button
                v-if="canListing(row.status)"
                size="small"
                link
                type="success"
                :disabled="isAuditing(row.status)"
                @click="listingProduct(row)"
              >
                上架
              </el-button>
              <el-button
                v-if="isListed(row.status)"
                size="small"
                link
                type="warning"
                @click="delistingProduct(row)"
              >
                下架
              </el-button>
              <el-button
                size="small"
                link
                type="danger"
                :disabled="isAuditing(row.status)"
                @click="onDelete(row)"
              >
                删除
              </el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <div v-if="!selectedShopId" class="empty-state">请选择店铺后点击「同步商品」拉取微信小店真实商品。</div>
      <p v-else class="muted-hint">共 {{ total }} 个商品，当前显示 {{ displayedProducts.length }} 个。</p>
    </div>

    <el-dialog v-model="stockDialogVisible" title="调整库存" width="420px">
      <el-form label-width="92px">
        <el-form-item label="SKU">
          <span>{{ stockForm.skuLabel }}</span>
        </el-form-item>
        <el-form-item label="当前库存">
          <span>{{ stockForm.currentStock }}</span>
        </el-form-item>
        <el-form-item label="修改方式">
          <el-radio-group v-model="stockForm.diffType">
            <el-radio :value="1">增加</el-radio>
            <el-radio :value="2">减少</el-radio>
            <el-radio :value="3">设置为</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="数量">
          <el-input-number v-model="stockForm.num" :min="0" :step="1" />
        </el-form-item>
        <el-form-item label="调整后">
          <strong>{{ stockPreview }}</strong>
          <span class="muted-hint" style="margin-left: 8px">（高并发下「设置」可能被覆盖，建议用增减）</span>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="stockDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="stockSubmitting" @click="submitStock">确认</el-button>
      </template>
    </el-dialog>
  </section>
</template>
