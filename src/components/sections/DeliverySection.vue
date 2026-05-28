<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  deliveryCompanyOptions,
  deliverySettings,
  deliveryShipments,
  deliveryShipmentTotal,
  deliveryStatusFilter,
  formatDateTime,
  recordOrderShipment,
  Refresh,
  refreshDeliveryShipments,
  retryDeliveryShipment,
  runDeliverySubmissionOnce,
  setAutoSendDelivery,
  shipmentForm,
  statusType,
  syncDeliveryCompanies,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>人工物流回填</h2>
              <p>物流单号先人工录入；自动发货开关关闭时，只保存为待确认发货。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="syncDeliveryCompanies">同步快递公司</el-button>
              <el-switch
                v-model="deliverySettings.auto_send_delivery"
                active-text="自动发货"
                inactive-text="仅回填"
                @change="setAutoSendDelivery"
              />
            </div>
          </div>
          <div class="form-grid">
            <el-input v-model="shipmentForm.order_id" placeholder="本地订单 ID，可选" />
            <el-input v-model="shipmentForm.shop_id" placeholder="店铺 ID，未填订单 ID 时必填" />
            <el-input v-model="shipmentForm.wechat_order_id" placeholder="微信订单号，未填订单 ID 时必填" />
            <el-select v-model="shipmentForm.deliver_type" placeholder="发货方式">
              <el-option :value="1" label="自寄快递" />
              <el-option :value="3" label="虚拟无需物流" />
            </el-select>
            <el-select
              v-model="shipmentForm.delivery_id"
              :disabled="shipmentForm.deliver_type !== 1"
              placeholder="快递公司"
            >
              <el-option
                v-for="company in deliveryCompanyOptions"
                :key="company.value"
                :label="company.label"
                :value="company.value"
              />
            </el-select>
            <el-input
              v-model="shipmentForm.waybill_id"
              :disabled="shipmentForm.deliver_type !== 1"
              placeholder="快递单号"
            />
            <el-button type="primary" :icon="UploadFilled" @click="recordOrderShipment">保存物流</el-button>
            <el-button :icon="Refresh" @click="runDeliverySubmissionOnce">提交微信发货</el-button>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>发货队列</h2>
              <p>失败单可以重新放回待提交队列；真正调用微信仍由队列任务执行。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="deliveryStatusFilter"
                class="status-filter"
                @change="refreshDeliveryShipments"
              >
                <el-option value="all" label="全部状态" />
                <el-option value="waiting_confirmation" label="待确认" />
                <el-option value="ready_to_send" label="待提交" />
                <el-option value="send_failed" label="发货失败" />
                <el-option value="wechat_shipped" label="已发货" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshDeliveryShipments">刷新</el-button>
              <el-tag>{{ deliveryShipments.length }} / {{ deliveryShipmentTotal }}</el-tag>
            </div>
          </div>
          <el-table :data="deliveryShipments" class="dense-table">
            <el-table-column prop="id" label="物流单 ID" min-width="230" />
            <el-table-column prop="status" label="状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170" />
            <el-table-column label="物流" min-width="190">
              <template #default="{ row }">
                <span>{{ row.waybill_id || "-" }}</span>
                <small v-if="row.delivery_name || row.delivery_id" class="subtext">
                  {{ row.delivery_name || row.delivery_id }}
                </small>
              </template>
            </el-table-column>
            <el-table-column prop="error_summary" label="失败原因" min-width="260" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.error_summary || row.error_code || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="更新时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
            </el-table-column>
            <el-table-column label="操作" width="110">
              <template #default="{ row }">
                <el-button
                  size="small"
                  :disabled="row.status === 'wechat_shipped'"
                  @click="retryDeliveryShipment(row)"
                >
                  重试
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
