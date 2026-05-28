import { computed, ref, type Ref } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ElMessage, ElMessageBox } from "element-plus";
import type {
  CollectionPublishRequest,
  CollectionReviewBatchResult,
  CollectionReviewConfirmRequest,
  CollectionTaskView,
  PublishJobCreated,
  PublishPricingStrategy,
  ShopListItem,
} from "../../types/app";
type Command = <T = unknown>(name: string, args?: Record<string, unknown>) => Promise<T>;
interface CollectionWorkflowDeps {
  command: Command;
  isTauriRuntime: boolean;
  latestTaskId: Ref<string>;
  publishPricingStrategy: Ref<PublishPricingStrategy>;
  queriedTaskId: Ref<string>;
  queryJob: () => Promise<void>;
  refreshAll: () => Promise<void>;
  savePublishPricingStrategy: (strategy: PublishPricingStrategy) => Promise<boolean>;
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
  shops,
}: CollectionWorkflowDeps) {
  const collectionImportVisible = ref(false);
  const collectionFilePath = ref("");
  const collectionFileName = computed(() => {
    const path = collectionFilePath.value.trim();
    if (!path)
      return "";
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
  const publishPricingDialogVisible = ref(false);
  const publishPricingForm = ref<PublishPricingStrategy>({ ...publishPricingStrategy.value });
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
    const candidates = selectedCollectionReviewResult.value?.category?.candidates;
    return Array.isArray(candidates) ? candidates : [];
  });
  const selectedCollectionDetailJson = computed(() => {
    if (selectedCollectionDetailProduct.value) {
      return JSON.stringify(selectedCollectionDetailProduct.value, null, 2);
    }
    return selectedCollectionDetailTask.value?.collected_data || "";
  });
  const selectedCollectionMainImages = computed(() => {
    const images = selectedCollectionDetailProduct.value?.images;
    return Array.isArray(images) ? images.slice(0, 8) : [];
  });
  const selectedCollectionDetailImages = computed(() => {
    const images = selectedCollectionDetailProduct.value?.detail_images;
    return Array.isArray(images) ? images.slice(0, 8) : [];
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
  const publishPricingPreviewRows = computed(() => {
    const rows: Array<{
      task_id: string;
      title: string;
      sku_label: string;
      cost_price_yuan: number;
      sale_price_cents: number;
    }> = [];
    for (const task of selectedCollectionTasks.value) {
      const product = parseCollectionProduct(task);
      const skus = Array.isArray(product?.skus) ? product.skus : [];
      for (const sku of skus) {
        const costPrice = Number(sku?.cost_price);
        if (!Number.isFinite(costPrice) || costPrice <= 0) {
          continue;
        }
        rows.push({
          task_id: task.id,
          title: product?.title || task.title,
          sku_label: formatSkuSpecs(sku),
          cost_price_yuan: costPrice,
          sale_price_cents: computeSalePriceCents(costPrice, publishPricingForm.value),
        });
        if (rows.length >= 20) {
          return rows;
        }
      }
    }
    return rows;
  });
  async function refreshCollectionTasks() {
    try {
      collectionTasks.value = await command<CollectionTaskView[]>("get_collection_tasks");
      const latestById = new Map(collectionTasks.value.map((task) => [task.id, task]));
      selectedCollectionTasks.value = selectedCollectionTasks.value
        .map((task) => latestById.get(task.id))
        .filter((task): task is CollectionTaskView => Boolean(task && canSelectCollectionTask(task)));
      if (selectedCollectionDetailTask.value) {
        selectedCollectionDetailTask.value =
          latestById.get(selectedCollectionDetailTask.value.id) || selectedCollectionDetailTask.value;
      }
      normalizeCollectionTaskPage();
    }
    catch (err: any) {
      console.error("加载采集任务失败：", err);
    }
  }
  const collectionTaskTotal = computed(() => collectionTasks.value.length);
  const paginatedCollectionTasks = computed(() => {
    const start = (collectionTaskPage.value - 1) * collectionTaskPageSize.value;
    return collectionTasks.value.slice(start, start + collectionTaskPageSize.value);
  });
  const selectableCurrentCollectionTasks = computed(() => paginatedCollectionTasks.value.filter(canSelectCollectionTask));
  const selectedCollectionTaskIds = computed(() => new Set(selectedCollectionTasks.value.map((task) => task.id)));
  const isCurrentCollectionPageAllSelected = computed(() => {
    const rows = selectableCurrentCollectionTasks.value;
    return rows.length > 0 && rows.every((task) => selectedCollectionTaskIds.value.has(task.id));
  });
  const isCurrentCollectionPageIndeterminate = computed(() => {
    const rows = selectableCurrentCollectionTasks.value;
    if (rows.length === 0)
      return false;
    const selectedCount = rows.filter((task) => selectedCollectionTaskIds.value.has(task.id)).length;
    return selectedCount > 0 && selectedCount < rows.length;
  });
  const selectedCollectionPublishable = computed(() => {
    return selectedCollectionTasks.value.length > 0
      && selectedCollectionTasks.value.every((task) => task.review_status === "passed");
  });
  function normalizeCollectionTaskPage() {
    const maxPage = Math.max(1, Math.ceil(collectionTaskTotal.value / collectionTaskPageSize.value));
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
  function setCollectionTaskSelected(row: CollectionTaskView, selected: boolean) {
    if (!canSelectCollectionTask(row))
      return;
    if (selected) {
      if (!selectedCollectionTaskIds.value.has(row.id)) {
        selectedCollectionTasks.value = [...selectedCollectionTasks.value, row];
      }
      return;
    }
    selectedCollectionTasks.value = selectedCollectionTasks.value.filter((task) => task.id !== row.id);
  }
  function toggleCurrentCollectionPageSelection(selected: boolean) {
    const pageRows = selectableCurrentCollectionTasks.value;
    if (selected) {
      const selectedIds = selectedCollectionTaskIds.value;
      const additions = pageRows.filter((task) => !selectedIds.has(task.id));
      selectedCollectionTasks.value = [...selectedCollectionTasks.value, ...additions];
      return;
    }
    const pageIds = new Set(pageRows.map((task) => task.id));
    selectedCollectionTasks.value = selectedCollectionTasks.value.filter((task) => !pageIds.has(task.id));
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
  function collectionReviewStatusLabel(status: CollectionTaskView["review_status"]) {
    const labels: Record<CollectionTaskView["review_status"], string> = {
      pending: "待审查",
      passed: "审查通过",
      needs_review: "需确认",
      blocked: "已拦截",
      failed: "审查失败",
    };
    return labels[status] || status;
  }
  function collectionReviewStatusType(status: CollectionTaskView["review_status"]) {
    const types: Record<CollectionTaskView["review_status"], "success" | "warning" | "danger" | "info"> = {
      pending: "info",
      passed: "success",
      needs_review: "warning",
      blocked: "danger",
      failed: "danger",
    };
    return types[status] || "info";
  }
  function parseCollectionProduct(task: CollectionTaskView, preferReviewed = false): any | null {
    const raw = preferReviewed ? (task.reviewed_data || task.collected_data) : task.collected_data;
    if (!raw) {
      return null;
    }
    try {
      return JSON.parse(raw);
    }
    catch {
      return null;
    }
  }
  function parseCollectionReviewResult(task: CollectionTaskView): any | null {
    if (!task.review_result_json) {
      return null;
    }
    try {
      return JSON.parse(task.review_result_json);
    }
    catch {
      return null;
    }
  }
  function collectionProductSummary(task: CollectionTaskView) {
    const product = parseCollectionProduct(task);
    if (!product) {
      return "未生成商品数据";
    }
    const imageCount = Array.isArray(product.images) ? product.images.length : 0;
    const detailImageCount = Array.isArray(product.detail_images) ? product.detail_images.length : 0;
    const skuCount = Array.isArray(product.skus) ? product.skus.length : 0;
    return `主图 ${imageCount} / 详情图 ${detailImageCount} / SKU ${skuCount}`;
  }
  function openCollectionDetail(task: CollectionTaskView) {
    selectedCollectionDetailTask.value = task;
    const candidates = parseCollectionReviewResult(task)?.category?.candidates;
    if (Array.isArray(candidates) && candidates.length > 0) {
      const first = candidates[0];
      selectedReviewCategoryKey.value = Array.isArray(first?.category_ids)
        ? first.category_ids.join("/")
        : "";
    }
    else {
      selectedReviewCategoryKey.value = "";
    }
    collectionDetailVisible.value = true;
  }
  function shopNamesByIds(shopIds: string[]) {
    if (shopIds.length === 0) {
      return "-";
    }
    return shopIds
      .map((shopId) => shops.value.find((shop) => shop.id === shopId)?.name || shopId)
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
    return Number.isInteger(rate) ? String(rate) : rate.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
  }
  function formatSkuSpecs(sku: any) {
    const specs = sku?.specs;
    if (!specs) {
      return sku?.external_sku_id || "默认规格";
    }
    if (typeof specs === "string") {
      return specs;
    }
    if (Array.isArray(specs)) {
      return specs.join(" / ");
    }
    return Object.entries(specs)
      .map(([key, value]) => `${key}：${value}`)
      .join(" / ") || sku?.external_sku_id || "默认规格";
  }
  function computeSalePriceCents(costPriceYuan: number, strategy: PublishPricingStrategy) {
    const computed = Math.ceil(costPriceYuan * 100 * strategy.sale_price_markup_rate)
      + strategy.sale_price_fixed_cents;
    return Math.max(strategy.sale_price_floor_cents, computed);
  }
  function openPublishPricingDialog() {
    publishPricingForm.value = { ...publishPricingStrategy.value };
    publishPricingDialogVisible.value = true;
  }
  async function savePublishPricingStrategyFromForm() {
    const saved = await savePublishPricingStrategy({ ...publishPricingForm.value });
    if (saved) {
      publishPricingDialogVisible.value = false;
    }
  }
  async function createPublishJobFromSelectedCollections() {
    if (selectedCollectionTasks.value.length === 0) {
      ElMessage.warning("请先选择审查通过的商品");
      return;
    }
    if (!selectedCollectionPublishable.value) {
      ElMessage.warning("只有审查通过的商品才能创建铺货任务");
      return;
    }
    if (collectionPublishTargetShopIds.value.length === 0) {
      ElMessage.warning("请选择至少一个目标微信小店");
      return;
    }
    collectionPublishing.value = true;
    try {
      const request: CollectionPublishRequest = {
        collection_task_ids: selectedCollectionTasks.value.map((task) => task.id),
        target_shop_ids: collectionPublishTargetShopIds.value,
        pricing_strategy: { ...publishPricingStrategy.value },
      };
      const result = await command<PublishJobCreated>("create_publish_job_from_collection_tasks", { request });
      latestTaskId.value = result.task_id;
      queriedTaskId.value = result.task_id;
      ElMessage.success(`铺货任务已创建：${result.task_id}`);
      selectedCollectionTasks.value = [];
      await Promise.all([refreshCollectionTasks(), refreshAll(), queryJob()]);
    }
    catch (error) {
      ElMessage.error(String(error));
    }
    finally {
      collectionPublishing.value = false;
    }
  }
  async function reviewSelectedCollectionTasks() {
    const taskIds = selectedCollectionTasks.value.length > 0
      ? selectedCollectionTasks.value.map((task) => task.id)
      : paginatedCollectionTasks.value
        .filter((task) => task.status === "success" && task.review_status !== "passed")
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
    collectionReviewing.value = true;
    try {
      const result = await command<CollectionReviewBatchResult>("run_collection_review_once", {
        request: {
          task_ids: taskIds,
          target_shop_ids: collectionPublishTargetShopIds.value,
          limit: taskIds.length,
        },
      });
      ElMessage.success(`审查完成：通过 ${result.passed_items}，需确认 ${result.needs_review_items}，拦截 ${result.blocked_items}`);
      await refreshCollectionTasks();
    }
    catch (error) {
      ElMessage.error(`审查失败：${error}`);
    }
    finally {
      collectionReviewing.value = false;
    }
  }
  async function confirmSelectedCollectionReview() {
    const task = selectedCollectionDetailTask.value;
    if (!task) {
      return;
    }
    const selectedCandidate = selectedCollectionCategoryCandidates.value.find((candidate: any) => {
      return Array.isArray(candidate?.category_ids) && candidate.category_ids.join("/") === selectedReviewCategoryKey.value;
    });
    const request: CollectionReviewConfirmRequest = {
      task_id: task.id,
      title: selectedCollectionDetailProduct.value?.title || task.title,
    };
    if (selectedCandidate) {
      request.category_ids = selectedCandidate.category_ids;
      request.category_path = selectedCandidate.category_path;
    }
    collectionReviewConfirming.value = true;
    try {
      const updated = await command<CollectionTaskView>("confirm_collection_review", { request });
      selectedCollectionDetailTask.value = updated;
      ElMessage.success("已确认审查通过");
      await refreshCollectionTasks();
    }
    catch (error) {
      ElMessage.error(`确认失败：${error}`);
    }
    finally {
      collectionReviewConfirming.value = false;
    }
  }
  async function resetSelectedCollectionReview(task: CollectionTaskView) {
    try {
      const updated = await command<CollectionTaskView>("reset_collection_review", { taskId: task.id });
      if (selectedCollectionDetailTask.value?.id === task.id) {
        selectedCollectionDetailTask.value = updated;
      }
      ElMessage.success("已重置审查状态");
      await refreshCollectionTasks();
    }
    catch (error) {
      ElMessage.error(`重置失败：${error}`);
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
      }
      else if (Array.isArray(selected) && typeof selected[0] === "string") {
        collectionFilePath.value = selected[0];
      }
    }
    catch (error) {
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
    }
    catch (err: any) {
      ElMessage.error(`导入失败：${err}`);
    }
    finally {
      collectionImporting.value = false;
    }
  }
  async function triggerTaobaoLogin() {
    collectionLoggingIn.value = true;
    try {
      ElMessage.info("正在拉起淘宝登录窗口，请稍候...");
      await command("open_taobao_login");
      ElMessage.success("淘宝登录会话已保存");
    }
    catch (err: any) {
      ElMessage.error(`打开淘宝登录失败：${err}`);
    }
    finally {
      collectionLoggingIn.value = false;
    }
  }
  async function retryCollection(taskId: string) {
    try {
      await command("retry_collection_task", { taskId });
      ElMessage.success("已加入重新采集队列");
      await refreshCollectionTasks();
      startCollectionPolling();
    }
    catch (err: any) {
      ElMessage.error(`操作失败：${err}`);
    }
  }
  async function resumeCollectionTasks() {
    try {
      const count = await command<number>("resume_collection_tasks");
      if (count > 0) {
        ElMessage.success(`已触发采集恢复：${count} 个待处理任务`);
        startCollectionPolling();
      }
      else {
        ElMessage.info("没有等待或正在采集的任务");
      }
      await refreshCollectionTasks();
    }
    catch (err: any) {
      ElMessage.error(`恢复采集失败：${err}`);
    }
  }
  async function clearCollectionHistory() {
    try {
      await command("clear_collection_tasks");
      ElMessage.success("采集任务列表已清空");
      selectedCollectionTasks.value = [];
      await refreshCollectionTasks();
    }
    catch (err: any) {
      ElMessage.error(`清空失败：${err}`);
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
        ElMessage.warning(res.access_limit_message || "淘宝账号处于访问限制冷却期，暂不建议采集。");
      }
      else if (res?.captcha_cooling) {
        ElMessage.warning(res.captcha_cooling_message || "淘宝验证码自动处理失败后处于本地保护冷却期，暂不建议采集。");
      }
      else if (res && res.logged_in) {
        ElMessage.success(`登录态有效：${res.detail || ""}`);
      }
      else {
        ElMessage.warning(res?.detail || "未检测到有效登录态，请先点击\"淘宝登录(保持状态)\"完成登录。");
      }
    }
    catch (err: any) {
      ElMessage.error(`检测登录态失败：${err}`);
    }
    finally {
      collectionCheckingLogin.value = false;
    }
  }
  async function refreshTaobaoAccessLimitState() {
    collectionAccessLimitChecking.value = true;
    try {
      const res: any = await command("get_taobao_access_limit_state");
      collectionAccessLimitState.value = res;
      if (res?.access_limited) {
        ElMessage.warning(res.message || "淘宝账号处于访问限制冷却期，暂不建议采集。");
      }
      else if (res?.captcha_cooling) {
        ElMessage.warning(res.captcha_cooling_message || "淘宝验证码自动处理失败后处于本地保护冷却期，暂不建议采集。");
      }
      else {
        ElMessage.success("当前没有本地淘宝访问限制冷却记录");
      }
    }
    catch (err: any) {
      ElMessage.error(`查询访问限制状态失败：${err}`);
    }
    finally {
      collectionAccessLimitChecking.value = false;
    }
  }
  async function clearTaobaoAccessLimitState() {
    try {
      await ElMessageBox.confirm("只会清除本地冷却记录，不会访问淘宝。请确认你已经手动验证账号可以正常打开淘宝商品页。", "清除淘宝保护冷却标记", { type: "warning" });
    }
    catch {
      return;
    }
    collectionAccessLimitChecking.value = true;
    try {
      const res: any = await command("clear_taobao_access_limit_state");
      collectionAccessLimitState.value = res;
      ElMessage.success("已清除本地淘宝保护冷却标记");
    }
    catch (err: any) {
      ElMessage.error(`清除访问限制标记失败：${err}`);
    }
    finally {
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
      const res: any = await command("test_taobao_collect", { url, headed: testCollectHeaded.value });
      testCollectResult.value = res;
      if (res?.success) {
        ElMessage.success("抓取成功");
      }
      else {
        ElMessage.error(res?.error || "抓取失败");
      }
    }
    catch (err: any) {
      ElMessage.error(`测试抓取失败：${err}`);
      testCollectResult.value = { success: false, error: String(err), raw: null, stderr: "" };
    }
    finally {
      collectionTesting.value = false;
    }
  }
  function startCollectionPolling() {
    if (collectionPollTimer)
      return;
    collectionPollTimer = window.setInterval(async () => {
      await refreshCollectionTasks();
      const hasActive = collectionTasks.value.some(t => t.status === "pending" || t.status === "running");
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
    collectionLoggingIn,
    collectionProductSummary,
    collectionPublishedShopNames,
    collectionPublishJobText,
    collectionPublishing,
    collectionPublishTargetShopIds,
    collectionReviewConfirming,
    collectionReviewing,
    collectionReviewStatusLabel,
    collectionReviewStatusType,
    publishPricingDialogVisible,
    publishPricingForm,
    publishPricingPreviewRows,
    publishPricingSummary,
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
    retryCollection,
    runTestCollect,
    confirmSelectedCollectionReview,
    resetSelectedCollectionReview,
    reviewSingleCollectionTask,
    reviewSelectedCollectionTasks,
    savePublishPricingStrategyFromForm,
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
  };
}
