import { computed, ref, type Ref } from "../../runtime/reactive";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ElMessage, ElMessageBox } from "../../runtime/feedback";
import type {
  CategoryCacheView,
  CategoryCatalogListResult,
  CollectionImageRemoveRequest,
  CollectionImageUploadRequest,
  CollectionPublishRequest,
  CollectionReviewBatchResult,
  CollectionReviewConfirmRequest,
  CollectionTaskView,
  PublishJobCreated,
  PublishPricingStrategy,
  ShopListItem,
} from "../../types/app";
type Command = <T = unknown>(
  name: string,
  args?: Record<string, unknown>,
) => Promise<T>;
type CollectionReviewCategoryOption = {
  category_ids: number[];
  category_path: string;
  score?: number;
  source?: string;
  shop_id?: string;
  shop_name?: string;
};

function isCollectionPreviewJunkImage(url: unknown) {
  const lower = String(url || "")
    .trim()
    .toLowerCase();
  if (!lower) {
    return true;
  }
  return (
    /-\d+-tps-\d+-\d+/.test(lower) ||
    lower.includes("-tps-") ||
    lower.includes("shopmanager") ||
    lower.includes("-0-shopmanager") ||
    lower.includes("userheaderimgshow") ||
    lower.includes("wwc.alicdn.com") ||
    lower.includes("/shophead/")
  );
}

function filterCollectionPreviewImages(images: unknown) {
  if (!Array.isArray(images)) {
    return [];
  }
  const seen = new Set<string>();
  return images.filter((image): image is string => {
    const url = String(image || "").trim();
    if (!url || seen.has(url) || isCollectionPreviewJunkImage(url)) {
      return false;
    }
    seen.add(url);
    return true;
  });
}

interface CollectionWorkflowDeps {
  command: Command;
  isTauriRuntime: boolean;
  latestTaskId: Ref<string>;
  publishPricingStrategy: Ref<PublishPricingStrategy>;
  queriedTaskId: Ref<string>;
  queryJob: () => Promise<void>;
  refreshAll: () => Promise<void>;
  savePublishPricingStrategy: (
    strategy: PublishPricingStrategy,
  ) => Promise<boolean>;
  selectedSection: Ref<string>;
  shops: Ref<ShopListItem[]>;
}
export function useCollectionWorkflow({
  command,
  isTauriRuntime,
  latestTaskId,
  publishPricingStrategy,
  queriedTaskId,
  queryJob,
  refreshAll,
  savePublishPricingStrategy,
  selectedSection,
  shops,
}: CollectionWorkflowDeps) {
  const collectionImportVisible = ref(false);
  const collectionFilePath = ref("");
  const collectionFileName = computed(() => {
    const path = collectionFilePath.value.trim();
    if (!path) return "";
    return path.split(/[\\/]/).filter(Boolean).pop() || path;
  });
  const collectionImporting = ref(false);
  const collectionLoggingIn = ref(false);
  const collectionTasks = ref<CollectionTaskView[]>([]);
  const selectedCollectionTasks = ref<CollectionTaskView[]>([]);
  const collectionTaskPage = ref(1);
  const collectionTaskPageSize = ref(30);
  const collectionTaskPageSizeOptions = [20, 30, 50, 100];
  const collectionPublishTargetShopIds = ref<string[]>([]);
  const collectionPublishing = ref(false);
  const collectionReviewing = ref(false);
  const collectionReviewConfirming = ref(false);
  const selectedReviewCategoryKey = ref("");
  const collectionReviewCategoryKeyword = ref("");
  const collectionReviewCategoryOptions = ref<CollectionReviewCategoryOption[]>(
    [],
  );
  const collectionReviewCategorySearching = ref(false);
  const collectionImageUpdating = ref(false);
  const publishPricingDialogVisible = ref(false);
  const publishPricingForm = ref<PublishPricingStrategy>({
    ...publishPricingStrategy.value,
  });
  let collectionPollTimer: number | null = null;
  const collectionAccessLimitState = ref<any | null>(null);
  const collectionAccessLimitChecking = ref(false);
  const collectionCheckingLogin = ref(false);
  const collectionTesting = ref(false);
  const testCollectVisible = ref(false);
  const testCollectUrl = ref("");
  const testCollectHeaded = ref(true);
  const testCollectResult = ref<any | null>(null);
  const collectionDetailVisible = ref(false);
  const selectedCollectionDetailTask = ref<CollectionTaskView | null>(null);
  const selectedCollectionDetailProduct = computed(() => {
    const task = selectedCollectionDetailTask.value;
    return task ? parseCollectionProduct(task, true) : null;
  });
  const selectedCollectionOriginalProduct = computed(() => {
    const task = selectedCollectionDetailTask.value;
    return task ? parseCollectionProduct(task, false) : null;
  });
  const selectedCollectionReviewResult = computed(() => {
    const task = selectedCollectionDetailTask.value;
    return task ? parseCollectionReviewResult(task) : null;
  });
  const selectedCollectionReviewIssues = computed(() => {
    const issues = selectedCollectionReviewResult.value?.issues;
    return Array.isArray(issues) ? issues : [];
  });
  const selectedCollectionCategoryCandidates = computed(() => {
    const candidates =
      selectedCollectionReviewResult.value?.category?.candidates;
    return Array.isArray(candidates) ? candidates : [];
  });
  const selectedCollectionReviewCategoryOptions = computed(() => {
    const options = new Map<string, CollectionReviewCategoryOption>();
    for (const candidate of selectedCollectionCategoryCandidates.value) {
      const ids = normalizeCategoryIds(candidate?.category_ids);
      const categoryPath = String(candidate?.category_path || "").trim();
      if (ids.length < 3 || !categoryPath) {
        continue;
      }
      options.set(categoryOptionKey(ids), {
        category_ids: ids,
        category_path: categoryPath,
        score: Number(candidate?.score) || undefined,
        source: String(candidate?.source || "review_candidate"),
      });
    }
    for (const option of collectionReviewCategoryOptions.value) {
      options.set(categoryOptionKey(option.category_ids), option);
    }
    return Array.from(options.values());
  });
  const selectedCollectionDetailJson = computed(() => {
    if (selectedCollectionDetailProduct.value) {
      return JSON.stringify(selectedCollectionDetailProduct.value, null, 2);
    }
    return selectedCollectionDetailTask.value?.collected_data || "";
  });
  const selectedCollectionMainImages = computed(() => {
    const images = selectedCollectionDetailProduct.value?.images;
    return filterCollectionPreviewImages(images);
  });
  const selectedCollectionDetailImages = computed(() => {
    const images = selectedCollectionDetailProduct.value?.detail_images;
    return filterCollectionPreviewImages(images);
  });
  const selectedCollectionSkuPreview = computed(() => {
    const skus = selectedCollectionDetailProduct.value?.skus;
    return Array.isArray(skus) ? skus.slice(0, 8) : [];
  });
  const selectedCollectionRemovedImages = computed(() => {
    const removed = selectedCollectionReviewResult.value?.images?.removed;
    return Array.isArray(removed) ? removed : [];
  });
  const publishPricingSummary = computed(() => {
    const strategy = publishPricingStrategy.value;
    return `成本×${formatRate(strategy.sale_price_markup_rate)} + ${centsToYuan(strategy.sale_price_fixed_cents)}元，最低${centsToYuan(strategy.sale_price_floor_cents)}元`;
  });
  async function refreshCollectionTasks() {
    try {
      collectionTasks.value = await command<CollectionTaskView[]>(
        "get_collection_tasks",
      );
      const latestById = new Map(
        collectionTasks.value.map((task) => [task.id, task]),
      );
      selectedCollectionTasks.value = selectedCollectionTasks.value
        .map((task) => latestById.get(task.id))
        .filter((task): task is CollectionTaskView =>
          Boolean(task && canSelectCollectionTask(task)),
        );
      if (selectedCollectionDetailTask.value) {
        selectedCollectionDetailTask.value =
          latestById.get(selectedCollectionDetailTask.value.id) ||
          selectedCollectionDetailTask.value;
      }
      normalizeCollectionTaskPage();
    } catch (err: any) {
      console.error("加载采集任务失败：", err);
    }
  }
  const collectionTaskTotal = computed(() => collectionTasks.value.length);
  const paginatedCollectionTasks = computed(() => {
    const start = (collectionTaskPage.value - 1) * collectionTaskPageSize.value;
    return collectionTasks.value.slice(
      start,
      start + collectionTaskPageSize.value,
    );
  });
  const selectableCurrentCollectionTasks = computed(() =>
    paginatedCollectionTasks.value.filter(canSelectCollectionTask),
  );
  const selectedCollectionTaskIds = computed(
    () => new Set(selectedCollectionTasks.value.map((task) => task.id)),
  );
  const isCurrentCollectionPageAllSelected = computed(() => {
    const rows = selectableCurrentCollectionTasks.value;
    return (
      rows.length > 0 &&
      rows.every((task) => selectedCollectionTaskIds.value.has(task.id))
    );
  });
  const isCurrentCollectionPageIndeterminate = computed(() => {
    const rows = selectableCurrentCollectionTasks.value;
    if (rows.length === 0) return false;
    const selectedCount = rows.filter((task) =>
      selectedCollectionTaskIds.value.has(task.id),
    ).length;
    return selectedCount > 0 && selectedCount < rows.length;
  });
  const selectedCollectionPublishable = computed(() => {
    return (
      selectedCollectionTasks.value.length > 0 &&
      selectedCollectionTasks.value.every(
        (task) => task.review_status === "passed",
      )
    );
  });
  const collectionTargetShopSelectionRequired = computed(() => {
    return (
      collectionPublishTargetShopIds.value.length === 0 &&
      shops.value.length > 1
    );
  });
  function normalizeCollectionTaskPage() {
    const maxPage = Math.max(
      1,
      Math.ceil(collectionTaskTotal.value / collectionTaskPageSize.value),
    );
    if (collectionTaskPage.value > maxPage) {
      collectionTaskPage.value = maxPage;
    }
    if (collectionTaskPage.value < 1) {
      collectionTaskPage.value = 1;
    }
  }
  function canSelectCollectionTask(row: CollectionTaskView) {
    return row.status === "success" && Boolean(row.collected_data?.trim());
  }
  function isCollectionTaskSelected(row: CollectionTaskView) {
    return selectedCollectionTaskIds.value.has(row.id);
  }
  function setCollectionTaskSelected(
    row: CollectionTaskView,
    selected: boolean,
  ) {
    if (!canSelectCollectionTask(row)) return;
    if (selected) {
      if (!selectedCollectionTaskIds.value.has(row.id)) {
        selectedCollectionTasks.value = [...selectedCollectionTasks.value, row];
      }
      return;
    }
    selectedCollectionTasks.value = selectedCollectionTasks.value.filter(
      (task) => task.id !== row.id,
    );
  }
  function toggleCurrentCollectionPageSelection(selected: boolean) {
    const pageRows = selectableCurrentCollectionTasks.value;
    if (selected) {
      const selectedIds = selectedCollectionTaskIds.value;
      const additions = pageRows.filter((task) => !selectedIds.has(task.id));
      selectedCollectionTasks.value = [
        ...selectedCollectionTasks.value,
        ...additions,
      ];
      return;
    }
    const pageIds = new Set(pageRows.map((task) => task.id));
    selectedCollectionTasks.value = selectedCollectionTasks.value.filter(
      (task) => !pageIds.has(task.id),
    );
  }
  function handleCollectionTaskPageChange(page: number) {
    collectionTaskPage.value = page;
    normalizeCollectionTaskPage();
  }
  function handleCollectionTaskPageSizeChange(size: number) {
    collectionTaskPageSize.value = size;
    collectionTaskPage.value = 1;
    normalizeCollectionTaskPage();
  }
  function collectionTaskStatusLabel(status: CollectionTaskView["status"]) {
    const labels: Record<CollectionTaskView["status"], string> = {
      pending: "等待采集",
      running: "正在采集",
      success: "采集成功",
      failed: "采集失败",
    };
    return labels[status] || status;
  }
  function collectionReviewStatusLabel(
    status: CollectionTaskView["review_status"],
  ) {
    const labels: Record<CollectionTaskView["review_status"], string> = {
      pending: "待审查",
      passed: "审查通过",
      needs_review: "需确认",
      blocked: "已拦截",
      failed: "审查失败",
    };
    return labels[status] || status;
  }
  function collectionReviewStatusType(
    status: CollectionTaskView["review_status"],
  ) {
    const types: Record<
      CollectionTaskView["review_status"],
      "success" | "warning" | "danger" | "info"
    > = {
      pending: "info",
      passed: "success",
      needs_review: "warning",
      blocked: "danger",
      failed: "danger",
    };
    return types[status] || "info";
  }
  function parseCollectionProduct(
    task: CollectionTaskView,
    preferReviewed = false,
  ): any | null {
    const raw = preferReviewed
      ? task.reviewed_data || task.collected_data
      : task.collected_data;
    if (!raw) {
      return null;
    }
    try {
      return JSON.parse(raw);
    } catch {
      return null;
    }
  }
  function parseCollectionReviewResult(task: CollectionTaskView): any | null {
    if (!task.review_result_json) {
      return null;
    }
    try {
      return JSON.parse(task.review_result_json);
    } catch {
      return null;
    }
  }
  function collectionProductSummary(task: CollectionTaskView) {
    const product = parseCollectionProduct(task);
    if (!product) {
      return "未生成商品数据";
    }
    const imageCount = filterCollectionPreviewImages(product.images).length;
    const detailImageCount = filterCollectionPreviewImages(
      product.detail_images,
    ).length;
    const skuCount = Array.isArray(product.skus) ? product.skus.length : 0;
    return `主图 ${imageCount} / 详情图 ${detailImageCount} / SKU ${skuCount}`;
  }
  function openCollectionDetail(task: CollectionTaskView) {
    selectedCollectionDetailTask.value = task;
    collectionReviewCategoryOptions.value = [];
    collectionReviewCategoryKeyword.value =
      collectionCategorySearchKeywordFromHint(task);
    const candidates = parseCollectionReviewResult(task)?.category?.candidates;
    if (Array.isArray(candidates) && candidates.length > 0) {
      const first = candidates[0];
      selectedReviewCategoryKey.value = Array.isArray(first?.category_ids)
        ? first.category_ids.join("/")
        : "";
    } else {
      selectedReviewCategoryKey.value = "";
      if (collectionReviewCategoryKeyword.value) {
        void searchCollectionReviewCategories(
          collectionReviewCategoryKeyword.value,
        );
      }
    }
    collectionDetailVisible.value = true;
  }
  function collectionCategorySearchKeywordFromHint(task: CollectionTaskView) {
    const product =
      parseCollectionProduct(task, true) || parseCollectionProduct(task, false);
    const hint = String(
      product?.category_hint || task.category_path || "",
    ).trim();
    if (!hint) {
      return "";
    }
    const parts = hint
      .split(/[>\/／|｜]/)
      .map((part) => part.trim())
      .filter(Boolean);
    return parts[parts.length - 1] || hint;
  }
  function normalizeCategoryIds(value: unknown) {
    return Array.isArray(value)
      ? value
          .map((item) => Number(item))
          .filter((item) => Number.isFinite(item))
      : [];
  }
  function categoryOptionKey(categoryIds: number[]) {
    return categoryIds.join("/");
  }
  function resolveCollectionReviewCategoryShopId(task: CollectionTaskView) {
    return (
      collectionPublishTargetShopIds.value[0] ||
      task.target_shop_ids?.[0] ||
      (shops.value.length === 1 ? shops.value[0].id : "")
    );
  }
  function buildCollectionReviewCategoryOptions(
    categories: CategoryCacheView[],
  ) {
    const byShop = new Map<string, Map<number, CategoryCacheView>>();
    for (const category of categories) {
      if (!byShop.has(category.shop_id)) {
        byShop.set(category.shop_id, new Map());
      }
      byShop.get(category.shop_id)?.set(category.cat_id, category);
    }
    const options = new Map<string, CollectionReviewCategoryOption>();
    for (const category of categories) {
      if (!category.is_available_for_shop) {
        continue;
      }
      const shopCategories = byShop.get(category.shop_id);
      if (!shopCategories) {
        continue;
      }
      const path = buildCategoryPath(category, shopCategories);
      if (path.length < 3) {
        continue;
      }
      const ids = path.map((item) => item.cat_id);
      const key = categoryOptionKey(ids);
      if (options.has(key)) {
        continue;
      }
      options.set(key, {
        category_ids: ids,
        category_path: path.map((item) => item.name).join(" > "),
        source: "local_category_cache",
        shop_id: category.shop_id,
        shop_name: category.shop_name,
      });
    }
    return Array.from(options.values()).slice(0, 30);
  }
  function buildCategoryPath(
    category: CategoryCacheView,
    categoriesById: Map<number, CategoryCacheView>,
  ) {
    const path: CategoryCacheView[] = [];
    const visited = new Set<number>();
    let current: CategoryCacheView | undefined = category;
    while (current && !visited.has(current.cat_id)) {
      visited.add(current.cat_id);
      path.unshift(current);
      current =
        current.parent_cat_id == null
          ? undefined
          : categoriesById.get(current.parent_cat_id);
    }
    return path;
  }
  function shopNamesByIds(shopIds: string[]) {
    if (shopIds.length === 0) {
      return "-";
    }
    return shopIds
      .map(
        (shopId) =>
          shops.value.find((shop) => shop.id === shopId)?.name || shopId,
      )
      .join("、");
  }
  function collectionPublishedShopNames(task: CollectionTaskView) {
    return shopNamesByIds(task.published_shop_ids || []);
  }
  function collectionPublishJobText(task: CollectionTaskView) {
    if (!task.publish_job_ids || task.publish_job_ids.length === 0) {
      return "-";
    }
    return task.publish_job_ids.join("、");
  }
  function centsToYuan(cents: number) {
    return (cents / 100).toFixed(cents % 100 === 0 ? 0 : 2);
  }
  function formatRate(rate: number) {
    return Number.isInteger(rate)
      ? String(rate)
      : rate.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
  }
  function computeSalePriceCents(
    costPriceYuan: number,
    strategy: PublishPricingStrategy,
  ) {
    const computed =
      Math.ceil(costPriceYuan * 100 * strategy.sale_price_markup_rate) +
      strategy.sale_price_fixed_cents;
    return Math.max(strategy.sale_price_floor_cents, computed);
  }
  function openPublishPricingDialog() {
    publishPricingForm.value = { ...publishPricingStrategy.value };
    publishPricingDialogVisible.value = true;
  }
  async function savePublishPricingStrategyFromForm() {
    const saved = await savePublishPricingStrategy({
      ...publishPricingForm.value,
    });
    if (saved) {
      publishPricingDialogVisible.value = false;
    }
  }
  async function createPublishJobFromSelectedCollections() {
    if (selectedCollectionTasks.value.length === 0) {
      ElMessage.warning("请先选择要铺货的商品");
      return;
    }
    if (collectionPublishTargetShopIds.value.length === 0) {
      ElMessage.warning("请选择至少一个目标微信小店");
      return;
    }
    const notCollected = selectedCollectionTasks.value.filter(
      (task) => task.status !== "success",
    );
    if (notCollected.length > 0) {
      ElMessage.warning(
        `有 ${notCollected.length} 个商品还没采集成功，暂不能铺货`,
      );
      return;
    }
    collectionPublishing.value = true;
    try {
      const reviewTaskIds = selectedCollectionTasks.value
        .filter((task) => task.review_status !== "passed")
        .map((task) => task.id);
      if (reviewTaskIds.length > 0) {
        collectionReviewing.value = true;
        const reviewResult = await command<CollectionReviewBatchResult>(
          "run_collection_review_once",
          {
            request: {
              task_ids: reviewTaskIds,
              target_shop_ids: collectionPublishTargetShopIds.value,
              limit: reviewTaskIds.length,
            },
          },
        );
        await refreshCollectionTasks();
        collectionReviewing.value = false;
        if (
          reviewResult.needs_review_items > 0 ||
          reviewResult.blocked_items > 0 ||
          reviewResult.failed_items > 0
        ) {
          ElMessage.warning(
            `自动审查后仍有 ${reviewResult.needs_review_items + reviewResult.blocked_items + reviewResult.failed_items} 个商品需要处理，请看异常原因`,
          );
        }
      }
      if (!selectedCollectionPublishable.value) {
        ElMessage.warning("选中的商品未全部通过自动审查，已停止创建铺货任务");
        return;
      }
      const request: CollectionPublishRequest = {
        collection_task_ids: selectedCollectionTasks.value.map(
          (task) => task.id,
        ),
        target_shop_ids: collectionPublishTargetShopIds.value,
        pricing_strategy: { ...publishPricingStrategy.value },
      };
      const result = await command<PublishJobCreated>(
        "create_publish_job_from_collection_tasks",
        { request },
      );
      latestTaskId.value = result.task_id;
      queriedTaskId.value = result.task_id;
      selectedSection.value = "publish-tasks";
      ElMessage.success(`铺货任务已创建：${result.task_id}`);
      selectedCollectionTasks.value = [];
      await Promise.all([refreshCollectionTasks(), refreshAll(), queryJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      collectionReviewing.value = false;
      collectionPublishing.value = false;
    }
  }
  async function reviewSelectedCollectionTasks() {
    const taskIds =
      selectedCollectionTasks.value.length > 0
        ? selectedCollectionTasks.value.map((task) => task.id)
        : paginatedCollectionTasks.value
            .filter(
              (task) =>
                task.status === "success" && task.review_status !== "passed",
            )
            .map((task) => task.id);
    await reviewCollectionTaskIds(taskIds);
  }
  async function reviewSingleCollectionTask(task: CollectionTaskView) {
    await reviewCollectionTaskIds([task.id]);
  }
  async function reviewCollectionTaskIds(taskIds: string[]) {
    if (taskIds.length === 0) {
      ElMessage.info("当前没有需要审查的采集结果");
      return;
    }
    if (collectionTargetShopSelectionRequired.value) {
      ElMessage.warning(
        "请先选择目标微信小店再审查；不同店铺的微信类目权限不同。",
      );
      return;
    }
    collectionReviewing.value = true;
    try {
      const result = await command<CollectionReviewBatchResult>(
        "run_collection_review_once",
        {
          request: {
            task_ids: taskIds,
            target_shop_ids: collectionPublishTargetShopIds.value,
            limit: taskIds.length,
          },
        },
      );
      ElMessage.success(
        `审查完成：通过 ${result.passed_items}，需确认 ${result.needs_review_items}，拦截 ${result.blocked_items}`,
      );
      await refreshCollectionTasks();
    } catch (error) {
      ElMessage.error(`审查失败：${error}`);
    } finally {
      collectionReviewing.value = false;
    }
  }
  async function confirmSelectedCollectionReview() {
    const task = selectedCollectionDetailTask.value;
    if (!task) {
      return;
    }
    const selectedCandidate =
      selectedCollectionReviewCategoryOptions.value.find((candidate) => {
        return (
          categoryOptionKey(candidate.category_ids) ===
          selectedReviewCategoryKey.value
        );
      });
    const request: CollectionReviewConfirmRequest = {
      task_id: task.id,
      title: selectedCollectionDetailProduct.value?.title || task.title,
      target_shop_ids: collectionPublishTargetShopIds.value,
    };
    if (selectedCandidate) {
      request.category_ids = selectedCandidate.category_ids;
      request.category_path = selectedCandidate.category_path;
    }
    collectionReviewConfirming.value = true;
    try {
      // 后端 confirm_collection_review 现返回 ()，确认后直接刷新列表（不再回填详情任务）
      await command<void>("confirm_collection_review", { request });
      ElMessage.success("已确认审查通过");
      collectionDetailVisible.value = false;
      await refreshCollectionTasks();
    } catch (error) {
      ElMessage.error(`确认失败：${error}`);
    } finally {
      collectionReviewConfirming.value = false;
    }
  }
  async function resetAllPassedReviews() {
    try {
      await ElMessageBox.confirm(
        "将把所有审查通过的任务重置为待审查状态，审查结果会被清空。确定继续？",
        "批量重置审查",
        {
          confirmButtonText: "确认重置",
          cancelButtonText: "取消",
          type: "warning",
        },
      );
    } catch {
      return;
    }
    let resetCount = 0;
    for (const task of collectionTasks.value) {
      if (task.review_status === "passed" && task.status === "success") {
        try {
          await command("reset_collection_review", { taskId: task.id });
          resetCount++;
        } catch (err: any) {
          ElMessage.warning(`${task.title} 重置失败：${err}`);
        }
      }
    }
    if (resetCount > 0) {
      ElMessage.success(`已重置 ${resetCount} 个审查，请重新执行审查`);
      await refreshCollectionTasks();
    } else {
      ElMessage.info("没有需要重置的审查");
    }
  }
  async function resetSelectedCollectionReview(task: CollectionTaskView) {
    try {
      const updated = await command<CollectionTaskView>(
        "reset_collection_review",
        { taskId: task.id },
      );
      if (selectedCollectionDetailTask.value?.id === task.id) {
        selectedCollectionDetailTask.value = updated;
      }
      ElMessage.success("已重置审查状态");
      await refreshCollectionTasks();
    } catch (error) {
      ElMessage.error(`重置失败：${error}`);
    }
  }
  function collectionImageSrc(image: string) {
    const value = String(image || "").trim();
    if (!value || /^(https?:|data:|blob:|asset:)/i.test(value)) {
      return value;
    }
    return isTauriRuntime ? convertFileSrc(value) : value;
  }
  function replaceCollectionTask(updated: CollectionTaskView) {
    collectionTasks.value = collectionTasks.value.map((task) =>
      task.id === updated.id ? updated : task,
    );
    selectedCollectionTasks.value = selectedCollectionTasks.value.map((task) =>
      task.id === updated.id ? updated : task,
    );
    if (selectedCollectionDetailTask.value?.id === updated.id) {
      selectedCollectionDetailTask.value = updated;
    }
  }
  async function uploadCollectionImage(kind: "main" | "detail") {
    const task = selectedCollectionDetailTask.value;
    if (!task) {
      return;
    }
    if (!isTauriRuntime) {
      ElMessage.info("浏览器预览模式下不能上传本地图片，请在桌面端使用");
      return;
    }
    const selected = await openDialog({
      multiple: false,
      filters: [
        {
          name: "图片文件",
          extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"],
        },
      ],
    });
    const filePath = Array.isArray(selected) ? selected[0] : selected;
    if (!filePath) {
      return;
    }
    collectionImageUpdating.value = true;
    try {
      const request: CollectionImageUploadRequest = {
        task_id: task.id,
        kind,
        file_path: filePath,
      };
      const updated = await command<CollectionTaskView>(
        "import_collection_task_image",
        { request },
      );
      replaceCollectionTask(updated);
      ElMessage.success("图片已上传，采集审查已重置");
    } catch (error) {
      ElMessage.error(`上传图片失败：${error}`);
    } finally {
      collectionImageUpdating.value = false;
    }
  }
  async function removeCollectionImage(
    kind: "main" | "detail",
    imageUrl: string,
  ) {
    const task = selectedCollectionDetailTask.value;
    if (!task || !imageUrl) {
      return;
    }
    collectionImageUpdating.value = true;
    try {
      const request: CollectionImageRemoveRequest = {
        task_id: task.id,
        kind,
        image_url: imageUrl,
      };
      const updated = await command<CollectionTaskView>(
        "remove_collection_task_image",
        { request },
      );
      replaceCollectionTask(updated);
      ElMessage.success("图片已删除，采集审查已重置");
    } catch (error) {
      ElMessage.error(`删除图片失败：${error}`);
    } finally {
      collectionImageUpdating.value = false;
    }
  }
  async function searchCollectionReviewCategories(keyword?: string) {
    const task = selectedCollectionDetailTask.value;
    if (!task) {
      return;
    }
    const normalizedKeyword = (
      keyword ?? collectionReviewCategoryKeyword.value
    ).trim();
    if (!normalizedKeyword) {
      ElMessage.warning("请输入类目关键词或 cat_id");
      return;
    }
    collectionReviewCategoryKeyword.value = normalizedKeyword;
    collectionReviewCategorySearching.value = true;
    try {
      const result = await command<CategoryCatalogListResult>(
        "list_category_catalog",
        {
          shopId: resolveCollectionReviewCategoryShopId(task),
          keyword: normalizedKeyword,
          limit: 300,
        },
      );
      collectionReviewCategoryOptions.value =
        buildCollectionReviewCategoryOptions(result.categories);
      if (collectionReviewCategoryOptions.value.length === 0) {
        ElMessage.warning(
          "没有找到可用于确认的微信类目，请先同步店铺类目权限或换关键词",
        );
        return;
      }
      if (!selectedReviewCategoryKey.value) {
        selectedReviewCategoryKey.value = categoryOptionKey(
          collectionReviewCategoryOptions.value[0].category_ids,
        );
      }
    } catch (error) {
      ElMessage.error(`类目搜索失败：${error}`);
    } finally {
      collectionReviewCategorySearching.value = false;
    }
  }
  async function selectCollectionExcelFile() {
    if (!isTauriRuntime) {
      ElMessage.info("浏览器预览模式下不能打开本地文件选择器，请在桌面端使用");
      return;
    }
    try {
      const selected = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "Excel 文件", extensions: ["xlsx"] }],
      });
      if (typeof selected === "string") {
        collectionFilePath.value = selected;
      } else if (Array.isArray(selected) && typeof selected[0] === "string") {
        collectionFilePath.value = selected[0];
      }
    } catch (error) {
      ElMessage.error(`选择文件失败：${error}`);
    }
  }
  function clearCollectionExcelFile() {
    collectionFilePath.value = "";
  }
  async function startExcelImport() {
    if (!collectionFilePath.value.trim()) {
      ElMessage.warning("请先选择 Excel 文件");
      return;
    }
    collectionImporting.value = true;
    try {
      const count = await command<number>("import_excel_for_collection", {
        filePath: collectionFilePath.value.trim(),
      });
      ElMessage.success(`成功导入 ${count} 个商品采集任务`);
      collectionImportVisible.value = false;
      collectionFilePath.value = "";
      await refreshCollectionTasks();
      startCollectionPolling();
    } catch (err: any) {
      ElMessage.error(`导入失败：${err}`);
    } finally {
      collectionImporting.value = false;
    }
  }
  async function triggerTaobaoLogin() {
    collectionLoggingIn.value = true;
    try {
      ElMessage.info("正在拉起淘宝登录窗口，请稍候...");
      await command("open_taobao_login");
      ElMessage.success("淘宝登录会话已保存");
    } catch (err: any) {
      ElMessage.error(`打开淘宝登录失败：${err}`);
    } finally {
      collectionLoggingIn.value = false;
    }
  }
  async function retryCollection(taskId: string) {
    try {
      await command("retry_collection_task", { taskId });
      ElMessage.success("已加入重新采集队列");
      await refreshCollectionTasks();
      startCollectionPolling();
    } catch (err: any) {
      ElMessage.error(`操作失败：${err}`);
    }
  }
  async function resumeCollectionTasks() {
    try {
      const count = await command<number>("resume_collection_tasks");
      if (count > 0) {
        ElMessage.success(`已触发采集恢复：${count} 个待处理任务`);
        startCollectionPolling();
      } else {
        ElMessage.info("没有等待或正在采集的任务");
      }
      await refreshCollectionTasks();
    } catch (err: any) {
      ElMessage.error(`恢复采集失败：${err}`);
    }
  }
  async function retryAllFailedCollections() {
    try {
      const count = await command<number>("retry_all_failed_collection_tasks");
      if (count > 0) {
        ElMessage.success(`已将 ${count} 个失败任务重新加入采集队列`);
        await refreshCollectionTasks();
        startCollectionPolling();
      } else {
        ElMessage.info("没有失败的采集任务");
      }
    } catch (err: any) {
      ElMessage.error(`操作失败：${err}`);
    }
  }
  async function clearCollectionHistory() {
    try {
      await command("clear_collection_tasks");
      ElMessage.success("采集任务列表已清空");
      selectedCollectionTasks.value = [];
      await refreshCollectionTasks();
    } catch (err: any) {
      ElMessage.error(`清空失败：${err}`);
    }
  }
  async function deleteTaobaoProfile() {
    try {
      await ElMessageBox.confirm(
        "将删除浏览器 Profile（包含登录态和冷却记录），删除后需要重新登录淘宝。确定继续？",
        "删除淘宝 Profile",
        {
          confirmButtonText: "确认删除",
          cancelButtonText: "取消",
          type: "warning",
        },
      );
    } catch {
      return;
    }
    try {
      const path = await command<string>("delete_taobao_profile");
      ElMessage.success(`淘宝 Profile 已删除：${path}`);
      await refreshTaobaoAccessLimitState();
    } catch (err: any) {
      ElMessage.error(`删除失败：${err}`);
    }
  }
  async function checkTaobaoLoginState() {
    collectionCheckingLogin.value = true;
    try {
      const res: any = await command("check_taobao_login_state");
      collectionAccessLimitState.value = {
        access_limited: Boolean(res?.access_limited),
        captcha_cooling: Boolean(res?.captcha_cooling),
        message: res?.access_limit_message || null,
        cooldown: res?.access_limit_cooldown || null,
        captcha_cooling_message: res?.captcha_cooling_message || null,
        captcha_failure_cooldown: res?.captcha_failure_cooldown || null,
      };
      if (res?.access_limited) {
        ElMessage.warning(
          res.access_limit_message ||
            "淘宝账号处于访问限制冷却期，暂不建议采集。",
        );
      } else if (res?.captcha_cooling) {
        ElMessage.warning(
          res.captcha_cooling_message ||
            "淘宝验证码自动处理失败后处于本地保护冷却期，暂不建议采集。",
        );
      } else if (res && res.logged_in) {
        ElMessage.success(`登录态有效：${res.detail || ""}`);
      } else {
        ElMessage.warning(
          res?.detail ||
            '未检测到有效登录态，请先点击"淘宝登录(保持状态)"完成登录。',
        );
      }
    } catch (err: any) {
      ElMessage.error(`检测登录态失败：${err}`);
    } finally {
      collectionCheckingLogin.value = false;
    }
  }
  async function refreshTaobaoAccessLimitState() {
    collectionAccessLimitChecking.value = true;
    try {
      const res: any = await command("get_taobao_access_limit_state");
      collectionAccessLimitState.value = res;
      if (res?.access_limited) {
        ElMessage.warning(
          res.message || "淘宝账号处于访问限制冷却期，暂不建议采集。",
        );
      } else if (res?.captcha_cooling) {
        ElMessage.warning(
          res.captcha_cooling_message ||
            "淘宝验证码自动处理失败后处于本地保护冷却期，暂不建议采集。",
        );
      } else {
        ElMessage.success("当前没有本地淘宝访问限制冷却记录");
      }
    } catch (err: any) {
      ElMessage.error(`查询访问限制状态失败：${err}`);
    } finally {
      collectionAccessLimitChecking.value = false;
    }
  }
  async function clearTaobaoAccessLimitState() {
    try {
      await ElMessageBox.confirm(
        "只会清除本地冷却记录，不会访问淘宝。请确认你已经手动验证账号可以正常打开淘宝商品页。",
        "清除淘宝保护冷却标记",
        { type: "warning" },
      );
    } catch {
      return;
    }
    collectionAccessLimitChecking.value = true;
    try {
      const res: any = await command("clear_taobao_access_limit_state");
      collectionAccessLimitState.value = res;
      ElMessage.success("已清除本地淘宝保护冷却标记");
    } catch (err: any) {
      ElMessage.error(`清除访问限制标记失败：${err}`);
    } finally {
      collectionAccessLimitChecking.value = false;
    }
  }
  function openTestCollectDialog() {
    testCollectResult.value = null;
    testCollectVisible.value = true;
  }
  async function runTestCollect() {
    const url = testCollectUrl.value.trim();
    if (!url) {
      ElMessage.warning("请输入淘宝商品链接");
      return;
    }
    collectionTesting.value = true;
    try {
      const res: any = await command("test_taobao_collect", {
        url,
        headed: testCollectHeaded.value,
      });
      testCollectResult.value = res;
      if (res?.success) {
        ElMessage.success("抓取成功");
      } else {
        ElMessage.error(res?.error || "抓取失败");
      }
    } catch (err: any) {
      ElMessage.error(`测试抓取失败：${err}`);
      testCollectResult.value = {
        success: false,
        error: String(err),
        raw: null,
        stderr: "",
      };
    } finally {
      collectionTesting.value = false;
    }
  }
  function startCollectionPolling() {
    if (collectionPollTimer) return;
    collectionPollTimer = window.setInterval(async () => {
      await refreshCollectionTasks();
      const hasActive = collectionTasks.value.some(
        (t) => t.status === "pending" || t.status === "running",
      );
      if (!hasActive && collectionPollTimer) {
        window.clearInterval(collectionPollTimer);
        collectionPollTimer = null;
      }
    }, 3000);
  }
  return {
    canSelectCollectionTask,
    checkTaobaoLoginState,
    clearCollectionExcelFile,
    clearCollectionHistory,
    clearTaobaoAccessLimitState,
    collectionAccessLimitChecking,
    collectionAccessLimitState,
    collectionCheckingLogin,
    collectionDetailVisible,
    collectionFileName,
    collectionFilePath,
    collectionImporting,
    collectionImportVisible,
    collectionImageSrc,
    collectionImageUpdating,
    collectionLoggingIn,
    collectionProductSummary,
    collectionPublishedShopNames,
    collectionPublishJobText,
    collectionReviewCategoryKeyword,
    collectionReviewCategorySearching,
    collectionPublishing,
    collectionPublishTargetShopIds,
    collectionReviewConfirming,
    collectionReviewing,
    collectionReviewStatusLabel,
    collectionReviewStatusType,
    collectionTargetShopSelectionRequired,
    publishPricingDialogVisible,
    publishPricingForm,
    publishPricingSummary,
    computeSalePriceCents,
    selectedCollectionDetailImages,
    selectedCollectionMainImages,
    selectedCollectionSkuPreview,
    collectionTaskPage,
    collectionTaskPageSize,
    collectionTaskPageSizeOptions,
    collectionTasks,
    collectionTaskStatusLabel,
    collectionTaskTotal,
    collectionTesting,
    createPublishJobFromSelectedCollections,
    handleCollectionTaskPageChange,
    handleCollectionTaskPageSizeChange,
    isCollectionTaskSelected,
    isCurrentCollectionPageAllSelected,
    isCurrentCollectionPageIndeterminate,
    openTestCollectDialog,
    openCollectionDetail,
    openPublishPricingDialog,
    paginatedCollectionTasks,
    refreshCollectionTasks,
    refreshTaobaoAccessLimitState,
    resumeCollectionTasks,
    retryAllFailedCollections,
    retryCollection,
    deleteTaobaoProfile,
    runTestCollect,
    confirmSelectedCollectionReview,
    removeCollectionImage,
    resetAllPassedReviews,
    resetSelectedCollectionReview,
    reviewSingleCollectionTask,
    reviewSelectedCollectionTasks,
    savePublishPricingStrategyFromForm,
    searchCollectionReviewCategories,
    selectCollectionExcelFile,
    selectedCollectionTasks,
    selectedCollectionDetailJson,
    selectedCollectionDetailProduct,
    selectedCollectionDetailTask,
    selectedCollectionOriginalProduct,
    selectedCollectionPublishable,
    selectedCollectionRemovedImages,
    selectedCollectionReviewIssues,
    selectedCollectionReviewResult,
    selectedCollectionCategoryCandidates,
    selectedCollectionReviewCategoryOptions,
    selectedReviewCategoryKey,
    setCollectionTaskSelected,
    startCollectionPolling,
    startExcelImport,
    testCollectHeaded,
    testCollectResult,
    testCollectUrl,
    testCollectVisible,
    toggleCurrentCollectionPageSelection,
    triggerTaobaoLogin,
    uploadCollectionImage,
  };
}
