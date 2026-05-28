<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  automationRunning,
  automationSettings,
  formatDateTime,
  lastAutomationResult,
  openTask,
  Refresh,
  runAftersaleSyncOnce,
  runGuaranteeSyncOnce,
  runOperationalAutomationOnce,
  runOrderDetailSyncOnce,
  runOrderSyncOnce,
  runPriceUpdateConfirmOnce,
  runPriceUpdatePrecheckOnce,
  runPriceUpdateSubmitOnce,
  runPublishAiAttributeSuggestionsOnce,
  runPublishAssetUploadsOnce,
  runPublishAttributeFillOnce,
  runPublishCategoryPrechecksOnce,
  runPublishListingOnce,
  runPublishStatusSyncOnce,
  runPublishSubmitsOnce,
  runPublishTasksOnce,
  runPurchaseTaskGenerationOnce,
  saveAutomationSettings,
  statusType,
  taskRuns,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>任务中心</h2>
              <p>前置校验会生成或验证微信发品参数草稿，通过后进入素材上传和真实发品；失败原因保留到商品任务。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="runOrderSyncOnce">同步待发货订单</el-button>
              <el-button :icon="Refresh" @click="runOrderDetailSyncOnce">同步订单详情</el-button>
              <el-button :icon="Refresh" @click="runAftersaleSyncOnce">同步售后</el-button>
              <el-button :icon="Refresh" @click="runGuaranteeSyncOnce">同步纠纷单</el-button>
              <el-button :icon="Refresh" @click="runPurchaseTaskGenerationOnce">生成采购任务</el-button>
              <el-button type="primary" :icon="Refresh" @click="runPublishTasksOnce">执行前置校验</el-button>
              <el-button :icon="Refresh" @click="runPublishAttributeFillOnce">补齐必填属性</el-button>
              <el-button :icon="Refresh" @click="runPublishAiAttributeSuggestionsOnce">AI 生成属性建议</el-button>
              <el-button :icon="Refresh" @click="runPublishCategoryPrechecksOnce">微信类目预检</el-button>
              <el-button :icon="Refresh" @click="runPriceUpdatePrecheckOnce">商品售价校验</el-button>
              <el-button :icon="UploadFilled" @click="runPriceUpdateSubmitOnce">提交商品售价</el-button>
              <el-button :icon="Refresh" @click="runPriceUpdateConfirmOnce">确认商品售价</el-button>
              <el-button :icon="UploadFilled" @click="runPublishAssetUploadsOnce">上传微信素材</el-button>
              <el-button :icon="UploadFilled" @click="runPublishSubmitsOnce">提交微信发品</el-button>
              <el-button :icon="Refresh" @click="runPublishStatusSyncOnce">同步审核状态</el-button>
              <el-button :icon="UploadFilled" @click="runPublishListingOnce">上架通过商品</el-button>
            </div>
          </div>
          <div class="sub-panel automation-panel">
            <div class="automation-head">
              <div>
                <h3>自动推进</h3>
                <p>按开关顺序推进订单同步、采购任务、发货、铺货发布链路和商品售价确认；单步失败会保留错误并继续后续步骤。</p>
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
                v-model="automationSettings.publish_precheck_enabled"
                active-text="铺货校验"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_attribute_fill_enabled"
                active-text="属性补齐"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_category_precheck_enabled"
                active-text="类目预检"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_asset_upload_enabled"
                active-text="素材上传"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_submit_enabled"
                active-text="提交发品"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_status_sync_enabled"
                active-text="铺货状态"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_listing_enabled"
                active-text="自动上架"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.price_confirm_enabled"
                active-text="确认商品售价"
                @change="saveAutomationSettings"
              />
            </div>
            <div v-if="lastAutomationResult" class="automation-result">
              <el-tag type="success">执行 {{ lastAutomationResult.executed_steps.length }}</el-tag>
              <el-tag type="info">跳过 {{ lastAutomationResult.skipped_steps.length }}</el-tag>
              <el-tag :type="lastAutomationResult.errors.length > 0 ? 'danger' : 'success'">
                失败 {{ lastAutomationResult.errors.length }}
              </el-tag>
              <span v-if="lastAutomationResult.errors.length > 0">
                {{ lastAutomationResult.errors.map((item) => `${item.step}: ${item.error}`).join("；") }}
              </span>
            </div>
          </div>
          <el-table :data="taskRuns" class="dense-table">
            <el-table-column prop="id" label="任务 ID" min-width="260" />
            <el-table-column prop="task_type" label="类型" min-width="210" />
            <el-table-column prop="status" label="状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="进度" width="190">
              <template #default="{ row }">
                <el-progress :percentage="row.progress" :stroke-width="8" />
              </template>
            </el-table-column>
            <el-table-column label="任务项" min-width="180">
              <template #default="{ row }">
                <span>待 {{ row.pending_count }}</span>
                <span class="split-stat">就绪 {{ row.ready_count }}</span>
                <span class="split-stat">失败 {{ row.failed_count }}</span>
              </template>
            </el-table-column>
            <el-table-column label="创建时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.created_at) }}</template>
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
