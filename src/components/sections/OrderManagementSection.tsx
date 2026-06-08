/* ============================================================================
   订单管理统一总览 —— Soft 视觉。集中查看订单同步、采购、发货、售后、利润状态，
   并作为跳转中枢导航到采购 / 售后 / 利润子页。忠实保留原 IA / 交互 / 中文文案。
   ============================================================================ */
import { Fragment, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select, Icon, Empty } from "../primitives";

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

  return (
    <div className="pad">
      <div className="wrap-wide">
        <div className="panel">
          {/* 标题 + 同步 / 刷新动作 */}
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

          {/* 主表格 */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th style={{ width: 40 }} />
                  <th>订单</th>
                  <th>管理状态</th>
                  <th>采购</th>
                  <th>发货</th>
                  <th>利润</th>
                  <th>商品</th>
                  <th>数量</th>
                  <th>成交额</th>
                  <th>预估毛利</th>
                  <th>详情同步</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {items.map((row) => {
                  const isOpen = expanded.has(row.order_id);
                  return (
                    <Fragment key={row.order_id}>
                      <tr>
                        <td>
                          <Button
                            variant="ghost"
                            size="sm"
                            iconOnly
                            icon={isOpen ? "chevronDown" : "chevronRight"}
                            onClick={() => toggleExpand(row.order_id)}
                            title={isOpen ? "收起明细" : "展开明细"}
                          />
                        </td>
                        <td>
                          <div className="cell-main">
                            <strong>{row.wechat_order_id || row.order_id}</strong>
                            <span>{row.shop_name}</span>
                            <span className="mono">
                              创建 {ctx.formatUnixTime(row.order_created_at)}
                            </span>
                          </div>
                        </td>
                        <td>
                          <Pill tone={ctx.statusType(row.management_status)}>
                            {ctx.orderManagementStatusLabel(row.management_status)}
                          </Pill>
                        </td>
                        <td>
                          <Pill tone={ctx.statusType(row.purchase_status)}>
                            {ctx.purchaseManagementStatusLabel(row.purchase_status)}
                          </Pill>
                        </td>
                        <td>
                          <Pill tone={ctx.statusType(row.shipment_status)}>
                            {ctx.shipmentStatusLabel(row.shipment_status)}
                          </Pill>
                        </td>
                        <td>
                          <Pill tone={ctx.statusType(row.profit_status)}>
                            {ctx.profitStatusLabel(row.profit_status)}
                          </Pill>
                        </td>
                        <td>{row.item_count}</td>
                        <td>{row.quantity}</td>
                        <td>{ctx.formatCents(row.revenue_cents)}</td>
                        <td>{ctx.formatSignedCents(row.estimated_profit_cents)}</td>
                        <td>
                          {row.detail_error ? (
                            <span className="subtext">{row.detail_error}</span>
                          ) : (
                            <span>{ctx.formatDateTime(row.detail_synced_at)}</span>
                          )}
                        </td>
                        <td>
                          <div className="row-actions">
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
                          <td colSpan={12}>
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
