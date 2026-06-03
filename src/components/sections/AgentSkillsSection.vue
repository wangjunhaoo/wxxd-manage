<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import type { AgentRunView, AgentSkillView } from "../../types/app";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  agentRuns,
  agentRunSceneFilter,
  agentRunStatusFilter,
  agentRunsLoading,
  agentSkillSaving,
  agentSkills,
  agentSkillsLoading,
  agentSkillTesting,
  aiProviderSettings,
  aiProviderRuntimeLabel,
  enabledAgentSkillCount,
  formatDateTime,
  readyAgentSkillCount,
  Refresh,
  refreshAgentRuns,
  refreshAgentSkills,
  saveAgentSkill,
  selectedAgentSkill,
  selectedAgentSkillName,
  testAgentSkill,
} = props.ctx;

function statusLabel(status: string) {
  const labels: Record<string, string> = {
    ready: "可用",
    sdk_missing: "缺 SDK",
    cli_missing: "缺 CLI",
    script_missing: "缺脚本",
    python_missing: "缺 Python",
    node_missing: "缺 Node",
    schema_invalid: "Schema 异常",
    skill_metadata_missing: "元数据缺失",
    success: "成功",
    failed: "失败",
  };
  return labels[status] || status;
}

function agentRunStatusLabel(status: string) {
  const labels: Record<string, string> = {
    running: "运行中",
    succeeded: "成功",
    needs_review: "待人工确认",
    blocked: "已拦截",
    failed: "失败",
    cancelled: "已取消",
  };
  return labels[status] || status;
}

function agentRunStatusType(status: string) {
  if (status === "succeeded") return "success";
  if (status === "running" || status === "needs_review" || status === "blocked")
    return "warning";
  return "danger";
}

function agentRunSceneLabel(scene: string) {
  const labels: Record<string, string> = {
    collection_review: "采集审查",
    publish_attribute: "铺货补齐",
    supplier_bridge: "供应商桥",
  };
  return labels[scene] || scene;
}

function formatAgentRunDuration(row: AgentRunView) {
  if (row.duration_ms === null || row.duration_ms === undefined) return "-";
  if (row.duration_ms < 1000) return `${row.duration_ms}ms`;
  return `${(row.duration_ms / 1000).toFixed(1)}s`;
}

function statusType(status: string) {
  if (status === "ready" || status === "success") return "success";
  if (
    status === "sdk_missing" ||
    status === "node_missing" ||
    status === "python_missing" ||
    status === "script_missing" ||
    status === "cli_missing"
  )
    return "warning";
  return "danger";
}

function selectSkill(row: AgentSkillView) {
  selectedAgentSkillName.value = row.name;
}
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>Agent 技能</h2>
          <p>管理 AI Agent 可调用的业务技能；启停会影响对应自动流程。</p>
        </div>
        <el-button
          :icon="Refresh"
          :loading="agentSkillsLoading"
          @click="refreshAgentSkills"
        >
          刷新校验
        </el-button>
      </div>

      <dl class="status-list compact">
        <div>
          <dt>技能数</dt>
          <dd>{{ agentSkills.length }}</dd>
        </div>
        <div>
          <dt>已启用</dt>
          <dd>{{ enabledAgentSkillCount }}</dd>
        </div>
        <div>
          <dt>运行就绪</dt>
          <dd>{{ readyAgentSkillCount }}</dd>
        </div>
        <div>
          <dt>AI Agent</dt>
          <dd>
            {{ aiProviderSettings.enabled ? aiProviderRuntimeLabel : "未启用" }}
          </dd>
        </div>
      </dl>
    </div>

    <div class="panel agent-skill-layout">
      <div class="agent-skill-list">
        <el-table
          v-loading="agentSkillsLoading"
          :data="agentSkills"
          class="dense-table compact-table"
          row-key="name"
          highlight-current-row
          @row-click="selectSkill"
        >
          <el-table-column label="技能" min-width="220" show-overflow-tooltip>
            <template #default="{ row }">
              <div class="skill-name-cell">
                <strong>{{ row.name }}</strong>
                <small>v{{ row.version }}</small>
              </div>
            </template>
          </el-table-column>
          <el-table-column label="状态" width="110">
            <template #default="{ row }">
              <el-tag :type="row.enabled ? 'success' : 'info'">
                {{ row.enabled ? "启用" : "停用" }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="文件" width="110">
            <template #default="{ row }">
              <el-tag :type="statusType(row.file_status)">
                {{ statusLabel(row.file_status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="运行时" width="110">
            <template #default="{ row }">
              <el-tag :type="statusType(row.runtime_status)">
                {{ statusLabel(row.runtime_status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column
            label="最近试跑"
            min-width="140"
            show-overflow-tooltip
          >
            <template #default="{ row }">
              {{ row.last_test_summary || "-" }}
            </template>
          </el-table-column>
          <el-table-column label="操作" width="180" fixed="right">
            <template #default="{ row }">
              <div class="row-actions">
                <el-button
                  size="small"
                  text
                  :type="row.enabled ? 'warning' : 'primary'"
                  :loading="agentSkillSaving === row.name"
                  @click.stop="saveAgentSkill(row, !row.enabled)"
                >
                  {{ row.enabled ? "停用" : "启用" }}
                </el-button>
                <el-button
                  size="small"
                  type="primary"
                  text
                  :loading="agentSkillTesting === row.name"
                  @click.stop="testAgentSkill(row)"
                >
                  试跑
                </el-button>
              </div>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <aside v-if="selectedAgentSkill" class="agent-skill-detail">
        <div class="detail-title">
          <div>
            <h3>{{ selectedAgentSkill.name }}</h3>
            <p>{{ selectedAgentSkill.description }}</p>
          </div>
          <el-tag :type="selectedAgentSkill.enabled ? 'success' : 'info'">
            {{ selectedAgentSkill.enabled ? "启用中" : "已停用" }}
          </el-tag>
        </div>

        <dl class="status-list compact">
          <div>
            <dt>版本</dt>
            <dd>{{ selectedAgentSkill.version }}</dd>
          </div>
          <div>
            <dt>运行时</dt>
            <dd>{{ selectedAgentSkill.runtime }}</dd>
          </div>
          <div>
            <dt>模型</dt>
            <dd>
              {{
                selectedAgentSkill.model ||
                aiProviderSettings.model ||
                "继承全局"
              }}
            </dd>
          </div>
          <div>
            <dt>Temperature</dt>
            <dd>
              {{
                selectedAgentSkill.temperature ?? aiProviderSettings.temperature
              }}
            </dd>
          </div>
        </dl>

        <div class="detail-lines">
          <div>
            <span>SKILL.md</span>
            <code>{{ selectedAgentSkill.skill_path }}</code>
          </div>
          <div>
            <span>输出 Schema</span>
            <code>{{ selectedAgentSkill.schema_path }}</code>
          </div>
          <div>
            <span>Checksum</span>
            <code>{{ selectedAgentSkill.checksum.slice(0, 16) }}</code>
          </div>
        </div>

        <div class="detail-lines">
          <div>
            <span>最近试跑</span>
            <code>{{
              selectedAgentSkill.last_test_at
                ? formatDateTime(selectedAgentSkill.last_test_at)
                : "未试跑"
            }}</code>
          </div>
          <div>
            <span>结果</span>
            <code>{{ selectedAgentSkill.last_test_summary || "-" }}</code>
          </div>
        </div>
      </aside>
    </div>

    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>最近运行</h2>
          <p>采集审查、铺货自动补齐和供应商桥的 Agent 执行记录。</p>
        </div>
        <div class="toolbar compact-toolbar">
          <el-select
            v-model="agentRunSceneFilter"
            size="small"
            style="width: 132px"
            @change="refreshAgentRuns"
          >
            <el-option label="全部场景" value="all" />
            <el-option label="采集审查" value="collection_review" />
            <el-option label="铺货补齐" value="publish_attribute" />
            <el-option label="供应商桥" value="supplier_bridge" />
          </el-select>
          <el-select
            v-model="agentRunStatusFilter"
            size="small"
            style="width: 132px"
            @change="refreshAgentRuns"
          >
            <el-option label="全部状态" value="all" />
            <el-option label="运行中" value="running" />
            <el-option label="成功" value="succeeded" />
            <el-option label="待人工确认" value="needs_review" />
            <el-option label="已拦截" value="blocked" />
            <el-option label="失败" value="failed" />
          </el-select>
          <el-button
            :icon="Refresh"
            :loading="agentRunsLoading"
            @click="refreshAgentRuns"
          >
            刷新
          </el-button>
        </div>
      </div>

      <el-table
        v-loading="agentRunsLoading"
        :data="agentRuns"
        class="dense-table compact-table"
        row-key="id"
      >
        <el-table-column label="场景" width="112">
          <template #default="{ row }">
            {{ agentRunSceneLabel(row.scene) }}
          </template>
        </el-table-column>
        <el-table-column label="状态" width="112">
          <template #default="{ row }">
            <el-tag :type="agentRunStatusType(row.status)">
              {{ agentRunStatusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="技能" min-width="190" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="skill-name-cell">
              <strong>{{ row.skill_name }}</strong>
              <small>v{{ row.skill_version }}</small>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="来源" min-width="160" show-overflow-tooltip>
          <template #default="{ row }">
            <code>{{ row.source_type }} / {{ row.source_id }}</code>
          </template>
        </el-table-column>
        <el-table-column label="输入摘要" min-width="260" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.input_summary }}
          </template>
        </el-table-column>
        <el-table-column label="决策" width="120" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.decision || "-" }}
          </template>
        </el-table-column>
        <el-table-column label="错误" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.error_summary || row.error_code || "-" }}
          </template>
        </el-table-column>
        <el-table-column label="耗时" width="90">
          <template #default="{ row }">
            {{ formatAgentRunDuration(row) }}
          </template>
        </el-table-column>
        <el-table-column label="时间" min-width="150" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatDateTime(row.created_at) }}
          </template>
        </el-table-column>
      </el-table>
    </div>
  </section>
</template>
