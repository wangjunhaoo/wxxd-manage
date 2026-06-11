/* ============================================================================
   AI 使用记录 —— 以 Agent 运行记录为主体：列表（场景/状态筛选）+ 选中运行的
   执行流程时间线（agent_run_events），技能启停收敛为顶部一条紧凑开关条。
   运行中的任务自动轮询刷新（列表 5s + 选中运行的事件流）。
   ============================================================================ */
import { useEffect, useMemo, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { PageHead, Button, Pill, Select, Switch, Callout } from "../primitives";
import type { AgentRunEventView, AgentRunView } from "../../types/app";

// ---- 纯映射函数 ------------------------------------------------------------

// 技能文件/运行时状态 → 中文
function skillStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    ready: "可用",
    sdk_missing: "缺 SDK",
    cli_missing: "缺 CLI",
    script_missing: "缺脚本",
    python_missing: "缺 Python",
    node_missing: "缺 Node",
    schema_invalid: "Schema 异常",
    skill_metadata_missing: "元数据缺失",
  };
  return labels[status] || status;
}

// 运行状态 → 中文 + tag 色调
function runStatusLabel(status: string): string {
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

function runStatusTone(status: string): "success" | "warning" | "danger" {
  if (status === "succeeded") return "success";
  if (status === "running" || status === "needs_review" || status === "blocked")
    return "warning";
  return "danger";
}

// 场景 → 中文
function sceneLabel(scene: string): string {
  const labels: Record<string, string> = {
    collection_review: "采集审查",
    publish_attribute: "铺货补齐",
    supplier_bridge: "供应商桥",
    skill_test: "技能试跑",
  };
  return labels[scene] || scene;
}

// 决策 → 中文（未知值原样展示）
function decisionLabel(decision: string | null): string {
  if (!decision) return "—";
  const labels: Record<string, string> = {
    passed: "通过",
    success: "成功",
    needs_review: "需人工确认",
    blocked: "已拦截",
    ready_to_publish: "可铺货",
  };
  return labels[decision] || decision;
}

// 事件类型 → 中文 + 时间线圆点色调
const EVENT_META: Record<string, { label: string; dot: string }> = {
  input_prepared: { label: "输入准备", dot: "" },
  finished: { label: "执行完成", dot: "ok" },
  human_gate_required: { label: "待人工确认", dot: "warn" },
  blocked: { label: "业务拦截", dot: "warn" },
  failed: { label: "执行失败", dot: "err" },
};

function eventMeta(event: AgentRunEventView): { label: string; dot: string } {
  const meta = EVENT_META[event.event_type];
  if (meta) return meta;
  // 未知事件类型按 level 着色，标签原样展示。
  const dot = event.level === "error" ? "err" : event.level === "warn" ? "warn" : "";
  return { label: event.event_type, dot };
}

function formatMs(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "—";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** data_json 美化：合法 JSON 缩进展示，超长截断防止撑爆面板。 */
function prettyEventData(raw: string | null): string | null {
  if (!raw) return null;
  let text = raw;
  try {
    text = JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    /* 非 JSON 原样展示 */
  }
  if (text.length > 20_000) {
    return `${text.slice(0, 20_000)}\n…（内容过长已截断）`;
  }
  return text;
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

export default function AiActivitySection() {
  const ctx = useApp();

  const skills = ctx.agentSkills.value;
  const runs = ctx.agentRuns.value;
  const provider = ctx.aiProviderSettings.value;
  const runsLoading = ctx.agentRunsLoading.value;

  // ---- 选中运行 + 事件流（本地状态，按需懒加载）----
  const [selectedRunId, setSelectedRunId] = useState<string>("");
  const [events, setEvents] = useState<AgentRunEventView[]>([]);
  const [eventsLoading, setEventsLoading] = useState(false);

  // 未手动选择时默认选最新一条，进页面即可看到执行流程。
  const activeRun: AgentRunView | undefined = useMemo(
    () => runs.find((run) => run.id === selectedRunId) ?? runs[0],
    [runs, selectedRunId],
  );

  async function loadEvents(runId: string, silent = false) {
    if (!silent) setEventsLoading(true);
    try {
      const list = await ctx.command<AgentRunEventView[]>("list_agent_run_events", {
        runId,
      });
      setEvents(list);
    } catch {
      if (!silent) setEvents([]);
    } finally {
      if (!silent) setEventsLoading(false);
    }
  }

  // 切换选中运行 → 重新拉事件流。
  useEffect(() => {
    if (!activeRun) {
      setEvents([]);
      return;
    }
    void loadEvents(activeRun.id);
  }, [activeRun?.id]);

  // 有运行中的任务时自动轮询：列表 5s 一刷，选中的运行同步刷事件流。
  useEffect(() => {
    const hasRunning = runs.some((run) => run.status === "running");
    if (!hasRunning) return;
    const timer = setInterval(() => {
      void ctx.refreshAgentRuns();
      if (activeRun && activeRun.status === "running") {
        void loadEvents(activeRun.id, true);
      }
    }, 5000);
    return () => clearInterval(timer);
  }, [runs, activeRun?.id]);

  // ---- KPI（基于已加载的最近运行，列表上限 80 条）----
  const stats = useMemo(() => {
    const succeeded = runs.filter((run) => run.status === "succeeded").length;
    const failed = runs.filter((run) => run.status === "failed").length;
    const attention = runs.filter((run) =>
      ["running", "needs_review", "blocked"].includes(run.status),
    ).length;
    const durations = runs
      .map((run) => run.duration_ms)
      .filter((value): value is number => value !== null && value !== undefined);
    const avgMs = durations.length
      ? Math.round(durations.reduce((sum, value) => sum + value, 0) / durations.length)
      : null;
    return { total: runs.length, succeeded, failed, attention, avgMs };
  }, [runs]);

  return (
    <div className="pad">
      <div className="wrap-wide">
        {/* ---- 标题 + KPI ---- */}
        <PageHead
          eyebrow="AI 使用记录 · Asia/Shanghai"
          title="AI 使用记录"
          desc="AI Agent 每次执行的输入、决策与完整事件流；下方技能开关影响对应自动流程。"
          actions={
            <Button
              icon="refresh"
              disabled={runsLoading}
              onClick={() => {
                void ctx.refreshAgentRuns();
                void ctx.refreshAgentSkills();
                if (activeRun) void loadEvents(activeRun.id, true);
              }}
            >
              {runsLoading ? "刷新中…" : "刷新"}
            </Button>
          }
        />

        <div className="kpis cols-5" style={{ marginBottom: 16 }}>
          <div className="kpi">
            <span>最近运行</span>
            <strong>{stats.total}</strong>
          </div>
          <div className="kpi">
            <span>成功</span>
            <strong>{stats.succeeded}</strong>
          </div>
          <div className="kpi">
            <span>失败</span>
            <strong>{stats.failed}</strong>
          </div>
          <div className="kpi">
            <span>待处理</span>
            <strong>{stats.attention}</strong>
          </div>
          <div className="kpi">
            <span>平均耗时</span>
            <strong>{formatMs(stats.avgMs)}</strong>
          </div>
        </div>

        {/* ---- 技能开关条（紧凑）---- */}
        <div className="panel tight" style={{ marginBottom: 16 }}>
          <div className="skill-strip">
            {skills.map((skill) => (
              <div key={skill.name} className="skill-strip-item">
                <div className="cell-main" style={{ minWidth: 0, flex: 1 }}>
                  <strong>
                    {skill.name}
                    <span className="mono" style={{ marginLeft: 6, fontWeight: 400 }}>
                      v{skill.version}
                    </span>
                  </strong>
                  <span className="ellipsis">{skill.description}</span>
                </div>
                <Pill tone={skill.runtime_status === "ready" ? "success" : "warning"}>
                  {skillStatusLabel(skill.runtime_status)}
                </Pill>
                <span className="text-muted" style={{ fontSize: 12 }}>
                  {skill.model || provider.model || "继承全局模型"}
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={ctx.agentSkillTesting.value === skill.name}
                  onClick={() => ctx.testAgentSkill(skill)}
                >
                  {ctx.agentSkillTesting.value === skill.name ? "试跑中…" : "试跑"}
                </Button>
                <Switch
                  checked={skill.enabled}
                  disabled={ctx.agentSkillSaving.value === skill.name}
                  onChange={(checked: boolean) => void ctx.saveAgentSkill(skill, checked)}
                  label={skill.enabled ? "启用" : "停用"}
                />
              </div>
            ))}
            {skills.length === 0 && <div className="empty">暂无注册的 Agent 技能。</div>}
          </div>
        </div>

        {/* ---- 运行列表 + 执行流程详情 ---- */}
        <div className="split">
          {/* 左：运行记录列表 */}
          <div className="panel tight">
            <div className="ph">
              <div>
                <h3>运行记录</h3>
                <p>点击一条记录查看右侧执行流程。</p>
              </div>
              <div className="toolbar">
                <Select
                  value={ctx.agentRunSceneFilter.value}
                  width={132}
                  options={SCENE_OPTIONS}
                  onChange={(v: string) => {
                    ctx.agentRunSceneFilter.value = v;
                    void ctx.refreshAgentRuns();
                  }}
                />
                <Select
                  value={ctx.agentRunStatusFilter.value}
                  width={132}
                  options={STATUS_OPTIONS}
                  onChange={(v: string) => {
                    ctx.agentRunStatusFilter.value = v;
                    void ctx.refreshAgentRuns();
                  }}
                />
              </div>
            </div>
            <div className="tbl-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th style={{ minWidth: 150 }}>时间</th>
                    <th>场景</th>
                    <th>状态</th>
                    <th>决策</th>
                    <th style={{ width: 80 }}>耗时</th>
                  </tr>
                </thead>
                <tbody>
                  {runs.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="text-muted" style={{ textAlign: "center", padding: 24 }}>
                        {runsLoading ? "加载中…" : "还没有 AI 运行记录；跑一次采集审查或技能试跑就会出现。"}
                      </td>
                    </tr>
                  ) : (
                    runs.map((row) => (
                      <tr
                        key={row.id}
                        className={
                          "clickable" + (activeRun?.id === row.id ? " selected" : "")
                        }
                        onClick={() => setSelectedRunId(row.id)}
                      >
                        <td className="mono" style={{ fontSize: 12 }}>
                          {ctx.formatDateTime(row.created_at)}
                        </td>
                        <td>
                          <div className="cell-main">
                            <strong>{sceneLabel(row.scene)}</strong>
                            <span>{row.skill_name}</span>
                          </div>
                        </td>
                        <td>
                          <Pill tone={runStatusTone(row.status)}>
                            {runStatusLabel(row.status)}
                          </Pill>
                        </td>
                        <td>{decisionLabel(row.decision)}</td>
                        <td>{formatMs(row.duration_ms)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>

          {/* 右：选中运行的执行流程 */}
          <aside className="panel tight sticky-col">
            {!activeRun ? (
              <div className="empty" style={{ margin: 16 }}>
                选择左侧一条运行记录查看执行流程。
              </div>
            ) : (
              <>
                <div className="ph">
                  <div>
                    <h3>{sceneLabel(activeRun.scene)}</h3>
                    <p>
                      {activeRun.skill_name}
                      <span className="mono"> v{activeRun.skill_version}</span>
                    </p>
                  </div>
                  <Pill tone={runStatusTone(activeRun.status)}>
                    {runStatusLabel(activeRun.status)}
                  </Pill>
                </div>

                <div className="kv-grid" style={{ marginBottom: 12 }}>
                  <div className="kv">
                    <dt>来源</dt>
                    <dd className="mono">
                      {activeRun.source_type} / {activeRun.source_id}
                    </dd>
                  </div>
                  <div className="kv">
                    <dt>模型</dt>
                    <dd className="mono">
                      {activeRun.model || "—"}
                      {activeRun.temperature !== null && activeRun.temperature !== undefined
                        ? ` · T=${activeRun.temperature}`
                        : ""}
                    </dd>
                  </div>
                  <div className="kv">
                    <dt>开始</dt>
                    <dd className="mono">
                      {activeRun.started_at ? ctx.formatDateTime(activeRun.started_at) : "—"}
                    </dd>
                  </div>
                  <div className="kv">
                    <dt>耗时</dt>
                    <dd className="mono">{formatMs(activeRun.duration_ms)}</dd>
                  </div>
                </div>

                <div className="kv-grid" style={{ marginBottom: 12 }}>
                  <div className="kv">
                    <dt>输入摘要</dt>
                    <dd>{activeRun.input_summary || "—"}</dd>
                  </div>
                  <div className="kv">
                    <dt>决策</dt>
                    <dd>{decisionLabel(activeRun.decision)}</dd>
                  </div>
                </div>

                {(activeRun.error_summary || activeRun.error_code) && (
                  <Callout tone="crit">
                    {activeRun.error_code ? `[${activeRun.error_code}] ` : ""}
                    {activeRun.error_summary || "运行失败"}
                  </Callout>
                )}

                <div className="ph" style={{ marginTop: 4 }}>
                  <div>
                    <h3 style={{ fontSize: 14 }}>执行流程</h3>
                    <p>从输入准备到收尾的全部事件，含每步原始数据。</p>
                  </div>
                </div>

                {eventsLoading && events.length === 0 ? (
                  <div className="empty">正在加载事件流…</div>
                ) : events.length === 0 ? (
                  <div className="empty">这条运行没有事件记录。</div>
                ) : (
                  <div className="tl">
                    {events.map((event) => {
                      const meta = eventMeta(event);
                      const data = prettyEventData(event.data_json);
                      return (
                        <div key={event.id} className="tl-item">
                          <span className={`tl-dot ${meta.dot}`} />
                          <div className="tl-head">
                            <strong>{meta.label}</strong>
                            <time>{ctx.formatDateTime(event.created_at)}</time>
                          </div>
                          <p className="tl-msg">{event.message}</p>
                          {data && (
                            <details className="tl-data">
                              <summary>查看数据</summary>
                              <pre className="mono">{data}</pre>
                            </details>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </>
            )}
          </aside>
        </div>
      </div>
    </div>
  );
}
