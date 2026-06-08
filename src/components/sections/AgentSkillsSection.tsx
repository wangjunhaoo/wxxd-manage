/* ============================================================================
   Agent 技能 —— 管理 AI Agent 可调用的业务技能（启停、详情、试跑）+ 最近运行记录
   忠实保留原 .vue 的 IA / 交互 / 中文文案，视觉换成 Soft（primitives + app.css）。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { PageHead, Button, Pill, Select } from "../primitives";
import type { AgentRunView, AgentSkillView } from "../../types/app";

// ---- 本地纯映射函数（原 .vue 内定义，非 ctx 成员）------------------------

// 技能文件/运行时状态 → 中文
function statusLabel(status: string): string {
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

// 技能文件/运行时状态 → tag 色调
function statusType(status: string): string {
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

// 运行状态 → 中文
function agentRunStatusLabel(status: string): string {
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

// 运行状态 → tag 色调
function agentRunStatusType(status: string): string {
  if (status === "succeeded") return "success";
  if (status === "running" || status === "needs_review" || status === "blocked")
    return "warning";
  return "danger";
}

// 场景 → 中文
function agentRunSceneLabel(scene: string): string {
  const labels: Record<string, string> = {
    collection_review: "采集审查",
    publish_attribute: "铺货补齐",
    supplier_bridge: "供应商桥",
  };
  return labels[scene] || scene;
}

// 耗时格式化
function formatAgentRunDuration(row: AgentRunView): string {
  if (row.duration_ms === null || row.duration_ms === undefined) return "-";
  if (row.duration_ms < 1000) return `${row.duration_ms}ms`;
  return `${(row.duration_ms / 1000).toFixed(1)}s`;
}

const SCENE_OPTIONS = [
  { label: "全部场景", value: "all" },
  { label: "采集审查", value: "collection_review" },
  { label: "铺货补齐", value: "publish_attribute" },
  { label: "供应商桥", value: "supplier_bridge" },
];

const STATUS_OPTIONS = [
  { label: "全部状态", value: "all" },
  { label: "运行中", value: "running" },
  { label: "成功", value: "succeeded" },
  { label: "待人工确认", value: "needs_review" },
  { label: "已拦截", value: "blocked" },
  { label: "失败", value: "failed" },
];

export default function AgentSkillsSection() {
  const ctx = useApp();

  const skills = ctx.agentSkills.value;
  const runs = ctx.agentRuns.value;
  const provider = ctx.aiProviderSettings.value;
  const selected: AgentSkillView | undefined = ctx.selectedAgentSkill.value;
  const skillsLoading = ctx.agentSkillsLoading.value;
  const runsLoading = ctx.agentRunsLoading.value;

  // 行点击 → 选中技能（驱动右侧详情）
  const selectSkill = (row: AgentSkillView) => {
    ctx.selectedAgentSkillName.value = row.name;
  };

  return (
    <div className="pad">
      <div className="wrap-wide">
        {/* ---- Panel A：标题 + KPI ---- */}
        <PageHead
          eyebrow="Agent 技能 · Asia/Shanghai"
          title="Agent 技能"
          desc="管理 AI Agent 可调用的业务技能；启停会影响对应自动流程。"
          actions={
            <Button
              icon="refresh"
              disabled={skillsLoading}
              onClick={() => ctx.refreshAgentSkills()}
            >
              {skillsLoading ? "校验中…" : "刷新校验"}
            </Button>
          }
        />

        <div className="kpis" style={{ marginBottom: 16 }}>
          <div className="kpi">
            <span>技能数</span>
            <strong>{skills.length}</strong>
          </div>
          <div className="kpi">
            <span>已启用</span>
            <strong>{ctx.enabledAgentSkillCount.value}</strong>
          </div>
          <div className="kpi">
            <span>运行就绪</span>
            <strong>{ctx.readyAgentSkillCount.value}</strong>
          </div>
          <div className="kpi">
            <span>AI Agent</span>
            <strong>
              {provider.enabled ? ctx.aiProviderRuntimeLabel.value : "未启用"}
            </strong>
          </div>
        </div>

        {/* ---- Panel B：技能列表 + 详情侧栏 ---- */}
        <div className="split" style={{ marginBottom: 16 }}>
          <div className="panel tight">
            <div className="tbl-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>技能</th>
                    <th>状态</th>
                    <th>文件</th>
                    <th>运行时</th>
                    <th>最近试跑</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {skills.length === 0 ? (
                    <tr>
                      <td colSpan={6} className="text-muted" style={{ textAlign: "center" }}>
                        暂无数据
                      </td>
                    </tr>
                  ) : (
                    skills.map((row) => (
                      <tr
                        key={row.name}
                        className={
                          "clickable" +
                          (ctx.selectedAgentSkillName.value === row.name ? " selected" : "")
                        }
                        onClick={() => selectSkill(row)}
                      >
                        <td>
                          <div className="cell-main">
                            <strong>{row.name}</strong>
                            <span className="mono">v{row.version}</span>
                          </div>
                        </td>
                        <td>
                          <Pill tone={row.enabled ? "success" : "info"}>
                            {row.enabled ? "启用" : "停用"}
                          </Pill>
                        </td>
                        <td>
                          <Pill tone={statusType(row.file_status)}>
                            {statusLabel(row.file_status)}
                          </Pill>
                        </td>
                        <td>
                          <Pill tone={statusType(row.runtime_status)}>
                            {statusLabel(row.runtime_status)}
                          </Pill>
                        </td>
                        <td>{row.last_test_summary || "-"}</td>
                        <td>
                          {/* 阻止冒泡，避免行内按钮误触发行选中（原 @click.stop） */}
                          <div
                            className="row-actions"
                            onClick={(e: React.MouseEvent) => e.stopPropagation()}
                          >
                            <Button
                              size="sm"
                              variant={row.enabled ? "ghost" : "accent"}
                              disabled={ctx.agentSkillSaving.value === row.name}
                              onClick={() => ctx.saveAgentSkill(row, !row.enabled)}
                            >
                              {row.enabled ? "停用" : "启用"}
                            </Button>
                            <Button
                              size="sm"
                              variant="accent"
                              disabled={ctx.agentSkillTesting.value === row.name}
                              onClick={() => ctx.testAgentSkill(row)}
                            >
                              试跑
                            </Button>
                          </div>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>

          {selected && (
            <aside className="panel tight sticky-col">
              <div className="ph">
                <div>
                  <h3>{selected.name}</h3>
                  <p>{selected.description}</p>
                </div>
                <Pill tone={selected.enabled ? "success" : "info"}>
                  {selected.enabled ? "启用中" : "已停用"}
                </Pill>
              </div>

              <div className="kpis" style={{ marginBottom: 12 }}>
                <div className="kpi">
                  <span>版本</span>
                  <strong>{selected.version}</strong>
                </div>
                <div className="kpi">
                  <span>运行时</span>
                  <strong>{selected.runtime}</strong>
                </div>
                <div className="kpi">
                  <span>模型</span>
                  <strong>{selected.model || provider.model || "继承全局"}</strong>
                </div>
                <div className="kpi">
                  <span>Temperature</span>
                  <strong>{selected.temperature ?? provider.temperature}</strong>
                </div>
              </div>

              <div className="kv-grid">
                <div className="kv">
                  <dt>SKILL.md</dt>
                  <dd className="mono">{selected.skill_path}</dd>
                </div>
                <div className="kv">
                  <dt>输出 Schema</dt>
                  <dd className="mono">{selected.schema_path}</dd>
                </div>
                <div className="kv">
                  <dt>Checksum</dt>
                  <dd className="mono">{selected.checksum.slice(0, 16)}</dd>
                </div>
              </div>

              <div className="kv-grid" style={{ marginTop: 12 }}>
                <div className="kv">
                  <dt>最近试跑</dt>
                  <dd className="mono">
                    {selected.last_test_at
                      ? ctx.formatDateTime(selected.last_test_at)
                      : "未试跑"}
                  </dd>
                </div>
                <div className="kv">
                  <dt>结果</dt>
                  <dd className="mono">{selected.last_test_summary || "-"}</dd>
                </div>
              </div>
            </aside>
          )}
        </div>

        {/* ---- Panel C：最近运行 ---- */}
        <div className="panel tight">
          <div className="ph">
            <div>
              <h3>最近运行</h3>
              <p>采集审查、铺货自动补齐和供应商桥的 Agent 执行记录。</p>
            </div>
            <div className="toolbar">
              <Select
                value={ctx.agentRunSceneFilter.value}
                width={132}
                options={SCENE_OPTIONS}
                onChange={(v: string) => {
                  ctx.agentRunSceneFilter.value = v;
                  ctx.refreshAgentRuns();
                }}
              />
              <Select
                value={ctx.agentRunStatusFilter.value}
                width={132}
                options={STATUS_OPTIONS}
                onChange={(v: string) => {
                  ctx.agentRunStatusFilter.value = v;
                  ctx.refreshAgentRuns();
                }}
              />
              <Button
                icon="refresh"
                size="sm"
                disabled={runsLoading}
                onClick={() => ctx.refreshAgentRuns()}
              >
                刷新
              </Button>
            </div>
          </div>

          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>场景</th>
                  <th>状态</th>
                  <th>技能</th>
                  <th>来源</th>
                  <th>输入摘要</th>
                  <th>决策</th>
                  <th>错误</th>
                  <th>耗时</th>
                  <th>时间</th>
                </tr>
              </thead>
              <tbody>
                {runs.length === 0 ? (
                  <tr>
                    <td colSpan={9} className="text-muted" style={{ textAlign: "center" }}>
                      暂无数据
                    </td>
                  </tr>
                ) : (
                  runs.map((row) => (
                    <tr key={row.id}>
                      <td>{agentRunSceneLabel(row.scene)}</td>
                      <td>
                        <Pill tone={agentRunStatusType(row.status)}>
                          {agentRunStatusLabel(row.status)}
                        </Pill>
                      </td>
                      <td>
                        <div className="cell-main">
                          <strong>{row.skill_name}</strong>
                          <span className="mono">v{row.skill_version}</span>
                        </div>
                      </td>
                      <td>
                        <code className="mono">
                          {row.source_type} / {row.source_id}
                        </code>
                      </td>
                      <td>{row.input_summary}</td>
                      <td>{row.decision || "-"}</td>
                      <td>{row.error_summary || row.error_code || "-"}</td>
                      <td>{formatAgentRunDuration(row)}</td>
                      <td>{ctx.formatDateTime(row.created_at)}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}
