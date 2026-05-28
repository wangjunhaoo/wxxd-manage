<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  backupRunning,
  createDatabaseBackup,
  dashboard,
  databaseBackups,
  formatBytes,
  formatDateTime,
  Refresh,
  refreshDatabaseBackups,
  restoreDatabaseBackup,
  revealBackup,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>数据备份</h2>
              <p>备份保存到应用数据目录，创建和恢复都会做 SQLite 完整性校验；恢复前会自动生成回滚备份。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshDatabaseBackups">刷新</el-button>
              <el-button
                type="primary"
                :icon="UploadFilled"
                :loading="backupRunning"
                @click="createDatabaseBackup"
              >
                创建备份
              </el-button>
            </div>
          </div>
          <dl class="status-list compact">
            <div>
              <dt>当前数据库</dt>
              <dd>{{ dashboard?.database_path || "-" }}</dd>
            </div>
            <div>
              <dt>备份数量</dt>
              <dd>{{ databaseBackups.length }}</dd>
            </div>
            <div>
              <dt>最近备份</dt>
              <dd>{{ databaseBackups[0]?.created_at ? formatDateTime(databaseBackups[0].created_at) : "尚未备份" }}</dd>
            </div>
          </dl>
          <el-table :data="databaseBackups" class="dense-table">
            <el-table-column prop="file_name" label="备份文件" min-width="270" show-overflow-tooltip />
            <el-table-column label="大小" width="110">
              <template #default="{ row }">
                {{ formatBytes(row.size_bytes) }}
              </template>
            </el-table-column>
            <el-table-column label="创建时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.created_at) }}</template>
            </el-table-column>
            <el-table-column label="校验" width="110">
              <template #default="{ row }">
                <el-tag :type="row.integrity_ok ? 'success' : 'danger'">
                  {{ row.integrity_ok ? "通过" : "失败" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="SHA-256" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.sha256 }}
              </template>
            </el-table-column>
            <el-table-column prop="integrity_message" label="校验信息" min-width="150" show-overflow-tooltip />
            <el-table-column label="操作" width="170">
              <template #default="{ row }">
                <el-button size="small" @click="revealBackup(row)">定位</el-button>
                <el-button
                  size="small"
                  type="warning"
                  :disabled="!row.integrity_ok || backupRunning"
                  @click="restoreDatabaseBackup(row)"
                >
                  恢复
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
