/* ============================================================================
   订单利润核算 —— Soft 设计移植
   原始: ProfitSection.vue
   内容/IA/交互/中文文案 100% 保留，视觉换成 Soft（../primitives + app.css）。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select, Empty } from "../primitives";

export default function ProfitSection() {
  const ctx = useApp();

  const totals = ctx.orderProfitTotals.value;
  const rows = ctx.orderProfits.value;

  return (
    <section className="content-stack">
      <div className="panel">
        {/* 面板头：标题 + 状态筛选 + 刷新 */}
        <div className="panel-title">
          <div>
            <h2>订单利润核算</h2>
            <p>按订单聚合成交额、采购成本、运费、退款、售后赔付和其他成本。</p>
          </div>
          <div className="button-group">
            <Select
              value={ctx.orderProfitStatusFilter.value}
              onChange={(v) => {
                ctx.orderProfitStatusFilter.value = v;
                ctx.refreshOrderProfits();
              }}
              options={[
                { value: "all", label: "全部利润状态" },
                { value: "missing_purchase_task", label: "缺采购任务" },
                { value: "missing_cost", label: "缺成本" },
                { value: "loss", label: "亏损" },
                { value: "profitable", label: "有毛利" },
              ]}
            />
            <Button icon="refresh" onClick={() => ctx.refreshOrderProfits()}>
              刷新
            </Button>
          </div>
        </div>

        {/* KPI 条：6 项指标 */}
        <dl className="status-list compact">
          <div>
            <dt>订单数</dt>
            <dd>{totals.order_count}</dd>
          </div>
          <div>
            <dt>成交额</dt>
            <dd>{ctx.formatCents(totals.revenue_cents)}</dd>
          </div>
          <div>
            <dt>采购成本</dt>
            <dd>{ctx.formatCents(totals.purchase_cost_cents)}</dd>
          </div>
          <div>
            <dt>预估毛利</dt>
            <dd>{ctx.formatSignedCents(totals.estimated_profit_cents)}</dd>
          </div>
          <div>
            <dt>实际毛利</dt>
            <dd>{ctx.formatSignedCents(totals.actual_profit_cents)}</dd>
          </div>
          <div>
            <dt>待补成本</dt>
            <dd>{totals.unknown_actual_order_count}</dd>
          </div>
        </dl>

        {/* 利润调整项子面板 */}
        <div className="sub-panel">
          <div className="panel-title tight">
            <div>
              <h2>利润调整项</h2>
              <p>记录订单级采购运费、退款、售后赔付、其他成本或收入。</p>
            </div>
          </div>
          <div className="form-grid">
            {/* 订单 ID */}
            <input
              className="inp"
              value={ctx.orderProfitAdjustmentForm.order_id}
              placeholder="订单 ID 或微信订单号"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.orderProfitAdjustmentForm.order_id = e.target.value;
              }}
            />
            {/* 调整类型 */}
            <Select
              value={ctx.orderProfitAdjustmentForm.kind}
              onChange={(v) => {
                ctx.orderProfitAdjustmentForm.kind = v;
              }}
              options={ctx.profitAdjustmentKindOptions}
              placeholder="调整类型"
            />
            {/* 金额（分） */}
            <input
              className="inp"
              value={ctx.orderProfitAdjustmentForm.amount_cents}
              placeholder="金额，单位分"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.orderProfitAdjustmentForm.amount_cents = e.target.value;
              }}
            />
            {/* 备注 */}
            <input
              className="inp"
              value={ctx.orderProfitAdjustmentForm.note}
              placeholder="备注，可选"
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.orderProfitAdjustmentForm.note = e.target.value;
              }}
            />
            {/* 记录按钮 */}
            <Button
              variant="accent"
              icon="upload"
              onClick={() => ctx.recordOrderProfitAdjustment()}
            >
              记录
            </Button>
          </div>
        </div>

        {/* 主表格 */}
        <div className="tbl-wrap">
          <table className="tbl dense-table">
            <thead>
              <tr>
                <th style={{ minWidth: 170 }}>微信订单号</th>
                <th style={{ minWidth: 130 }}>店铺</th>
                <th style={{ width: 140 }}>订单状态</th>
                <th style={{ width: 130 }}>利润状态</th>
                <th style={{ width: 110 }}>成交额</th>
                <th style={{ width: 110 }}>采购成本</th>
                <th style={{ width: 120 }}>退款/赔付</th>
                <th style={{ width: 110 }}>其他成本</th>
                <th style={{ width: 120 }}>预估毛利</th>
                <th style={{ width: 120 }}>实际毛利</th>
                <th style={{ width: 100 }}>缺成本项</th>
                <th style={{ minWidth: 190 }}>更新时间</th>
                <th style={{ width: 90 }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.length === 0 ? (
                <tr>
                  <td colSpan={13}>
                    <Empty>当前没有利润数据。</Empty>
                  </td>
                </tr>
              ) : (
                rows.map((row) => (
                  <tr key={row.order_id}>
                    <td className="mono">{row.wechat_order_id}</td>
                    <td>{row.shop_name}</td>
                    <td>
                      <Pill tone={ctx.statusType(row.order_status)}>
                        {row.order_status}
                      </Pill>
                    </td>
                    <td>
                      <Pill tone={ctx.statusType(row.profit_status)}>
                        {ctx.profitStatusLabel(row.profit_status)}
                      </Pill>
                    </td>
                    <td className="mono">{ctx.formatCents(row.revenue_cents)}</td>
                    <td className="mono">{ctx.formatCents(row.purchase_cost_cents)}</td>
                    <td className="mono">
                      {ctx.formatCents(row.refund_cents + row.aftersale_compensation_cents)}
                    </td>
                    <td className="mono">
                      {ctx.formatCents(row.purchase_freight_cents + row.other_cost_cents)}
                    </td>
                    <td className="mono">{ctx.formatSignedCents(row.estimated_profit_cents)}</td>
                    <td className="mono">{ctx.formatSignedCents(row.actual_profit_cents)}</td>
                    <td>{row.missing_cost_count}</td>
                    <td className="subtext">{ctx.formatDateTime(row.updated_at)}</td>
                    <td>
                      <div className="row-actions">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => ctx.selectOrderProfitAdjustment(row)}
                        >
                          调整
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* 页脚：加载数 / 总数 */}
        <div className="action-row">
          <Pill tone="info">
            {rows.length} / {ctx.orderProfitTotal.value}
          </Pill>
        </div>
      </div>
    </section>
  );
}
