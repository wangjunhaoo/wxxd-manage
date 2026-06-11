/* ============================================================================
   订单管理统一总览 —— Soft 视觉。集中查看订单同步、采购、发货、售后、利润状态，
   并作为跳转中枢导航到采购 / 售后 / 利润子页。
   布局原则：
   - 申请收件箱只在有待处理申请时以告警条出现（无申请不占订单空间），扫描入口收进工具栏；
   - 主表收敛为 8 列（双轴状态 + 角标 + 发货时效），不再横向溢出；
   - 详情同步 / 备注 / 留言 / 利润明细等长尾信息全部放进展开行。
   ============================================================================ */
import { Fragment, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select, Icon, Empty } from "../primitives";
import { orderRequestKindLabels } from "../../composables/useWxXdApp/constants";
import type { OrderManagementView } from "../../types/app";

/** 截止时间倒计时文案（改址超时=自动同意，换SKU超时=自动拒绝，发货超时=违约） */
function deadlineCountdown(deadlineAt: number | null): {
  text: string;
  urgent: boolean;
} {
  if (!deadlineAt) return { text: "—", urgent: false };
  const remain = deadlineAt - Math.floor(Date.now() / 1000);
  if (remain <= 0) return { text: "已超时", urgent: true };
  const hours = Math.floor(remain / 3600);
  const minutes = Math.floor((remain % 3600) / 60);
  if (hours >= 24) {
    return { text: `剩 ${Math.floor(hours / 24)} 天 ${hours % 24} 小时`, urgent: false };
  }
  return {
    text: hours > 0 ? `剩 ${hours} 小时 ${minutes} 分` : `剩 ${minutes} 分`,
    urgent: remain < 4 * 3600,
  };
}

/** 微信官方订单状态中文（双轴状态的官方轴） */
function wechatStatusLabel(status: number | null): string {
  if (status === null || status === undefined) return "未同步";
  const labels: Record<number, string> = {
    10: "待付款",
    12: "礼物待收下",
    13: "待成团",
    20: "待发货",
    21: "部分发货",
    30: "待收货",
    100: "已完成",
    200: "已取消·售后",
    250: "已取消",
  };
  return labels[status] ?? `状态 ${status}`;
}

/** 展开行概要的单项 */
function SummaryItem({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div style={{ display: "grid", gap: 2, minWidth: 0 }}>
      <span style={{ fontSize: 11.5, color: "var(--ink-3)" }}>{label}</span>
      <span style={{ fontSize: 13, color: "var(--ink)", wordBreak: "break-all" }}>
        {value ?? "—"}
      </span>
    </div>
  );
}

export default function OrderManagementSection() {
  const ctx = useApp();

  // ---- 展开行：记录已展开订单的 order_id ----
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const toggleExpand = (orderId: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(orderId)) {
        next.delete(orderId);
      } else {
        next.add(orderId);
      }
      return next;
    });
  };

  const items = ctx.orderManagementItems.value;
  const total = ctx.orderManagementTotal.value;

  // ---- 店铺筛选选项（含「全部店铺」）----
  const shopFilterOptions = [
    { value: "all", label: "全部店铺" },
    ...ctx.shops.value.map((shop) => ({ value: shop.id, label: shop.name })),
  ];

  // ---- KPI 汇总：按 management_status 计数 ----
  const orderSummary = {
    needsDetail: items.filter((item) => item.management_status === "needs_detail").length,
    needsPurchase: items.filter((item) => item.management_status === "needs_purchase").length,
    needsShipment: items.filter((item) => item.management_status === "needs_shipment").length,
    aftersale: items.filter((item) => item.management_status === "aftersale_active").length,
  };

  // ---- section 跳转（同复合容器内切换，非路由）----
  const goTo = (section: string) => {
    ctx.selectedSection.value = section;
  };

  const onKeywordEnter = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      ctx.refreshOrderManagementItems();
    }
  };

  const pendingRequests = ctx.orderRequests.value.filter(
    (request) => request.state === "pending",
  );

  /** 订单行的风险/协商角标（双轴之外的横向标志） */
  const renderFlags = (row: OrderManagementView) => {
    const flags: React.ReactNode[] = [];
    if (row.address_under_review) {
      flags.push(
        <Pill key="addr" tone="danger">
          改址审核中
        </Pill>,
      );
    }
    if (row.change_sku_state === 3) {
      flags.push(
        <Pill key="sku" tone="warning">
          换SKU待处理
        </Pill>,
      );
    }
    if (row.active_aftersale_count > 0) {
      flags.push(
        <Pill key="as" tone="danger">
          售后中 ×{row.active_aftersale_count}
        </Pill>,
      );
    }
    if (row.detail_error) {
      flags.push(
        <Pill key="err" tone="danger">
          详情失败
        </Pill>,
      );
    }
    if (flags.length === 0) return null;
    return (
      <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 4 }}>
        {flags}
      </div>
    );
  };

  return (
    <div className="pad">
      <div className="wrap-wide">
        {/* 发货拦截告警条：仅在存在待处理申请时出现（漏处理会错发，是最高优先级告警；
            无申请时彻底不渲染，不挤占订单空间） */}
        {pendingRequests.length > 0 && (
          <div
            className="panel"
            style={{
              marginBottom: 16,
              borderLeft: "4px solid var(--danger, #e5484d)",
            }}
          >
            <div className="ph" style={{ marginBottom: 8 }}>
              <div>
                <h3>
                  发货拦截：{pendingRequests.length} 个改址/换SKU申请待处理
                </h3>
                <p>
                  改址 12 小时不处理将被自动同意，换SKU 超时自动拒绝；处理完发货自动恢复。
                </p>
              </div>
            </div>
            <div className="tbl-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>订单</th>
                    <th>类型</th>
                    <th>剩余时间</th>
                    <th>风险</th>
                    <th style={{ width: 170 }}>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {pendingRequests.map((request) => {
                    const countdown = deadlineCountdown(request.deadline_at);
                    const risky =
                      request.kind === "address_change" &&
                      request.purchase_task_count > 0;
                    return (
                      <tr key={request.id}>
                        <td>
                          <div className="cell-main">
                            <strong>{request.wechat_order_id}</strong>
                            <span>{request.shop_name}</span>
                          </div>
                        </td>
                        <td>
                          <Pill tone={request.kind === "address_change" ? "warning" : "info"}>
                            {orderRequestKindLabels[request.kind] ?? request.kind}
                          </Pill>
                        </td>
                        <td>
                          <Pill tone={countdown.urgent ? "danger" : "info"}>
                            {countdown.text}
                          </Pill>
                        </td>
                        <td>
                          {risky ? (
                            <Pill tone="danger">已采购 {request.purchase_task_count} 单，建议拒绝</Pill>
                          ) : (
                            <span className="subtext">—</span>
                          )}
                        </td>
                        <td>
                          <div className="row-actions">
                            <Button
                              variant={risky ? "ghost" : "accent"}
                              size="sm"
                              onClick={() => ctx.decideOrderRequest(request, true)}
                            >
                              同意
                            </Button>
                            <Button
                              variant={risky ? "accent" : "ghost"}
                              size="sm"
                              onClick={() => ctx.decideOrderRequest(request, false)}
                            >
                              拒绝
                            </Button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        )}

        <div className="panel">
          {/* 标题 + 同步 / 申请扫描 / 刷新动作 */}
          <div className="ph">
            <div>
              <h3>订单管理</h3>
              <p>集中查看订单同步、采购、发货、售后和利润状态。</p>
            </div>
            <div className="row-actions">
              <Button icon="refresh" size="sm" onClick={() => ctx.runOrderSyncOnce()}>
                同步待发货
              </Button>
              <Button icon="refresh" size="sm" onClick={() => ctx.runOrderDetailSyncOnce()}>
                同步详情
              </Button>
              <Button
                icon="search"
                size="sm"
                title="扫描微信侧的改址/换SKU申请，有待处理时会在上方出现发货拦截条"
                onClick={() => ctx.runNegotiationScanOnce()}
              >
                扫描申请
              </Button>
              <Button icon="refresh" size="sm" onClick={() => ctx.refreshOrderManagementItems()}>
                刷新
              </Button>
            </div>
          </div>

          {/* 筛选条 */}
          <div className="form-grid cols-4" style={{ marginBottom: 16 }}>
            <Select
              value={ctx.orderManagementStatusFilter.value}
              onChange={(v: string) => {
                ctx.orderManagementStatusFilter.value = v;
                ctx.refreshOrderManagementItems();
              }}
              options={ctx.orderManagementStatusOptions}
              placeholder="订单状态"
            />
            <Select
              value={ctx.orderManagementShopFilter.value}
              onChange={(v: string) => {
                ctx.orderManagementShopFilter.value = v;
                ctx.refreshOrderManagementItems();
              }}
              options={shopFilterOptions}
              placeholder="店铺"
            />
            <input
              className="inp"
              value={ctx.orderManagementKeyword.value}
              placeholder="搜索订单号、店铺、商品或 SKU"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.orderManagementKeyword.value = e.target.value;
              }}
              onKeyUp={onKeywordEnter}
            />
            <Button
              variant="accent"
              icon="search"
              onClick={() => ctx.refreshOrderManagementItems()}
            >
              搜索
            </Button>
          </div>

          {/* KPI 条 */}
          <dl className="kpis cols-5" style={{ marginBottom: 16 }}>
            <div className="kpi">
              <span>匹配订单</span>
              <strong>{total}</strong>
            </div>
            <div className="kpi">
              <span>待同步详情</span>
              <strong>{orderSummary.needsDetail}</strong>
            </div>
            <div className="kpi">
              <span>待采购</span>
              <strong>{orderSummary.needsPurchase}</strong>
            </div>
            <div className="kpi">
              <span>待发货</span>
              <strong>{orderSummary.needsShipment}</strong>
            </div>
            <div className="kpi">
              <span>售后中</span>
              <strong>{orderSummary.aftersale}</strong>
            </div>
          </dl>

          {/* 主表格：8 列收敛布局（详情同步/备注等长尾信息在展开行） */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th style={{ width: 40 }} />
                  <th>订单</th>
                  <th>状态</th>
                  <th>采购</th>
                  <th>发货</th>
                  <th>商品</th>
                  <th>金额 / 毛利</th>
                  <th>发货时效</th>
                  <th style={{ width: 200 }}>操作</th>
                </tr>
              </thead>
              <tbody>
                {items.map((row) => {
                  const isOpen = expanded.has(row.order_id);
                  const shipDeadline =
                    row.order_status === "completed" ||
                    row.order_status === "cancelled" ||
                    row.order_status === "wechat_shipped" ||
                    row.shipment_status === "wechat_shipped"
                      ? null
                      : row.delivery_deadline;
                  const countdown = deadlineCountdown(shipDeadline);
                  return (
                    <Fragment key={row.order_id}>
                      <tr>
                        <td className="top">
                          <Button
                            variant="ghost"
                            size="sm"
                            iconOnly
                            icon={isOpen ? "chevronDown" : "chevronRight"}
                            onClick={() => toggleExpand(row.order_id)}
                            title={isOpen ? "收起明细" : "展开明细"}
                          />
                        </td>
                        <td className="top">
                          <div className="cell-main">
                            <strong>{row.wechat_order_id || row.order_id}</strong>
                            <span>{row.shop_name}</span>
                            {row.order_created_at !== null && (
                              <span className="mono">
                                {ctx.formatUnixTime(row.order_created_at)}
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="top">
                          <div className="cell-main">
                            <div>
                              <Pill tone={ctx.statusType(row.management_status)}>
                                {ctx.orderManagementStatusLabel(row.management_status)}
                              </Pill>
                            </div>
                            <span>微信侧：{wechatStatusLabel(row.wechat_status)}</span>
                          </div>
                          {renderFlags(row)}
                        </td>
                        <td className="top">
                          <Pill tone={ctx.statusType(row.purchase_status)}>
                            {ctx.purchaseManagementStatusLabel(row.purchase_status)}
                          </Pill>
                        </td>
                        <td className="top">
                          <Pill tone={ctx.statusType(row.shipment_status)}>
                            {ctx.shipmentStatusLabel(row.shipment_status)}
                          </Pill>
                        </td>
                        <td className="top">
                          <span className="nowrap">
                            {row.item_count} 商品 · {row.quantity} 件
                          </span>
                        </td>
                        <td className="top">
                          <div className="cell-main">
                            <strong>{ctx.formatCents(row.revenue_cents)}</strong>
                            <span
                              style={{
                                color:
                                  row.estimated_profit_cents >= 0
                                    ? "var(--success, #2e9e6b)"
                                    : "var(--danger, #e5484d)",
                              }}
                            >
                              毛利 {ctx.formatSignedCents(row.estimated_profit_cents)}
                            </span>
                          </div>
                        </td>
                        <td className="top">
                          {shipDeadline ? (
                            <Pill tone={countdown.urgent ? "danger" : "info"}>
                              {countdown.text}
                            </Pill>
                          ) : (
                            <span className="subtext">—</span>
                          )}
                        </td>
                        <td className="top">
                          <div className="row-actions" style={{ flexWrap: "nowrap" }}>
                            <Button variant="ghost" size="sm" onClick={() => goTo("procurement")}>
                              采购
                            </Button>
                            <Button variant="ghost" size="sm" onClick={() => goTo("exceptions")}>
                              售后
                            </Button>
                            <Button variant="ghost" size="sm" onClick={() => goTo("analytics")}>
                              利润
                            </Button>
                          </div>
                        </td>
                      </tr>
                      {isOpen && (
                        <tr>
                          <td colSpan={9} style={{ background: "var(--chip)" }}>
                            {/* 订单概要：长尾信息集中区 */}
                            <div
                              style={{
                                display: "grid",
                                gridTemplateColumns:
                                  "repeat(auto-fill, minmax(200px, 1fr))",
                                gap: "10px 20px",
                                padding: "4px 4px 14px",
                              }}
                            >
                              <SummaryItem
                                label="本地履约状态"
                                value={ctx.orderManagementStatusLabel(row.order_status)}
                              />
                              <SummaryItem
                                label="微信官方状态"
                                value={wechatStatusLabel(row.wechat_status)}
                              />
                              <SummaryItem
                                label="详情同步"
                                value={
                                  row.detail_error ? (
                                    <span style={{ color: "var(--danger, #e5484d)" }}>
                                      {row.detail_error}
                                    </span>
                                  ) : (
                                    ctx.formatDateTime(row.detail_synced_at)
                                  )
                                }
                              />
                              <SummaryItem
                                label="微信侧最近更新"
                                value={ctx.formatUnixTime(row.order_updated_at)}
                              />
                              <SummaryItem
                                label="采购任务"
                                value={
                                  row.purchase_task_count > 0
                                    ? `${row.purchase_task_count} 个${
                                        row.missing_cost_count > 0
                                          ? `（${row.missing_cost_count} 个缺成本）`
                                          : ""
                                      }`
                                    : "未生成"
                                }
                              />
                              <SummaryItem
                                label="实际利润"
                                value={
                                  row.actual_profit_cents !== null
                                    ? ctx.formatSignedCents(row.actual_profit_cents)
                                    : "待结算"
                                }
                              />
                              {row.customer_notes && (
                                <SummaryItem label="买家留言" value={row.customer_notes} />
                              )}
                              {row.merchant_notes && (
                                <SummaryItem label="商家备注" value={row.merchant_notes} />
                              )}
                            </div>
                            {/* 商品明细 */}
                            {row.items.length > 0 ? (
                              <table className="subtbl">
                                <thead>
                                  <tr>
                                    <th>商品</th>
                                    <th>外部商品 ID</th>
                                    <th>外部 SKU</th>
                                    <th>微信商品 ID</th>
                                    <th>数量</th>
                                    <th>实收</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {row.items.map((item) => (
                                    <tr key={item.id}>
                                      <td>{item.title}</td>
                                      <td>{item.external_product_id}</td>
                                      <td>{item.external_sku_id}</td>
                                      <td>{item.wechat_product_id}</td>
                                      <td>{item.quantity}</td>
                                      <td>{ctx.formatCents(item.real_price ?? item.sale_price)}</td>
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            ) : (
                              <div className="empty">还没有同步订单商品明细。</div>
                            )}
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          </div>

          {items.length === 0 && <Empty>当前没有匹配的订单。</Empty>}

          {/* 表格下方动作行 */}
          <div className="toolbar" style={{ marginTop: 16 }}>
            <Button icon="refresh" size="sm" onClick={() => ctx.runPurchaseTaskGenerationOnce()}>
              生成采购任务
            </Button>
            <Pill tone="info">
              <Icon name="receipt" size={14} /> {items.length} / {total}
            </Pill>
          </div>
        </div>
      </div>
    </div>
  );
}
