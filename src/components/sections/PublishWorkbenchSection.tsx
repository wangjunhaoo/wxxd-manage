/* ============================================================================
   商品工作台（铺货工作台）—— 采集 → AI 审查 → 铺货上架交互式流水线主页面
   导入即自动推进；用户只在「待确认」或「异常」时介入。套用 Soft 视觉。
   ============================================================================ */
import { useEffect, useRef, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { ElMessage } from "../../runtime/feedback";
import { usePipeline } from "../../composables/usePipeline";
import type { PipelineProductView } from "../../types/app";
import { PageHead, Button, Chip, Pill, Field, Empty, Modal, Drawer, Dropdown } from "../primitives";

/** 状态枚举：标签 + Pill 色调（兜底显示原值，色调用 info）。 */
const STATUS_META: Record<
  string,
  { label: string; tone: "info" | "warning" | "primary" | "success" | "danger" }
> = {
  pending_collect: { label: "待采集", tone: "info" },
  collecting: { label: "采集/审查中", tone: "info" },
  collected: { label: "待铺货", tone: "warning" },
  need_confirm: { label: "待确认", tone: "warning" },
  publishing: { label: "铺货中", tone: "primary" },
  listed: { label: "已上架", tone: "success" },
  error: { label: "异常", tone: "danger" },
};

function statusLabel(s: string): string {
  return STATUS_META[s]?.label ?? s;
}
function statusTone(s: string): "info" | "warning" | "primary" | "success" | "danger" {
  return STATUS_META[s]?.tone ?? "info";
}

/** 可补选店铺货的状态：待铺货（已采集审查完成）或仍在采集/审查中（提前补店）。 */
function canAddTargets(status: string): boolean {
  return (
    status === "collected" ||
    status === "pending_collect" ||
    status === "collecting"
  );
}

function progressPercent(row: PipelineProductView): number {
  if (row.total_shops <= 0) return 0;
  return Math.round((row.listed_shops / row.total_shops) * 100);
}

export default function PublishWorkbenchSection() {
  const ctx = useApp();

  // ---- 子 composable：流水线（只实例化一次 + 挂载启停 3s 轮询）----
  const pipeRef = useRef<ReturnType<typeof usePipeline>>();
  if (!pipeRef.current) pipeRef.current = usePipeline(ctx.command);
  const pipe = pipeRef.current;
  useEffect(() => {
    pipe.startPipelinePolling();
    return () => pipe.stopPipelinePolling();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const products = pipe.pipelineProducts.value;

  // ---- 概览过滤 ----
  const [statusFilter, setStatusFilter] = useState<string>("all");

  const stats: Record<string, number> = {
    all: products.length,
    collecting: 0,
    collected: 0,
    need_confirm: 0,
    publishing: 0,
    listed: 0,
    error: 0,
  };
  for (const p of products) {
    if (p.status === "pending_collect" || p.status === "collecting") stats.collecting += 1;
    else if (stats[p.status] !== undefined) stats[p.status] += 1;
  }

  const overviewItems: { key: string; label: string; value: number }[] = [
    { key: "all", label: "全部", value: stats.all },
    { key: "collecting", label: "采集中", value: stats.collecting },
    { key: "collected", label: "待铺货", value: stats.collected },
    { key: "need_confirm", label: "待确认", value: stats.need_confirm },
    { key: "publishing", label: "铺货中", value: stats.publishing },
    { key: "listed", label: "已上架", value: stats.listed },
    { key: "error", label: "异常", value: stats.error },
  ];

  const filteredProducts: PipelineProductView[] =
    statusFilter === "all"
      ? products
      : statusFilter === "collecting"
        ? products.filter(
            (p) => p.status === "pending_collect" || p.status === "collecting",
          )
        : products.filter((p) => p.status === statusFilter);

  // ---- 表格展开行 ----
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const toggleExpand = (id: string) =>
    setExpandedId((prev) => (prev === id ? null : id));

  // ---- 表格多选：仅可补货状态行可勾选，供批量补铺货 ----
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  // 轮询整表替换后：用最新数据重建选中集，剔除已不可补货（陈旧）的勾选，避免误操作。
  useEffect(() => {
    setSelectedIds((prev) => {
      if (prev.size === 0) return prev;
      const next = new Set<string>();
      for (const p of products) {
        if (prev.has(p.id) && canAddTargets(p.status)) next.add(p.id);
      }
      return next;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [products]);
  const selectedRows = products.filter((p) => selectedIds.has(p.id));
  const toggleRow = (id: string, checked: boolean) =>
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });

  // ---- 导入弹窗 ----
  const [importDialogVisible, setImportDialogVisible] = useState(false);
  const [importTargetShopIds, setImportTargetShopIds] = useState<string[]>([]);
  const [importing, setImporting] = useState(false);

  const openImport = () => setImportDialogVisible(true);

  const onImport = async () => {
    if (!ctx.collectionFilePath.value.trim()) {
      ElMessage.warning("请先选择 Excel 文件");
      return;
    }
    // 目标店可选：不选店则只采集不铺货，采集完成后可在列表中补选店铺货
    setImporting(true);
    const ok = await pipe.importExcel(
      ctx.collectionFilePath.value.trim(),
      importTargetShopIds,
    );
    setImporting(false);
    if (ok) {
      setImportDialogVisible(false);
      setImportTargetShopIds([]);
      ctx.clearCollectionExcelFile();
    }
  };

  // ---- 选店铺货弹窗：单个（行内按钮）或批量（顶部按钮）复用 ----
  const [shopPickerVisible, setShopPickerVisible] = useState(false);
  const [shopPickerProductIds, setShopPickerProductIds] = useState<string[]>([]);
  const [shopPickerShopIds, setShopPickerShopIds] = useState<string[]>([]);
  const [shopPickerSubmitting, setShopPickerSubmitting] = useState(false);

  const openShopPickerDrawer = (productIds: string[]) => {
    if (productIds.length === 0) {
      ElMessage.warning("请先选择要铺货的商品");
      return;
    }
    setShopPickerProductIds(productIds);
    setShopPickerShopIds([]);
    setShopPickerVisible(true);
  };

  const onShopPickerConfirm = async () => {
    if (shopPickerShopIds.length === 0) {
      ElMessage.warning("请先选择要铺货到的微信小店");
      return;
    }
    setShopPickerSubmitting(true);
    const ok = await pipe.addPublishTargets(shopPickerProductIds, shopPickerShopIds);
    setShopPickerSubmitting(false);
    if (ok) {
      setShopPickerVisible(false);
      setSelectedIds(new Set());
    }
  };

  // ---- 确认抽屉：need_confirm 商品改标题/选类目后确认进入铺货 ----
  const [confirmDrawerVisible, setConfirmDrawerVisible] = useState(false);
  const [confirmProduct, setConfirmProduct] = useState<PipelineProductView | null>(null);
  const [confirmTitle, setConfirmTitle] = useState("");
  const [confirmCategoryKeyword, setConfirmCategoryKeyword] = useState("");
  const [confirmCategoryKey, setConfirmCategoryKey] = useState("");
  const [confirming, setConfirming] = useState(false);

  const confirmShopId = confirmProduct?.shops[0]?.shop_id ?? "";

  const openConfirmDrawer = (product: PipelineProductView) => {
    setConfirmProduct(product);
    setConfirmTitle(product.title);
    setConfirmCategoryKeyword("");
    setConfirmCategoryKey("");
    pipe.categoryOptions.value = [];
    setConfirmDrawerVisible(true);
  };

  const doSearchCategories = async () => {
    if (!confirmShopId) {
      ElMessage.warning("该商品没有目标店铺");
      return;
    }
    await pipe.searchCategories(confirmShopId, confirmCategoryKeyword);
  };

  const onConfirm = async () => {
    if (!confirmProduct) return;
    const selected = pipe.categoryOptions.value.find(
      (o) => o.category_ids.join("/") === confirmCategoryKey,
    );
    setConfirming(true);
    const ok = await pipe.confirmWithCategory(confirmProduct.id, {
      title: confirmTitle.trim() || undefined,
      categoryIds: selected?.category_ids,
      categoryPath: selected?.category_path,
    });
    setConfirming(false);
    if (ok) setConfirmDrawerVisible(false);
  };

  // ---- 价格策略：全局加价规则。固定加价/最低售价以「元」交互、底层以「分」存储 ----
  const form = ctx.publishPricingForm.value;
  const fixedMarkupYuan = form.sale_price_fixed_cents / 100;
  const floorPriceYuan = form.sale_price_floor_cents / 100;
  // 流水线视图不含 SKU 成本，故以可调的「示例成本价」实时预演策略定价效果
  const [previewCostYuan, setPreviewCostYuan] = useState(10);
  const previewSaleCents = ctx.computeSalePriceCents(previewCostYuan || 0, form);

  const categoryOptions = pipe.categoryOptions.value;

  return (
    <div className="pad">
      <div className="wrap-wide">
        <PageHead
          eyebrow="铺货工作台 · Asia/Shanghai"
          title="商品工作台"
          desc="导入即自动采集 → AI 审查 → 铺货上架；只在「待确认」或「异常」时介入。"
          actions={
            <>
              <Button variant="accent" icon="upload" onClick={openImport}>
                导入铺货表
              </Button>
              {selectedRows.length > 0 && (
                <Button
                  variant="graphite"
                  onClick={() => openShopPickerDrawer(selectedRows.map((r) => r.id))}
                >
                  批量铺货到…（已选 {selectedRows.length}）
                </Button>
              )}
              <Dropdown
                label="淘宝采集"
                items={[
                  {
                    label: "淘宝登录（保持采集登录态）",
                    onClick: () => ctx.triggerTaobaoLogin(),
                  },
                  { label: "检测登录状态", onClick: () => ctx.checkTaobaoLoginState() },
                ]}
              />
              <Button
                title={ctx.publishPricingSummary.value}
                onClick={() => ctx.openPublishPricingDialog()}
              >
                价格策略
              </Button>
              <Button
                icon="refresh"
                disabled={pipe.pipelineLoading.value}
                onClick={() => pipe.refreshPipeline()}
              >
                刷新
              </Button>
            </>
          }
        />

        {/* 概览过滤 chips */}
        <div className="chips" style={{ marginBottom: 16 }}>
          {overviewItems.map((item) => (
            <Chip
              key={item.key}
              label={item.label}
              count={item.value}
              active={statusFilter === item.key}
              onClick={() => setStatusFilter(item.key)}
            />
          ))}
        </div>

        {/* 主表格 */}
        <div className="panel tight">
          {filteredProducts.length === 0 ? (
            <Empty>
              还没有商品。点「导入铺货表」选 Excel 和目标店，导入后自动采集铺货。
            </Empty>
          ) : (
            <div className="tbl-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th style={{ width: 36 }} />
                    <th style={{ width: 36 }} />
                    <th>商品</th>
                    <th style={{ width: 120 }}>状态</th>
                    <th style={{ width: 170 }}>进度</th>
                    <th style={{ width: 150 }}>目标店</th>
                    <th style={{ minWidth: 220 }}>异常 / 原因</th>
                    <th style={{ width: 220 }}>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredProducts.map((row) => (
                    <ProductRow
                      key={row.id}
                      row={row}
                      selectable={canAddTargets(row.status)}
                      checked={selectedIds.has(row.id)}
                      onToggleRow={(c) => toggleRow(row.id, c)}
                      expanded={expandedId === row.id}
                      onToggleExpand={() => toggleExpand(row.id)}
                      onConfirm={() => openConfirmDrawer(row)}
                      onRetry={() => pipe.retryProduct(row.id)}
                      onAddTargets={() => openShopPickerDrawer([row.id])}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      {/* 弹窗 1：铺货价格策略 */}
      <Modal
        open={ctx.publishPricingDialogVisible.value}
        title="铺货价格策略"
        onClose={() => {
          ctx.publishPricingDialogVisible.value = false;
        }}
        footer={
          <>
            <Button
              onClick={() => {
                ctx.publishPricingDialogVisible.value = false;
              }}
            >
              取消
            </Button>
            <Button
              variant="accent"
              disabled={ctx.publishPricingSaving.value}
              onClick={() => ctx.savePublishPricingStrategyFromForm()}
            >
              保存策略
            </Button>
          </>
        }
      >
        <p className="hint" style={{ margin: "0 0 16px", lineHeight: 1.6 }}>
          全局生效：所有商品铺货时由采集成本价按此策略计算上架售价；已显式指定售价的 SKU
          不受影响。
        </p>
        <div className="form-grid cols-3">
          <Field label="加价倍率">
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                className="inp"
                type="number"
                min={0.1}
                max={100}
                step={0.1}
                value={form.sale_price_markup_rate}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  form.sale_price_markup_rate = Number(e.target.value || 0);
                }}
              />
              <span className="text-muted">× 成本价</span>
            </div>
          </Field>
          <Field label="固定加价">
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                className="inp"
                type="number"
                min={0}
                max={100000}
                step={1}
                value={fixedMarkupYuan}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  form.sale_price_fixed_cents = Math.round(
                    Number(e.target.value || 0) * 100,
                  );
                }}
              />
              <span className="text-muted">元</span>
            </div>
          </Field>
          <Field label="最低售价">
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                className="inp"
                type="number"
                min={0.01}
                max={100000}
                step={1}
                value={floorPriceYuan}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  form.sale_price_floor_cents = Math.round(
                    Number(e.target.value || 0) * 100,
                  );
                }}
              />
              <span className="text-muted">元</span>
            </div>
          </Field>
        </div>

        <div className="callout info" style={{ marginTop: 12 }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "baseline",
              marginBottom: 12,
            }}
          >
            <strong>定价试算</strong>
            <span className="hint">{ctx.publishPricingSummary.value}</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <span className="text-muted">示例成本价</span>
            <input
              className="inp"
              type="number"
              min={0}
              max={100000}
              step={1}
              style={{ width: 120 }}
              value={previewCostYuan}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setPreviewCostYuan(Number(e.target.value || 0))
              }
            />
            <span className="text-muted">元</span>
            <span className="text-muted" style={{ marginLeft: 6 }}>
              → 上架售价
            </span>
            <strong style={{ fontSize: 17, color: "var(--accent)" }}>
              ¥{ctx.formatCents(previewSaleCents)}
            </strong>
          </div>
        </div>
      </Modal>

      {/* 弹窗 2：导入铺货表 */}
      <Modal
        open={importDialogVisible}
        title="导入铺货表"
        onClose={() => setImportDialogVisible(false)}
        footer={
          <>
            <Button onClick={() => setImportDialogVisible(false)}>取消</Button>
            <Button variant="accent" disabled={importing} onClick={onImport}>
              开始导入并采集
            </Button>
          </>
        }
      >
        <div className="stack">
          <div className="stack" style={{ gap: 8 }}>
            <strong>1. 选择 Excel 文件</strong>
            <div
              style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}
            >
              <Button
                variant="accent"
                icon="upload"
                onClick={() => ctx.selectCollectionExcelFile()}
              >
                选择文件
              </Button>
              <Button
                disabled={!ctx.collectionFilePath.value}
                onClick={() => ctx.clearCollectionExcelFile()}
              >
                清除
              </Button>
              <span className="text-muted">
                {ctx.collectionFileName.value || "尚未选择文件"}
              </span>
            </div>
            <p className="hint">
              Excel 无表头，三列：商品名 · 淘宝链接 · 微信类目路径（用 &gt; 连接）。
            </p>
          </div>
          <div className="stack" style={{ gap: 8 }}>
            <strong>2. 选择本批目标小店（可选）</strong>
            <ShopMultiSelect
              shops={ctx.shops.value}
              value={importTargetShopIds}
              onChange={setImportTargetShopIds}
              placeholder="不选则仅采集，采集完成后可在列表中补选店铺货"
            />
            <p className="hint">
              不选店则仅采集不铺货；采集审查完成后状态为「待铺货」，可在列表中补选店铺货。
            </p>
          </div>
        </div>
      </Modal>

      {/* 弹窗 3：选店铺货 */}
      <Modal
        open={shopPickerVisible}
        title="选店铺货"
        onClose={() => setShopPickerVisible(false)}
        footer={
          <>
            <Button onClick={() => setShopPickerVisible(false)}>取消</Button>
            <Button
              variant="accent"
              disabled={shopPickerSubmitting}
              onClick={onShopPickerConfirm}
            >
              确认铺货
            </Button>
          </>
        }
      >
        <div className="stack" style={{ gap: 8 }}>
          <strong>为 {shopPickerProductIds.length} 个商品选择目标小店</strong>
          <ShopMultiSelect
            shops={ctx.shops.value}
            value={shopPickerShopIds}
            onChange={setShopPickerShopIds}
            placeholder="选择要铺货到的微信小店"
          />
          <p className="hint">
            已采集完成的商品选店后直接进入铺货；仍在采集/审查中的商品会在审查通过后自动铺货。
          </p>
        </div>
      </Modal>

      {/* 抽屉：确认审查并进入铺货 */}
      <Drawer
        open={confirmDrawerVisible}
        title="确认审查并进入铺货"
        onClose={() => setConfirmDrawerVisible(false)}
      >
        {confirmProduct && (
          <div className="stack">
            <div className="stack" style={{ gap: 6 }}>
              <strong>商品</strong>
              <strong>{confirmProduct.title}</strong>
              {confirmProduct.error_reason && (
                <span className="danger-text">{confirmProduct.error_reason}</span>
              )}
            </div>

            <div className="stack" style={{ gap: 6 }}>
              <strong>标题（可修改）</strong>
              <input
                className="inp"
                placeholder="商品标题"
                value={confirmTitle}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                  setConfirmTitle(e.target.value)
                }
              />
            </div>

            <div className="stack" style={{ gap: 6 }}>
              <strong>微信类目（不选则沿用 AI 已选）</strong>
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  className="inp"
                  placeholder="输入类目关键词，如「连衣裙」"
                  value={confirmCategoryKeyword}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setConfirmCategoryKeyword(e.target.value)
                  }
                  onKeyUp={(e: React.KeyboardEvent<HTMLInputElement>) => {
                    if (e.key === "Enter") void doSearchCategories();
                  }}
                />
                <Button disabled={pipe.categorySearching.value} onClick={doSearchCategories}>
                  搜索
                </Button>
              </div>
              {categoryOptions.length > 0 && (
                <select
                  className="sel"
                  style={{ width: "100%", marginTop: 8 }}
                  value={confirmCategoryKey}
                  onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                    setConfirmCategoryKey(e.target.value)
                  }
                >
                  <option value="" disabled>
                    选择叶子类目
                  </option>
                  {categoryOptions.map((opt) => (
                    <option key={opt.category_ids.join("/")} value={opt.category_ids.join("/")}>
                      {opt.category_path}
                    </option>
                  ))}
                </select>
              )}
            </div>

            <div className="stack" style={{ gap: 6 }}>
              <strong>各店进度</strong>
              {confirmProduct.shops.map((t) => (
                <div key={t.id} style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <strong>{t.shop_name}</strong>
                  <span className="text-muted">{t.status_text}</span>
                </div>
              ))}
            </div>

            <div className="form-actions">
              <Button onClick={() => setConfirmDrawerVisible(false)}>取消</Button>
              <Button variant="accent" disabled={confirming} onClick={onConfirm}>
                确认并铺货
              </Button>
            </div>
          </div>
        )}
      </Drawer>
    </div>
  );
}

/** 一行主行 + 可选展开子行（各店目标状态）。 */
function ProductRow({
  row,
  selectable,
  checked,
  onToggleRow,
  expanded,
  onToggleExpand,
  onConfirm,
  onRetry,
  onAddTargets,
}: {
  row: PipelineProductView;
  selectable: boolean;
  checked: boolean;
  onToggleRow: (checked: boolean) => void;
  expanded: boolean;
  onToggleExpand: () => void;
  onConfirm: () => void;
  onRetry: () => void;
  onAddTargets: () => void;
}) {
  const hasAnyAction = row.can_confirm || row.can_retry || canAddTargets(row.status);
  return (
    <>
      <tr>
        <td>
          <input
            type="checkbox"
            className="cbx"
            checked={checked}
            disabled={!selectable}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
              onToggleRow(e.target.checked)
            }
          />
        </td>
        <td>
          <Button size="sm" variant="ghost" iconOnly onClick={onToggleExpand}>
            <span className="mono">{expanded ? "▾" : "▸"}</span>
          </Button>
        </td>
        <td>
          <div className="cell-main">
            <strong>{row.title}</strong>
            <a
              href={row.source_url}
              target="_blank"
              rel="noreferrer"
              className="link-src mono"
            >
              {row.source_url}
            </a>
            {row.category_path && <span className="subtext">{row.category_path}</span>}
          </div>
        </td>
        <td>
          <Pill tone={statusTone(row.status)}>{statusLabel(row.status)}</Pill>
        </td>
        <td>
          {(row.status === "publishing" || row.status === "listed") && (
            <div className="bar-track" style={{ marginBottom: 4 }}>
              <div
                className={`bar-fill ${row.status === "listed" ? "ok" : ""}`}
                style={{ width: `${progressPercent(row)}%` }}
              />
            </div>
          )}
          <span className="subtext">{row.progress_text || "—"}</span>
        </td>
        <td>
          <span>
            {row.listed_shops}/{row.total_shops} 已上架
          </span>
          {row.failed_shops > 0 && (
            <span className="danger-text"> · {row.failed_shops} 异常</span>
          )}
        </td>
        <td>
          {row.error_reason ? (
            <span className="danger-text">{row.error_reason}</span>
          ) : (
            <span className="text-muted">—</span>
          )}
        </td>
        <td>
          {hasAnyAction ? (
            <div className="row-actions">
              {row.can_confirm && (
                <Button size="sm" variant="accent" onClick={onConfirm}>
                  确认
                </Button>
              )}
              {row.can_retry && (
                <Button size="sm" onClick={onRetry}>
                  重试
                </Button>
              )}
              {canAddTargets(row.status) && (
                <Button size="sm" variant="graphite" onClick={onAddTargets}>
                  选店铺货
                </Button>
              )}
            </div>
          ) : (
            <span className="text-muted">—</span>
          )}
        </td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={8}>
            <div className="subtbl" style={{ padding: "8px 16px" }}>
              {row.shops.map((t) => (
                <div
                  key={t.id}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 10,
                    marginBottom: 8,
                  }}
                >
                  <strong>{t.shop_name}</strong>
                  <Pill tone={t.error_code ? "danger" : "info"}>{t.status_text}</Pill>
                  {t.error_reason && (
                    <span className="text-muted">{t.error_reason}</span>
                  )}
                </div>
              ))}
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

/** 多选微信小店（label = `name (group_name)`）。复刻 el-select multiple collapse-tags 行为。 */
function ShopMultiSelect({
  shops,
  value,
  onChange,
  placeholder,
}: {
  shops: { id: string; name: string; group_name: string }[];
  value: string[];
  onChange: (ids: string[]) => void;
  placeholder: string;
}) {
  const toggle = (id: string, checked: boolean) => {
    if (checked) onChange([...value, id]);
    else onChange(value.filter((v) => v !== id));
  };
  return (
    <div>
      {value.length === 0 && <p className="hint">{placeholder}</p>}
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {shops.map((shop) => (
          <label key={shop.id} style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <input
              type="checkbox"
              className="cbx"
              checked={value.includes(shop.id)}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                toggle(shop.id, e.target.checked)
              }
            />
            <span>
              {shop.name} ({shop.group_name})
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}
