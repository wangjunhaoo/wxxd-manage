import { onUnmounted, ref } from "vue";
import { ElMessage } from "element-plus";
import type {
  CategoryCacheView,
  CategoryCatalogListResult,
  PipelineProductDetailView,
  PipelineProductView,
} from "../types/app";

/** 类目候选选项（ExceptionDrawer 选类目确认用）。 */
export type CategoryOption = {
  category_ids: number[];
  category_path: string;
  shop_id: string;
  shop_name: string;
};

type CommandFn = <T>(name: string, args?: Record<string, unknown>) => Promise<T>;

/**
 * 统一商品流水线前端状态：以 list_pipeline_products 为唯一数据源，3 秒轮询刷新，
 * 提供「人工重试」「确认审查」「导入采集」三个用户动作。driver 在后端自动推进，
 * 前端是观察者 + 异常介入者。
 */
export function usePipeline(command: CommandFn) {
  const pipelineProducts = ref<PipelineProductView[]>([]);
  const pipelineLoading = ref(false);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function refreshPipeline() {
    pipelineLoading.value = true;
    try {
      pipelineProducts.value = await command<PipelineProductView[]>(
        "list_pipeline_products",
      );
    } catch (error) {
      ElMessage.error(`获取流水线商品失败：${error}`);
    } finally {
      pipelineLoading.value = false;
    }
  }

  function startPipelinePolling() {
    if (timer) return;
    void refreshPipeline();
    timer = setInterval(() => void refreshPipeline(), 3000);
  }

  function stopPipelinePolling() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  /** 重新铺货：把铺货失败/卡住的店推回重推（已上架的店不动）。 */
  async function republishProduct(productId: string) {
    try {
      await command<number>("retry_pipeline_product", { productId });
      ElMessage.success("已重新排队铺货，driver 将自动推进");
      await refreshPipeline();
    } catch (error) {
      ElMessage.error(`重新铺货失败：${error}`);
    }
  }

  /** 重新采集：退回采集流程重抓数据重走（保留店、跳过已上架店）。 */
  async function recollectProduct(productId: string) {
    try {
      await command<void>("recollect_pipeline_product", { productId });
      ElMessage.success("已退回重新采集，driver 将自动重采");
      await refreshPipeline();
    } catch (error) {
      ElMessage.error(`重新采集失败：${error}`);
    }
  }

  /** 批量重新铺货：逐个重推铺货失败的店，单个失败不中断整批。 */
  async function republishProducts(productIds: string[]) {
    let ok = 0;
    const failed: string[] = [];
    for (const id of productIds) {
      try {
        await command<number>("retry_pipeline_product", { productId: id });
        ok += 1;
      } catch {
        failed.push(id);
      }
    }
    await refreshPipeline();
    if (failed.length === 0) {
      ElMessage.success(`已对 ${ok} 个商品重新铺货`);
    } else {
      ElMessage.warning(`重新铺货完成 ${ok} 个，${failed.length} 个失败`);
    }
  }

  /** 批量重新采集：逐个退回采集重走，单个失败不中断整批。 */
  async function recollectProducts(productIds: string[]) {
    let ok = 0;
    const failed: string[] = [];
    for (const id of productIds) {
      try {
        await command<void>("recollect_pipeline_product", { productId: id });
        ok += 1;
      } catch {
        failed.push(id);
      }
    }
    await refreshPipeline();
    if (failed.length === 0) {
      ElMessage.success(`已对 ${ok} 个商品重新采集`);
    } else {
      ElMessage.warning(`重新采集完成 ${ok} 个，${failed.length} 个失败`);
    }
  }

  /** 确认审查通过（接受 AI 已选类目/属性），商品进入铺货阶段。 */
  async function confirmReview(productId: string) {
    try {
      await command<void>("confirm_collection_review", {
        request: { task_id: productId, target_shop_ids: [] },
      });
      ElMessage.success("已确认，进入铺货");
      await refreshPipeline();
    } catch (error) {
      ElMessage.error(`确认失败：${error}`);
    }
  }

  const categoryOptions = ref<CategoryOption[]>([]);
  const categorySearching = ref(false);

  function buildCategoryOptions(categories: CategoryCacheView[]): CategoryOption[] {
    const byId = new Map<number, CategoryCacheView>();
    for (const c of categories) byId.set(c.cat_id, c);
    const buildPath = (cat: CategoryCacheView): CategoryCacheView[] => {
      const path: CategoryCacheView[] = [];
      const visited = new Set<number>();
      let cur: CategoryCacheView | undefined = cat;
      while (cur && !visited.has(cur.cat_id)) {
        visited.add(cur.cat_id);
        path.unshift(cur);
        cur = cur.parent_cat_id == null ? undefined : byId.get(cur.parent_cat_id);
      }
      return path;
    };
    const options = new Map<string, CategoryOption>();
    for (const c of categories) {
      if (!c.is_available_for_shop) continue;
      const path = buildPath(c);
      if (path.length < 3) continue;
      const ids = path.map((p) => p.cat_id);
      const key = ids.join("/");
      if (options.has(key)) continue;
      options.set(key, {
        category_ids: ids,
        category_path: path.map((p) => p.name).join(" > "),
        shop_id: c.shop_id,
        shop_name: c.shop_name,
      });
    }
    return Array.from(options.values()).slice(0, 30);
  }

  /** 按店搜索可用叶子类目（≥3 层、店铺有权限），供 ExceptionDrawer 选类目。 */
  async function searchCategories(shopId: string, keyword: string) {
    if (!shopId || !keyword.trim()) {
      ElMessage.warning("请输入类目关键词");
      return;
    }
    categorySearching.value = true;
    try {
      const result = await command<CategoryCatalogListResult>(
        "list_category_catalog",
        { shopId, keyword: keyword.trim(), limit: 300 },
      );
      categoryOptions.value = buildCategoryOptions(result.categories);
      if (categoryOptions.value.length === 0) {
        ElMessage.warning("没有找到可用类目，请先同步店铺类目权限或换关键词");
      }
    } catch (error) {
      ElMessage.error(`类目搜索失败：${error}`);
    } finally {
      categorySearching.value = false;
    }
  }

  /**
   * 确认审查（可附带人工选定的标题/类目），商品进入铺货。
   * 传 targetShopIds 时用于「没店的只采集商品在确认抽屉补选店」：后端据此建 target 即铺货。
   */
  async function confirmWithCategory(
    productId: string,
    opts: {
      title?: string;
      categoryIds?: number[];
      categoryPath?: string;
      targetShopIds?: string[];
    },
  ): Promise<boolean> {
    try {
      await command<void>("confirm_collection_review", {
        request: {
          task_id: productId,
          title: opts.title ?? null,
          category_ids: opts.categoryIds ?? null,
          category_path: opts.categoryPath ?? null,
          target_shop_ids: opts.targetShopIds ?? [],
        },
      });
      ElMessage.success("已确认，进入铺货");
      await refreshPipeline();
      return true;
    } catch (error) {
      ElMessage.error(`确认失败：${error}`);
      return false;
    }
  }

  /** 导入 Excel 并指定本批目标店，商品自动开始采集→审查→铺货。 */
  async function importExcel(
    filePath: string,
    targetShopIds: string[],
  ): Promise<boolean> {
    try {
      const count = await command<number>("import_excel_for_collection", {
        filePath,
        targetShopIds,
      });
      ElMessage.success(`已导入 ${count} 个商品，开始采集`);
      await refreshPipeline();
      return true;
    } catch (error) {
      ElMessage.error(`导入失败：${error}`);
      return false;
    }
  }

  /** 给已采集的商品补选目标店并铺货（「只采集」后再选店，或采集中提前补店）。 */
  async function addPublishTargets(
    productIds: string[],
    targetShopIds: string[],
  ): Promise<boolean> {
    try {
      await command<void>("add_publish_targets", { productIds, targetShopIds });
      ElMessage.success(`已为 ${productIds.length} 个商品加入铺货队列`);
      await refreshPipeline();
      return true;
    } catch (error) {
      ElMessage.error(`铺货失败：${error}`);
      return false;
    }
  }

  /** 拉取单个商品的采集明细（点商品标题打开详情抽屉用，按需加载）。 */
  async function loadProductDetail(
    productId: string,
  ): Promise<PipelineProductDetailView | null> {
    try {
      return await command<PipelineProductDetailView>(
        "get_pipeline_product_detail",
        { productId },
      );
    } catch (error) {
      ElMessage.error(`获取采集明细失败：${error}`);
      return null;
    }
  }

  onUnmounted(() => stopPipelinePolling());

  return {
    pipelineProducts,
    pipelineLoading,
    refreshPipeline,
    startPipelinePolling,
    stopPipelinePolling,
    recollectProduct,
    republishProduct,
    recollectProducts,
    republishProducts,
    confirmReview,
    confirmWithCategory,
    searchCategories,
    categoryOptions,
    categorySearching,
    importExcel,
    addPublishTargets,
    loadProductDetail,
  };
}
