/* ============================================================================
   任务中心 —— 后台任务日志 + 自动推进开关控制面板
   忠实移植自 TasksSection.vue，视觉换成 Soft 设计。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Switch } from "../primitives";

// ---- 本地纯函数（照抄 Vue <script setup> 中的同名函数）----

function isPublishTask(taskType: string): boolean {
  return taskType === "publish.create_external_job";
}

function taskTypeLabel(taskType: string): string {
  return isPublishTask(taskType) ? "铺货" : taskType;
}

function taskStatusLabel(taskType: string, status: string): string {
  if (!isPublishTask(taskType)) {
    return status;
  }
  if (status === "success") return "已上架";
  if (status === "failed") return "异常";
  if (status === "pending" || status === "queued") return "待铺货";
  return "铺货中";
}

// 返回 Element 色调字符串，Pill 内部已做映射
function taskStatusType(
  statusType: (s: string) => string,
  taskType: string,
  status: string,
): string {
  if (!isPublishTask(taskType)) {
    return statusType(status);
  }
  if (status === "success") return "success";
  if (status === "failed") return "danger";
  if (status === "pending" || status === "queued") return "info";
  return "primary";
}

export default function TasksSection() {
  const ctx = useApp();

  // ---- 复合计算：自动铺货开关（所有 7 个 publish 标志全为 true 才算开启）----
  const settings = ctx.automationSettings.value;
  const publishAutomationEnabled = [
    settings.publish_precheck_enabled,
    settings.publish_attribute_fill_enabled,
    settings.publish_category_precheck_enabled,
    settings.publish_asset_upload_enabled,
    settings.publish_submit_enabled,
    settings.publish_status_sync_enabled,
    settings.publish_listing_enabled,
  ].every(Boolean);

  async function togglePublishAutomation(value: boolean) {
    const s = ctx.automationSettings.value;
    s.publish_precheck_enabled = value;
    s.publish_attribute_fill_enabled = value;
    s.publish_category_precheck_enabled = value;
    s.publish_asset_upload_enabled = value;
    s.publish_submit_enabled = value;
    s.publish_status_sync_enabled = value;
    s.publish_listing_enabled = value;
    await ctx.saveAutomationSettings();
  }

  const result = ctx.lastAutomationResult.value;
  const runs = ctx.taskRuns.value;

  return (
    <section className="content-stack">
      <div className="panel">
        {/* 面板标题 + 快捷操作按钮组 */}
        <div className="panel-title">
          <div>
            <h2>任务中心</h2>
            <p>铺货只保留一个推进入口，底层微信预检、素材、提交和上架由系统自动处理。</p>
          </div>
          <div className="button-group">
            <Button icon="refresh" onClick={() => ctx.runOrderSyncOnce()}>
              同步待发货订单
            </Button>
            <Button icon="refresh" onClick={() => ctx.runOrderDetailSyncOnce()}>
              同步订单详情
            </Button>
            <Button icon="refresh" onClick={() => ctx.runAftersaleSyncOnce()}>
              同步售后
            </Button>
            <Button icon="refresh" onClick={() => ctx.runGuaranteeSyncOnce()}>
              同步纠纷单
            </Button>
            <Button icon="refresh" onClick={() => ctx.runPurchaseTaskGenerationOnce()}>
              生成采购任务
            </Button>
            <Button
              variant="accent"
              icon="refresh"
              disabled={ctx.publishRetryRunning.value}
              onClick={() => ctx.resumePublishFailuresOnce()}
            >
              {ctx.publishRetryRunning.value ? "推进中…" : "推进铺货"}
            </Button>
            <Button icon="refresh" onClick={() => ctx.runPriceUpdatePrecheckOnce()}>
              商品售价校验
            </Button>
            <Button icon="upload" onClick={() => ctx.runPriceUpdateSubmitOnce()}>
              提交商品售价
            </Button>
            <Button icon="refresh" onClick={() => ctx.runPriceUpdateConfirmOnce()}>
              确认商品售价
            </Button>
          </div>
        </div>

        {/* 自动推进子面板 */}
        <div className="subcard">
          <div className="automation-head" style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 16 }}>
            <div>
              <h3 style={{ margin: "0 0 4px" }}>自动推进</h3>
              <p className="text-muted" style={{ margin: 0 }}>
                铺货作为一个整体开关，不再拆成类目、素材、提交和上架多个开关。
              </p>
            </div>
            <Button
              variant="accent"
              icon="refresh"
              disabled={ctx.automationRunning.value}
              onClick={() => ctx.runOperationalAutomationOnce()}
            >
              {ctx.automationRunning.value ? "推进中…" : "自动推进一轮"}
            </Button>
          </div>

          {/* 自动化开关条 */}
          <div className="automation-switches" style={{ display: "flex", flexWrap: "wrap", gap: "12px 24px", marginBottom: result ? 12 : 0 }}>
            <Switch
              checked={ctx.automationSettings.value.order_sync_enabled}
              label="同步订单"
              onChange={(v) => {
                ctx.automationSettings.value.order_sync_enabled = v;
                ctx.saveAutomationSettings();
              }}
            />
            <Switch
              checked={ctx.automationSettings.value.order_detail_sync_enabled}
              label="同步详情"
              onChange={(v) => {
                ctx.automationSettings.value.order_detail_sync_enabled = v;
                ctx.saveAutomationSettings();
              }}
            />
            <Switch
              checked={ctx.automationSettings.value.aftersale_sync_enabled}
              label="售后同步"
              onChange={(v) => {
                ctx.automationSettings.value.aftersale_sync_enabled = v;
                ctx.saveAutomationSettings();
              }}
            />
            <Switch
              checked={ctx.automationSettings.value.purchase_task_enabled}
              label="采购任务"
              onChange={(v) => {
                ctx.automationSettings.value.purchase_task_enabled = v;
                ctx.saveAutomationSettings();
              }}
            />
            <Switch
              checked={ctx.automationSettings.value.delivery_submission_enabled}
              label="微信发货"
              onChange={(v) => {
                ctx.automationSettings.value.delivery_submission_enabled = v;
                ctx.saveAutomationSettings();
              }}
            />
            <Switch
              checked={publishAutomationEnabled}
              label="自动铺货"
              onChange={(v) => togglePublishAutomation(v)}
            />
            <Switch
              checked={ctx.automationSettings.value.price_confirm_enabled}
              label="确认商品售价"
              onChange={(v) => {
                ctx.automationSettings.value.price_confirm_enabled = v;
                ctx.saveAutomationSettings();
              }}
            />
          </div>

          {/* 上次自动推进结果摘要（仅有结果时显示）*/}
          {result && (
            <div className="automation-result" style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginTop: 8 }}>
              <Pill tone="success">执行 {result.executed_steps.length}</Pill>
              <Pill tone="info">跳过 {result.skipped_steps.length}</Pill>
              <Pill tone={result.errors.length > 0 ? "danger" : "success"}>
                失败 {result.errors.length}
              </Pill>
              {result.errors.length > 0 && (
                <span className="text-muted" style={{ fontSize: 12 }}>
                  {result.errors.map((item) => `${item.step}: ${item.error}`).join("；")}
                </span>
              )}
            </div>
          )}
        </div>

        {/* 任务运行日志表格 */}
        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th style={{ minWidth: 260 }}>任务 ID</th>
                <th style={{ minWidth: 210 }}>类型</th>
                <th style={{ width: 140 }}>状态</th>
                <th style={{ width: 190 }}>进度</th>
                <th style={{ minWidth: 180 }}>任务项</th>
                <th style={{ minWidth: 190 }}>创建时间</th>
                <th style={{ width: 110 }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {runs.length === 0 ? (
                <tr>
                  <td colSpan={7} style={{ textAlign: "center", padding: "24px 0", color: "var(--ink-4)" }}>
                    暂无数据
                  </td>
                </tr>
              ) : (
                runs.map((row) => (
                  <tr key={row.id}>
                    <td className="mono" style={{ fontSize: 12 }}>{row.id}</td>
                    <td>{taskTypeLabel(row.task_type)}</td>
                    <td>
                      <Pill tone={taskStatusType(ctx.statusType, row.task_type, row.status)}>
                        {taskStatusLabel(row.task_type, row.status)}
                      </Pill>
                    </td>
                    <td>
                      {/* 进度条：替代 el-progress */}
                      <div className="progress-wrap" style={{ display: "flex", alignItems: "center", gap: 6 }}>
                        <div className="bar-track" style={{ flex: 1, height: 8, borderRadius: 4, background: "var(--surface-2)", overflow: "hidden" }}>
                          <div
                            className="bar-fill"
                            style={{ width: `${row.progress}%`, height: "100%", background: "var(--accent)", borderRadius: 4, transition: "width .3s" }}
                          />
                        </div>
                        <span className="hint" style={{ flexShrink: 0, fontSize: 11, color: "var(--ink-3)", width: 32, textAlign: "right" }}>
                          {row.progress}%
                        </span>
                      </div>
                    </td>
                    <td>
                      {isPublishTask(row.task_type) ? (
                        <>
                          <span>待处理 {row.pending_count + row.ready_count}</span>
                          <span className="split-stat">异常 {row.failed_count}</span>
                        </>
                      ) : (
                        <>
                          <span>待 {row.pending_count}</span>
                          <span className="split-stat">就绪 {row.ready_count}</span>
                          <span className="split-stat">失败 {row.failed_count}</span>
                        </>
                      )}
                    </td>
                    <td className="mono" style={{ fontSize: 12 }}>{ctx.formatDateTime(row.created_at)}</td>
                    <td>
                      <div className="row-actions">
                        <Button size="sm" onClick={() => ctx.openTask(row)}>查看</Button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
