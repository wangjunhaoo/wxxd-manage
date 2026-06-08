/* ============================================================================
   本地 API — Soft 移植
   面板 1：本地主控 HTTP API 配置 + 密钥管理
   面板 2：外部 API 审计日志表
   面板 3：创建外部铺货任务（手工 JSON）
   无本地 state，全部来自 ctx。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Pill, Button, Callout } from "../primitives";

export default function PublishApiSection() {
  const ctx = useApp();
  const cfg = ctx.localApiConfig.value;
  const logs = ctx.externalApiLogs.value;

  return (
    <section className="content-stack">
      {/* ------------------------------------------------------------------ */}
      {/* 面板 1 — 本地主控 HTTP API                                          */}
      {/* ------------------------------------------------------------------ */}
      <div className="panel">
        <div className="panel-title">
          <div>
            <h2>本地主控 HTTP API</h2>
            <p>外部系统调用本机地址创建铺货任务、商品售价调整任务和查询状态。</p>
          </div>
          <Pill tone={cfg.has_api_key ? "success" : "warning"}>
            {cfg.has_api_key ? "已生成 Key" : "未生成 Key"}
          </Pill>
        </div>

        {/* KPI / 状态条 */}
        <dl className="status-list compact">
          <div>
            <dt>Base URL</dt>
            <dd>{cfg.base_url}</dd>
          </div>
          <div>
            <dt>认证头</dt>
            <dd>{cfg.auth_header}</dd>
          </div>
          <div>
            <dt>Key 状态</dt>
            <dd>{cfg.api_key_hint || "未生成"}</dd>
          </div>
        </dl>

        {/* API 端点清单 */}
        <div className="api-endpoints">
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

        {/* 操作行 */}
        <div className="action-row">
          <Button variant="accent" onClick={() => ctx.rotateLocalApiKey()}>
            {cfg.has_api_key ? "重置 API Key" : "生成 API Key"}
          </Button>
          <Pill tone="info">只监听 127.0.0.1:{cfg.port}</Pill>
        </div>

        {/* 一次性 Key 提示（仅刚生成后显示） */}
        {ctx.rotatedLocalApiKey.value && (
          <Callout tone="warn">
            <strong>API Key 只展示这一次，外部系统请保存后使用。</strong>
            <div style={{ marginTop: 8 }}>
              <input
                className="inp mono"
                value={ctx.rotatedLocalApiKey.value}
                readOnly
                style={{ width: "100%" }}
              />
            </div>
          </Callout>
        )}
      </div>

      {/* ------------------------------------------------------------------ */}
      {/* 面板 2 — 外部 API 审计日志                                          */}
      {/* ------------------------------------------------------------------ */}
      <div className="panel">
        <div className="panel-title">
          <div>
            <h2>外部 API 审计</h2>
            <p>
              只记录调用方法、路径、状态、耗时和安全摘要，不保存 API Key
              或完整请求体。
            </p>
          </div>
          <div className="button-group">
            <Button icon="refresh" onClick={() => ctx.refreshExternalApiLogs()}>
              刷新
            </Button>
            <Pill tone="neu">{logs.length} 条</Pill>
          </div>
        </div>

        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>时间</th>
                <th>方法</th>
                <th>路径</th>
                <th>状态</th>
                <th>耗时</th>
                <th>请求摘要</th>
                <th>响应摘要</th>
              </tr>
            </thead>
            <tbody>
              {logs.length === 0 ? (
                <tr>
                  <td colSpan={7}>
                    <div className="empty">暂无数据</div>
                  </td>
                </tr>
              ) : (
                logs.map((row) => (
                  <tr key={row.id}>
                    <td className="mono" style={{ whiteSpace: "nowrap" }}>
                      {ctx.formatDateTime(row.created_at)}
                    </td>
                    <td>{row.method}</td>
                    <td
                      className="mono"
                      style={{
                        maxWidth: 230,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.path}
                    >
                      {row.path}
                    </td>
                    <td>
                      <Pill tone={row.status === "success" ? "success" : "danger"}>
                        {row.status_code}
                      </Pill>
                    </td>
                    <td className="mono" style={{ whiteSpace: "nowrap" }}>
                      {row.duration_ms} ms
                    </td>
                    <td
                      style={{
                        maxWidth: 260,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.request_summary ?? undefined}
                    >
                      {row.request_summary || "-"}
                    </td>
                    <td
                      style={{
                        maxWidth: 150,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={
                        row.response_summary ?? row.error_code ?? undefined
                      }
                    >
                      {row.response_summary || row.error_code || "-"}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* ------------------------------------------------------------------ */}
      {/* 面板 3 — 创建外部铺货任务（手工 JSON）                              */}
      {/* ------------------------------------------------------------------ */}
      <div className="panel">
        <div className="panel-title">
          <div>
            <h2>创建外部铺货任务</h2>
            <p>
              外部系统可直接按同一 JSON 协议调用本机 HTTP
              API，桌面端保留手工创建入口。
            </p>
          </div>
        </div>

        <textarea
          className="ta mono json-editor"
          value={ctx.publishPayload.value}
          onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => {
            ctx.publishPayload.value = e.target.value;
          }}
          rows={18}
          spellCheck={false}
          style={{ resize: "vertical", minHeight: "18lh", maxHeight: "28lh" }}
        />

        <div className="action-row">
          <Button variant="accent" icon="upload" onClick={() => ctx.createPublishJob()}>
            创建铺货任务
          </Button>
          {ctx.latestTaskId.value && (
            <Pill tone="info">最新任务：{ctx.latestTaskId.value}</Pill>
          )}
        </div>
      </div>
    </section>
  );
}
