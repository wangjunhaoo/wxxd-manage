<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  formatCents,
  formatDateTime,
  formatSignedCents,
  inventoryRiskLabel,
  productSalesAnalysis,
  productSalesAnalysisStatusFilter,
  productSalesAnalysisTotal,
  productSalesAnalysisTotals,
  productSalesStatusLabel,
  productSalesStatusOptions,
  Refresh,
  refreshProductSalesAnalysis,
  statusType,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>商品动销分析</h2>
              <p>按外部商品聚合真实订单、采购成本、售后关联、铺货店铺和库存风险，输出继续铺货、调价、补货或观察建议。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="productSalesAnalysisStatusFilter"
                class="status-filter"
                @change="refreshProductSalesAnalysis"
              >
                <el-option
                  v-for="status in productSalesStatusOptions"
                  :key="status.value"
                  :label="status.label"
                  :value="status.value"
                />
              </el-select>
              <el-button :icon="Refresh" @click="refreshProductSalesAnalysis">刷新</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>商品数</dt>
              <dd>{{ productSalesAnalysisTotals.product_count }}</dd>
            </div>
            <div>
              <dt>已动销</dt>
              <dd>{{ productSalesAnalysisTotals.sold_product_count }}</dd>
            </div>
            <div>
              <dt>销量</dt>
              <dd>{{ productSalesAnalysisTotals.total_units_sold }}</dd>
            </div>
            <div>
              <dt>成交额</dt>
              <dd>{{ formatCents(productSalesAnalysisTotals.revenue_cents) }}</dd>
            </div>
            <div>
              <dt>粗毛利</dt>
              <dd>{{ formatSignedCents(productSalesAnalysisTotals.gross_profit_cents) }}</dd>
            </div>
            <div>
              <dt>可放量</dt>
              <dd>{{ productSalesAnalysisTotals.scale_candidate_count }}</dd>
            </div>
            <div>
              <dt>风险</dt>
              <dd>{{ productSalesAnalysisTotals.risk_product_count }}</dd>
            </div>
            <div>
              <dt>缺成本</dt>
              <dd>{{ productSalesAnalysisTotals.missing_cost_product_count }}</dd>
            </div>
          </dl>

          <el-table :data="productSalesAnalysis" class="dense-table">
            <el-table-column prop="external_product_id" label="外部商品 ID" min-width="170" />
            <el-table-column prop="title" label="商品" min-width="220" show-overflow-tooltip />
            <el-table-column prop="operation_status" label="运营状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.operation_status)">{{ productSalesStatusLabel(row.operation_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="inventory_risk_status" label="库存" width="120">
              <template #default="{ row }">
                <el-tag :type="statusType(row.inventory_risk_status)">{{ inventoryRiskLabel(row.inventory_risk_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="active_shop_count" label="店铺" width="80" />
            <el-table-column prop="order_count" label="订单" width="80" />
            <el-table-column prop="units_sold" label="销量" width="80" />
            <el-table-column label="成交额" width="110">
              <template #default="{ row }">{{ formatCents(row.revenue_cents) }}</template>
            </el-table-column>
            <el-table-column label="采购成本" width="110">
              <template #default="{ row }">{{ formatCents(row.purchase_cost_cents) }}</template>
            </el-table-column>
            <el-table-column label="粗毛利" width="110">
              <template #default="{ row }">
                {{ formatSignedCents(row.revenue_cents - row.purchase_cost_cents - row.related_refund_cents) }}
              </template>
            </el-table-column>
            <el-table-column prop="missing_cost_count" label="缺成本" width="90" />
            <el-table-column prop="related_aftersale_count" label="售后" width="80" />
            <el-table-column prop="available_stock" label="可用库存" width="100" />
            <el-table-column prop="recommendation" label="建议" min-width="300" show-overflow-tooltip />
            <el-table-column label="最近订单" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.last_order_at) }}</template>
            </el-table-column>
          </el-table>

          <div class="action-row">
            <el-tag>{{ productSalesAnalysis.length }} / {{ productSalesAnalysisTotal }}</el-tag>
          </div>
        </div>
      </section>
</template>
