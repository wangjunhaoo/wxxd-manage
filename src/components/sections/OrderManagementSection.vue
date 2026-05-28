<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  formatCents,
  formatDateTime,
  formatSignedCents,
  formatUnixTime,
  orderManagementItems,
  orderManagementKeyword,
  orderManagementShopFilter,
  orderManagementStatusFilter,
  orderManagementStatusLabel,
  orderManagementStatusOptions,
  orderManagementTotal,
  profitStatusLabel,
  purchaseManagementStatusLabel,
  Refresh,
  refreshOrderManagementItems,
  runOrderDetailSyncOnce,
  runOrderSyncOnce,
  runPurchaseTaskGenerationOnce,
  Search,
  selectedSection,
  shipmentStatusLabel,
  shops,
  statusType,
} = props.ctx;

const shopFilterOptions = computed(() => [
  { value: "all", label: "全部店铺" },
  ...shops.value.map((shop) => ({ value: shop.id, label: shop.name })),
]);

const orderSummary = computed(() => ({
  needsDetail: orderManagementItems.value.filter((item) => item.management_status === "needs_detail").length,
  needsPurchase: orderManagementItems.value.filter((item) => item.management_status === "needs_purchase").length,
  needsShipment: orderManagementItems.value.filter((item) => item.management_status === "needs_shipment").length,
  aftersale: orderManagementItems.value.filter((item) => item.management_status === "aftersale_active").length,
}));

function goTo(section: string) {
  selectedSection.value = section;
}
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>订单管理</h2>
          <p>集中查看订单同步、采购、发货、售后和利润状态。</p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="runOrderSyncOnce">同步待发货</el-button>
          <el-button :icon="Refresh" @click="runOrderDetailSyncOnce">同步详情</el-button>
          <el-button :icon="Refresh" @click="refreshOrderManagementItems">刷新</el-button>
        </div>
      </div>

      <div class="form-grid">
        <el-select v-model="orderManagementStatusFilter" placeholder="订单状态" @change="refreshOrderManagementItems">
          <el-option
            v-for="option in orderManagementStatusOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-select v-model="orderManagementShopFilter" placeholder="店铺" @change="refreshOrderManagementItems">
          <el-option
            v-for="option in shopFilterOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-input
          v-model="orderManagementKeyword"
          placeholder="搜索订单号、店铺、商品或 SKU"
          clearable
          @keyup.enter="refreshOrderManagementItems"
        />
        <el-button type="primary" :icon="Search" @click="refreshOrderManagementItems">搜索</el-button>
      </div>

      <dl class="status-list compact">
        <div>
          <dt>匹配订单</dt>
          <dd>{{ orderManagementTotal }}</dd>
        </div>
        <div>
          <dt>待同步详情</dt>
          <dd>{{ orderSummary.needsDetail }}</dd>
        </div>
        <div>
          <dt>待采购</dt>
          <dd>{{ orderSummary.needsPurchase }}</dd>
        </div>
        <div>
          <dt>待发货</dt>
          <dd>{{ orderSummary.needsShipment }}</dd>
        </div>
        <div>
          <dt>售后中</dt>
          <dd>{{ orderSummary.aftersale }}</dd>
        </div>
      </dl>

      <el-table :data="orderManagementItems" class="dense-table">
        <el-table-column type="expand">
          <template #default="{ row }">
            <div v-if="row.items.length > 0" class="sub-panel">
              <el-table :data="row.items" class="dense-table">
                <el-table-column prop="title" label="商品" min-width="220" show-overflow-tooltip />
                <el-table-column prop="external_product_id" label="外部商品 ID" min-width="160" show-overflow-tooltip />
                <el-table-column prop="external_sku_id" label="外部 SKU" min-width="130" show-overflow-tooltip />
                <el-table-column prop="wechat_product_id" label="微信商品 ID" min-width="160" show-overflow-tooltip />
                <el-table-column prop="quantity" label="数量" width="80" />
                <el-table-column label="实收" width="110">
                  <template #default="{ row: item }">{{ formatCents(item.real_price ?? item.sale_price) }}</template>
                </el-table-column>
              </el-table>
            </div>
            <div v-else class="empty-state">还没有同步订单商品明细。</div>
          </template>
        </el-table-column>
        <el-table-column label="订单" min-width="240" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.wechat_order_id || row.order_id }}</strong>
              <span>{{ row.shop_name }}</span>
              <span>创建 {{ formatUnixTime(row.order_created_at) }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="management_status" label="管理状态" width="130">
          <template #default="{ row }">
            <el-tag :type="statusType(row.management_status)">
              {{ orderManagementStatusLabel(row.management_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="purchase_status" label="采购" width="120">
          <template #default="{ row }">
            <el-tag :type="statusType(row.purchase_status)">
              {{ purchaseManagementStatusLabel(row.purchase_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="shipment_status" label="发货" width="120">
          <template #default="{ row }">
            <el-tag :type="statusType(row.shipment_status)">
              {{ shipmentStatusLabel(row.shipment_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="profit_status" label="利润" width="120">
          <template #default="{ row }">
            <el-tag :type="statusType(row.profit_status)">
              {{ profitStatusLabel(row.profit_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="item_count" label="商品" width="80" />
        <el-table-column prop="quantity" label="数量" width="80" />
        <el-table-column label="成交额" width="110">
          <template #default="{ row }">{{ formatCents(row.revenue_cents) }}</template>
        </el-table-column>
        <el-table-column label="预估毛利" width="120">
          <template #default="{ row }">{{ formatSignedCents(row.estimated_profit_cents) }}</template>
        </el-table-column>
        <el-table-column label="详情同步" min-width="170">
          <template #default="{ row }">
            <span v-if="row.detail_error" class="subtext">{{ row.detail_error }}</span>
            <span v-else>{{ formatDateTime(row.detail_synced_at) }}</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="240" fixed="right">
          <template #default>
            <div class="button-group compact">
              <el-button size="small" link type="primary" @click="goTo('procurement')">采购</el-button>
              <el-button size="small" link type="primary" @click="goTo('exceptions')">售后</el-button>
              <el-button size="small" link type="primary" @click="goTo('analytics')">利润</el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <div class="action-row">
        <el-button :icon="Refresh" @click="runPurchaseTaskGenerationOnce">生成采购任务</el-button>
        <el-tag>{{ orderManagementItems.length }} / {{ orderManagementTotal }}</el-tag>
      </div>
    </div>
  </section>
</template>
