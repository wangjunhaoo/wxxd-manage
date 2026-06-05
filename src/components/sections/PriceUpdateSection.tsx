/* ============================================================================
   未付款订单改价（price）—— 批量给未付款订单人工让利，提交到微信。
   忠实保留原 .vue 的 IA / 交互 / 中文文案，视觉换成 Soft（原子组件 + app.css 类）。
   ============================================================================ */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select } from "../primitives";
import { ElMessage } from "../../runtime/feedback";

// 表单录入行（本地状态，不在 ctx）
type OrderPriceRow = {
  shop_id: string;
  wechat_order_id: string;
  product_id: string;
  sku_id: string;
  change_price_yuan: string;
  change_express: boolean;
  express_fee_yuan: string;
  note: string;
};

function makeRow(shopId = ""): OrderPriceRow {
  return {
    shop_id: shopId,
    wechat_order_id: "",
    product_id: "",
    sku_id: "",
    change_price_yuan: "",
    change_express: false,
    express_fee_yuan: "",
    note: "未付款订单人工让利",
  };
}

function yuanToCents(value: string): number {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue)) {
    return NaN;
  }
  return Math.round(numberValue * 100);
}

export default function PriceUpdateSection() {
  const ctx = useApp();
  const job = ctx.currentOrderPriceAdjustmentJob.value;

  // 表单录入行（初始一行）
  const [orderRows, setOrderRows] = useState<OrderPriceRow[]>([makeRow()]);
  // 高级 JSON 协议折叠区（默认折叠）
  const [jsonOpen, setJsonOpen] = useState(false);

  // ---- 行操作 ----
  function patchRow(index: number, patch: Partial<OrderPriceRow>) {
    setOrderRows((rows) => rows.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  }

  function addOrderRow() {
    // 新行继承上一行的店铺，其余清空，备注回到默认
    setOrderRows((rows) => {
      const lastRow = rows[rows.length - 1];
      return [...rows, makeRow(lastRow?.shop_id ?? "")];
    });
  }

  function removeOrderRow(index: number) {
    setOrderRows((rows) => (rows.length === 1 ? rows : rows.filter((_, i) => i !== index)));
  }

  // ---- 详情面板状态统计（遍历 items） ----
  const statusCounts = { total: 0, pending: 0, success: 0, failed: 0 };
  for (const item of job?.items ?? []) {
    statusCounts.total += 1;
    if (item.status === "pending" || item.status === "submitting") {
      statusCounts.pending += 1;
    } else if (item.status === "success") {
      statusCounts.success += 1;
    } else if (item.status === "failed") {
      statusCounts.failed += 1;
    }
  }

  // ---- 表单 → JSON → 创建任务 ----
  async function createOrderPriceAdjustmentFromForm() {
    const grouped = new Map<
      string,
      {
        shop_id: string;
        wechat_order_id: string;
        change_express: boolean;
        express_fee_cents: number | null;
        note: string;
        lines: Array<{ product_id: string; sku_id: string; change_price_cents: number }>;
      }
    >();

    for (const row of orderRows) {
      const shopId = row.shop_id.trim();
      const orderId = row.wechat_order_id.trim();
      const productId = row.product_id.trim();
      const skuId = row.sku_id.trim();
      const changePriceCents = yuanToCents(row.change_price_yuan);
      const expressFeeCents = row.change_express
        ? yuanToCents(row.express_fee_yuan || "0")
        : null;

      if (
        !shopId ||
        !orderId ||
        !productId ||
        !skuId ||
        !Number.isFinite(changePriceCents) ||
        changePriceCents <= 0
      ) {
        ElMessage.error("请填写店铺、待付款订单号、商品 ID、SKU ID 和大于 0 的商品目标总价");
        return;
      }
      if (row.change_express && (!Number.isFinite(expressFeeCents) || Number(expressFeeCents) < 0)) {
        ElMessage.error("运费目标不能小于 0");
        return;
      }

      const key = `${shopId}:${orderId}`;
      const existing = grouped.get(key);
      if (existing) {
        // 同一订单的多个商品行合并为 lines
        existing.lines.push({
          product_id: productId,
          sku_id: skuId,
          change_price_cents: changePriceCents,
        });
        if (row.change_express) {
          existing.change_express = true;
          existing.express_fee_cents = Number(expressFeeCents);
        }
        continue;
      }

      grouped.set(key, {
        shop_id: shopId,
        wechat_order_id: orderId,
        change_express: row.change_express,
        express_fee_cents: row.change_express ? Number(expressFeeCents) : null,
        note: row.note.trim() || "未付款订单人工让利",
        lines: [
          {
            product_id: productId,
            sku_id: skuId,
            change_price_cents: changePriceCents,
          },
        ],
      });
    }

    const orders = Array.from(grouped.values());
    if (orders.length === 0) {
      ElMessage.error("请至少填写一个待付款订单");
      return;
    }

    ctx.orderPriceAdjustmentPayload.value = JSON.stringify(
      {
        request_id: `order-price-${Date.now()}`,
        orders,
      },
      null,
      2,
    );
    await ctx.createOrderPriceAdjustmentJob();
  }

  return (
    <div className="pad">
      <div className="wrap-wide">
        {/* Panel ① 未付款订单改价（构建器） */}
        <div className="panel">
          <div className="panel-title">
            <div>
              <h2>未付款订单改价</h2>
              <p>按订单填写商品 SKU 的最新总价；微信只支持付款前改低价格，商品未填则不改。</p>
            </div>
          </div>

          <div className="price-builder order-price-builder">
            <div className="order-price-toolbar">
              <Button icon="refresh" onClick={() => ctx.runUnpaidOrderSyncOnce()}>
                同步待付款订单
              </Button>
              <Button icon="refresh" onClick={() => ctx.runOrderDetailSyncOnce()}>
                同步订单详情
              </Button>
              <span className="hint">
                先同步待付款订单和详情，再批量提交改价，避免填错商品 SKU。
              </span>
            </div>

            <div className="price-product-list">
              {orderRows.map((row, index) => (
                <div key={index} className="price-product-row order-price-row">
                  <Select
                    value={row.shop_id}
                    onChange={(v: string) => patchRow(index, { shop_id: v })}
                    placeholder="选择店铺"
                    options={ctx.shops.value.map((shop) => ({
                      label: `${shop.name}（${shop.group_name}）`,
                      value: shop.id,
                    }))}
                  />
                  <input
                    className="inp"
                    placeholder="待付款订单号"
                    value={row.wechat_order_id}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      patchRow(index, { wechat_order_id: e.target.value })
                    }
                  />
                  <input
                    className="inp"
                    placeholder="订单商品 product_id"
                    value={row.product_id}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      patchRow(index, { product_id: e.target.value })
                    }
                  />
                  <input
                    className="inp"
                    placeholder="订单 SKU ID"
                    value={row.sku_id}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      patchRow(index, { sku_id: e.target.value })
                    }
                  />
                  <input
                    className="inp"
                    placeholder="商品目标总价，元"
                    value={row.change_price_yuan}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      patchRow(index, { change_price_yuan: e.target.value })
                    }
                  />
                  <label className="cbx">
                    <input
                      type="checkbox"
                      checked={row.change_express}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                        patchRow(index, { change_express: e.target.checked })
                      }
                    />
                    改运费
                  </label>
                  <input
                    className="inp"
                    placeholder="运费目标，元"
                    disabled={!row.change_express}
                    value={row.express_fee_yuan}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      patchRow(index, { express_fee_yuan: e.target.value })
                    }
                  />
                  <input
                    className="inp"
                    placeholder="改价备注"
                    value={row.note}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                      patchRow(index, { note: e.target.value })
                    }
                  />
                  <Button
                    variant="danger"
                    size="sm"
                    disabled={orderRows.length === 1}
                    onClick={() => removeOrderRow(index)}
                  >
                    删除
                  </Button>
                </div>
              ))}
            </div>
          </div>

          <div className="action-row">
            <Button variant="accent" icon="upload" onClick={createOrderPriceAdjustmentFromForm}>
              创建订单改价任务
            </Button>
            <Button onClick={addOrderRow}>添加订单商品</Button>
            <Button icon="upload" onClick={() => ctx.runOrderPriceAdjustmentOnce()}>
              提交微信订单改价
            </Button>
            {ctx.latestOrderPriceAdjustmentTaskId.value && (
              <Pill tone="info">最新任务：{ctx.latestOrderPriceAdjustmentTaskId.value}</Pill>
            )}
          </div>

          {/* 高级 JSON 协议（默认折叠） */}
          <div className="ops-advanced-collapse">
            <button
              className="collapse-head"
              type="button"
              onClick={() => setJsonOpen((v) => !v)}
            >
              <span>高级 JSON 协议</span>
              <span className="hint">{jsonOpen ? "收起" : "展开"}</span>
            </button>
            {jsonOpen && (
              <div className="collapse-body">
                <textarea
                  className="ta mono json-editor"
                  spellCheck={false}
                  rows={10}
                  value={ctx.orderPriceAdjustmentPayload.value}
                  onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => {
                    ctx.orderPriceAdjustmentPayload.value = e.target.value;
                  }}
                />
                <div className="action-row">
                  <Button
                    variant="accent"
                    icon="upload"
                    onClick={() => ctx.createOrderPriceAdjustmentJob()}
                  >
                    按 JSON 创建任务
                  </Button>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Panel ② 查询订单改价任务 */}
        <div className="panel">
          <div className="panel-title">
            <h2>查询订单改价任务</h2>
            <p>可以看到订单是否未付款、商品明细是否匹配，以及微信接口返回的改价失败原因。</p>
          </div>
          <div className="inline-form">
            <input
              className="inp"
              placeholder="order-price-xxx"
              value={ctx.queriedOrderPriceAdjustmentTaskId.value}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.queriedOrderPriceAdjustmentTaskId.value = e.target.value;
              }}
            />
            <Button
              variant="accent"
              icon="search"
              onClick={() => ctx.queryOrderPriceAdjustmentJob()}
            >
              查询
            </Button>
          </div>
        </div>

        {/* Panel ③ 任务详情（仅当存在当前任务时渲染） */}
        {job && (
          <div className="panel">
            <div className="panel-title">
              <h2>{job.id}</h2>
              <Pill tone={ctx.statusType(job.status)}>{job.status}</Pill>
            </div>
            <dl className="status-list compact">
              <div>
                <dt>request_id</dt>
                <dd>{job.request_id}</dd>
              </div>
              <div>
                <dt>订单数</dt>
                <dd>{job.accepted_order_count}</dd>
              </div>
              <div>
                <dt>待提交</dt>
                <dd>{statusCounts.pending}</dd>
              </div>
              <div>
                <dt>成功</dt>
                <dd>{statusCounts.success}</dd>
              </div>
              <div>
                <dt>失败</dt>
                <dd>{statusCounts.failed}</dd>
              </div>
            </dl>
            <div className="tbl-wrap">
              <table className="tbl dense-table">
                <thead>
                  <tr>
                    <th>订单号</th>
                    <th>店铺</th>
                    <th>商品目标总价</th>
                    <th>运费</th>
                    <th>状态</th>
                    <th>处理原因</th>
                    <th>更新时间</th>
                  </tr>
                </thead>
                <tbody>
                  {job.items.map((row) => (
                    <tr key={row.id}>
                      <td>{row.wechat_order_id}</td>
                      <td>{row.shop_name}</td>
                      <td>
                        {row.change_order_infos.map((line) => (
                          <div
                            key={`${line.product_id}-${line.sku_id}`}
                            className="order-price-line"
                          >
                            <span>
                              {line.product_id} / {line.sku_id}
                            </span>
                            <b>{ctx.formatCents(line.change_price_cents)}</b>
                          </div>
                        ))}
                      </td>
                      <td>
                        {row.change_express
                          ? ctx.formatCents(row.express_fee_cents || 0)
                          : "不修改"}
                      </td>
                      <td>
                        <Pill tone={ctx.statusType(row.status)}>{row.status}</Pill>
                      </td>
                      <td title={row.error_summary || row.error_code || "-"}>
                        {row.error_summary || row.error_code || "-"}
                      </td>
                      <td>{ctx.formatDateTime(row.updated_at)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
