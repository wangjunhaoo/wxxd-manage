<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  automationRunning,
  automationSettings,
  formatDateTime,
  lastAutomationResult,
  openTask,
  Refresh,
  publishRetryRunning,
  resumePublishFailuresOnce,
  runAftersaleSyncOnce,
  runGuaranteeSyncOnce,
  runOperationalAutomationOnce,
  runOrderDetailSyncOnce,
  runOrderSyncOnce,
  runPriceUpdateConfirmOnce,
  runPriceUpdatePrecheckOnce,
  runPriceUpdateSubmitOnce,
  runPurchaseTaskGenerationOnce,
  saveAutomationSettings,
  statusType,
  taskRuns,
  UploadFilled,
} = props.ctx;

const publishAutomationEnabled = computed(() =>
  [
    automationSettings.value.publish_precheck_enabled,
    automationSettings.value.publish_attribute_fill_enabled,
    automationSettings.value.publish_category_precheck_enabled,
    automationSettings.value.publish_asset_upload_enabled,
    automationSettings.value.publish_submit_enabled,
    automationSettings.value.publish_status_sync_enabled,
    automationSettings.value.publish_listing_enabled,
  ].every(Boolean),
);

async function togglePublishAutomation(value: boolean | string | number) {
  const enabled = Boolean(value);
  automationSettings.value.publish_precheck_enabled = enabled;
  automationSettings.value.publish_attribute_fill_enabled = enabled;
  automationSettings.value.publish_category_precheck_enabled = enabled;
  automationSettings.value.publish_asset_upload_enabled = enabled;
  automationSettings.value.publish_submit_enabled = enabled;
  automationSettings.value.publish_status_sync_enabled = enabled;
  automationSettings.value.publish_listing_enabled = enabled;
  await saveAutomationSettings();
}

function isPublishTask(taskType: string) {
  return taskType === "publish.create_external_job";
}

function taskStatusLabel(taskType: string, status: string) {
  if (!isPublishTask(taskType)) {
    return status;
  }
  if (status === "success") {
    return "已上架";
  }
  if (status === "failed") {
    return "异常";
  }
  if (status === "pending" || status === "queued") {
    return "待铺货";
  }
  return "铺货中";
}

function taskStatusType(taskType: string, status: string) {
  if (!isPublishTask(taskType)) {
    return statusType(status);
  }
  if (status === "success") {
    return "success";
  }
  if (status === "failed") {
    return "danger";
  }
  if (status === "pending" || status === "queued") {
    return "info";
  }
  return "primary";
}

function taskTypeLabel(taskType: string) {
  return isPublishTask(taskType) ? "铺货" : taskType;
}
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>任务中心</h2>
          <p>
            铺货只保留一个推进入口，底层微信预检、素材、提交和上架由系统自动处理。
          </p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="runOrderSyncOnce"
            >同步待发货订单</el-button
          >
          <el-button :icon="Refresh" @click="runOrderDetailSyncOnce"
            >同步订单详情</el-button
          >
          <el-button :icon="Refresh" @click="runAftersaleSyncOnce"
            >同步售后</el-button
          >
          <el-button :icon="Refresh" @click="runGuaranteeSyncOnce"
            >同步纠纷单</el-button
          >
          <el-button :icon="Refresh" @click="runPurchaseTaskGenerationOnce"
            >生成采购任务</el-button
          >
          <el-button
            type="primary"
            :icon="Refresh"
            :loading="publishRetryRunning"
            @click="resumePublishFailuresOnce"
          >
            推进铺货
          </el-button>
          <el-button :icon="Refresh" @click="runPriceUpdatePrecheckOnce"
            >商品售价校验</el-button
          >
          <el-button :icon="UploadFilled" @click="runPriceUpdateSubmitOnce"
            >提交商品售价</el-button
          >
          <el-button :icon="Refresh" @click="runPriceUpdateConfirmOnce"
            >确认商品售价</el-button
          >
        </div>
      </div>
      <div class="sub-panel automation-panel">
        <div class="automation-head">
          <div>
            <h3>自动推进</h3>
            <p>
              铺货作为一个整体开关，不再拆成类目、素材、提交和上架多个开关。
            </p>
          </div>
          <el-button
            type="primary"
            :icon="Refresh"
            :loading="automationRunning"
            @click="runOperationalAutomationOnce"
          >
            自动推进一轮
          </el-button>
        </div>
        <div class="automation-switches">
          <el-switch
            v-model="automationSettings.order_sync_enabled"
            active-text="同步订单"
            @change="saveAutomationSettings"
          />
          <el-switch
            v-model="automationSettings.order_detail_sync_enabled"
            active-text="同步详情"
            @change="saveAutomationSettings"
          />
          <el-switch
            v-model="automationSettings.aftersale_sync_enabled"
            active-text="售后同步"
            @change="saveAutomationSettings"
          />
          <el-switch
            v-model="automationSettings.purchase_task_enabled"
            active-text="采购任务"
            @change="saveAutomationSettings"
          />
          <el-switch
            v-model="automationSettings.delivery_submission_enabled"
            active-text="微信发货"
            @change="saveAutomationSettings"
          />
          <el-switch
            :model-value="publishAutomationEnabled"
            active-text="自动铺货"
            @change="togglePublishAutomation"
          />
          <el-switch
            v-model="automationSettings.price_confirm_enabled"
            active-text="确认商品售价"
            @change="saveAutomationSettings"
          />
        </div>
        <div v-if="lastAutomationResult" class="automation-result">
          <el-tag type="success"
            >执行 {{ lastAutomationResult.executed_steps.length }}</el-tag
          >
          <el-tag type="info"
            >跳过 {{ lastAutomationResult.skipped_steps.length }}</el-tag
          >
          <el-tag
            :type="
              lastAutomationResult.errors.length > 0 ? 'danger' : 'success'
            "
          >
            失败 {{ lastAutomationResult.errors.length }}
          </el-tag>
          <span v-if="lastAutomationResult.errors.length > 0">
            {{
              lastAutomationResult.errors
                .map((item) => `${item.step}: ${item.error}`)
                .join("；")
            }}
          </span>
        </div>
      </div>
      <el-table :data="taskRuns" class="dense-table">
        <el-table-column prop="id" label="任务 ID" min-width="260" />
        <el-table-column label="类型" min-width="210">
          <template #default="{ row }">{{
            taskTypeLabel(row.task_type)
          }}</template>
        </el-table-column>
        <el-table-column prop="status" label="状态" width="140">
          <template #default="{ row }">
            <el-tag :type="taskStatusType(row.task_type, row.status)">
              {{ taskStatusLabel(row.task_type, row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="进度" width="190">
          <template #default="{ row }">
            <el-progress :percentage="row.progress" :stroke-width="8" />
          </template>
        </el-table-column>
        <el-table-column label="任务项" min-width="180">
          <template #default="{ row }">
            <template v-if="isPublishTask(row.task_type)">
              <span>待处理 {{ row.pending_count + row.ready_count }}</span>
              <span class="split-stat">异常 {{ row.failed_count }}</span>
            </template>
            <template v-else>
              <span>待 {{ row.pending_count }}</span>
              <span class="split-stat">就绪 {{ row.ready_count }}</span>
              <span class="split-stat">失败 {{ row.failed_count }}</span>
            </template>
          </template>
        </el-table-column>
        <el-table-column label="创建时间" min-width="190">
          <template #default="{ row }">{{
            formatDateTime(row.created_at)
          }}</template>
        </el-table-column>
        <el-table-column label="操作" width="110">
          <template #default="{ row }">
            <el-button size="small" @click="openTask(row)">查看</el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>
  </section>
</template>
