<script setup lang="ts">
import { computed, ref } from "vue";
import type { PurchaseTaskView } from "../../types/app";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  applySupplierAgentResults,
  Bell,
  CircleCheck,
  deliverySettings,
  deliveryCompanyOptions,
  deliveryShipments,
  deliveryShipmentTotal,
  deliveryStatusFilter,
  exportPurchaseTasks,
  exportSupplierAgentTasks,
  fillSupplierAgentTemplate,
  formatCents,
  formatDateTime,
  markPurchaseTaskIssue,
  purchaseExportPath,
  purchaseIssueForm,
  purchaseIssueTypeOptions,
  purchaseMappingForm,
  purchaseShipmentForm,
  purchaseStatusFilter,
  purchaseTasks,
  purchaseTaskTotal,
  recordPurchaseTaskShipment,
  Refresh,
  refreshDeliveryShipments,
  refreshPurchaseTasks,
  resolvePurchaseTaskMapping,
  retryDeliveryShipment,
  runDeliverySubmissionOnce,
  runPurchaseTaskGenerationOnce,
  Search,
  selectPurchaseTaskIssue,
  selectPurchaseTaskMapping,
  selectPurchaseTaskShipment,
  setAutoSendDelivery,
  statusType,
  syncDeliveryCompanies,
  supplierAgentApplyResult,
  supplierAgentApplyText,
  supplierAgentContinueOnError,
  supplierAgentDryRun,
  supplierAgentExportFormat,
  supplierAgentExportPath,
  UploadFilled,
} = props.ctx;

type PurchaseAction = "mapping" | "shipment" | "issue";

const supplierIssueStatuses = new Set([
  "supplier_out_of_stock",
  "supplier_price_changed",
  "supplier_quality_risk",
  "supplier_cancelled",
  "supplier_exception",
]);

const activePurchaseAction = ref<PurchaseAction>("shipment");
const advancedPanels = ref<string[]>([]);

const purchaseStatusOptions = computed(() => [
  { value: "all", label: "全部", count: purchaseTasks.value.length },
  {
    value: "pending_purchase",
    label: "待下单",
    count: purchaseTasks.value.filter((task) => task.status === "pending_purchase").length,
  },
  {
    value: "needs_mapping",
    label: "缺货源",
    count: purchaseTasks.value.filter((task) => task.status === "needs_mapping").length,
  },
  {
    value: "supplier_issue",
    label: "买不了",
    count: purchaseTasks.value.filter((task) => supplierIssueStatuses.has(task.status)).length,
  },
  {
    value: "supplier_shipped",
    label: "已填物流",
    count: purchaseTasks.value.filter((task) => task.status === "supplier_shipped").length,
  },
]);

const selectedPurchaseTaskId = computed(
  () =>
    purchaseShipmentForm.purchase_task_id
    || purchaseMappingForm.purchase_task_id
    || purchaseIssueForm.purchase_task_id,
);

const purchaseSummary = computed(() => ({
  todo: purchaseTasks.value.filter((task) => ["pending_purchase", "needs_mapping"].includes(task.status)).length,
  shipped: purchaseTasks.value.filter((task) => task.status === "supplier_shipped").length,
  issue: purchaseTasks.value.filter((task) => supplierIssueStatuses.has(task.status)).length,
}));

function purchaseStatusLabel(status: string) {
  const labels: Record<string, string> = {
    pending_purchase: "去供应商下单",
    needs_mapping: "先补货源信息",
    supplier_shipped: "物流已回填",
    supplier_out_of_stock: "供应商缺货",
    supplier_price_changed: "供应商涨价",
    supplier_quality_risk: "质量风险",
    supplier_cancelled: "供应商取消",
    supplier_exception: "供应商异常",
    send_failed: "微信发货失败",
    wechat_shipped: "微信已发货",
    completed: "已完成",
  };
  return labels[status] || status;
}

function deliveryStatusLabel(status: string) {
  const labels: Record<string, string> = {
    waiting_confirmation: "待确认发货",
    ready_to_send: "待提交微信",
    send_failed: "发货失败",
    wechat_shipped: "微信已发货",
  };
  return labels[status] || status;
}

async function setPurchaseFilter(status: string) {
  purchaseStatusFilter.value = status;
  await refreshPurchaseTasks();
}

async function setDeliveryFilter(status: string) {
  deliveryStatusFilter.value = status;
  await refreshDeliveryShipments();
}

function openSource(url?: string | null) {
  if (!url) {
    return;
  }
  window.open(url, "_blank", "noreferrer");
}

function handlePrimaryAction(row: PurchaseTaskView) {
  if (row.status === "needs_mapping") {
    startMapping(row);
    return;
  }
  if (supplierIssueStatuses.has(row.status)) {
    startIssue(row);
    return;
  }
  startShipment(row);
}

function startMapping(row: PurchaseTaskView) {
  selectPurchaseTaskMapping(row);
  activePurchaseAction.value = "mapping";
}

function startShipment(row: PurchaseTaskView) {
  selectPurchaseTaskShipment(row);
  activePurchaseAction.value = "shipment";
}

function startIssue(row: PurchaseTaskView) {
  selectPurchaseTaskIssue(row);
  activePurchaseAction.value = "issue";
}
</script>

<template>
  <section class="content-stack">
    <div class="panel procurement-panel">
      <div class="panel-title">
        <div>
          <h2>采购下单工作台</h2>
          <p>先按货源下单，再填供应商物流；买不了的单独标记异常。</p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="runPurchaseTaskGenerationOnce">拉取新订单</el-button>
          <el-button :icon="Refresh" @click="refreshPurchaseTasks">刷新</el-button>
          <el-button type="primary" :icon="UploadFilled" @click="exportPurchaseTasks">导出给采购</el-button>
        </div>
      </div>

      <div class="ops-filter-bar">
        <button
          v-for="option in purchaseStatusOptions"
          :key="option.value"
          class="ops-filter-pill"
          :class="{ active: purchaseStatusFilter === option.value }"
          @click="setPurchaseFilter(option.value)"
        >
          <span>{{ option.label }}</span>
          <strong>{{ option.count }}</strong>
        </button>
      </div>

      <div class="procurement-kpis">
        <div>
          <span>待处理</span>
          <strong>{{ purchaseSummary.todo }}</strong>
        </div>
        <div>
          <span>已填物流</span>
          <strong>{{ purchaseSummary.shipped }}</strong>
        </div>
        <div>
          <span>异常</span>
          <strong>{{ purchaseSummary.issue }}</strong>
        </div>
        <div>
          <span>匹配总数</span>
          <strong>{{ purchaseTaskTotal }}</strong>
        </div>
      </div>

      <el-table :data="purchaseTasks" class="dense-table operator-table">
        <el-table-column label="要买什么" min-width="270">
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.title || "未同步商品标题" }}</strong>
              <span>{{ row.external_sku_id || "缺外部 SKU" }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="去哪买" min-width="250">
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.supplier_name || "未填供应商" }}</strong>
              <span>{{ row.external_product_id || "缺外部商品 ID" }}</span>
              <el-button
                size="small"
                link
                type="primary"
                :disabled="!row.source_url"
                @click="openSource(row.source_url)"
              >
                打开货源
              </el-button>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="订单" min-width="210">
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.shop_name }}</strong>
              <span>{{ row.wechat_order_id }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="数量/金额" width="130">
          <template #default="{ row }">
            <div class="operator-main-cell compact">
              <strong>x{{ row.quantity }}</strong>
              <span>{{ formatCents(row.estimated_revenue) }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="供应商物流" min-width="190">
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.supplier_waybill_id || "未填物流" }}</strong>
              <span>{{ row.supplier_delivery_name || row.supplier_delivery_id || "-" }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="现在该做" min-width="180">
          <template #default="{ row }">
            <el-tag :type="statusType(row.status)">{{ purchaseStatusLabel(row.status) }}</el-tag>
            <small v-if="row.error_summary" class="subtext">{{ row.error_summary }}</small>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="260" fixed="right">
          <template #default="{ row }">
            <div class="operator-actions">
              <el-button size="small" type="primary" @click="handlePrimaryAction(row)">
                {{ row.status === "needs_mapping" ? "补货源" : supplierIssueStatuses.has(row.status) ? "处理异常" : "填物流" }}
              </el-button>
              <el-button size="small" @click="startMapping(row)">补资料</el-button>
              <el-button size="small" type="warning" @click="startIssue(row)">买不了</el-button>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <div v-if="purchaseExportPath" class="action-row">
        <el-tag>最近导出：{{ purchaseExportPath }}</el-tag>
      </div>
    </div>

    <div class="panel purchase-action-panel">
      <div class="panel-title">
        <div>
          <h2>处理选中的采购单</h2>
          <p>{{ selectedPurchaseTaskId || "先在上面的表格点“填物流 / 补货源 / 买不了”" }}</p>
        </div>
        <el-radio-group v-model="activePurchaseAction">
          <el-radio-button label="shipment">填物流</el-radio-button>
          <el-radio-button label="mapping">补货源</el-radio-button>
          <el-radio-button label="issue">买不了</el-radio-button>
        </el-radio-group>
      </div>

      <template v-if="selectedPurchaseTaskId">
        <div v-if="activePurchaseAction === 'shipment'" class="operator-form-grid">
          <el-input v-model="purchaseShipmentForm.purchase_task_id" placeholder="采购任务 ID" />
          <el-select v-model="purchaseShipmentForm.deliver_type" placeholder="发货方式">
            <el-option :value="1" label="快递发货" />
            <el-option :value="3" label="无需物流" />
          </el-select>
          <el-select
            v-model="purchaseShipmentForm.delivery_id"
            :disabled="purchaseShipmentForm.deliver_type !== 1"
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
            v-model="purchaseShipmentForm.waybill_id"
            :disabled="purchaseShipmentForm.deliver_type !== 1"
            placeholder="物流单号"
          />
          <el-input v-model="purchaseShipmentForm.estimated_cost" placeholder="采购成本，选填" />
          <el-button type="primary" :icon="UploadFilled" @click="recordPurchaseTaskShipment">保存物流</el-button>
        </div>

        <div v-else-if="activePurchaseAction === 'mapping'" class="operator-form-grid">
          <el-input v-model="purchaseMappingForm.purchase_task_id" placeholder="采购任务 ID" />
          <el-input v-model="purchaseMappingForm.external_product_id" placeholder="外部商品 ID" />
          <el-input v-model="purchaseMappingForm.external_sku_id" placeholder="外部 SKU" />
          <el-input v-model="purchaseMappingForm.source_url" placeholder="货源链接，选填" />
          <el-input v-model="purchaseMappingForm.supplier_name" placeholder="供应商，选填" />
          <el-input v-model="purchaseMappingForm.estimated_cost" placeholder="采购成本，选填" />
          <el-input v-model="purchaseMappingForm.note" placeholder="备注，选填" />
          <el-button type="primary" :icon="CircleCheck" @click="resolvePurchaseTaskMapping">保存货源信息</el-button>
        </div>

        <div v-else class="operator-form-grid">
          <el-input v-model="purchaseIssueForm.purchase_task_id" placeholder="采购任务 ID" />
          <el-select v-model="purchaseIssueForm.issue_type" placeholder="异常类型">
            <el-option
              v-for="issue in purchaseIssueTypeOptions"
              :key="issue.value"
              :label="issue.label"
              :value="issue.value"
            />
          </el-select>
          <el-input v-model="purchaseIssueForm.note" placeholder="处理备注，选填" />
          <el-button type="warning" :icon="Bell" @click="markPurchaseTaskIssue">标记买不了</el-button>
        </div>
      </template>
      <div v-else class="empty-state">从上面的采购单选择一个动作后再填写。</div>
    </div>

    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>待发货队列</h2>
          <p>供应商物流保存后会进入这里；自动发货关闭时只进入待确认。</p>
        </div>
        <div class="button-group">
          <el-select
            v-model="deliveryStatusFilter"
            class="status-filter"
            @change="setDeliveryFilter"
          >
            <el-option value="all" label="全部" />
            <el-option value="waiting_confirmation" label="待确认" />
            <el-option value="ready_to_send" label="待提交" />
            <el-option value="send_failed" label="发货失败" />
            <el-option value="wechat_shipped" label="已发货" />
          </el-select>
          <el-button :icon="Refresh" @click="syncDeliveryCompanies">同步快递公司</el-button>
          <el-button :icon="Refresh" @click="refreshDeliveryShipments">刷新</el-button>
          <el-button type="primary" :icon="UploadFilled" @click="runDeliverySubmissionOnce">提交微信发货</el-button>
          <el-switch
            v-model="deliverySettings.auto_send_delivery"
            active-text="自动发货"
            inactive-text="只保存"
            @change="setAutoSendDelivery"
          />
        </div>
      </div>

      <el-table :data="deliveryShipments" class="dense-table operator-table">
        <el-table-column label="状态" width="150">
          <template #default="{ row }">
            <el-tag :type="statusType(row.status)">{{ deliveryStatusLabel(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="shop_name" label="店铺" min-width="130" />
        <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170" />
        <el-table-column label="物流" min-width="190">
          <template #default="{ row }">
            <div class="operator-main-cell">
              <strong>{{ row.waybill_id || "-" }}</strong>
              <span>{{ row.delivery_name || row.delivery_id || "-" }}</span>
            </div>
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

      <div class="action-row">
        <el-tag>{{ deliveryShipments.length }} / {{ deliveryShipmentTotal }}</el-tag>
      </div>
    </div>

    <el-collapse v-model="advancedPanels" class="ops-advanced-collapse">
      <el-collapse-item title="高级批量工具" name="agent">
        <div class="sub-panel supplier-agent-panel">
          <div class="panel-title tight">
            <div>
              <h2>供应商 Agent 桥</h2>
              <p>只导出非敏采购字段；外部结果写回前会校验字段。</p>
            </div>
            <div class="button-group">
              <el-select v-model="supplierAgentExportFormat" class="status-filter">
                <el-option value="jsonl" label="JSONL" />
                <el-option value="json" label="JSON" />
                <el-option value="md" label="Markdown" />
              </el-select>
              <el-button type="primary" :icon="UploadFilled" @click="exportSupplierAgentTasks">导出 Agent 任务</el-button>
              <el-button :icon="Search" @click="fillSupplierAgentTemplate">填入模板</el-button>
            </div>
          </div>
          <div class="supplier-agent-grid">
            <el-input
              v-model="supplierAgentApplyText"
              type="textarea"
              :rows="7"
              class="json-editor"
              placeholder='每行一条 JSON：{"purchase_task_id":"...","action":"shipment","delivery_id":"SF","waybill_id":"..."}'
            />
            <div class="supplier-agent-actions">
              <el-switch v-model="supplierAgentDryRun" active-text="干跑校验" inactive-text="直接写回" />
              <el-switch v-model="supplierAgentContinueOnError" active-text="遇错继续" inactive-text="遇错停止" />
              <el-button type="primary" :icon="CircleCheck" @click="applySupplierAgentResults">
                {{ supplierAgentDryRun ? "校验结果" : "写回结果" }}
              </el-button>
            </div>
          </div>
          <div v-if="supplierAgentExportPath" class="action-row">
            <el-tag>最近 Agent 导出：{{ supplierAgentExportPath }}</el-tag>
          </div>
          <div v-if="supplierAgentApplyResult" class="supplier-agent-result">
            <el-tag :type="supplierAgentApplyResult.failed > 0 ? 'warning' : 'success'">
              处理 {{ supplierAgentApplyResult.processed }} 条，成功 {{ supplierAgentApplyResult.succeeded }} 条，失败 {{ supplierAgentApplyResult.failed }} 条
            </el-tag>
            <el-table :data="supplierAgentApplyResult.results" class="dense-table compact-table">
              <el-table-column prop="index" label="#" width="70" />
              <el-table-column prop="purchase_task_id" label="采购任务 ID" min-width="180" />
              <el-table-column prop="action" label="动作" width="110" />
              <el-table-column prop="status" label="状态" width="110">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="error" label="错误" min-width="260" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.error || "-" }}
                </template>
              </el-table-column>
            </el-table>
          </div>
        </div>
      </el-collapse-item>
    </el-collapse>
  </section>
</template>
