<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  formatCents,
  formatDateTime,
  inventoryRiskLabel,
  productManagementItems,
  productManagementKeyword,
  productManagementShopFilter,
  productManagementStatusFilter,
  productManagementStatusLabel,
  productManagementStatusOptions,
  productManagementTotal,
  productSalesStatusLabel,
  Refresh,
  refreshProductManagementItems,
  runInventoryRiskScan,
  Search,
  selectedSection,
  shops,
  statusType,
} = props.ctx;

const shopFilterOptions = computed(() => [
  { value: "all", label: "全部店铺" },
  ...shops.value.map((shop) => ({ value: shop.id, label: shop.name })),
]);

const productSummary = computed(() => ({
  failed: productManagementItems.value.filter((item) => item.management_status === "publish_failed").length,
  risk: productManagementItems.value.filter((item) => item.management_status === "inventory_risk").length,
  listed: productManagementItems.value.filter((item) => item.active_shop_count > 0).length,
  sold: productManagementItems.value.filter((item) => item.order_count > 0).length,
}));

function openSource(url?: string | null) {
  if (!url) {
    return;
  }
  window.open(url, "_blank", "noreferrer");
}

function goTo(section: string) {
  selectedSection.value = section;
}
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>商品管理</h2>
          <p>集中查看货源商品、店铺铺货状态、库存风险和真实动销表现。</p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="refreshProductManagementItems">刷新</el-button>
          <el-button :icon="Refresh" @click="runInventoryRiskScan">库存扫描</el-button>
        </div>
      </div>

      <div class="form-grid">
        <el-select v-model="productManagementStatusFilter" placeholder="商品状态" @change="refreshProductManagementItems">
          <el-option
            v-for="option in productManagementStatusOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-select v-model="productManagementShopFilter" placeholder="店铺" @change="refreshProductManagementItems">
          <el-option
            v-for="option in shopFilterOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-input
          v-model="productManagementKeyword"
          placeholder="搜索商品、货源、供应商或微信商品 ID"
          clearable
          @keyup.enter="refreshProductManagementItems"
        />
        <el-button type="primary" :icon="Search" @click="refreshProductManagementItems">搜索</el-button>
      </div>

      <dl class="status-list compact">
        <div>
          <dt>匹配商品</dt>
          <dd>{{ productManagementTotal }}</dd>
        </div>
        <div>
          <dt>铺货失败</dt>
          <dd>{{ productSummary.failed }}</dd>
        </div>
        <div>
          <dt>库存风险</dt>
          <dd>{{ productSummary.risk }}</dd>
        </div>
        <div>
          <dt>已铺货</dt>
          <dd>{{ productSummary.listed }}</dd>
        </div>
        <div>
          <dt>已动销</dt>
          <dd>{{ productSummary.sold }}</dd>
        </div>
      </dl>

      <el-table :data="productManagementItems" class="dense-table">
        <el-table-column type="expand">
          <template #default="{ row }">
            <div v-if="row.shops.length > 0" class="sub-panel">
              <el-table :data="row.shops" class="dense-table">
                <el-table-column prop="shop_name" label="店铺" min-width="150" />
                <el-table-column prop="status" label="铺货状态" width="120">
                  <template #default="{ row: shop }">
                    <el-tag :type="statusType(shop.status)">{{ shop.status }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="wechat_product_id" label="微信商品 ID" min-width="170" show-overflow-tooltip />
                <el-table-column label="当前售价" width="110">
                  <template #default="{ row: shop }">{{ formatCents(shop.current_price_cents) }}</template>
                </el-table-column>
                <el-table-column label="状态同步" min-width="170">
                  <template #default="{ row: shop }">{{ formatDateTime(shop.last_status_sync_at) }}</template>
                </el-table-column>
                <el-table-column prop="audit_summary" label="审核摘要" min-width="240" show-overflow-tooltip />
              </el-table>
            </div>
            <div v-else class="empty-state">这个商品还没有店铺铺货记录。</div>
          </template>
        </el-table-column>
        <el-table-column label="商品" min-width="280" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.title }}</strong>
              <span>{{ row.external_product_id }}</span>
              <span>{{ row.supplier_name || "未记录供应商" }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="management_status" label="管理状态" width="130">
          <template #default="{ row }">
            <el-tag :type="statusType(row.management_status)">
              {{ productManagementStatusLabel(row.management_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="inventory_risk_status" label="库存" width="120">
          <template #default="{ row }">
            <el-tag :type="statusType(row.inventory_risk_status)">
              {{ inventoryRiskLabel(row.inventory_risk_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="operation_status" label="动销" width="120">
          <template #default="{ row }">
            <el-tag :type="statusType(row.operation_status)">
              {{ productSalesStatusLabel(row.operation_status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="active_shop_count" label="店铺" width="80" />
        <el-table-column prop="order_count" label="订单" width="80" />
        <el-table-column prop="units_sold" label="销量" width="80" />
        <el-table-column label="成交额" width="110">
          <template #default="{ row }">{{ formatCents(row.revenue_cents) }}</template>
        </el-table-column>
        <el-table-column label="可用库存" width="100">
          <template #default="{ row }">{{ row.available_stock }} / {{ row.total_stock }}</template>
        </el-table-column>
        <el-table-column prop="recommendation" label="运营建议" min-width="280" show-overflow-tooltip />
        <el-table-column label="操作" width="210" fixed="right">
          <template #default="{ row }">
            <div class="button-group compact">
              <el-button size="small" link type="primary" :disabled="!row.source_url" @click="openSource(row.source_url)">
                货源
              </el-button>
              <el-button size="small" link type="primary" @click="goTo('publish')">铺货</el-button>
              <el-button size="small" link type="primary" @click="goTo('analytics')">分析</el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </div>
  </section>
</template>
