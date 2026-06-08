/* ============================================================================
   商品动销分析（SalesSection）—— Soft 设计移植
   忠实保留原 Vue SFC 的内容/IA/交互/中文文案，视觉换用 Soft 原子组件 + app.css 类。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select } from "../primitives";

export default function SalesSection() {
  const ctx = useApp();

  const rows = ctx.productSalesAnalysis.value;
  const total = ctx.productSalesAnalysisTotal.value;
  const totals = ctx.productSalesAnalysisTotals.value;
  const statusFilter = ctx.productSalesAnalysisStatusFilter.value;
  const statusOptions = ctx.productSalesStatusOptions;

  return (
    <section className="content-stack">
      <div className="panel">
        {/* 面板头：标题 + 筛选 + 刷新 */}
        <div className="panel-title">
          <div>
            <h2>商品动销分析</h2>
            <p>
              按外部商品聚合真实订单、采购成本、售后关联、铺货店铺和库存风险，输出继续铺货、调价、补货或观察建议。
            </p>
          </div>
          <div className="button-group">
            <Select
              value={statusFilter}
              onChange={(v) => {
                ctx.productSalesAnalysisStatusFilter.value = v;
                ctx.refreshProductSalesAnalysis();
              }}
              options={statusOptions}
              width={140}
            />
            <Button icon="refresh" onClick={() => ctx.refreshProductSalesAnalysis()}>
              刷新
            </Button>
          </div>
        </div>

        {/* KPI 条：8 格 */}
        <dl className="status-list compact">
          <div>
            <dt>商品数</dt>
            <dd>{totals.product_count}</dd>
          </div>
          <div>
            <dt>已动销</dt>
            <dd>{totals.sold_product_count}</dd>
          </div>
          <div>
            <dt>销量</dt>
            <dd>{totals.total_units_sold}</dd>
          </div>
          <div>
            <dt>成交额</dt>
            <dd>{ctx.formatCents(totals.revenue_cents)}</dd>
          </div>
          <div>
            <dt>粗毛利</dt>
            <dd>{ctx.formatSignedCents(totals.gross_profit_cents)}</dd>
          </div>
          <div>
            <dt>可放量</dt>
            <dd>{totals.scale_candidate_count}</dd>
          </div>
          <div>
            <dt>风险</dt>
            <dd>{totals.risk_product_count}</dd>
          </div>
          <div>
            <dt>缺成本</dt>
            <dd>{totals.missing_cost_product_count}</dd>
          </div>
        </dl>

        {/* 数据表格 */}
        <div className="tbl-wrap">
          <table className="tbl dense-table">
            <thead>
              <tr>
                <th style={{ minWidth: 170 }}>外部商品 ID</th>
                <th style={{ minWidth: 220 }}>商品</th>
                <th style={{ width: 130 }}>运营状态</th>
                <th style={{ width: 120 }}>库存</th>
                <th style={{ width: 80 }}>店铺</th>
                <th style={{ width: 80 }}>订单</th>
                <th style={{ width: 80 }}>销量</th>
                <th style={{ width: 110 }}>成交额</th>
                <th style={{ width: 110 }}>采购成本</th>
                <th style={{ width: 110 }}>粗毛利</th>
                <th style={{ width: 90 }}>缺成本</th>
                <th style={{ width: 80 }}>售后</th>
                <th style={{ width: 100 }}>可用库存</th>
                <th style={{ minWidth: 300 }}>建议</th>
                <th style={{ minWidth: 190 }}>最近订单</th>
              </tr>
            </thead>
            <tbody>
              {rows.length === 0 ? (
                <tr>
                  <td colSpan={15}>
                    <div className="empty">暂无数据</div>
                  </td>
                </tr>
              ) : (
                rows.map((row) => (
                  <tr key={row.external_product_id}>
                    <td className="mono">{row.external_product_id}</td>
                    <td>
                      <div className="cell-main" title={row.title}>
                        <span
                          style={{
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                            display: "block",
                            maxWidth: 216,
                          }}
                        >
                          {row.title}
                        </span>
                      </div>
                    </td>
                    <td>
                      <Pill tone={ctx.statusType(row.operation_status)}>
                        {ctx.productSalesStatusLabel(row.operation_status)}
                      </Pill>
                    </td>
                    <td>
                      <Pill tone={ctx.statusType(row.inventory_risk_status)}>
                        {ctx.inventoryRiskLabel(row.inventory_risk_status)}
                      </Pill>
                    </td>
                    <td>{row.active_shop_count}</td>
                    <td>{row.order_count}</td>
                    <td>{row.units_sold}</td>
                    <td className="mono">{ctx.formatCents(row.revenue_cents)}</td>
                    <td className="mono">{ctx.formatCents(row.purchase_cost_cents)}</td>
                    <td className="mono">
                      {ctx.formatSignedCents(
                        row.revenue_cents - row.purchase_cost_cents - row.related_refund_cents,
                      )}
                    </td>
                    <td>{row.missing_cost_count}</td>
                    <td>{row.related_aftersale_count}</td>
                    <td>{row.available_stock}</td>
                    <td>
                      <span
                        title={row.recommendation}
                        style={{
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          display: "block",
                          maxWidth: 296,
                        }}
                      >
                        {row.recommendation}
                      </span>
                    </td>
                    <td className="mono">{ctx.formatDateTime(row.last_order_at)}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* 页脚计数 */}
        <div className="action-row">
          <Pill tone="info">
            {rows.length} / {total}
          </Pill>
        </div>
      </div>
    </section>
  );
}
