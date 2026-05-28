<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  formatCents,
  formatDateTime,
  formatSignedCents,
  orderProfitAdjustmentForm,
  orderProfits,
  orderProfitStatusFilter,
  orderProfitTotal,
  orderProfitTotals,
  profitAdjustmentKindOptions,
  profitStatusLabel,
  recordOrderProfitAdjustment,
  Refresh,
  refreshOrderProfits,
  selectOrderProfitAdjustment,
  statusType,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>订单利润核算</h2>
              <p>按订单聚合成交额、采购成本、运费、退款、售后赔付和其他成本。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="orderProfitStatusFilter"
                class="status-filter"
                @change="refreshOrderProfits"
              >
                <el-option value="all" label="全部利润状态" />
                <el-option value="missing_purchase_task" label="缺采购任务" />
                <el-option value="missing_cost" label="缺成本" />
                <el-option value="loss" label="亏损" />
                <el-option value="profitable" label="有毛利" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshOrderProfits">刷新</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>订单数</dt>
              <dd>{{ orderProfitTotals.order_count }}</dd>
            </div>
            <div>
              <dt>成交额</dt>
              <dd>{{ formatCents(orderProfitTotals.revenue_cents) }}</dd>
            </div>
            <div>
              <dt>采购成本</dt>
              <dd>{{ formatCents(orderProfitTotals.purchase_cost_cents) }}</dd>
            </div>
            <div>
              <dt>预估毛利</dt>
              <dd>{{ formatSignedCents(orderProfitTotals.estimated_profit_cents) }}</dd>
            </div>
            <div>
              <dt>实际毛利</dt>
              <dd>{{ formatSignedCents(orderProfitTotals.actual_profit_cents) }}</dd>
            </div>
            <div>
              <dt>待补成本</dt>
              <dd>{{ orderProfitTotals.unknown_actual_order_count }}</dd>
            </div>
          </dl>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>利润调整项</h2>
                <p>记录订单级采购运费、退款、售后赔付、其他成本或收入。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="orderProfitAdjustmentForm.order_id" placeholder="订单 ID 或微信订单号" />
              <el-select v-model="orderProfitAdjustmentForm.kind" placeholder="调整类型">
                <el-option
                  v-for="kind in profitAdjustmentKindOptions"
                  :key="kind.value"
                  :label="kind.label"
                  :value="kind.value"
                />
              </el-select>
              <el-input v-model="orderProfitAdjustmentForm.amount_cents" placeholder="金额，单位分" />
              <el-input v-model="orderProfitAdjustmentForm.note" placeholder="备注，可选" />
              <el-button type="primary" :icon="UploadFilled" @click="recordOrderProfitAdjustment">记录</el-button>
            </div>
          </div>

          <el-table :data="orderProfits" class="dense-table">
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170" />
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="order_status" label="订单状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.order_status)">{{ row.order_status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="profit_status" label="利润状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.profit_status)">{{ profitStatusLabel(row.profit_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="成交额" width="110">
              <template #default="{ row }">{{ formatCents(row.revenue_cents) }}</template>
            </el-table-column>
            <el-table-column label="采购成本" width="110">
              <template #default="{ row }">{{ formatCents(row.purchase_cost_cents) }}</template>
            </el-table-column>
            <el-table-column label="退款/赔付" width="120">
              <template #default="{ row }">
                {{ formatCents(row.refund_cents + row.aftersale_compensation_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="其他成本" width="110">
              <template #default="{ row }">
                {{ formatCents(row.purchase_freight_cents + row.other_cost_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="预估毛利" width="120">
              <template #default="{ row }">{{ formatSignedCents(row.estimated_profit_cents) }}</template>
            </el-table-column>
            <el-table-column label="实际毛利" width="120">
              <template #default="{ row }">{{ formatSignedCents(row.actual_profit_cents) }}</template>
            </el-table-column>
            <el-table-column label="缺成本项" width="100">
              <template #default="{ row }">{{ row.missing_cost_count }}</template>
            </el-table-column>
            <el-table-column label="更新时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
            </el-table-column>
            <el-table-column label="操作" width="90">
              <template #default="{ row }">
                <el-button size="small" @click="selectOrderProfitAdjustment(row)">调整</el-button>
              </template>
            </el-table-column>
          </el-table>

          <div class="action-row">
            <el-tag>{{ orderProfits.length }} / {{ orderProfitTotal }}</el-tag>
          </div>
        </div>
      </section>
</template>
