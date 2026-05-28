<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  dashboard,
  formatDateTime,
  unreadNotificationCount,
} = props.ctx;
</script>

<template>
      <section class="section-grid">
        <div class="metric critical">
          <span>待处理订单</span>
          <strong>{{ dashboard?.pending_order_count ?? 0 }}</strong>
          <em>待采购 / 待发货 / 售后 / 超时</em>
        </div>
        <div class="metric warning">
          <span>异常店铺</span>
          <strong>{{ dashboard?.abnormal_shop_count ?? 0 }}</strong>
          <em>未验证、接口失败或暂停同步</em>
        </div>
        <div class="metric danger">
          <span>铺货失败商品</span>
          <strong>{{ dashboard?.failed_publish_product_count ?? 0 }}</strong>
          <em>按商品聚合查看失败原因</em>
        </div>
        <div class="metric warning">
          <span>未读通知</span>
          <strong>{{ dashboard?.unread_notification_count ?? unreadNotificationCount }}</strong>
          <em>铺货、履约、售后和同步异常</em>
        </div>
        <div class="metric neutral">
          <span>运行中任务</span>
          <strong>{{ dashboard?.running_task_count ?? 0 }}</strong>
          <em>{{ dashboard?.controller_status || "主控机状态未知" }}</em>
        </div>

        <div class="panel wide">
          <div class="panel-title">
            <h2>最近状态</h2>
            <el-tag type="success">{{ formatDateTime(dashboard?.now_shanghai) }}</el-tag>
          </div>
          <dl class="status-list">
            <div>
              <dt>最近订单同步</dt>
              <dd>{{ dashboard?.last_order_sync_at ? formatDateTime(dashboard.last_order_sync_at) : "尚未同步" }}</dd>
            </div>
            <div>
              <dt>最近铺货任务</dt>
              <dd>{{ dashboard?.last_publish_summary || "尚未创建" }}</dd>
            </div>
          </dl>
        </div>
      </section>
</template>
