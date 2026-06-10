/* ============================================================================
   商品管理（products）—— 店铺级微信小店真实商品管理：同步、上下架、删除、库存调整。
   单页表格：店铺/状态/关键词三筛选 + 5 项 KPI + 主表（展开行懒加载 SKU）+ 改库存对话框。
   忠实保留原 .vue 的 IA / 交互 / 中文文案，视觉换成 Soft。
   ============================================================================ */
import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { ElMessage, ElMessageBox } from "../../runtime/feedback";
import { Button, Pill, Select, Modal, Segmented } from "../primitives";
import {
  useShopProducts,
  shopProductStatusLabel,
  shopProductStatusTone,
  isListed,
  canListing,
  isAuditing,
} from "../../composables/useShopProducts";
import type {
  WechatShopProductSkuView,
  WechatShopProductView,
} from "../../types/app";

interface StockForm {
  product: WechatShopProductView | null;
  skuId: string;
  skuLabel: string;
  diffType: number;
  num: number;
  currentStock: number;
}

/** SKU 标签：有属性（且非 "null"）则拼上属性，否则用 sku_code/sku_id。 */
function skuLabelOf(sku: WechatShopProductSkuView): string {
  if (sku.sku_attrs && sku.sku_attrs !== "null") {
    return `${sku.sku_code ?? sku.sku_id}（${sku.sku_attrs}）`;
  }
  return sku.sku_code ?? sku.sku_id;
}

export default function ProductManagementSection() {
  const ctx = useApp();

  // useShopProducts 是返回一组 ref + 方法的工厂，跨渲染只实例化一次。
  const spRef = useRef<ReturnType<typeof useShopProducts>>();
  if (!spRef.current) spRef.current = useShopProducts(ctx.command);
  const sp = spRef.current;

  // ---- 读取响应式状态 ----
  const shopProducts = sp.shopProducts.value;
  const total = sp.total.value;
  const loading = sp.loading.value;
  const syncing = sp.syncing.value;
  const refreshingStock = sp.refreshingStock.value;
  const cleaningDrafts = sp.cleaningDrafts.value;
  const selectedShopId = sp.selectedShopId.value;
  const statusFilter = sp.statusFilter.value;
  const keyword = sp.keyword.value;
  const detailSkus = sp.detailSkus.value;

  // ---- 展开行（记录已展开的 row id）----
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  // ---- 库存调整对话框 ----
  const [stockDialogVisible, setStockDialogVisible] = useState(false);
  const [stockSubmitting, setStockSubmitting] = useState(false);
  const [stockForm, setStockForm] = useState<StockForm>({
    product: null,
    skuId: "",
    skuLabel: "",
    diffType: 1,
    num: 0,
    currentStock: 0,
  });

  // ---- 派生选项 ----
  const shopOptions = useMemo(
    () => ctx.shops.value.map((shop) => ({ value: shop.id, label: shop.name })),
    [ctx.shops.value],
  );

  /** 状态筛选选项：基于全量商品统计各状态数量，保证切换时选项稳定。 */
  const statusOptions = useMemo(() => {
    const counts = new Map<number, number>();
    for (const product of shopProducts) {
      counts.set(product.status, (counts.get(product.status) ?? 0) + 1);
    }
    const options: Array<{ value: number | "all"; label: string }> = [
      { value: "all", label: `全部 (${shopProducts.length})` },
    ];
    for (const [status, count] of [...counts.entries()].sort((a, b) => a[0] - b[0])) {
      options.push({ value: status, label: `${shopProductStatusLabel(status)} (${count})` });
    }
    return options;
  }, [shopProducts]);

  // ---- KPI 概览 ----
  const summary = useMemo(
    () => ({
      total: shopProducts.length,
      listed: shopProducts.filter((product) => isListed(product.status)).length,
      delisted: shopProducts.filter((product) =>
        [11, 12, 13, 14, 15].includes(product.status),
      ).length,
      auditing: shopProducts.filter((product) => isAuditing(product.status)).length,
      zeroStock: shopProducts.filter((product) => product.total_stock <= 0).length,
    }),
    [shopProducts],
  );

  /** 前端过滤：状态 + 关键词（标题/微信商品ID/外部商品ID）。 */
  const displayedProducts = useMemo(() => {
    let list = shopProducts;
    if (statusFilter !== "all") {
      list = list.filter((product) => product.status === statusFilter);
    }
    const kw = keyword.trim().toLowerCase();
    if (kw) {
      list = list.filter(
        (product) =>
          product.title.toLowerCase().includes(kw) ||
          product.wechat_product_id.toLowerCase().includes(kw) ||
          (product.out_product_id ?? "").toLowerCase().includes(kw),
      );
    }
    return list;
  }, [shopProducts, statusFilter, keyword]);

  /** 调整后库存预览：1 增 / 2 减 / 3 设置。 */
  const stockPreview = useMemo(() => {
    if (stockForm.diffType === 1) return stockForm.currentStock + stockForm.num;
    if (stockForm.diffType === 2) return Math.max(0, stockForm.currentStock - stockForm.num);
    return stockForm.num;
  }, [stockForm]);

  // ---- 交互 ----
  function onShopChange(value: string) {
    sp.selectedShopId.value = value;
    sp.statusFilter.value = "all";
    sp.keyword.value = "";
    setExpandedIds(new Set());
    void sp.refreshList();
  }

  function onStatusChange(value: string) {
    sp.statusFilter.value = value === "all" ? "all" : Number(value);
  }

  /** 展开/收起某行：首次展开且无缓存时懒加载 SKU。 */
  async function toggleExpand(row: WechatShopProductView) {
    const next = new Set(expandedIds);
    if (next.has(row.id)) {
      next.delete(row.id);
      setExpandedIds(next);
    } else {
      next.add(row.id);
      setExpandedIds(next);
      if (!detailSkus[row.id]) {
        await sp.loadDetail(row.id);
      }
    }
  }

  async function onDelete(product: WechatShopProductView) {
    try {
      await ElMessageBox.confirm(
        `确定删除商品「${product.title}」？此操作会从微信小店彻底删除，无法恢复。`,
        "删除商品",
        { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
      );
    } catch {
      return;
    }
    await sp.deleteProduct(product);
  }

  function openStockDialog(product: WechatShopProductView, sku: WechatShopProductSkuView) {
    setStockForm({
      product,
      skuId: sku.sku_id,
      skuLabel: skuLabelOf(sku),
      diffType: 1,
      num: 0,
      currentStock: sku.stock_num ?? 0,
    });
    setStockDialogVisible(true);
  }

  async function submitStock() {
    const form = stockForm;
    if (!form.product) return;
    if (form.num < 0) {
      ElMessage.warning("数量不能为负");
      return;
    }
    setStockSubmitting(true);
    const ok = await sp.updateStock(form.product, form.skuId, form.diffType, form.num);
    setStockSubmitting(false);
    if (ok) setStockDialogVisible(false);
  }

  // 同步/刷新库存后会清空 detailSkus 缓存，已展开的行同步收起，避免展示陈旧 SKU。
  useEffect(() => {
    if (expandedIds.size > 0 && Object.keys(detailSkus).length === 0) {
      setExpandedIds(new Set());
    }
  }, [detailSkus]);

  // 主表列数（展开 + 主图 + 商品 + 状态 + 最低价 + 总库存 + SKU数 + 同步时间 + 操作）。
  const colSpan = 9;

  return (
    <div className="pad">
      <div className="wrap-wide">
        <div className="panel">
          {/* 头部：标题 + 三个操作按钮 */}
          <div className="ph">
            <div>
              <h3>商品管理</h3>
              <p>
                按店铺直连微信小店 API，管理真实在售/在审商品：同步、上下架、删除、库存调整。
              </p>
            </div>
            <div className="row-actions">
              <Button
                variant="accent"
                size="sm"
                disabled={!selectedShopId || syncing}
                onClick={() => sp.syncProducts()}
              >
                {syncing ? "同步中…" : "同步商品"}
              </Button>
              <Button
                size="sm"
                disabled={!selectedShopId || refreshingStock}
                onClick={() => sp.refreshStock()}
              >
                {refreshingStock ? "刷新中…" : "刷新库存"}
              </Button>
              <Button
                size="sm"
                disabled={!selectedShopId || cleaningDrafts}
                onClick={async () => {
                  try {
                    await ElMessageBox.confirm(
                      "将删除草稿箱里「已上架商品的重复草稿」，并把「独有未上架草稿」尝试上架转正、上架失败的删除。批量删除微信草稿不可恢复，确认继续？",
                      "清理孤儿草稿",
                      {
                        type: "warning",
                        confirmButtonText: "确认清理",
                        cancelButtonText: "取消",
                      },
                    );
                  } catch {
                    return;
                  }
                  await sp.cleanupDrafts();
                }}
              >
                {cleaningDrafts ? "清理中…" : "清理草稿"}
              </Button>
              <Button
                size="sm"
                icon="refresh"
                disabled={!selectedShopId}
                onClick={() => sp.refreshList()}
              >
                刷新
              </Button>
            </div>
          </div>

          {/* 筛选区：店铺 / 状态 / 关键词 */}
          <div className="form-grid cols-3">
            <div className="field">
              <Select
                value={selectedShopId}
                onChange={onShopChange}
                options={shopOptions}
                placeholder="选择店铺（必选）"
              />
            </div>
            <div className="field">
              <Select
                value={statusFilter}
                onChange={onStatusChange}
                options={statusOptions}
                placeholder="商品状态"
                disabled={!selectedShopId}
              />
            </div>
            <div className="field">
              <input
                className="inp"
                value={keyword}
                placeholder="搜索标题、微信商品 ID 或外部商品 ID"
                disabled={!selectedShopId}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  sp.keyword.value = e.target.value;
                }}
              />
            </div>
          </div>

          {/* KPI 概览 */}
          <div className="kpis cols-5" style={{ marginTop: 18 }}>
            <div className="kpi">
              <span>商品总数</span>
              <strong>{summary.total}</strong>
            </div>
            <div className="kpi">
              <span>已上架</span>
              <strong>{summary.listed}</strong>
            </div>
            <div className="kpi">
              <span>已下架</span>
              <strong>{summary.delisted}</strong>
            </div>
            <div className="kpi">
              <span>审核中</span>
              <strong>{summary.auditing}</strong>
            </div>
            <div className="kpi">
              <span>零库存</span>
              <strong>{summary.zeroStock}</strong>
            </div>
          </div>

          {/* 主表 */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th style={{ width: 40 }} />
                  <th style={{ width: 70 }}>主图</th>
                  <th style={{ minWidth: 280 }}>商品</th>
                  <th style={{ width: 120 }}>状态</th>
                  <th style={{ width: 110 }}>最低价</th>
                  <th style={{ width: 90 }}>总库存</th>
                  <th style={{ width: 80 }}>SKU 数</th>
                  <th style={{ minWidth: 170 }}>同步时间</th>
                  <th style={{ width: 200 }}>操作</th>
                </tr>
              </thead>
              <tbody>
                {loading && displayedProducts.length === 0 ? (
                  <tr>
                    <td colSpan={colSpan} className="text-muted" style={{ textAlign: "center" }}>
                      加载中…
                    </td>
                  </tr>
                ) : displayedProducts.length === 0 ? (
                  <tr>
                    <td colSpan={colSpan} className="text-muted" style={{ textAlign: "center", padding: "28px" }}>
                      {selectedShopId ? "没有匹配的商品，换个状态或关键词试试。" : "请先选择店铺。"}
                    </td>
                  </tr>
                ) : (
                  displayedProducts.map((row) => (
                    <ProductRow
                      key={row.id}
                      row={row}
                      expanded={expandedIds.has(row.id)}
                      skus={detailSkus[row.id]}
                      colSpan={colSpan}
                      formatCents={ctx.formatCents}
                      formatDateTime={ctx.formatDateTime}
                      onToggle={() => void toggleExpand(row)}
                      onListing={() => sp.listingProduct(row)}
                      onDelisting={() => sp.delistingProduct(row)}
                      onDelete={() => void onDelete(row)}
                      onOpenStock={(sku) => openStockDialog(row, sku)}
                    />
                  ))
                )}
              </tbody>
            </table>
          </div>

          {/* 表格下方提示 */}
          {!selectedShopId ? (
            <div className="empty" style={{ marginTop: 8 }}>
              请选择店铺后点击「同步商品」拉取微信小店真实商品。
            </div>
          ) : (
            <p className="hint" style={{ margin: "12px 2px 4px" }}>
              共 {total} 个商品，当前显示 {displayedProducts.length} 个。
            </p>
          )}
        </div>
      </div>

      {/* 库存调整对话框 */}
      <Modal
        open={stockDialogVisible}
        title="调整库存"
        onClose={() => setStockDialogVisible(false)}
        footer={
          <>
            <Button onClick={() => setStockDialogVisible(false)}>取消</Button>
            <Button variant="accent" disabled={stockSubmitting} onClick={() => void submitStock()}>
              {stockSubmitting ? "提交中…" : "确认"}
            </Button>
          </>
        }
      >
        <div className="kv-grid" style={{ gridTemplateColumns: "1fr 1fr", marginBottom: 16 }}>
          <div className="kv">
            <dt>SKU</dt>
            <dd>{stockForm.skuLabel}</dd>
          </div>
          <div className="kv">
            <dt>当前库存</dt>
            <dd>{stockForm.currentStock}</dd>
          </div>
        </div>
        <div className="field" style={{ marginBottom: 14 }}>
          <label>修改方式</label>
          <Segmented
            value={String(stockForm.diffType)}
            onChange={(v: string) =>
              setStockForm((prev) => ({ ...prev, diffType: Number(v) }))
            }
            options={[
              { label: "增加", value: "1" },
              { label: "减少", value: "2" },
              { label: "设置为", value: "3" },
            ]}
          />
        </div>
        <div className="field" style={{ marginBottom: 14 }}>
          <label>数量</label>
          <input
            className="inp"
            type="number"
            min={0}
            step={1}
            value={stockForm.num}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
              setStockForm((prev) => ({ ...prev, num: Number(e.target.value) }))
            }
            style={{ width: 160 }}
          />
        </div>
        <div className="field">
          <label>调整后</label>
          <div>
            <strong style={{ fontSize: 16 }}>{stockPreview}</strong>
            <span className="hint" style={{ marginLeft: 8 }}>
              （高并发下「设置」可能被覆盖，建议用增减）
            </span>
          </div>
        </div>
      </Modal>
    </div>
  );
}

/** 单行商品 + 其展开的 SKU 子表。 */
function ProductRow({
  row,
  expanded,
  skus,
  colSpan,
  formatCents,
  formatDateTime,
  onToggle,
  onListing,
  onDelisting,
  onDelete,
  onOpenStock,
}: {
  row: WechatShopProductView;
  expanded: boolean;
  skus: WechatShopProductSkuView[] | undefined;
  colSpan: number;
  formatCents: (value: number | null) => string;
  formatDateTime: (value: string | null | undefined) => string;
  onToggle: () => void;
  onListing: () => void;
  onDelisting: () => void;
  onDelete: () => void;
  onOpenStock: (sku: WechatShopProductSkuView) => void;
}) {
  return (
    <>
      <tr>
        <td>
          <Button
            size="sm"
            variant="ghost"
            iconOnly
            icon={expanded ? "chevronDown" : "chevronRight"}
            onClick={onToggle}
            title={expanded ? "收起" : "展开 SKU"}
          />
        </td>
        <td>
          {row.head_img ? (
            <img
              src={row.head_img}
              alt=""
              loading="lazy"
              style={{ width: 44, height: 44, borderRadius: 6, objectFit: "cover" }}
            />
          ) : (
            <span className="text-muted">—</span>
          )}
        </td>
        <td>
          <div className="cell-main">
            <strong>{row.title}</strong>
            <span>微信 ID：{row.wechat_product_id}</span>
            {row.out_product_id ? <span>外部：{row.out_product_id}</span> : null}
          </div>
        </td>
        <td>
          <Pill tone={shopProductStatusTone(row.status)}>
            {shopProductStatusLabel(row.status)}
          </Pill>
        </td>
        <td>{formatCents(row.min_price_cents)}</td>
        <td>{row.total_stock}</td>
        <td>{row.sku_count}</td>
        <td>{formatDateTime(row.synced_at)}</td>
        <td>
          <div className="row-actions">
            {canListing(row.status) && (
              <Button
                size="sm"
                variant="ghost"
                disabled={isAuditing(row.status)}
                onClick={onListing}
              >
                上架
              </Button>
            )}
            {isListed(row.status) && (
              <Button size="sm" variant="ghost" onClick={onDelisting}>
                下架
              </Button>
            )}
            <Button
              size="sm"
              variant="danger"
              disabled={isAuditing(row.status)}
              onClick={onDelete}
            >
              删除
            </Button>
          </div>
        </td>
      </tr>

      {expanded && (
        <tr>
          <td colSpan={colSpan} style={{ background: "var(--chip)" }}>
            {skus && skus.length > 0 ? (
              <table className="subtbl">
                <thead>
                  <tr>
                    <th style={{ minWidth: 200 }}>SKU</th>
                    <th style={{ width: 120 }}>售价</th>
                    <th style={{ width: 100 }}>库存</th>
                    <th style={{ minWidth: 140 }}>外部 SKU</th>
                    <th style={{ width: 120 }}>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {skus.map((sku) => (
                    <tr key={sku.sku_id}>
                      <td>
                        <div className="cell-main">
                          <strong>{sku.sku_code || sku.sku_id}</strong>
                          {sku.sku_attrs && sku.sku_attrs !== "null" ? (
                            <span>{sku.sku_attrs}</span>
                          ) : null}
                        </div>
                      </td>
                      <td>{formatCents(sku.sale_price_cents)}</td>
                      <td>{sku.stock_num ?? "—"}</td>
                      <td className="mono">{sku.out_sku_id}</td>
                      <td>
                        <Button size="sm" variant="ghost" onClick={() => onOpenStock(sku)}>
                          改库存
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div className="empty" style={{ padding: "24px 20px" }}>
                这个商品没有 SKU 记录。
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  );
}
