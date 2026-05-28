<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const ctx = props.ctx;

const activeCollections = computed(() =>
  ctx.collectionTasks.value.filter((task) => task.status === "pending" || task.status === "running").length,
);

const readyCollections = computed(() =>
  ctx.collectionTasks.value.filter((task) =>
    task.status === "success" && task.publish_job_ids.length === 0,
  ).length,
);

const failedCollections = computed(() =>
  ctx.collectionTasks.value.filter((task) => task.status === "failed").length,
);

const pendingPurchaseCount = computed(() =>
  ctx.purchaseTasks.value.filter((task) => task.status === "pending_purchase").length,
);

const mappingCount = computed(() =>
  ctx.purchaseTasks.value.filter((task) => task.status === "needs_mapping").length,
);

const supplierIssueCount = computed(() =>
  ctx.purchaseTasks.value.filter((task) =>
    ["supplier_out_of_stock", "supplier_price_changed", "supplier_quality_risk", "supplier_cancelled"].includes(task.status),
  ).length,
);

const shipmentTodoCount = computed(() =>
  ctx.deliveryShipments.value.filter((shipment) =>
    ["waiting_confirmation", "ready_to_send", "send_failed"].includes(shipment.status),
  ).length,
);

const priceTodoCount = computed(() =>
  ctx.currentOrderPriceAdjustmentJob.value?.items.filter((item) =>
    ["pending", "submitting", "failed"].includes(item.status),
  ).length ?? 0,
);

const latestNotifications = computed(() => ctx.notifications.value.slice(0, 5));

function showSection(section: "publish" | "procurement" | "price" | "exceptions" | "analytics" | "settings") {
  ctx.selectedSection.value = section;
}

async function showPurchaseStatus(status: string) {
  ctx.selectedSection.value = "procurement";
  ctx.purchaseStatusFilter.value = status;
  await ctx.refreshPurchaseTasks();
}

async function showDeliveryStatus(status: string) {
  ctx.selectedSection.value = "procurement";
  ctx.deliveryStatusFilter.value = status;
  await ctx.refreshDeliveryShipments();
}

async function showSalesStatus(status: string) {
  ctx.selectedSection.value = "analytics";
  ctx.productSalesAnalysisStatusFilter.value = status;
  await ctx.refreshProductSalesAnalysis();
}
</script>

<template>
  <section class="ops-workbench">
    <div class="ops-hero">
      <div>
        <p class="eyebrow">运营今日优先级</p>
        <h2>先铺货，再处理采购下单和物流回填</h2>
        <p>
          首页只放今天需要运营处理的动作；接口、任务日志和系统配置都收进设置区。
        </p>
      </div>
      <div class="ops-hero-actions">
        <el-button type="primary" :icon="ctx.Refresh" :loading="ctx.automationRunning.value" @click="ctx.runOperationalAutomationOnce">
          自动推进一轮
        </el-button>
        <el-button :icon="ctx.Refresh" @click="ctx.refreshAll">刷新数据</el-button>
      </div>
    </div>

    <div class="ops-metrics">
      <button class="ops-metric-card danger" @click="showSection('exceptions')">
        <span>待处理订单</span>
        <strong>{{ ctx.dashboard.value?.pending_order_count ?? 0 }}</strong>
        <small>采购 / 发货 / 售后 / 超时</small>
      </button>
      <button class="ops-metric-card warning" @click="showSection('publish')">
        <span>铺货失败商品</span>
        <strong>{{ ctx.dashboard.value?.failed_publish_product_count ?? 0 }}</strong>
        <small>优先看失败原因和属性建议</small>
      </button>
      <button class="ops-metric-card critical" @click="showSection('exceptions')">
        <span>未读通知</span>
        <strong>{{ ctx.dashboard.value?.unread_notification_count ?? ctx.unreadNotificationCount.value }}</strong>
        <small>铺货、采购、发货、售后异常</small>
      </button>
      <button class="ops-metric-card neutral" @click="showSection('settings')">
        <span>运行中任务</span>
        <strong>{{ ctx.dashboard.value?.running_task_count ?? 0 }}</strong>
        <small>{{ ctx.dashboard.value?.controller_status || "主控机状态未知" }}</small>
      </button>
    </div>

    <div class="ops-lanes">
      <div class="ops-lane">
        <div class="ops-lane-title">
          <h3>铺货</h3>
          <el-button type="primary" text @click="showSection('publish')">进入铺货</el-button>
        </div>
        <button class="workflow-item" @click="showSection('publish')">
          <span>采集中 / 待采集</span>
          <strong>{{ activeCollections }}</strong>
        </button>
        <button class="workflow-item" @click="showSection('publish')">
          <span>已采集待创建铺货</span>
          <strong>{{ readyCollections }}</strong>
        </button>
        <button class="workflow-item danger" @click="showSection('publish')">
          <span>采集失败</span>
          <strong>{{ failedCollections }}</strong>
        </button>
        <button class="workflow-item warning" @click="showSection('publish')">
          <span>待采纳属性建议</span>
          <strong>{{ ctx.pendingAttributeSuggestionCount.value }}</strong>
        </button>
      </div>

      <div class="ops-lane">
        <div class="ops-lane-title">
          <h3>采购下单</h3>
          <el-button type="primary" text @click="showSection('procurement')">进入采购</el-button>
        </div>
        <button class="workflow-item" @click="showPurchaseStatus('pending_purchase')">
          <span>待采购</span>
          <strong>{{ pendingPurchaseCount }}</strong>
        </button>
        <button class="workflow-item warning" @click="showPurchaseStatus('needs_mapping')">
          <span>缺货源信息</span>
          <strong>{{ mappingCount }}</strong>
        </button>
        <button class="workflow-item danger" @click="showPurchaseStatus('all')">
          <span>供应商异常</span>
          <strong>{{ supplierIssueCount }}</strong>
        </button>
        <button class="workflow-item" @click="showDeliveryStatus('all')">
          <span>待发货 / 发货失败</span>
          <strong>{{ shipmentTodoCount }}</strong>
        </button>
      </div>

      <div class="ops-lane">
        <div class="ops-lane-title">
          <h3>价格与经营</h3>
          <el-button type="primary" text @click="showSection('price')">进入改价</el-button>
        </div>
        <button class="workflow-item" @click="showSection('price')">
          <span>当前订单改价</span>
          <strong>{{ priceTodoCount }}</strong>
        </button>
        <button class="workflow-item" @click="showSalesStatus('scale_candidate')">
          <span>可继续放量商品</span>
          <strong>{{ ctx.productSalesAnalysisTotals.value.scale_candidate_count }}</strong>
        </button>
        <button class="workflow-item warning" @click="showSection('analytics')">
          <span>库存 / 毛利风险</span>
          <strong>{{ ctx.productSalesAnalysisTotals.value.risk_product_count }}</strong>
        </button>
        <button class="workflow-item" @click="showSection('analytics')">
          <span>缺采购成本</span>
          <strong>{{ ctx.productSalesAnalysisTotals.value.missing_cost_product_count }}</strong>
        </button>
      </div>
    </div>

    <div class="panel ops-notification-panel">
      <div class="panel-title">
        <div>
          <h2>最近提醒</h2>
          <p>只展示脱敏摘要，点“处理”进入对应运营页。</p>
        </div>
        <el-button text type="primary" @click="showSection('exceptions')">查看全部</el-button>
      </div>
      <div v-if="latestNotifications.length === 0" class="empty-state">
        当前没有新的运营提醒。
      </div>
      <div v-else class="ops-notification-list">
        <button
          v-for="notification in latestNotifications"
          :key="notification.id"
          class="ops-notification-item"
          @click="ctx.openNotification(notification)"
        >
          <el-tag :type="ctx.notificationSeverityType(notification.severity)">
            {{ ctx.notificationSeverityLabel(notification.severity) }}
          </el-tag>
          <span>{{ ctx.notificationSourceLabel(notification.source_type) }}</span>
          <strong>{{ notification.title }}</strong>
          <small>{{ ctx.formatDateTime(notification.updated_at) }}</small>
        </button>
      </div>
    </div>
  </section>
</template>
