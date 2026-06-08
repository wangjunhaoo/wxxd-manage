/* ============================================================================
   AI 设置面板 — Soft 设计移植（原 AiSettingsSection.vue）
   纯设置/表单面板，无分页/表格/对话框。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Callout, Icon, Pill, Select, Switch } from "../primitives";

export default function AiSettingsSection() {
  const ctx = useApp();

  // ---- 已保存设置（只读展示） ----
  const saved = ctx.aiProviderSettings.value;

  // ---- 派生：provider 状态标签色调 ----
  const savedStatusTone = saved.enabled ? "success" : "info";

  // ---- 表单字段（reactive 对象直接读写，无需 .value） ----
  const form = ctx.aiProviderForm;

  // ---- computed 映射（ctx 已计算好，直接读 .value） ----
  const isCustom = ctx.aiProviderFormIsCustom.value;
  const runtimeLabel = ctx.aiProviderFormRuntimeLabel.value;
  const savedRuntimeLabel = ctx.aiProviderRuntimeLabel.value;
  const requiresBaseUrl = ctx.aiProviderFormRequiresBaseUrl.value;
  const requiresApiKey = ctx.aiProviderFormRequiresApiKey.value;
  const modelPlaceholder = ctx.aiProviderModelPlaceholder.value;
  const baseUrlPlaceholder = ctx.aiProviderBaseUrlPlaceholder.value;
  const apiKeyPlaceholder = ctx.aiProviderApiKeyPlaceholder.value;
  const canTest = ctx.aiProviderCanTest.value;

  return (
    <section className="content-stack">
      <div className="panel">
        {/* 面板标题 + 状态标签 */}
        <div className="panel-title">
          <div>
            <h2>AI Agent</h2>
            <p>用于采集结果审查和铺货自动补齐；铺货过程会自动调用，不需要单独人工执行。</p>
          </div>
          <Pill tone={savedStatusTone}>
            {saved.enabled ? savedRuntimeLabel : "已关闭"}
          </Pill>
        </div>

        {/* 已保存设置摘要 */}
        <dl className="status-list compact">
          <div>
            <dt>Provider</dt>
            <dd>
              {saved.provider_type === "custom"
                ? saved.custom_provider_id
                : saved.provider_type}
            </dd>
          </div>
          <div>
            <dt>模型</dt>
            <dd>{saved.model || "未配置"}</dd>
          </div>
          <div>
            <dt>密钥</dt>
            <dd>
              {saved.has_api_key
                ? saved.api_key_hint || "已保存"
                : "未保存"}
            </dd>
          </div>
          <div>
            <dt>更新时间</dt>
            <dd>
              {saved.updated_at
                ? ctx.formatDateTime(saved.updated_at)
                : "尚未保存"}
            </dd>
          </div>
        </dl>

        {/* 编辑表单 */}
        <div className="ai-settings-form">
          {/* 启用开关 */}
          <Switch
            checked={form.enabled}
            onChange={(v: boolean) => {
              form.enabled = v;
            }}
            label={form.enabled ? "启用 AI Agent" : "关闭"}
          />

          {/* Provider 选择 */}
          <Select
            value={form.provider_type}
            onChange={(v: string) => {
              form.provider_type = v;
            }}
            options={ctx.aiProviderOptions.map((o) => ({
              label: o.label,
              value: o.value,
            }))}
            width="100%"
          />

          {/* 运行时标签（只读展示） */}
          <input
            className="inp"
            value={runtimeLabel}
            disabled
            readOnly
            onChange={() => undefined}
          />

          {/* 仅 custom 显示：Custom provider id */}
          {isCustom && (
            <input
              className="inp"
              value={form.custom_provider_id}
              placeholder="Custom provider id，例如 wx-xd-custom"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                form.custom_provider_id = e.target.value;
              }}
            />
          )}

          {/* 仅 custom 显示：API 类型选择 */}
          {isCustom && (
            <Select
              value={form.api}
              onChange={(v: string) => {
                form.api = v;
              }}
              options={ctx.aiProviderApiOptions.map((o) => ({
                label: o.label,
                value: o.value,
              }))}
              width="100%"
            />
          )}

          {/* Base URL */}
          <input
            className="inp"
            value={form.base_url}
            placeholder={baseUrlPlaceholder}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              form.base_url = e.target.value;
            }}
          />

          {/* 模型 */}
          <input
            className="inp"
            value={form.model}
            placeholder={modelPlaceholder}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              form.model = e.target.value;
            }}
          />

          {/* temperature */}
          <input
            className="inp"
            value={form.temperature}
            placeholder="temperature，0 到 1"
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              form.temperature = e.target.value;
            }}
          />

          {/* 仅 custom 显示：context_window */}
          {isCustom && (
            <input
              className="inp"
              value={form.context_window}
              placeholder="context_window，例如 128000"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                form.context_window = e.target.value;
              }}
            />
          )}

          {/* 仅 custom 显示：max_tokens */}
          {isCustom && (
            <input
              className="inp"
              value={form.max_tokens}
              placeholder="max_tokens，例如 4096"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                form.max_tokens = e.target.value;
              }}
            />
          )}

          {/* API Key（密码输入） */}
          <input
            className="inp"
            type="password"
            autoComplete="new-password"
            value={form.api_key}
            placeholder={apiKeyPlaceholder}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              form.api_key = e.target.value;
            }}
          />
        </div>

        {/* 信息提示 */}
        <Callout tone="ok">
          运行时固定为 pi-coding-agent，内置文件、命令和编辑工具已关闭；内置 provider 使用 Pi 模型定义，Custom 按所选 API 类型注册。
        </Callout>

        {/* 操作行 */}
        <div className="action-row ai-actions">
          {saved.has_api_key ? (
            <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer" }}>
              <input
                type="checkbox"
                className="cbx"
                checked={form.clear_api_key}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  form.clear_api_key = e.target.checked;
                }}
              />
              清除已保存 API Key
            </label>
          ) : (
            <span />
          )}
          <div className="button-group">
            <Button
              variant="outline"
              icon="refresh"
              disabled={!canTest || ctx.aiProviderTesting.value}
              onClick={() => ctx.testAiProvider()}
            >
              {ctx.aiProviderTesting.value ? (
                <>
                  <Icon name="refresh" size={14} />
                  试连中…
                </>
              ) : (
                "试连"
              )}
            </Button>
            <Button
              variant="accent"
              disabled={ctx.aiProviderSaving.value}
              onClick={() => ctx.saveAiProviderSettings()}
            >
              {ctx.aiProviderSaving.value ? "保存中…" : "保存"}
            </Button>
          </div>
        </div>

        {/* 当前表单派生提示 */}
        <dl className="status-list compact provider-hints">
          <div>
            <dt>当前运行时</dt>
            <dd>{runtimeLabel}</dd>
          </div>
          <div>
            <dt>Custom API</dt>
            <dd>
              {isCustom ? form.api : "使用 Pi 内置模型定义"}
            </dd>
          </div>
          <div>
            <dt>Base URL</dt>
            <dd>{requiresBaseUrl ? "必填" : "可选"}</dd>
          </div>
          <div>
            <dt>API Key</dt>
            <dd>{requiresApiKey ? "必填" : "可选"}</dd>
          </div>
        </dl>
      </div>
    </section>
  );
}
