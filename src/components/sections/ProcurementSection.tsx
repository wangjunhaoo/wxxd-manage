/* ============================================================================
   采购下单工作台 —— Soft 视觉
   ----------------------------------------------------------------------------
   忠实移植自 ProcurementSection.vue：内联「待采购订单表 + 供应商物流回填 + 发货队列」。
   先按货源下单，再填供应商物流；买不了的单独标记异常。
   IA / 交互 / 中文文案 100% 保留，仅把 Element Plus 换成 ../primitives + app.css 类。
   ============================================================================ */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import {
  PageHead,
  Button,
  Pill,
  Chip,
  Select,
  Switch,
  Segmented,
  Empty,
} from "../primitives";
import type { PurchaseTaskView } from "../../types/app";

type PurchaseAction = "mapping" | "shipment" | "issue";

// 供应商「买不了」相关状态集合
const supplierIssueStatuses = new Set([
  "supplier_out_of_stock",
  "supplier_price_changed",
  "supplier_quality_risk",
  "supplier_cancelled",
  "supplier_exception",
]);

function purchaseStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    pending_purchase: "去供应商下单",
    needs_mapping: "先补货源信息",
    supplier_shipped: "物流已回填",
    supplier_out_of_stock: "供应商缺货",
    supplier_price_changed: "供应商涨价",
    supplier_quality_risk: "质量风险",
    supplier_cancelled: "供应商取消",
    supplier_exception: "供应商异常",
    send_failed: "微信发货失败",
    wechat_shipped: "微信已发货",
    completed: "已完成",
  };
  return labels[status] || status;
}

function deliveryStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    waiting_confirmation: "待确认发货",
    ready_to_send: "待提交微信",
    send_failed: "发货失败",
    wechat_shipped: "微信已发货",
  };
  return labels[status] || status;
}

export default function ProcurementSection() {
  const ctx = useApp();

  const [activePurchaseAction, setActivePurchaseAction] =
    useState<PurchaseAction>("shipment");
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const tasks = ctx.purchaseTasks.value;
  const shipments = ctx.deliveryShipments.value;

  // ---- 筛选胶囊选项 ----
  const purchaseStatusOptions = [
    { value: "all", label: "全部", count: tasks.length },
    {
      value: "pending_purchase",
      label: "待下单",
      count: tasks.filter((t) => t.status === "pending_purchase").length,
    },
    {
      value: "needs_mapping",
      label: "缺货源",
      count: tasks.filter((t) => t.status === "needs_mapping").length,
    },
    {
      value: "supplier_issue",
      label: "买不了",
      count: tasks.filter((t) => supplierIssueStatuses.has(t.status)).length,
    },
    {
      value: "supplier_shipped",
      label: "已填物流",
      count: tasks.filter((t) => t.status === "supplier_shipped").length,
    },
  ];

  // ---- KPI 汇总 ----
  const purchaseSummary = {
    todo: tasks.filter((t) =>
      ["pending_purchase", "needs_mapping"].includes(t.status),
    ).length,
    shipped: tasks.filter((t) => t.status === "supplier_shipped").length,
    issue: tasks.filter((t) => supplierIssueStatuses.has(t.status)).length,
  };

  const selectedPurchaseTaskId =
    ctx.purchaseShipmentForm.purchase_task_id ||
    ctx.purchaseMappingForm.purchase_task_id ||
    ctx.purchaseIssueForm.purchase_task_id;

  // ---- 交互助手 ----
  async function setPurchaseFilter(status: string) {
    ctx.purchaseStatusFilter.value = status;
    await ctx.refreshPurchaseTasks();
  }

  async function setDeliveryFilter(status: string) {
    ctx.deliveryStatusFilter.value = status;
    await ctx.refreshDeliveryShipments();
  }

  function openSource(url?: string | null) {
    if (!url) {
      return;
    }
    window.open(url, "_blank", "noreferrer");
  }

  function startMapping(row: PurchaseTaskView) {
    ctx.selectPurchaseTaskMapping(row);
    setActivePurchaseAction("mapping");
  }

  function startShipment(row: PurchaseTaskView) {
    ctx.selectPurchaseTaskShipment(row);
    setActivePurchaseAction("shipment");
  }

  function startIssue(row: PurchaseTaskView) {
    ctx.selectPurchaseTaskIssue(row);
    setActivePurchaseAction("issue");
  }

  function handlePrimaryAction(row: PurchaseTaskView) {
    if (row.status === "needs_mapping") {
      startMapping(row);
      return;
    }
    if (supplierIssueStatuses.has(row.status)) {
      startIssue(row);
      return;
    }
    startShipment(row);
  }

  const agentResult = ctx.supplierAgentApplyResult.value;

  return (
    <div className="pad">
      <div className="wrap-wide stack">
        {/* ===== 采购下单工作台 ===== */}
        <div className="panel">
          <PageHead
            eyebrow="采购下单 · Asia/Shanghai"
            title="采购下单工作台"
            desc="先按货源下单，再填供应商物流；买不了的单独标记异常。"
            actions={
              <>
                <Button icon="refresh" onClick={() => ctx.runPurchaseTaskGenerationOnce()}>
                  拉取新订单
                </Button>
                <Button icon="refresh" onClick={() => ctx.refreshPurchaseTasks()}>
                  刷新
                </Button>
                <Button variant="accent" icon="upload" onClick={() => ctx.exportPurchaseTasks()}>
                  导出给采购
                </Button>
              </>
            }
          />

          <div className="chips">
            {purchaseStatusOptions.map((option) => (
              <Chip
                key={option.value}
                label={option.label}
                count={option.count}
                active={ctx.purchaseStatusFilter.value === option.value}
                onClick={() => setPurchaseFilter(option.value)}
              />
            ))}
          </div>

          <div className="kpis">
            <div className="kpi">
              <span>待处理</span>
              <strong>{purchaseSummary.todo}</strong>
            </div>
            <div className="kpi">
              <span>已填物流</span>
              <strong>{purchaseSummary.shipped}</strong>
            </div>
            <div className="kpi">
              <span>异常</span>
              <strong className="warn">{purchaseSummary.issue}</strong>
            </div>
            <div className="kpi">
              <span>匹配总数</span>
              <strong>{ctx.purchaseTaskTotal.value}</strong>
            </div>
          </div>

          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>要买什么</th>
                  <th>去哪买</th>
                  <th>订单</th>
                  <th>数量/金额</th>
                  <th>供应商物流</th>
                  <th>现在该做</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {tasks.map((row) => (
                  <tr key={row.id}>
                    <td>
                      <div className="cell-main">
                        <strong>{row.title || "未同步商品标题"}</strong>
                        <span className="mono">{row.external_sku_id || "缺外部 SKU"}</span>
                      </div>
                    </td>
                    <td>
                      <div className="cell-main">
                        <strong>{row.supplier_name || "未填供应商"}</strong>
                        <span className="mono">{row.external_product_id || "缺外部商品 ID"}</span>
                        <button
                          className="link-src"
                          disabled={!row.source_url}
                          onClick={() => openSource(row.source_url)}
                        >
                          打开货源
                        </button>
                      </div>
                    </td>
                    <td>
                      <div className="cell-main">
                        <strong>{row.shop_name}</strong>
                        <span className="mono">{row.wechat_order_id}</span>
                      </div>
                    </td>
                    <td>
                      <div className="cell-main">
                        <strong>x{row.quantity}</strong>
                        <span className="mono">{ctx.formatCents(row.estimated_revenue)}</span>
                      </div>
                    </td>
                    <td>
                      <div className="cell-main">
                        <strong>{row.supplier_waybill_id || "未填物流"}</strong>
                        <span className="mono">
                          {row.supplier_delivery_name || row.supplier_delivery_id || "-"}
                        </span>
                      </div>
                    </td>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>
                        {purchaseStatusLabel(row.status)}
                      </Pill>
                      {row.error_summary && (
                        <small className="subtext">{row.error_summary}</small>
                      )}
                    </td>
                    <td>
                      <div className="row-actions">
                        <Button
                          size="sm"
                          variant="accent"
                          onClick={() => handlePrimaryAction(row)}
                        >
                          {row.status === "needs_mapping"
                            ? "补货源"
                            : supplierIssueStatuses.has(row.status)
                              ? "处理异常"
                              : "填物流"}
                        </Button>
                        <Button size="sm" onClick={() => startMapping(row)}>
                          补资料
                        </Button>
                        <Button size="sm" variant="danger" onClick={() => startIssue(row)}>
                          买不了
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {ctx.purchaseExportPath.value && (
            <div className="toolbar">
              <Pill tone="info">最近导出：{ctx.purchaseExportPath.value}</Pill>
            </div>
          )}
        </div>

        {/* ===== 处理选中的采购单 ===== */}
        <div className="panel">
          <div className="ph">
            <div>
              <h3>处理选中的采购单</h3>
              <p>{selectedPurchaseTaskId || "先在上面的表格点“填物流 / 补货源 / 买不了”"}</p>
            </div>
            <Segmented
              options={[
                { label: "填物流", value: "shipment" },
                { label: "补货源", value: "mapping" },
                { label: "买不了", value: "issue" },
              ]}
              value={activePurchaseAction}
              onChange={(v: string) => setActivePurchaseAction(v as PurchaseAction)}
            />
          </div>

          {selectedPurchaseTaskId ? (
            activePurchaseAction === "shipment" ? (
              <div className="form-grid cols-3">
                <input
                  className="inp"
                  placeholder="采购任务 ID"
                  value={ctx.purchaseShipmentForm.purchase_task_id}
                  onChange={(e) => {
                    ctx.purchaseShipmentForm.purchase_task_id = e.target.value;
                  }}
                />
                <Select
                  value={ctx.purchaseShipmentForm.deliver_type}
                  onChange={(v: string) => {
                    ctx.purchaseShipmentForm.deliver_type = Number(v);
                  }}
                  options={[
                    { value: 1, label: "快递发货" },
                    { value: 3, label: "无需物流" },
                  ]}
                  placeholder="发货方式"
                />
                <Select
                  value={ctx.purchaseShipmentForm.delivery_id}
                  onChange={(v: string) => {
                    ctx.purchaseShipmentForm.delivery_id = v;
                  }}
                  options={ctx.deliveryCompanyOptions.value}
                  placeholder="快递公司"
                  disabled={ctx.purchaseShipmentForm.deliver_type !== 1}
                />
                <input
                  className="inp"
                  placeholder="物流单号"
                  disabled={ctx.purchaseShipmentForm.deliver_type !== 1}
                  value={ctx.purchaseShipmentForm.waybill_id}
                  onChange={(e) => {
                    ctx.purchaseShipmentForm.waybill_id = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="采购成本，选填"
                  value={ctx.purchaseShipmentForm.estimated_cost}
                  onChange={(e) => {
                    ctx.purchaseShipmentForm.estimated_cost = e.target.value;
                  }}
                />
                <Button
                  variant="accent"
                  icon="upload"
                  onClick={() => ctx.recordPurchaseTaskShipment()}
                >
                  保存物流
                </Button>
              </div>
            ) : activePurchaseAction === "mapping" ? (
              <div className="form-grid cols-3">
                <input
                  className="inp"
                  placeholder="采购任务 ID"
                  value={ctx.purchaseMappingForm.purchase_task_id}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.purchase_task_id = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="外部商品 ID"
                  value={ctx.purchaseMappingForm.external_product_id}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.external_product_id = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="外部 SKU"
                  value={ctx.purchaseMappingForm.external_sku_id}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.external_sku_id = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="货源链接，选填"
                  value={ctx.purchaseMappingForm.source_url}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.source_url = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="供应商，选填"
                  value={ctx.purchaseMappingForm.supplier_name}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.supplier_name = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="采购成本，选填"
                  value={ctx.purchaseMappingForm.estimated_cost}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.estimated_cost = e.target.value;
                  }}
                />
                <input
                  className="inp"
                  placeholder="备注，选填"
                  value={ctx.purchaseMappingForm.note}
                  onChange={(e) => {
                    ctx.purchaseMappingForm.note = e.target.value;
                  }}
                />
                <Button
                  variant="accent"
                  icon="check"
                  onClick={() => ctx.resolvePurchaseTaskMapping()}
                >
                  保存货源信息
                </Button>
              </div>
            ) : (
              <div className="form-grid cols-3">
                <input
                  className="inp"
                  placeholder="采购任务 ID"
                  value={ctx.purchaseIssueForm.purchase_task_id}
                  onChange={(e) => {
                    ctx.purchaseIssueForm.purchase_task_id = e.target.value;
                  }}
                />
                <Select
                  value={ctx.purchaseIssueForm.issue_type}
                  onChange={(v: string) => {
                    ctx.purchaseIssueForm.issue_type = v;
                  }}
                  options={ctx.purchaseIssueTypeOptions}
                  placeholder="异常类型"
                />
                <input
                  className="inp"
                  placeholder="处理备注，选填"
                  value={ctx.purchaseIssueForm.note}
                  onChange={(e) => {
                    ctx.purchaseIssueForm.note = e.target.value;
                  }}
                />
                <Button
                  variant="danger"
                  icon="bell"
                  onClick={() => ctx.markPurchaseTaskIssue()}
                >
                  标记买不了
                </Button>
              </div>
            )
          ) : (
            <Empty>从上面的采购单选择一个动作后再填写。</Empty>
          )}
        </div>

        {/* ===== 待发货队列 ===== */}
        <div className="panel">
          <div className="ph">
            <div>
              <h3>待发货队列</h3>
              <p>供应商物流保存后会进入这里；自动发货关闭时只进入待确认。</p>
            </div>
            <div className="row-actions">
              <Select
                value={ctx.deliveryStatusFilter.value}
                onChange={(v: string) => setDeliveryFilter(v)}
                options={[
                  { value: "all", label: "全部" },
                  { value: "waiting_confirmation", label: "待确认" },
                  { value: "ready_to_send", label: "待提交" },
                  { value: "send_failed", label: "发货失败" },
                  { value: "wechat_shipped", label: "已发货" },
                ]}
                width={140}
              />
              <Button icon="refresh" onClick={() => ctx.syncDeliveryCompanies()}>
                同步快递公司
              </Button>
              <Button icon="refresh" onClick={() => ctx.refreshDeliveryShipments()}>
                刷新
              </Button>
              <Button variant="accent" icon="upload" onClick={() => ctx.runDeliverySubmissionOnce()}>
                提交微信发货
              </Button>
              <Switch
                checked={ctx.deliverySettings.value.auto_send_delivery}
                onChange={(v: boolean) => {
                  ctx.deliverySettings.value.auto_send_delivery = v;
                  ctx.setAutoSendDelivery(v);
                }}
                label={ctx.deliverySettings.value.auto_send_delivery ? "自动发货" : "只保存"}
              />
            </div>
          </div>

          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>状态</th>
                  <th>店铺</th>
                  <th>微信订单号</th>
                  <th>物流</th>
                  <th>失败原因</th>
                  <th>更新时间</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {shipments.map((row) => (
                  <tr key={row.id}>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>
                        {deliveryStatusLabel(row.status)}
                      </Pill>
                    </td>
                    <td>{row.shop_name}</td>
                    <td className="mono">{row.wechat_order_id}</td>
                    <td>
                      <div className="cell-main">
                        <strong>{row.waybill_id || "-"}</strong>
                        <span className="mono">
                          {row.delivery_name || row.delivery_id || "-"}
                        </span>
                      </div>
                    </td>
                    <td>{row.error_summary || row.error_code || "-"}</td>
                    <td className="mono">{ctx.formatDateTime(row.updated_at)}</td>
                    <td>
                      <Button
                        size="sm"
                        disabled={row.status === "wechat_shipped"}
                        onClick={() => ctx.retryDeliveryShipment(row)}
                      >
                        重试
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="toolbar">
            <Pill tone="info">
              {shipments.length} / {ctx.deliveryShipmentTotal.value}
            </Pill>
          </div>
        </div>

        {/* ===== 高级批量工具（折叠） ===== */}
        <div className="panel tight">
          <button
            className="ph"
            style={{
              width: "100%",
              background: "none",
              border: "none",
              cursor: "pointer",
              textAlign: "left",
            }}
            onClick={() => setAdvancedOpen((open) => !open)}
          >
            <div>
              <h3>高级批量工具</h3>
            </div>
            <span className="more">{advancedOpen ? "收起 ›" : "展开 ›"}</span>
          </button>

          {advancedOpen && (
            <div className="subcard stack">
              <div className="ph">
                <div>
                  <h3>供应商 Agent 桥</h3>
                  <p>只导出非敏采购字段；外部结果写回前会校验字段。</p>
                </div>
                <div className="row-actions">
                  <Select
                    value={ctx.supplierAgentExportFormat.value}
                    onChange={(v: string) => {
                      ctx.supplierAgentExportFormat.value = v;
                    }}
                    options={[
                      { value: "jsonl", label: "JSONL" },
                      { value: "json", label: "JSON" },
                      { value: "md", label: "Markdown" },
                    ]}
                    width={140}
                  />
                  <Button
                    variant="accent"
                    icon="upload"
                    onClick={() => ctx.exportSupplierAgentTasks()}
                  >
                    导出 Agent 任务
                  </Button>
                  <Button icon="search" onClick={() => ctx.fillSupplierAgentTemplate()}>
                    填入模板
                  </Button>
                </div>
              </div>

              <div className="stack">
                <textarea
                  className="ta mono"
                  rows={7}
                  placeholder={
                    '每行一条 JSON：{"purchase_task_id":"...","action":"shipment","delivery_id":"SF","waybill_id":"..."}'
                  }
                  value={ctx.supplierAgentApplyText.value}
                  onChange={(e) => {
                    ctx.supplierAgentApplyText.value = e.target.value;
                  }}
                />
                <div className="row-actions">
                  <Switch
                    checked={ctx.supplierAgentDryRun.value}
                    onChange={(v: boolean) => {
                      ctx.supplierAgentDryRun.value = v;
                    }}
                    label={ctx.supplierAgentDryRun.value ? "干跑校验" : "直接写回"}
                  />
                  <Switch
                    checked={ctx.supplierAgentContinueOnError.value}
                    onChange={(v: boolean) => {
                      ctx.supplierAgentContinueOnError.value = v;
                    }}
                    label={ctx.supplierAgentContinueOnError.value ? "遇错继续" : "遇错停止"}
                  />
                  <Button
                    variant="accent"
                    icon="check"
                    onClick={() => ctx.applySupplierAgentResults()}
                  >
                    {ctx.supplierAgentDryRun.value ? "校验结果" : "写回结果"}
                  </Button>
                </div>
              </div>

              {ctx.supplierAgentExportPath.value && (
                <div className="toolbar">
                  <Pill tone="info">最近 Agent 导出：{ctx.supplierAgentExportPath.value}</Pill>
                </div>
              )}

              {agentResult && (
                <div className="stack">
                  <Pill tone={agentResult.failed > 0 ? "warning" : "success"}>
                    处理 {agentResult.processed} 条，成功 {agentResult.succeeded} 条，失败{" "}
                    {agentResult.failed} 条
                  </Pill>
                  <div className="tbl-wrap">
                    <table className="tbl">
                      <thead>
                        <tr>
                          <th>#</th>
                          <th>采购任务 ID</th>
                          <th>动作</th>
                          <th>状态</th>
                          <th>错误</th>
                        </tr>
                      </thead>
                      <tbody>
                        {agentResult.results.map((item) => (
                          <tr key={item.index}>
                            <td>{item.index}</td>
                            <td className="mono">{item.purchase_task_id}</td>
                            <td>{item.action}</td>
                            <td>
                              <Pill tone={ctx.statusType(item.status)}>{item.status}</Pill>
                            </td>
                            <td>{item.error || "-"}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
