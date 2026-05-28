<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  Bell,
  CircleCheck,
  criticalNotificationCount,
  formatDateTime,
  markAllNotificationsRead,
  markNotificationRead,
  notifications,
  notificationSeverityFilter,
  notificationSeverityLabel,
  notificationSeverityType,
  notificationSourceLabel,
  notificationStatusFilter,
  notificationTotal,
  openNotification,
  Refresh,
  refreshNotifications,
  statusType,
  unreadNotificationCount,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>通知中心</h2>
              <p>集中展示铺货失败、采购映射缺失、履约发货失败、售后待处理和店铺同步异常。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="notificationStatusFilter"
                class="status-filter"
                @change="refreshNotifications"
              >
                <el-option value="unread" label="未读" />
                <el-option value="all" label="全部状态" />
                <el-option value="read" label="已读" />
              </el-select>
              <el-select
                v-model="notificationSeverityFilter"
                class="status-filter"
                @change="refreshNotifications"
              >
                <el-option value="all" label="全部级别" />
                <el-option value="critical" label="严重" />
                <el-option value="warning" label="提醒" />
                <el-option value="info" label="信息" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshNotifications">刷新</el-button>
              <el-button :icon="CircleCheck" @click="markAllNotificationsRead">全部已读</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>未读通知</dt>
              <dd>{{ unreadNotificationCount }}</dd>
            </div>
            <div>
              <dt>严重未读</dt>
              <dd>{{ criticalNotificationCount }}</dd>
            </div>
            <div>
              <dt>当前筛选</dt>
              <dd>{{ notificationTotal }}</dd>
            </div>
          </dl>

          <el-table :data="notifications" class="dense-table">
            <el-table-column label="级别" width="90">
              <template #default="{ row }">
                <el-tag :type="notificationSeverityType(row.severity)">
                  {{ notificationSeverityLabel(row.severity) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="90">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status === "unread" ? "未读" : "已读" }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="来源" min-width="120">
              <template #default="{ row }">
                {{ notificationSourceLabel(row.source_type) }}
              </template>
            </el-table-column>
            <el-table-column label="店铺" min-width="130">
              <template #default="{ row }">
                {{ row.shop_name || row.shop_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="通知" min-width="380" show-overflow-tooltip>
              <template #default="{ row }">
                <strong class="notification-title">{{ row.title }}</strong>
                <span class="notification-body">{{ row.body }}</span>
              </template>
            </el-table-column>
            <el-table-column label="更新时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
            </el-table-column>
            <el-table-column label="操作" width="170">
              <template #default="{ row }">
                <el-button size="small" :icon="Bell" @click="openNotification(row)">定位</el-button>
                <el-button
                  v-if="row.status !== 'read'"
                  size="small"
                  :icon="CircleCheck"
                  @click="markNotificationRead(row)"
                >
                  已读
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
