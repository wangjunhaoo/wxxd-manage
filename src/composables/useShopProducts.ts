import { ref } from "../runtime/reactive";
import { ElMessage } from "../runtime/feedback";
import type {
  BatchShopProductActionResult,
  CleanupOrphanDraftsResult,
  ShopProductListResult,
  ShopProductSummaryView,
  ShopProductTaskProgress,
  SyncShopProductsResult,
  WechatShopProductDetailView,
  WechatShopProductSkuView,
  WechatShopProductView,
} from "../types/app";

type CommandFn = <T>(name: string, args?: Record<string, unknown>) => Promise<T>;

type TagTone = "primary" | "success" | "info" | "warning" | "danger";

/** 列表每页条数：分页下推数据库，前端只渲染一页，大店（2000+ 商品）不再截断/卡顿。 */
export const SHOP_PRODUCT_PAGE_SIZE = 50;

/** 微信商品 status 枚举 → 中文标签 + Element 标签色调（见 getproduct 文档 status 字段）。 */
export const PRODUCT_STATUS_META: Record<number, { label: string; tone: TagTone }> = {
  // status=0 在 getproductlist 语境即「从未上架的草稿」（getproduct 字段名为「初始值」），用「草稿」更直观。
  0: { label: "草稿", tone: "info" },
  1: { label: "编辑中", tone: "info" },
  2: { label: "审核中", tone: "warning" },
  3: { label: "审核失败", tone: "danger" },
  4: { label: "审核成功", tone: "success" },
  5: { label: "已上架", tone: "success" },
  6: { label: "回收站", tone: "info" },
  7: { label: "上传中", tone: "info" },
  8: { label: "上传失败", tone: "danger" },
  9: { label: "已删除", tone: "info" },
  10: { label: "冻结", tone: "warning" },
  11: { label: "已下架", tone: "info" },
  12: { label: "售罄下架", tone: "warning" },
  13: { label: "违规下架", tone: "danger" },
  14: { label: "保证金不足下架", tone: "danger" },
  15: { label: "品牌过期下架", tone: "danger" },
  20: { label: "已封禁", tone: "danger" },
  21: { label: "SKU 删除", tone: "info" },
  30: { label: "不存在", tone: "info" },
  70: { label: "异步提审中", tone: "warning" },
  71: { label: "质检不通过", tone: "danger" },
};

export function shopProductStatusLabel(status: number): string {
  return PRODUCT_STATUS_META[status]?.label ?? `状态 ${status}`;
}

export function shopProductStatusTone(status: number): TagTone {
  return PRODUCT_STATUS_META[status]?.tone ?? "info";
}

/** 已上架（可下架）。 */
export function isListed(status: number): boolean {
  return status === 5;
}

/** 草稿 / 已下架 / 审核成功未上架等可上架的状态。
 *  草稿（0）可直接 listing 上架转正（清理草稿的转正路径已真机验证），个别失败会以单品错误返回。 */
export function canListing(status: number): boolean {
  return [0, 4, 11, 12, 13, 14, 15].includes(status);
}

/** 审核中的商品不能编辑/删除（微信 10020047/10020049），上下架删除按钮需禁用。 */
export function isAuditing(status: number): boolean {
  return status === 2 || status === 70;
}

export type BatchProductAction = "listing" | "delisting" | "delete";

/**
 * 商品管理页前端状态：以本地缓存为列表数据源（list_cached_shop_products），
 * 状态/关键词筛选与分页全部下推数据库；KPI 与状态计数走 get_shop_product_summary 聚合。
 * 同步 / 批量操作期间轮询 get_shop_product_task_progress 展示进度条。
 */
export function useShopProducts(command: CommandFn) {
  const shopProducts = ref<WechatShopProductView[]>([]);
  const total = ref(0);
  const loading = ref(false);
  const syncing = ref(false);
  const refreshingStock = ref(false);
  const cleaningDrafts = ref(false);
  const batching = ref(false);
  const selectedShopId = ref<string>("");
  const statusFilter = ref<number | "all">("all");
  const keyword = ref("");
  const page = ref(1);
  /** 店铺级概要统计（KPI 卡 + 状态筛选选项），与列表筛选无关、基于全集稳定。 */
  const summary = ref<ShopProductSummaryView | null>(null);
  /** 同步/批量任务进度（轮询所得），null = 当前无任务展示。 */
  const progress = ref<ShopProductTaskProgress | null>(null);
  /** row_id → SKU 列表缓存（展开行懒加载）。 */
  const detailSkus = ref<Record<string, WechatShopProductSkuView[]>>({});
  /** 正在懒加载 SKU 的 row_id 集合（展开行加载态）。 */
  const loadingSkuIds = ref<Record<string, true>>({});
  /** 批量操作选择集：row_id → 商品行。跨页保留，切店清空。 */
  const selected = ref<Record<string, WechatShopProductView>>({});

  // ---- 进度轮询 ----
  let progressTimer: ReturnType<typeof setInterval> | null = null;

  function startProgressPolling() {
    stopProgressPolling();
    const shopId = selectedShopId.value;
    if (!shopId) return;
    progressTimer = setInterval(() => {
      void command<ShopProductTaskProgress | null>("get_shop_product_task_progress", {
        shopId,
      })
        .then((value) => {
          progress.value = value;
        })
        .catch(() => {
          /* 轮询失败静默忽略，下个周期重试 */
        });
    }, 600);
  }

  function stopProgressPolling() {
    if (progressTimer) clearInterval(progressTimer);
    progressTimer = null;
    progress.value = null;
  }

  // ---- 列表 / 概要 ----

  /** 列表刷新后用最新行数据更新选择集中的同 id 条目，避免批量按钮按陈旧状态判断可执行性。 */
  function syncSelectionWith(items: WechatShopProductView[]) {
    if (Object.keys(selected.value).length === 0) return;
    let changed = false;
    const next = { ...selected.value };
    for (const item of items) {
      if (next[item.id]) {
        next[item.id] = item;
        changed = true;
      }
    }
    if (changed) selected.value = next;
  }

  /** 从本地缓存读取当前页商品（店铺/状态/关键词筛选全部下推数据库）。 */
  async function refreshList() {
    if (!selectedShopId.value) {
      shopProducts.value = [];
      total.value = 0;
      return;
    }
    loading.value = true;
    try {
      const result = await command<ShopProductListResult>("list_cached_shop_products", {
        shopId: selectedShopId.value,
        status: statusFilter.value === "all" ? null : statusFilter.value,
        keyword: keyword.value.trim() || null,
        limit: SHOP_PRODUCT_PAGE_SIZE,
        offset: (page.value - 1) * SHOP_PRODUCT_PAGE_SIZE,
      });
      // 筛选变化/删除后页码可能越界（当前页为空但总数非零），回退到最后一页重取一次。
      if (result.items.length === 0 && result.total > 0 && page.value > 1) {
        page.value = Math.max(1, Math.ceil(result.total / SHOP_PRODUCT_PAGE_SIZE));
        const retry = await command<ShopProductListResult>("list_cached_shop_products", {
          shopId: selectedShopId.value,
          status: statusFilter.value === "all" ? null : statusFilter.value,
          keyword: keyword.value.trim() || null,
          limit: SHOP_PRODUCT_PAGE_SIZE,
          offset: (page.value - 1) * SHOP_PRODUCT_PAGE_SIZE,
        });
        shopProducts.value = retry.items;
        total.value = retry.total;
        syncSelectionWith(retry.items);
        return;
      }
      shopProducts.value = result.items;
      total.value = result.total;
      syncSelectionWith(result.items);
    } catch (error) {
      ElMessage.error(`获取商品失败：${error}`);
    } finally {
      loading.value = false;
    }
  }

  /** 拉取店铺级概要统计（KPI + 各状态计数）。 */
  async function refreshSummary() {
    if (!selectedShopId.value) {
      summary.value = null;
      return;
    }
    try {
      summary.value = await command<ShopProductSummaryView>("get_shop_product_summary", {
        shopId: selectedShopId.value,
      });
    } catch (error) {
      ElMessage.error(`获取商品统计失败：${error}`);
    }
  }

  /** 列表 + 概要一起刷新（任何写操作之后都该调这个，保证 KPI/筛选计数同步更新）。 */
  async function refreshAll() {
    await Promise.all([refreshList(), refreshSummary()]);
  }

  // ---- 选择集 ----

  function toggleSelect(row: WechatShopProductView) {
    const next = { ...selected.value };
    if (next[row.id]) delete next[row.id];
    else next[row.id] = row;
    selected.value = next;
  }

  /** 批量勾选/取消当前页的若干行（表头全选用）。 */
  function selectRows(rows: WechatShopProductView[], on: boolean) {
    const next = { ...selected.value };
    for (const row of rows) {
      if (on) next[row.id] = row;
      else delete next[row.id];
    }
    selected.value = next;
  }

  function clearSelection() {
    selected.value = {};
  }

  /** 把当前筛选条件下的全部商品（跨页）加入选择集，返回选中总数。 */
  async function selectAllFiltered(): Promise<number> {
    if (!selectedShopId.value) return 0;
    try {
      const result = await command<ShopProductListResult>("list_cached_shop_products", {
        shopId: selectedShopId.value,
        status: statusFilter.value === "all" ? null : statusFilter.value,
        keyword: keyword.value.trim() || null,
        limit: 10_000,
        offset: 0,
      });
      const next = { ...selected.value };
      for (const row of result.items) next[row.id] = row;
      selected.value = next;
      return Object.keys(next).length;
    } catch (error) {
      ElMessage.error(`全选失败：${error}`);
      return Object.keys(selected.value).length;
    }
  }

  // ---- 同步 / 批量 ----

  /** 调微信接口全量同步当前店铺商品到本地缓存（列表+详情+库存），期间轮询进度。 */
  async function syncProducts(): Promise<SyncShopProductsResult | null> {
    if (!selectedShopId.value) {
      ElMessage.warning("请先选择店铺");
      return null;
    }
    syncing.value = true;
    startProgressPolling();
    try {
      const result = await command<SyncShopProductsResult>("sync_shop_products", {
        shopId: selectedShopId.value,
      });
      if (result.failed_count > 0) {
        ElMessage.warning(
          `同步完成：成功 ${result.synced_count}/${result.total_num}，失败 ${result.failed_count}`,
        );
      } else {
        ElMessage.success(`同步完成，共 ${result.synced_count} 个商品`);
      }
      // 同步改动了多个商品，清空展开行 SKU 缓存以便重新懒加载最新数据。
      detailSkus.value = {};
      await refreshAll();
      return result;
    } catch (error) {
      ElMessage.error(`同步失败：${error}`);
      return null;
    } finally {
      syncing.value = false;
      stopProgressPolling();
    }
  }

  /**
   * 批量上架/下架/删除，期间轮询进度。返回执行结果（含失败明细）供组件层展示；
   * 成功的商品自动移出选择集，失败的保留以便修正后重试。
   */
  async function batchAction(
    action: BatchProductAction,
    rows: WechatShopProductView[],
  ): Promise<BatchShopProductActionResult | null> {
    if (!selectedShopId.value || rows.length === 0) return null;
    batching.value = true;
    startProgressPolling();
    try {
      const result = await command<BatchShopProductActionResult>("batch_shop_product_action", {
        shopId: selectedShopId.value,
        action,
        wechatProductIds: rows.map((row) => row.wechat_product_id),
      });
      const failedIds = new Set(result.failed.map((item) => item.product_id));
      const next = { ...selected.value };
      for (const row of rows) {
        if (!failedIds.has(row.wechat_product_id)) delete next[row.id];
      }
      selected.value = next;
      detailSkus.value = {};
      await refreshAll();
      return result;
    } catch (error) {
      ElMessage.error(`批量操作失败：${error}`);
      return null;
    } finally {
      batching.value = false;
      stopProgressPolling();
    }
  }

  /** 批量刷新当前店铺商品库存（不重拉详情，比全量同步快）。 */
  async function refreshStock() {
    if (!selectedShopId.value) {
      ElMessage.warning("请先选择店铺");
      return;
    }
    refreshingStock.value = true;
    try {
      const updated = await command<number>("refresh_shop_stock", {
        shopId: selectedShopId.value,
      });
      ElMessage.success(`已刷新 ${updated} 个 SKU 的库存`);
      // 库存已批量变动，清空展开行 SKU 缓存以便重新懒加载最新库存。
      detailSkus.value = {};
      await refreshAll();
    } catch (error) {
      ElMessage.error(`刷新库存失败：${error}`);
    } finally {
      refreshingStock.value = false;
    }
  }

  /** 清理孤儿草稿：删已上架重复草稿 + 独有草稿尝试上架转正（危险操作，组件层二次确认后调）。 */
  async function cleanupDrafts(): Promise<boolean> {
    if (!selectedShopId.value) {
      ElMessage.warning("请先选择店铺");
      return false;
    }
    cleaningDrafts.value = true;
    // 清理内部会跑两轮同步，同样有任务进度可看。
    startProgressPolling();
    try {
      const result = await command<CleanupOrphanDraftsResult>("cleanup_orphan_drafts", {
        shopId: selectedShopId.value,
      });
      ElMessage.success(
        `清理完成：删除 ${result.deleted} 个重复/无效草稿，${result.listed_promoted} 个独有草稿上架转正，剩余草稿 ${result.remaining_after}`,
      );
      detailSkus.value = {};
      await refreshAll();
      return true;
    } catch (error) {
      ElMessage.error(`清理草稿失败：${error}`);
      return false;
    } finally {
      cleaningDrafts.value = false;
      stopProgressPolling();
    }
  }

  /** 懒加载某商品的 SKU 列表（展开行用），带加载态。 */
  async function loadDetail(rowId: string): Promise<WechatShopProductSkuView[]> {
    loadingSkuIds.value = { ...loadingSkuIds.value, [rowId]: true };
    try {
      const detail = await command<WechatShopProductDetailView>(
        "get_cached_shop_product_detail",
        { rowId },
      );
      detailSkus.value = { ...detailSkus.value, [rowId]: detail.skus };
      return detail.skus;
    } catch (error) {
      ElMessage.error(`获取 SKU 失败：${error}`);
      return [];
    } finally {
      const next = { ...loadingSkuIds.value };
      delete next[rowId];
      loadingSkuIds.value = next;
    }
  }

  async function listingProduct(product: WechatShopProductView): Promise<boolean> {
    try {
      await command<void>("listing_shop_product", {
        shopId: product.shop_id,
        wechatProductId: product.wechat_product_id,
      });
      ElMessage.success("已上架");
      await refreshAll();
      return true;
    } catch (error) {
      ElMessage.error(`上架失败：${error}`);
      return false;
    }
  }

  async function delistingProduct(product: WechatShopProductView): Promise<boolean> {
    try {
      await command<void>("delisting_shop_product", {
        shopId: product.shop_id,
        wechatProductId: product.wechat_product_id,
      });
      ElMessage.success("已下架");
      await refreshAll();
      return true;
    } catch (error) {
      ElMessage.error(`下架失败：${error}`);
      return false;
    }
  }

  async function deleteProduct(product: WechatShopProductView): Promise<boolean> {
    try {
      await command<void>("delete_shop_product", {
        shopId: product.shop_id,
        wechatProductId: product.wechat_product_id,
      });
      ElMessage.success("已删除");
      // 已删除的行不应留在选择集里。
      const next = { ...selected.value };
      delete next[product.id];
      selected.value = next;
      await refreshAll();
      return true;
    } catch (error) {
      ElMessage.error(`删除失败：${error}`);
      return false;
    }
  }

  /** 调整某 SKU 库存（diffType：1 增 / 2 减 / 3 设置）。 */
  async function updateStock(
    product: WechatShopProductView,
    skuId: string,
    diffType: number,
    num: number,
  ): Promise<boolean> {
    try {
      const newStock = await command<number>("update_shop_product_stock", {
        shopId: product.shop_id,
        wechatProductId: product.wechat_product_id,
        skuId,
        diffType,
        num,
      });
      ElMessage.success(`库存已更新为 ${newStock}`);
      await refreshAll();
      await loadDetail(product.id);
      return true;
    } catch (error) {
      ElMessage.error(`库存更新失败：${error}`);
      return false;
    }
  }

  return {
    shopProducts,
    total,
    loading,
    syncing,
    refreshingStock,
    cleaningDrafts,
    batching,
    selectedShopId,
    statusFilter,
    keyword,
    page,
    summary,
    progress,
    detailSkus,
    loadingSkuIds,
    selected,
    refreshList,
    refreshSummary,
    refreshAll,
    toggleSelect,
    selectRows,
    clearSelection,
    selectAllFiltered,
    syncProducts,
    batchAction,
    refreshStock,
    cleanupDrafts,
    loadDetail,
    listingProduct,
    delistingProduct,
    deleteProduct,
    updateStock,
    shopProductStatusLabel,
    shopProductStatusTone,
    isListed,
    canListing,
    isAuditing,
    PRODUCT_STATUS_META,
  };
}
