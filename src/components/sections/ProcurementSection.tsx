/* ============================================================================
   采购下单工作台 —— Soft 视觉
   ----------------------------------------------------------------------------
   忠实移植自 ProcurementSection.vue：内联「待采购订单表 + 供应商物流回填 + 发货队列」。
   先按货源下单，再填供应商物流；买不了的单独标记异常。
   IA / 交互 / 中文文案 100% 保留，仅把 Element Plus 换成 ../primitives + app.css 类。
   ============================================================================ */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { ElMessage } from "../../runtime/feedback";
import {
  PageHead,
  Button,
  Pill,
  Chip,
  Select,
  Switch,
  Segmented,
  Modal,
} from "../primitives";
import type { PurchaseTaskView, ShipmentView } from "../../types/app";

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
    submitting: "提交中",
    send_failed: "发货失败",
    blocked: "守卫拦截",
    wechat_shipped: "微信已发货",
  };
  return labels[status] || status;
}

// 补发原因官方枚举（delivery/compensation）
const compensateReasonOptions = [
  { value: 1, label: "漏发补寄" },
  { value: 2, label: "拆包发货" },
  { value: 3, label: "坏损补寄" },
  { value: 4, label: "赠品" },
];

export default function ProcurementSection() {
  const ctx = useApp();

  const [activePurchaseAction, setActivePurchaseAction] =
    useState<PurchaseAction>("shipment");
  // 处理弹窗：非空 = 弹窗打开并展示该任务的上下文
  const [actionTask, setActionTask] = useState<PurchaseTaskView | null>(null);
  const [actionSubmitting, setActionSubmitting] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  // ---- 物流修正（三期 §8）：改运单（≤3 次）/ 补发（≤10 个包裹）----
  const [logisticsFix, setLogisticsFix] = useState<{
    mode: "change" | "compensate";
    shipment: ShipmentView;
  } | null>(null);
  const [fixDeliveryId, setFixDeliveryId] = useState("");
  const [fixWaybillId, setFixWaybillId] = useState("");
  const [fixReason, setFixReason] = useState(1);
  // 在途提交锁：改运单（≤3 次）/补发（≤10 个）烧的是微信侧不可恢复的硬配额，
  // 双击重复调用会白耗配额并造成本地计数与官方错位
  const [fixSubmitting, setFixSubmitting] = useState(false);

  function openLogisticsFix(mode: "change" | "compensate", shipment: ShipmentView) {
    setLogisticsFix({ mode, shipment });
    setFixDeliveryId("");
    setFixWaybillId("");
    setFixReason(1);
    setFixSubmitting(false);
  }

  // 改运单时新值与原运单完全一致 = 原样重报，白烧 1 次配额，直接禁用提交
  const fixUnchanged =
    logisticsFix?.mode === "change" &&
    fixDeliveryId.trim() === (logisticsFix.shipment.delivery_id ?? "") &&
    fixWaybillId.trim() === (logisticsFix.shipment.waybill_id ?? "");

  async function submitLogisticsFix() {
    if (!logisticsFix || fixSubmitting) return;
    if (!fixDeliveryId.trim() || !fixWaybillId.trim() || fixUnchanged) {
      return;
    }
    const companyName =
      ctx.deliveryCompanyOptions.value.find(
        (item) => item.value === fixDeliveryId,
      )?.label ?? null;
    const { mode, shipment } = logisticsFix;
    setFixSubmitting(true);
    try {
      const result =
        mode === "change"
          ? await ctx.changeShipmentDeliveryInfo({
              orderId: shipment.order_id,
              oldDeliveryId: shipment.delivery_id,
              oldWaybillId: shipment.waybill_id,
              deliveryId: fixDeliveryId.trim(),
              deliveryName: companyName,
              waybillId: fixWaybillId.trim(),
            })
          : await ctx.compensateOrderDelivery({
              orderId: shipment.order_id,
              deliveryId: fixDeliveryId.trim(),
              deliveryName: companyName,
              waybillId: fixWaybillId.trim(),
              reason: fixReason,
            });
      if (result) {
        setLogisticsFix(null);
      }
    } finally {
      setFixSubmitting(false);
    }
  }

  const tasks = ctx.purchaseTasks.value;
  const shipments = ctx.deliveryShipments.value;

  // ---- 采购双节点（订单履约重设计 §7）：待下单 / 待回运单 两条队列（客户端按 purchased_at 拆分）----
  const [purchaseQueue, setPurchaseQueue] = useState<
    "all" | "to_buy" | "awaiting_waybill"
  >("all");
  const visibleTasks = tasks.filter((t) => {
    if (purchaseQueue === "to_buy") {
      return t.status === "pending_purchase" && !t.purchased_at;
    }
    if (purchaseQueue === "awaiting_waybill") {
      return t.status === "pending_purchase" && !!t.purchased_at;
    }
    return true;
  });

  // ---- 筛选胶囊选项 ----
  const purchaseStatusOptions = [
    { value: "all", label: "全部", count: tasks.length },
    {
      value: "to_buy",
      label: "待下单",
      count: tasks.filter(
        (t) => t.status === "pending_purchase" && !t.purchased_at,
      ).length,
    },
    {
      value: "awaiting_waybill",
      label: "待回运单",
      count: tasks.filter(
        (t) => t.status === "pending_purchase" && !!t.purchased_at,
      ).length,
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

  // ---- 交互助手 ----
  async function setPurchaseFilter(status: string) {
    // 双节点虚拟队列：后端仍按 pending_purchase 过滤，前端按 purchased_at 拆分
    if (status === "to_buy" || status === "awaiting_waybill") {
      setPurchaseQueue(status);
      ctx.purchaseStatusFilter.value = "pending_purchase";
    } else {
      setPurchaseQueue("all");
      ctx.purchaseStatusFilter.value = status;
    }
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

  // 点行内按钮 → 预填对应表单并弹出处理弹窗（actionTask 非空即弹窗打开）
  function startMapping(row: PurchaseTaskView) {
    ctx.selectPurchaseTaskMapping(row);
    setActivePurchaseAction("mapping");
    setActionTask(row);
  }

  function startShipment(row: PurchaseTaskView) {
    ctx.selectPurchaseTaskShipment(row);
    setActivePurchaseAction("shipment");
    setActionTask(row);
  }

  function startIssue(row: PurchaseTaskView) {
    ctx.selectPurchaseTaskIssue(row);
    setActivePurchaseAction("issue");
    setActionTask(row);
  }

  // 弹窗内切换动作时，用当前任务重新预填目标表单，避免带出上一个任务的数据
  function switchAction(v: PurchaseAction) {
    if (actionTask) {
      if (v === "mapping") ctx.selectPurchaseTaskMapping(actionTask);
      else if (v === "shipment") ctx.selectPurchaseTaskShipment(actionTask);
      else ctx.selectPurchaseTaskIssue(actionTask);
    }
    setActivePurchaseAction(v);
  }

  // 发货失败/被拦截的单子微信侧并未发货，「改运单」（官方已发货纠错接口）用不上；
  // 正确路径 = 修改采购任务的运单号重新回填，旧失败行会被 supersede 自动作废
  async function startFixWaybill(row: ShipmentView) {
    let pool = ctx.purchaseTasks.value.filter((t) => t.order_id === row.order_id);
    if (pool.length === 0) {
      // 当前筛选可能不含该任务：切回全部再找一次
      setPurchaseQueue("all");
      ctx.purchaseStatusFilter.value = "all";
      await ctx.refreshPurchaseTasks();
      pool = ctx.purchaseTasks.value.filter((t) => t.order_id === row.order_id);
    }
    const matched =
      pool.find(
        (t) => t.supplier_waybill_id && t.supplier_waybill_id === row.waybill_id,
      ) ?? (pool.length === 1 ? pool[0] : undefined);
    if (matched) {
      startShipment(matched);
      return;
    }
    ElMessage.warning(
      pool.length > 1
        ? "该订单有多个采购任务且运单号未能对应，请在上方采购列表逐个核对修改"
        : "未找到该订单的采购任务，请在上方采购列表核实后修改运单号",
    );
  }

  // 提交锁防双击重复提交；成功后关闭弹窗
  async function submitPurchaseAction(action: () => Promise<boolean>) {
    if (actionSubmitting) return;
    setActionSubmitting(true);
    try {
      const ok = await action();
      if (ok) setActionTask(null);
    } finally {
      setActionSubmitting(false);
    }
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
                active={
                  option.value === "to_buy" || option.value === "awaiting_waybill"
                    ? purchaseQueue === option.value
                    : purchaseQueue === "all" &&
                      ctx.purchaseStatusFilter.value === option.value
                }
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
                  <th>收货地址</th>
                  <th>数量/金额</th>
                  <th>供应商物流</th>
                  <th>现在该做</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {visibleTasks.map((row) => (
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
                        <strong>{row.decoded_region || (row.has_decoded_address ? "已解密" : "未解密")}</strong>
                        {row.has_decoded_address ? (
                          <button
                            className="link-src"
                            onClick={() => ctx.copyDecodedAddress(row.order_id)}
                          >
                            复制完整地址
                          </button>
                        ) : (
                          <button
                            className="link-src"
                            onClick={() => ctx.decodeOrderAddressNow(row.order_id)}
                          >
                            解密地址
                          </button>
                        )}
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
                        {row.status === "pending_purchase" && row.purchased_at
                          ? "已下单，等快递单号"
                          : purchaseStatusLabel(row.status)}
                      </Pill>
                      {row.error_summary && (
                        <small className="subtext">{row.error_summary}</small>
                      )}
                    </td>
                    <td>
                      <div className="row-actions">
                        {row.status === "pending_purchase" && !row.purchased_at && (
                          <Button
                            size="sm"
                            variant="accent"
                            onClick={() => ctx.markPurchaseTaskPurchased(row.id, true)}
                          >
                            标记已下单
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant={
                            row.status === "pending_purchase" && !row.purchased_at
                              ? "ghost"
                              : "accent"
                          }
                          onClick={() => handlePrimaryAction(row)}
                        >
                          {row.status === "needs_mapping"
                            ? "补货源"
                            : supplierIssueStatuses.has(row.status)
                              ? "处理异常"
                              : "填物流"}
                        </Button>
                        {row.status === "pending_purchase" && row.purchased_at && (
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => ctx.markPurchaseTaskPurchased(row.id, false)}
                          >
                            撤销下单
                          </Button>
                        )}
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

        {/* ===== 处理采购单弹窗：点行内「填物流 / 补资料 / 买不了」即弹出 ===== */}
        <Modal
          open={!!actionTask}
          title="处理采购单"
          onClose={() => setActionTask(null)}
        >
          {actionTask && (
            <div className="stack" style={{ gap: 16 }}>
              <div className="cell-main">
                <strong>{actionTask.title || "未同步商品标题"}</strong>
                <span className="mono">
                  {actionTask.shop_name} · {actionTask.wechat_order_id} · x
                  {actionTask.quantity}
                </span>
                <span className="mono">{actionTask.id}</span>
              </div>
              <Segmented
                options={[
                  { label: "填物流", value: "shipment" },
                  { label: "补资料", value: "mapping" },
                  { label: "买不了", value: "issue" },
                ]}
                value={activePurchaseAction}
                onChange={(v: string) => switchAction(v as PurchaseAction)}
              />
              {activePurchaseAction === "shipment" ? (
                <div className="form-grid cols-2">
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
                    disabled={actionSubmitting}
                    onClick={() =>
                      submitPurchaseAction(() => ctx.recordPurchaseTaskShipment())
                    }
                  >
                    {actionSubmitting ? "保存中…" : "保存物流"}
                  </Button>
                </div>
              ) : activePurchaseAction === "mapping" ? (
                <div className="form-grid cols-2">
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
                    disabled={actionSubmitting}
                    onClick={() =>
                      submitPurchaseAction(() => ctx.resolvePurchaseTaskMapping())
                    }
                  >
                    {actionSubmitting ? "保存中…" : "保存货源信息"}
                  </Button>
                </div>
              ) : (
                <div className="form-grid cols-2">
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
                    disabled={actionSubmitting}
                    onClick={() =>
                      submitPurchaseAction(() => ctx.markPurchaseTaskIssue())
                    }
                  >
                    {actionSubmitting ? "提交中…" : "标记买不了"}
                  </Button>
                </div>
              )}
            </div>
          )}
        </Modal>

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
                  { value: "blocked", label: "守卫拦截" },
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
              <Switch
                checked={ctx.deliverySettings.value.multi_package_enabled}
                onChange={(v: boolean) => {
                  ctx.deliverySettings.value.multi_package_enabled = v;
                  ctx.setMultiPackageEnabled(v);
                }}
                label="多运单拆包"
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
                    <td>
                      {row.status === "blocked"
                        ? row.blocked_reason || "守卫拦截"
                        : row.error_summary || row.error_code || "-"}
                    </td>
                    <td className="mono">{ctx.formatDateTime(row.updated_at)}</td>
                    <td>
                      <div className="row-actions">
                        <Button
                          size="sm"
                          disabled={row.status === "wechat_shipped"}
                          title="按当前单号原样重新提交微信发货"
                          onClick={() => ctx.retryDeliveryShipment(row)}
                        >
                          重试
                        </Button>
                        {(row.status === "send_failed" ||
                          row.status === "blocked") && (
                          <Button
                            size="sm"
                            variant="accent"
                            title="修改运单号后重新提交（单号填错/被其他订单占用时用这个）"
                            onClick={() => startFixWaybill(row)}
                          >
                            改单号
                          </Button>
                        )}
                        <Button
                          size="sm"
                          disabled={row.status !== "wechat_shipped"}
                          title={
                            row.status !== "wechat_shipped"
                              ? "仅微信已发货的订单可改运单（官方接口前提）；未发货成功请用「改单号」"
                              : "微信侧已发货后修改运单（官方限 3 次）"
                          }
                          onClick={() => openLogisticsFix("change", row)}
                        >
                          改运单
                        </Button>
                        <Button
                          size="sm"
                          disabled={row.status !== "wechat_shipped"}
                          title={
                            row.status !== "wechat_shipped"
                              ? "仅微信已发货的订单可补发"
                              : "漏发/坏损补寄一个新包裹（官方限 10 个）"
                          }
                          onClick={() => openLogisticsFix("compensate", row)}
                        >
                          补发
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* 物流修正表单：改运单走 deliveryinfo/update（≤3 次），补发走 delivery/compensation（≤10 个包裹） */}
          {logisticsFix && (
            <div className="subcard">
              <div className="ph">
                <div>
                  <h3>
                    {logisticsFix.mode === "change" ? "改运单" : "补发包裹"}：订单{" "}
                    {logisticsFix.shipment.wechat_order_id}
                  </h3>
                  <p>
                    {logisticsFix.mode === "change"
                      ? `原运单 ${logisticsFix.shipment.waybill_id || "-"}（${
                          logisticsFix.shipment.delivery_name ||
                          logisticsFix.shipment.delivery_id ||
                          "-"
                        }）→ 新运单；微信限同一订单最多改 3 次`
                      : "前置要求订单商品已全部发货且在售后期内；微信限同一订单最多补发 10 个包裹"}
                  </p>
                </div>
              </div>
              <div className="form-grid cols-3">
                <Select
                  value={fixDeliveryId}
                  onChange={(v: string) => setFixDeliveryId(v)}
                  options={ctx.deliveryCompanyOptions.value}
                  placeholder={
                    logisticsFix.mode === "change" ? "新快递公司" : "补发快递公司"
                  }
                />
                <input
                  className="inp"
                  placeholder={
                    logisticsFix.mode === "change" ? "新运单号" : "补发运单号"
                  }
                  value={fixWaybillId}
                  onChange={(e) => setFixWaybillId(e.target.value)}
                />
                {logisticsFix.mode === "compensate" && (
                  <Select
                    value={fixReason}
                    onChange={(v: string) => setFixReason(Number(v))}
                    options={compensateReasonOptions}
                    placeholder="补发原因"
                  />
                )}
                <div className="row-actions">
                  <Button
                    variant="accent"
                    icon="upload"
                    disabled={
                      fixSubmitting ||
                      !fixDeliveryId.trim() ||
                      !fixWaybillId.trim() ||
                      fixUnchanged
                    }
                    onClick={() => submitLogisticsFix()}
                  >
                    {fixSubmitting
                      ? "提交中…"
                      : logisticsFix.mode === "change"
                        ? "提交改运单"
                        : "提交补发"}
                  </Button>
                  <Button disabled={fixSubmitting} onClick={() => setLogisticsFix(null)}>
                    取消
                  </Button>
                </div>
              </div>
            </div>
          )}

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
