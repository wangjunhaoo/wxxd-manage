/* ============================================================================
   商品管理（products）—— 店铺级微信小店真实商品管理：同步、上下架、删除、库存调整。
   单页表格：店铺/状态/关键词三筛选（筛选+分页下推数据库）+ 5 项 KPI（SQL 聚合）
   + 勾选批量操作（上架/下架/删除，跨页选择）+ 同步/批量实时进度条
   + 主表（展开行懒加载 SKU，带加载态）+ 改库存对话框 + 失败明细弹窗。
   ============================================================================ */
import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { ElMessage, ElMessageBox } from "../../runtime/feedback";
import { Button, Pill, Select, Modal, Segmented, Pagination } from "../primitives";
import {
  useShopProducts,
  shopProductStatusLabel,
  shopProductStatusTone,
  isListed,
  canListing,
  isAuditing,
  SHOP_PRODUCT_PAGE_SIZE,
  type BatchProductAction,
} from "../../composables/useShopProducts";
import type {
  ShopProductFailedItem,
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

interface FailureModalData {
  title: string;
  /** 失败明细（title 为缓存中的商品标题，可能拿不到则只显示 id）。 */
  items: Array<ShopProductFailedItem & { title?: string }>;
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
  const batching = sp.batching.value;
  const selectedShopId = sp.selectedShopId.value;
  const statusFilter = sp.statusFilter.value;
  const page = sp.page.value;
  const summary = sp.summary.value;
  const progress = sp.progress.value;
  const detailSkus = sp.detailSkus.value;
  const loadingSkuIds = sp.loadingSkuIds.value;
  const selected = sp.selected.value;

  const busy = syncing || refreshingStock || cleaningDrafts || batching;

  // ---- 展开行（记录已展开的 row id）----
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  // ---- 关键词输入（300ms 防抖后下推数据库查询）----
  const [keywordInput, setKeywordInput] = useState("");
  useEffect(() => {
    const timer = setTimeout(() => {
      if (sp.keyword.value !== keywordInput) {
        sp.keyword.value = keywordInput;
        sp.page.value = 1;
        void sp.refreshList();
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [keywordInput]);

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

  // ---- 失败明细弹窗（同步失败 / 批量操作失败共用）----
  const [failureModal, setFailureModal] = useState<FailureModalData | null>(null);

  // ---- 派生选项 ----
  const shopOptions = useMemo(
    () => ctx.shops.value.map((shop) => ({ value: shop.id, label: shop.name })),
    [ctx.shops.value],
  );

  /** 状态筛选选项：基于店铺级 SQL 聚合计数，与当前筛选/分页无关、选项稳定。 */
  const statusOptions = useMemo(() => {
    const options: Array<{ value: number | "all"; label: string }> = [
      { value: "all", label: `全部 (${summary?.total ?? 0})` },
    ];
    for (const { status, count } of summary?.status_counts ?? []) {
      options.push({ value: status, label: `${shopProductStatusLabel(status)} (${count})` });
    }
    return options;
  }, [summary]);

  // ---- 选择集派生：各批量操作的可执行行 ----
  const selectedRows = useMemo(() => Object.values(selected), [selected]);
  const listableRows = useMemo(
    () => selectedRows.filter((row) => canListing(row.status)),
    [selectedRows],
  );
  const delistableRows = useMemo(
    () => selectedRows.filter((row) => isListed(row.status)),
    [selectedRows],
  );
  const deletableRows = useMemo(
    () => selectedRows.filter((row) => !isAuditing(row.status)),
    [selectedRows],
  );
  const pageAllSelected =
    shopProducts.length > 0 && shopProducts.every((row) => Boolean(selected[row.id]));

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
    sp.page.value = 1;
    sp.clearSelection();
    setKeywordInput("");
    setExpandedIds(new Set());
    void sp.refreshAll();
  }

  function onStatusChange(value: string) {
    sp.statusFilter.value = value === "all" ? "all" : Number(value);
    sp.page.value = 1;
    void sp.refreshList();
  }

  function onPageChange(next: number) {
    sp.page.value = next;
    void sp.refreshList();
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

  async function onSync() {
    const result = await sp.syncProducts();
    if (result && result.failed_items.length > 0) {
      setFailureModal({
        title: `同步失败明细（${result.failed_items.length} 个商品）`,
        items: result.failed_items,
      });
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

  /** 批量操作统一入口：二次确认 → 执行 → 结果提示 + 失败明细弹窗。 */
  async function runBatch(action: BatchProductAction, rows: WechatShopProductView[]) {
    if (rows.length === 0) return;
    const label =
      action === "listing" ? "批量上架" : action === "delisting" ? "批量下架" : "批量删除";
    const warning =
      action === "delete"
        ? `确定删除选中的 ${rows.length} 个商品？此操作会从微信小店彻底删除，无法恢复。`
        : `确定对选中的 ${rows.length} 个商品执行${label}？`;
    try {
      await ElMessageBox.confirm(warning, label, {
        type: "warning",
        confirmButtonText: "确认执行",
        cancelButtonText: "取消",
      });
    } catch {
      return;
    }
    const titleById = new Map(rows.map((row) => [row.wechat_product_id, row.title]));
    const result = await sp.batchAction(action, rows);
    if (!result) return;
    if (result.failed.length > 0) {
      ElMessage.warning(
        `${label}完成：成功 ${result.succeeded}/${result.total}，失败 ${result.failed.length}`,
      );
      setFailureModal({
        title: `${label}失败明细（${result.failed.length} 个商品）`,
        items: result.failed.map((item) => ({
          ...item,
          title: titleById.get(item.product_id),
        })),
      });
    } else {
      ElMessage.success(`${label}完成：成功 ${result.succeeded} 个`);
    }
  }

  async function onSelectAllFiltered() {
    const count = await sp.selectAllFiltered();
    if (count > 0) ElMessage.success(`已选中筛选结果共 ${count} 个商品`);
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

  // 主表列数（勾选 + 展开 + 主图 + 商品 + 状态 + 最低价 + 总库存 + SKU数 + 同步时间 + 操作）。
  const colSpan = 10;

  const progressPercent =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.done / progress.total) * 100))
      : null;

  return (
    <div className="pad">
      <div className="wrap-wide">
        <div className="panel">
          {/* 头部：标题 + 操作按钮 */}
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
                disabled={!selectedShopId || busy}
                onClick={() => void onSync()}
              >
                {syncing ? "同步中…" : "同步商品"}
              </Button>
              <Button
                size="sm"
                disabled={!selectedShopId || busy}
                onClick={() => sp.refreshStock()}
              >
                {refreshingStock ? "刷新中…" : "刷新库存"}
              </Button>
              <Button
                size="sm"
                disabled={!selectedShopId || busy}
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
                onClick={() => sp.refreshAll()}
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
                value={keywordInput}
                placeholder="搜索标题、微信商品 ID 或外部商品 ID"
                disabled={!selectedShopId}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  setKeywordInput(e.target.value);
                }}
              />
            </div>
          </div>

          {/* KPI 概览（店铺级 SQL 聚合，与筛选无关） */}
          <div className="kpis cols-5" style={{ marginTop: 18 }}>
            <div className="kpi">
              <span>商品总数</span>
              <strong>{summary?.total ?? 0}</strong>
            </div>
            <div className="kpi">
              <span>已上架</span>
              <strong>{summary?.listed ?? 0}</strong>
            </div>
            <div className="kpi">
              <span>已下架</span>
              <strong>{summary?.delisted ?? 0}</strong>
            </div>
            <div className="kpi">
              <span>审核中</span>
              <strong>{summary?.auditing ?? 0}</strong>
            </div>
            <div className="kpi">
              <span>零库存</span>
              <strong>{summary?.zero_stock ?? 0}</strong>
            </div>
          </div>

          {/* 同步 / 批量任务进度条（轮询 get_shop_product_task_progress） */}
          {busy && progress && (
            <div className="progress-wrap" style={{ marginTop: 18, marginBottom: 0 }}>
              <div className="progress-top">
                <strong>{progressPercent !== null ? `${progressPercent}%` : "…"}</strong>
                <span>
                  {progress.message}
                  {progress.total > 0 ? `（${progress.done}/${progress.total}）` : ""}
                </span>
              </div>
              <div className="bar-track">
                <div
                  className="bar-fill"
                  style={{ width: `${progressPercent ?? (progress.finished ? 100 : 6)}%` }}
                />
              </div>
            </div>
          )}

          {/* 批量操作工具栏（有勾选时出现） */}
          {selectedRows.length > 0 && (
            <div
              className="row-actions"
              style={{
                marginTop: 14,
                padding: "10px 14px",
                background: "var(--chip)",
                borderRadius: "var(--r)",
                alignItems: "center",
                flexWrap: "wrap",
              }}
            >
              <span style={{ fontSize: 13, color: "var(--ink-2)" }}>
                已选 <strong>{selectedRows.length}</strong> 个
              </span>
              <Button
                size="sm"
                disabled={busy || listableRows.length === 0}
                onClick={() => void runBatch("listing", listableRows)}
              >
                批量上架 ({listableRows.length})
              </Button>
              <Button
                size="sm"
                disabled={busy || delistableRows.length === 0}
                onClick={() => void runBatch("delisting", delistableRows)}
              >
                批量下架 ({delistableRows.length})
              </Button>
              <Button
                size="sm"
                variant="danger"
                disabled={busy || deletableRows.length === 0}
                onClick={() => void runBatch("delete", deletableRows)}
              >
                批量删除 ({deletableRows.length})
              </Button>
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => void onSelectAllFiltered()}>
                全选筛选结果（共 {total} 个）
              </Button>
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => sp.clearSelection()}>
                清空选择
              </Button>
            </div>
          )}

          {/* 主表 */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th style={{ width: 36 }}>
                    <input
                      type="checkbox"
                      className="cbx"
                      checked={pageAllSelected}
                      disabled={shopProducts.length === 0}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                        sp.selectRows(shopProducts, e.target.checked)
                      }
                      title="全选本页"
                    />
                  </th>
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
                {loading && shopProducts.length === 0 ? (
                  <tr>
                    <td colSpan={colSpan} className="text-muted" style={{ textAlign: "center" }}>
                      加载中…
                    </td>
                  </tr>
                ) : shopProducts.length === 0 ? (
                  <tr>
                    <td colSpan={colSpan} className="text-muted" style={{ textAlign: "center", padding: "28px" }}>
                      {selectedShopId ? "没有匹配的商品，换个状态或关键词试试。" : "请先选择店铺。"}
                    </td>
                  </tr>
                ) : (
                  shopProducts.map((row) => (
                    <ProductRow
                      key={row.id}
                      row={row}
                      checked={Boolean(selected[row.id])}
                      expanded={expandedIds.has(row.id)}
                      skus={detailSkus[row.id]}
                      skuLoading={Boolean(loadingSkuIds[row.id])}
                      colSpan={colSpan}
                      formatCents={ctx.formatCents}
                      formatDateTime={ctx.formatDateTime}
                      onToggleSelect={() => sp.toggleSelect(row)}
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

          {/* 分页器 + 表格下方提示 */}
          {!selectedShopId ? (
            <div className="empty" style={{ marginTop: 8 }}>
              请选择店铺后点击「同步商品」拉取微信小店真实商品。
            </div>
          ) : (
            <>
              <Pagination
                page={page}
                pageSize={SHOP_PRODUCT_PAGE_SIZE}
                total={total}
                onChange={onPageChange}
              />
              <p className="hint" style={{ margin: "12px 2px 4px" }}>
                筛选结果共 {total} 个商品，本页 {shopProducts.length} 个
                {selectedRows.length > 0 ? `，已选 ${selectedRows.length} 个（跨页保留）` : ""}。
              </p>
            </>
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

      {/* 失败明细弹窗（同步失败 / 批量操作失败共用） */}
      <Modal
        open={Boolean(failureModal)}
        title={failureModal?.title ?? ""}
        onClose={() => setFailureModal(null)}
        footer={<Button onClick={() => setFailureModal(null)}>关闭</Button>}
      >
        <div style={{ maxHeight: 380, overflowY: "auto" }}>
          <table className="subtbl">
            <thead>
              <tr>
                <th style={{ minWidth: 220 }}>商品</th>
                <th style={{ minWidth: 240 }}>失败原因</th>
              </tr>
            </thead>
            <tbody>
              {(failureModal?.items ?? []).map((item, index) => (
                <tr key={`${item.product_id}-${index}`}>
                  <td>
                    <div className="cell-main">
                      {item.title ? <strong>{item.title}</strong> : null}
                      <span className="mono">{item.product_id || "（未知商品）"}</span>
                    </div>
                  </td>
                  <td>{item.error}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <p className="hint" style={{ margin: "10px 2px 0" }}>
          失败的商品仍保留在选择集中，可修正后直接重试；同步类失败可再点一次「同步商品」补拉。
        </p>
      </Modal>
    </div>
  );
}

/** 单行商品 + 其展开的 SKU 子表。 */
function ProductRow({
  row,
  checked,
  expanded,
  skus,
  skuLoading,
  colSpan,
  formatCents,
  formatDateTime,
  onToggleSelect,
  onToggle,
  onListing,
  onDelisting,
  onDelete,
  onOpenStock,
}: {
  row: WechatShopProductView;
  checked: boolean;
  expanded: boolean;
  skus: WechatShopProductSkuView[] | undefined;
  skuLoading: boolean;
  colSpan: number;
  formatCents: (value: number | null) => string;
  formatDateTime: (value: string | null | undefined) => string;
  onToggleSelect: () => void;
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
          <input type="checkbox" className="cbx" checked={checked} onChange={onToggleSelect} />
        </td>
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
            {skuLoading && !skus ? (
              <div className="empty" style={{ padding: "20px" }}>
                正在加载 SKU…
              </div>
            ) : skus && skus.length > 0 ? (
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
