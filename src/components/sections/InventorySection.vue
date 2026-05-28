<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  inventoryRiskLabel,
  inventoryRisks,
  inventoryRiskStats,
  inventoryRiskStatusFilter,
  inventoryRiskStatusOptions,
  inventoryRiskTotal,
  formatDateTime,
  Refresh,
  refreshInventoryRisks,
  runInventoryRiskScan,
  statusType,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>库存风控</h2>
              <p>基于外部商品 SKU 库存、采购占用、供应商异常和铺货店铺数生成运营提醒。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="inventoryRiskStatusFilter"
                class="status-filter"
                @change="refreshInventoryRisks"
              >
                <el-option
                  v-for="status in inventoryRiskStatusOptions"
                  :key="status.value"
                  :label="status.label"
                  :value="status.value"
                />
              </el-select>
              <el-button :icon="Refresh" @click="refreshInventoryRisks">刷新</el-button>
              <el-button type="primary" :icon="UploadFilled" @click="runInventoryRiskScan">扫描并通知</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>风险商品</dt>
              <dd>{{ inventoryRiskTotal }}</dd>
            </div>
            <div>
              <dt>断货</dt>
              <dd>{{ inventoryRiskStats.out_of_stock_count }}</dd>
            </div>
            <div>
              <dt>低库存/压力</dt>
              <dd>{{ inventoryRiskStats.low_stock_count }}</dd>
            </div>
            <div>
              <dt>供应商异常</dt>
              <dd>{{ inventoryRiskStats.issue_count }}</dd>
            </div>
          </dl>

          <el-table :data="inventoryRisks" class="dense-table">
            <el-table-column prop="external_product_id" label="外部商品 ID" min-width="170" />
            <el-table-column prop="title" label="商品" min-width="220" show-overflow-tooltip />
            <el-table-column prop="supplier_name" label="供应商" min-width="130">
              <template #default="{ row }">{{ row.supplier_name || "-" }}</template>
            </el-table-column>
            <el-table-column prop="risk_status" label="状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.risk_status)">{{ inventoryRiskLabel(row.risk_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="total_stock" label="货源库存" width="100" />
            <el-table-column prop="reserved_quantity" label="采购占用" width="100" />
            <el-table-column prop="available_stock" label="可用库存" width="100" />
            <el-table-column prop="active_shop_count" label="已铺店铺" width="100" />
            <el-table-column prop="recommendation" label="建议" min-width="280" show-overflow-tooltip />
            <el-table-column label="更新时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
