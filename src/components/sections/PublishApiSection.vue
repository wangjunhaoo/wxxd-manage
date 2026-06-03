<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  createPublishJob,
  externalApiLogs,
  formatDateTime,
  latestTaskId,
  localApiConfig,
  publishPayload,
  Refresh,
  refreshExternalApiLogs,
  rotatedLocalApiKey,
  rotateLocalApiKey,
  UploadFilled,
} = props.ctx;
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>本地主控 HTTP API</h2>
          <p>外部系统调用本机地址创建铺货任务、商品售价调整任务和查询状态。</p>
        </div>
        <el-tag :type="localApiConfig.has_api_key ? 'success' : 'warning'">
          {{ localApiConfig.has_api_key ? "已生成 Key" : "未生成 Key" }}
        </el-tag>
      </div>
      <dl class="status-list compact">
        <div>
          <dt>Base URL</dt>
          <dd>{{ localApiConfig.base_url }}</dd>
        </div>
        <div>
          <dt>认证头</dt>
          <dd>{{ localApiConfig.auth_header }}</dd>
        </div>
        <div>
          <dt>Key 状态</dt>
          <dd>{{ localApiConfig.api_key_hint || "未生成" }}</dd>
        </div>
      </dl>
      <div class="api-endpoints">
        <code>GET /api/shop-groups</code>
        <code>POST /api/publish-jobs</code>
        <code>GET /api/publish-jobs/:task_id</code>
        <code>POST /api/price-update-jobs（商品售价）</code>
        <code>GET /api/price-update-jobs/:task_id（商品售价）</code>
        <code>GET /api/task-runs</code>
        <code>POST /api/runners/price-submit</code>
        <code>POST /api/runners/publish-pipeline</code>
        <code>POST /api/runners/operations</code>
      </div>
      <div class="action-row">
        <el-button type="primary" @click="rotateLocalApiKey">
          {{ localApiConfig.has_api_key ? "重置 API Key" : "生成 API Key" }}
        </el-button>
        <el-tag type="info">只监听 127.0.0.1:{{ localApiConfig.port }}</el-tag>
      </div>
      <el-alert
        v-if="rotatedLocalApiKey"
        type="warning"
        :closable="false"
        show-icon
        title="API Key 只展示这一次，外部系统请保存后使用。"
      >
        <template #default>
          <el-input v-model="rotatedLocalApiKey" readonly />
        </template>
      </el-alert>
    </div>

    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>外部 API 审计</h2>
          <p>
            只记录调用方法、路径、状态、耗时和安全摘要，不保存 API Key
            或完整请求体。
          </p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="refreshExternalApiLogs"
            >刷新</el-button
          >
          <el-tag>{{ externalApiLogs.length }} 条</el-tag>
        </div>
      </div>
      <el-table :data="externalApiLogs" class="dense-table">
        <el-table-column label="时间" min-width="190">
          <template #default="{ row }">{{
            formatDateTime(row.created_at)
          }}</template>
        </el-table-column>
        <el-table-column prop="method" label="方法" width="90" />
        <el-table-column
          prop="path"
          label="路径"
          min-width="230"
          show-overflow-tooltip
        />
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="row.status === 'success' ? 'success' : 'danger'">
              {{ row.status_code }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="耗时" width="100">
          <template #default="{ row }"> {{ row.duration_ms }} ms </template>
        </el-table-column>
        <el-table-column
          prop="request_summary"
          label="请求摘要"
          min-width="260"
          show-overflow-tooltip
        >
          <template #default="{ row }">
            {{ row.request_summary || "-" }}
          </template>
        </el-table-column>
        <el-table-column
          prop="response_summary"
          label="响应摘要"
          min-width="150"
          show-overflow-tooltip
        >
          <template #default="{ row }">
            {{ row.response_summary || row.error_code || "-" }}
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div class="panel">
      <div class="panel-title">
        <h2>创建外部铺货任务</h2>
        <p>
          外部系统可直接按同一 JSON 协议调用本机 HTTP
          API，桌面端保留手工创建入口。
        </p>
      </div>
      <el-input
        v-model="publishPayload"
        type="textarea"
        :autosize="{ minRows: 18, maxRows: 28 }"
        spellcheck="false"
        class="json-editor"
      />
      <div class="action-row">
        <el-button type="primary" :icon="UploadFilled" @click="createPublishJob"
          >创建铺货任务</el-button
        >
        <el-tag v-if="latestTaskId">最新任务：{{ latestTaskId }}</el-tag>
      </div>
    </div>
  </section>
</template>
