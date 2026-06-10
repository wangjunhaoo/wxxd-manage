import { ref } from "../runtime/reactive";
import { ElMessage } from "../runtime/feedback";
import type {
  CleanupOrphanDraftsResult,
  ShopProductListResult,
  SyncShopProductsResult,
  WechatShopProductDetailView,
  WechatShopProductSkuView,
  WechatShopProductView,
} from "../types/app";

type CommandFn = <T>(name: string, args?: Record<string, unknown>) => Promise<T>;

type TagTone = "primary" | "success" | "info" | "warning" | "danger";

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

/** 已下架 / 审核成功未上架等可上架的状态。 */
export function canListing(status: number): boolean {
  return [4, 11, 12, 13, 14, 15].includes(status);
}

/** 审核中的商品不能编辑/删除（微信 10020047/10020049），上下架删除按钮需禁用。 */
export function isAuditing(status: number): boolean {
  return status === 2 || status === 70;
}

/**
 * 商品管理页前端状态：以本地缓存为列表数据源（list_cached_shop_products），
 * 同步 / 上下架 / 删除 / 库存调整都走后端命令，操作后刷新列表。
 */
export function useShopProducts(command: CommandFn) {
  const shopProducts = ref<WechatShopProductView[]>([]);
  const total = ref(0);
  const loading = ref(false);
  const syncing = ref(false);
  const refreshingStock = ref(false);
  const cleaningDrafts = ref(false);
  const selectedShopId = ref<string>("");
  const statusFilter = ref<number | "all">("all");
  const keyword = ref("");
  /** row_id → SKU 列表缓存（展开行懒加载）。 */
  const detailSkus = ref<Record<string, WechatShopProductSkuView[]>>({});

  /** 从本地缓存读取商品列表（按当前店铺/状态/关键词筛选）。 */
  async function refreshList() {
    if (!selectedShopId.value) {
      shopProducts.value = [];
      total.value = 0;
      return;
    }
    loading.value = true;
    try {
      // 拉取该店全部商品；状态/关键词由前端过滤，使筛选选项基于全集稳定、切换即时。
      const result = await command<ShopProductListResult>("list_cached_shop_products", {
        shopId: selectedShopId.value,
        status: null,
        keyword: null,
        limit: 2000,
      });
      shopProducts.value = result.items;
      total.value = result.total;
    } catch (error) {
      ElMessage.error(`获取商品失败：${error}`);
    } finally {
      loading.value = false;
    }
  }

  /** 调微信接口全量同步当前店铺商品到本地缓存（列表+详情+库存）。 */
  async function syncProducts() {
    if (!selectedShopId.value) {
      ElMessage.warning("请先选择店铺");
      return;
    }
    syncing.value = true;
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
      await refreshList();
    } catch (error) {
      ElMessage.error(`同步失败：${error}`);
    } finally {
      syncing.value = false;
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
      await refreshList();
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
    try {
      const result = await command<CleanupOrphanDraftsResult>("cleanup_orphan_drafts", {
        shopId: selectedShopId.value,
      });
      ElMessage.success(
        `清理完成：删除 ${result.deleted} 个重复/无效草稿，${result.listed_promoted} 个独有草稿上架转正，剩余草稿 ${result.remaining_after}`,
      );
      detailSkus.value = {};
      await refreshList();
      return true;
    } catch (error) {
      ElMessage.error(`清理草稿失败：${error}`);
      return false;
    } finally {
      cleaningDrafts.value = false;
    }
  }

  /** 懒加载某商品的 SKU 列表（展开行用）。 */
  async function loadDetail(rowId: string): Promise<WechatShopProductSkuView[]> {
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
    }
  }

  async function listingProduct(product: WechatShopProductView): Promise<boolean> {
    try {
      await command<void>("listing_shop_product", {
        shopId: product.shop_id,
        wechatProductId: product.wechat_product_id,
      });
      ElMessage.success("已上架");
      await refreshList();
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
      await refreshList();
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
      await refreshList();
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
      await refreshList();
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
    selectedShopId,
    statusFilter,
    keyword,
    detailSkus,
    refreshList,
    syncProducts,
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
