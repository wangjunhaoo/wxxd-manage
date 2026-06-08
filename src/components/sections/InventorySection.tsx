/* ============================================================================
   库存风控 —— Soft 设计，忠实移植自 InventorySection.vue
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select, Empty } from "../primitives";

export default function InventorySection() {
  const ctx = useApp();

  const risks = ctx.inventoryRisks.value;
  const stats = ctx.inventoryRiskStats.value;

  // 状态下拉选项（inventoryRiskStatusOptions 是普通数组，非 ref）
  const statusOptions = ctx.inventoryRiskStatusOptions.map((o) => ({
    label: o.label,
    value: o.value,
  }));

  return (
    <section className="content-stack">
      <div className="panel">
        {/* 面板标题区 */}
        <div className="panel-title">
          <div>
            <h2>库存风控</h2>
            <p>基于外部商品 SKU 库存、采购占用、供应商异常和铺货店铺数生成运营提醒。</p>
          </div>
          <div className="button-group" style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <Select
              value={ctx.inventoryRiskStatusFilter.value}
              onChange={(v: string) => {
                ctx.inventoryRiskStatusFilter.value = v;
                ctx.refreshInventoryRisks();
              }}
              options={statusOptions}
              width={140}
            />
            <Button icon="refresh" onClick={() => ctx.refreshInventoryRisks()}>
              刷新
            </Button>
            <Button variant="accent" icon="upload" onClick={() => ctx.runInventoryRiskScan()}>
              扫描并通知
            </Button>
          </div>
        </div>

        {/* KPI 统计条 */}
        <dl className="status-list compact">
          <div>
            <dt>风险商品</dt>
            <dd>{ctx.inventoryRiskTotal.value}</dd>
          </div>
          <div>
            <dt>断货</dt>
            <dd>{stats.out_of_stock_count}</dd>
          </div>
          <div>
            <dt>低库存/压力</dt>
            <dd>{stats.low_stock_count}</dd>
          </div>
          <div>
            <dt>供应商异常</dt>
            <dd>{stats.issue_count}</dd>
          </div>
        </dl>

        {/* 风险列表 */}
        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th style={{ minWidth: 170 }}>外部商品 ID</th>
                <th style={{ minWidth: 220 }}>商品</th>
                <th style={{ minWidth: 130 }}>供应商</th>
                <th style={{ width: 130 }}>状态</th>
                <th style={{ width: 100 }}>货源库存</th>
                <th style={{ width: 100 }}>采购占用</th>
                <th style={{ width: 100 }}>可用库存</th>
                <th style={{ width: 100 }}>已铺店铺</th>
                <th style={{ minWidth: 280 }}>建议</th>
                <th style={{ minWidth: 190 }}>更新时间</th>
              </tr>
            </thead>
            <tbody>
              {risks.length === 0 ? (
                <tr>
                  <td colSpan={10}>
                    <Empty>暂无数据。</Empty>
                  </td>
                </tr>
              ) : (
                risks.map((row) => (
                  <tr key={row.external_product_id}>
                    <td className="mono">{row.external_product_id}</td>
                    <td
                      style={{
                        maxWidth: 220,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.title}
                    >
                      {row.title}
                    </td>
                    <td>{row.supplier_name || "-"}</td>
                    <td>
                      <Pill tone={ctx.statusType(row.risk_status) as "success" | "warning" | "danger" | "info" | "primary"}>
                        {ctx.inventoryRiskLabel(row.risk_status)}
                      </Pill>
                    </td>
                    <td>{row.total_stock}</td>
                    <td>{row.reserved_quantity}</td>
                    <td>{row.available_stock}</td>
                    <td>{row.active_shop_count}</td>
                    <td
                      style={{
                        maxWidth: 280,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.recommendation}
                    >
                      {row.recommendation}
                    </td>
                    <td>{ctx.formatDateTime(row.updated_at)}</td>
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
