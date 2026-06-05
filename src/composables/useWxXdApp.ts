import { computed, onMounted, reactive, ref } from "../runtime/reactive";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ElMessage, ElMessageBox } from "../runtime/feedback";
import type {
  DashboardSummary,
  ShopGroup,
  ShopListItem,
  ShopCredentialCheck,
  ShopBasicInfoSyncResult,
  ApiQuotaCheckResult,
  CategoryCatalogShopSummary,
  CategoryCacheView,
  CategoryRelationView,
  FreightTemplateView,
  CategoryCatalogListResult,
  CategoryCatalogSyncResult,
  CategoryRuleSyncResult,
  PublishJobCreated,
  TaskRunView,
  PublishPipelineRunResult,
  PriceUpdateJobCreated,
  PriceUpdatePrecheckBatchResult,
  PriceUpdateSubmitBatchResult,
  PriceUpdateConfirmBatchResult,
  OrderPriceAdjustmentBatchResult,
  OrderPriceAdjustmentJobCreated,
  OrderPriceAdjustmentJobView,
  OrderSyncBatchResult,
  OrderDetailSyncBatchResult,
  AftersaleSyncBatchResult,
  AftersaleView,
  AftersaleListResult,
  AftersaleResponsibilityResult,
  AftersaleActionResult,
  AftersaleRejectReasonView,
  AftersaleRejectReasonSyncResult,
  GuaranteeSyncBatchResult,
  GuaranteeOrderView,
  GuaranteeOrderListResult,
  GuaranteeFollowupResult,
  AftersaleEvidenceView,
  AftersaleEvidenceListResult,
  AftersaleEvidenceRecordResult,
  AftersaleEvidenceStatusUpdateResult,
  AftersaleEvidenceExportResult,
  SupplierAftersaleFollowupView,
  SupplierAftersaleFollowupListResult,
  SupplierAftersaleFollowupRecordResult,
  PurchaseTaskBatchResult,
  PurchaseTaskView,
  PurchaseTaskListResult,
  PurchaseTaskExportResult,
  SupplierAgentExportResult,
  SupplierAgentApplyResult,
  PurchaseTaskMappingResult,
  PurchaseTaskIssueResult,
  PurchaseTaskShipmentResult,
  OrderProfitView,
  OrderProfitTotals,
  OrderProfitListResult,
  InventoryRiskView,
  InventoryRiskListResult,
  InventoryRiskScanResult,
  ProductSalesAnalysisView,
  ProductSalesAnalysisTotals,
  ProductSalesAnalysisListResult,
  ProductManagementView,
  ProductManagementListResult,
  OrderManagementView,
  OrderManagementListResult,
  OrderProfitAdjustmentResult,
  DeliverySettings,
  DeliveryCompanyView,
  DeliveryCompanySyncResult,
  LocalApiConfig,
  LocalApiKeyRotationResult,
  AiProviderSettings,
  AiProviderTestResult,
  AgentRunView,
  AgentSkillSettingsRequest,
  AgentSkillTestResult,
  AgentSkillView,
  ExternalApiLogView,
  NotificationView,
  NotificationListResult,
  NotificationMarkResult,
  BackupInfo,
  BackupCreateResult,
  BackupRestoreResult,
  CollectionPublishWorkspaceResetResult,
  ShipmentRecordResult,
  ShipmentView,
  ShipmentListResult,
  ShipmentRetryResult,
  DeliverySubmitBatchResult,
  OperationalAutomationSettings,
  OperationalAutomationRunResult,
  PublishJobView,
  PublishPricingStrategy,
  PriceUpdateJobView,
  CollectionTaskView,
} from "../types/app";
import {
  aftersaleResponsibilityOptions,
  aftersaleTerminalStatuses,
  createDefaultOrderPriceAdjustmentPayload,
  createDefaultPriceUpdatePayload,
  createDefaultPublishPayload,
  defaultAutomationSettings,
  defaultPublishPricingStrategy,
  evidenceStatusOptions,
  evidenceTargetTypeOptions,
  evidenceTypeOptions,
  fallbackDeliveryCompanyOptions,
  guaranteeHandlingStatusOptions,
  inventoryRiskStatusOptions,
  orderManagementStatusOptions,
  productManagementStatusOptions,
  productSalesStatusOptions,
  profitAdjustmentKindOptions,
  purchaseIssueTypeOptions,
  statusTone,
  supplierFollowupStatusOptions,
  supplierFollowupTypeOptions,
} from "./useWxXdApp/constants";
import {
  formatBytes,
  formatCents,
  formatDateTime,
  formatSignedCents,
  formatUnixTime,
} from "./useWxXdApp/formatters";
import { useCollectionWorkflow } from "./useWxXdApp/collectionWorkflow";
import { createPreviewMode } from "./useWxXdApp/previewMode";
import {
  createPreviewGroups,
  createPreviewShops,
  createPreviewTaskRuns,
  createPreviewCollectionTasks,
  createPreviewShipments,
  createPreviewPurchaseTasks,
  createPreviewAftersales,
  createPreviewAftersaleRejectReasons,
  createPreviewGuaranteeOrders,
  createPreviewAftersaleEvidence,
  createPreviewSupplierAftersaleFollowups,
  createPreviewProfitAdjustments,
  createPreviewDatabaseBackups,
  createPreviewExternalApiLogs,
  createPreviewNotifications,
  createPreviewCategoryCatalogShops,
  createPreviewCategoryCache,
  createPreviewCategoryRelations,
  createPreviewFreightTemplates,
} from "./useWxXdApp/previewFixtures";

export function useWxXdApp() {
  const dashboard = ref<DashboardSummary | null>(null);
  const groups = ref<ShopGroup[]>([]);
  const shops = ref<ShopListItem[]>([]);
  const selectedSection = ref("workbench");
  const loading = ref(false);
  const backupRunning = ref(false);
  const workspaceResetRunning = ref(false);
  const publishRetryRunning = ref(false);
  const latestTaskId = ref("");
  const queriedTaskId = ref("");
  const currentJob = ref<PublishJobView | null>(null);
  const publishPricingStrategy = ref<PublishPricingStrategy>(
    defaultPublishPricingStrategy(),
  );
  const publishPricingSaving = ref(false);

  // 采集与导入相关状态

  const latestPriceTaskId = ref("");
  const queriedPriceTaskId = ref("");
  const currentPriceJob = ref<PriceUpdateJobView | null>(null);
  const latestOrderPriceAdjustmentTaskId = ref("");
  const queriedOrderPriceAdjustmentTaskId = ref("");
  const currentOrderPriceAdjustmentJob =
    ref<OrderPriceAdjustmentJobView | null>(null);
  const taskRuns = ref<TaskRunView[]>([]);
  const purchaseTasks = ref<PurchaseTaskView[]>([]);
  const purchaseTaskTotal = ref(0);
  const purchaseStatusFilter = ref("all");
  const purchaseExportPath = ref("");
  const supplierAgentExportFormat = ref("jsonl");
  const supplierAgentExportPath = ref("");
  const supplierAgentApplyText = ref("");
  const supplierAgentDryRun = ref(true);
  const supplierAgentContinueOnError = ref(true);
  const supplierAgentApplyResult = ref<SupplierAgentApplyResult | null>(null);
  const aftersales = ref<AftersaleView[]>([]);
  const aftersaleTotal = ref(0);
  const aftersaleStatusFilter = ref("active");
  const aftersaleRejectReasons = ref<AftersaleRejectReasonView[]>([]);
  const aftersaleEvidence = ref<AftersaleEvidenceView[]>([]);
  const aftersaleEvidenceTotal = ref(0);
  const evidenceStatusFilter = ref("all");
  const evidenceTargetTypeFilter = ref("all");
  const evidenceTargetIdFilter = ref("");
  const evidenceExportFormat = ref("md");
  const evidenceExportPath = ref("");
  const supplierAftersaleFollowups = ref<SupplierAftersaleFollowupView[]>([]);
  const supplierAftersaleFollowupTotal = ref(0);
  const supplierFollowupTargetTypeFilter = ref("all");
  const supplierFollowupTargetIdFilter = ref("");
  const supplierFollowupStatusFilter = ref("all");
  const guaranteeOrders = ref<GuaranteeOrderView[]>([]);
  const guaranteeOrderTotal = ref(0);
  const guaranteeStatusFilter = ref("active");
  const orderProfits = ref<OrderProfitView[]>([]);
  const orderProfitTotal = ref(0);
  const orderProfitStatusFilter = ref("all");
  const orderManagementItems = ref<OrderManagementView[]>([]);
  const orderManagementTotal = ref(0);
  const orderManagementStatusFilter = ref("all");
  const orderManagementShopFilter = ref("all");
  const orderManagementKeyword = ref("");
  const inventoryRisks = ref<InventoryRiskView[]>([]);
  const inventoryRiskTotal = ref(0);
  const inventoryRiskStatusFilter = ref("all");
  const inventoryRiskStats = ref({
    low_stock_count: 0,
    out_of_stock_count: 0,
    issue_count: 0,
  });
  const productSalesAnalysis = ref<ProductSalesAnalysisView[]>([]);
  const productSalesAnalysisTotal = ref(0);
  const productSalesAnalysisStatusFilter = ref("all");
  const productManagementItems = ref<ProductManagementView[]>([]);
  const productManagementTotal = ref(0);
  const productManagementStatusFilter = ref("all");
  const productManagementShopFilter = ref("all");
  const productManagementKeyword = ref("");
  const productSalesAnalysisTotals = ref<ProductSalesAnalysisTotals>({
    product_count: 0,
    sold_product_count: 0,
    total_units_sold: 0,
    revenue_cents: 0,
    purchase_cost_cents: 0,
    gross_profit_cents: 0,
    missing_cost_product_count: 0,
    scale_candidate_count: 0,
    risk_product_count: 0,
  });
  const orderProfitTotals = ref<OrderProfitTotals>({
    order_count: 0,
    revenue_cents: 0,
    purchase_cost_cents: 0,
    purchase_freight_cents: 0,
    refund_cents: 0,
    aftersale_compensation_cents: 0,
    other_cost_cents: 0,
    other_income_cents: 0,
    estimated_profit_cents: 0,
    actual_profit_cents: 0,
    unknown_actual_order_count: 0,
  });
  const automationSettings = ref<OperationalAutomationSettings>(
    defaultAutomationSettings(),
  );
  const automationRunning = ref(false);
  const lastAutomationResult = ref<OperationalAutomationRunResult | null>(null);
  const deliverySettings = ref<DeliverySettings>({ auto_send_delivery: false });
  const deliveryCompanies = ref<DeliveryCompanyView[]>([]);
  const deliveryShipments = ref<ShipmentView[]>([]);
  const deliveryShipmentTotal = ref(0);
  const deliveryStatusFilter = ref("all");
  const localApiConfig = ref<LocalApiConfig>({
    enabled: true,
    host: "127.0.0.1",
    port: 17890,
    base_url: "http://127.0.0.1:17890",
    has_api_key: false,
    api_key_hint: null,
    auth_header: "x-wx-xd-api-key",
  });
  const rotatedLocalApiKey = ref("");
  const aiProviderOptions = [
    {
      value: "xiaomi",
      label: "Xiaomi MiMo",
      modelPlaceholder: "mimo-v2.5-pro",
    },
    {
      value: "xiaomi-token-plan-cn",
      label: "Xiaomi Token Plan CN",
      modelPlaceholder: "mimo-v2.5-pro",
    },
    { value: "openai", label: "OpenAI", modelPlaceholder: "gpt-5.1-mini" },
    {
      value: "anthropic",
      label: "Anthropic",
      modelPlaceholder: "claude-sonnet-4-5",
    },
    {
      value: "google",
      label: "Google Gemini",
      modelPlaceholder: "gemini-3-pro",
    },
    {
      value: "deepseek",
      label: "DeepSeek",
      modelPlaceholder: "deepseek-v4-pro",
    },
    {
      value: "openrouter",
      label: "OpenRouter",
      modelPlaceholder: "anthropic/claude-sonnet-4.5",
    },
    { value: "xai", label: "xAI", modelPlaceholder: "grok-4" },
    { value: "zai", label: "ZAI", modelPlaceholder: "glm-4.6" },
    {
      value: "kimi-coding",
      label: "Kimi For Coding",
      modelPlaceholder: "kimi-k2.5",
    },
    {
      value: "moonshotai",
      label: "Moonshot AI",
      modelPlaceholder: "kimi-k2.5",
    },
    {
      value: "mistral",
      label: "Mistral",
      modelPlaceholder: "mistral-large-latest",
    },
    { value: "groq", label: "Groq", modelPlaceholder: "openai/gpt-oss-120b" },
    { value: "custom", label: "Custom", modelPlaceholder: "mimo-v2.5-pro" },
  ];
  const aiProviderApiOptions = [
    { value: "openai-completions", label: "OpenAI Chat Completions" },
    { value: "openai-responses", label: "OpenAI Responses" },
    { value: "anthropic-messages", label: "Anthropic Messages" },
    { value: "google-generative-ai", label: "Google Generative AI" },
  ];
  const aiProviderSettings = ref<AiProviderSettings>({
    enabled: false,
    provider_type: "custom",
    custom_provider_id: "wx-xd-custom",
    api: "openai-completions",
    base_url: "",
    model: "",
    temperature: 0.1,
    context_window: 128000,
    max_tokens: 4096,
    has_api_key: false,
    api_key_hint: null,
    updated_at: null,
  });
  function aiProviderLabel(providerType: string) {
    return (
      aiProviderOptions.find((option) => option.value === providerType)
        ?.label ||
      providerType ||
      "Custom"
    );
  }
  const aiProviderRuntimeLabel = computed(() => {
    return `pi-coding-agent / ${aiProviderLabel(aiProviderSettings.value.provider_type)}`;
  });
  const aiProviderRequiresApiKey = computed(() => {
    return false;
  });
  const aiProviderCanTest = computed(() => {
    if (!aiProviderSettings.value.enabled) return false;
    if (!aiProviderSettings.value.model) return false;
    if (
      aiProviderSettings.value.provider_type === "custom" &&
      !aiProviderSettings.value.base_url
    )
      return false;
    if (aiProviderRequiresApiKey.value && !aiProviderSettings.value.has_api_key)
      return false;
    return true;
  });
  const aiProviderForm = reactive({
    enabled: false,
    provider_type: "custom",
    custom_provider_id: "wx-xd-custom",
    api: "openai-completions",
    base_url: "",
    model: "",
    temperature: "0.1",
    context_window: "128000",
    max_tokens: "4096",
    api_key: "",
    clear_api_key: false,
  });
  const aiProviderFormRuntimeLabel = computed(() => {
    return `pi-coding-agent / ${aiProviderLabel(aiProviderForm.provider_type)}`;
  });
  const aiProviderFormIsCustom = computed(
    () => aiProviderForm.provider_type === "custom",
  );
  const aiProviderFormRequiresBaseUrl = computed(
    () => aiProviderFormIsCustom.value,
  );
  const aiProviderFormRequiresApiKey = computed(() => false);
  const aiProviderModelPlaceholder = computed(() => {
    const option = aiProviderOptions.find(
      (item) => item.value === aiProviderForm.provider_type,
    );
    return `模型，例如 ${option?.modelPlaceholder || "gpt-5.1-mini"}`;
  });
  const aiProviderBaseUrlPlaceholder = computed(() => {
    if (aiProviderFormIsCustom.value)
      return "Custom Base URL，例如 http://172.29.53.205:8080/v1";
    return "可选：代理或网关 Base URL；留空使用 Pi 内置 provider 默认地址";
  });
  const aiProviderFormMatchesSavedCredential = computed(() => {
    if (aiProviderForm.provider_type !== aiProviderSettings.value.provider_type)
      return false;
    if (aiProviderForm.provider_type !== "custom") return true;
    return (
      aiProviderForm.custom_provider_id ===
      aiProviderSettings.value.custom_provider_id
    );
  });
  const aiProviderApiKeyPlaceholder = computed(() => {
    if (
      aiProviderSettings.value.has_api_key &&
      aiProviderFormMatchesSavedCredential.value
    )
      return "留空则继续使用已保存 Key";
    if (aiProviderFormIsCustom.value)
      return "API Key，可选；本地兼容网关无鉴权可留空";
    return "API Key；也可使用 Pi 支持的环境变量或 auth.json";
  });
  const aiProviderSaving = ref(false);
  const aiProviderTesting = ref(false);
  const agentSkills = ref<AgentSkillView[]>([]);
  const agentSkillsLoading = ref(false);
  const agentSkillSaving = ref("");
  const agentSkillTesting = ref("");
  const selectedAgentSkillName = ref("");
  const agentRuns = ref<AgentRunView[]>([]);
  const agentRunsLoading = ref(false);
  const agentRunSceneFilter = ref("all");
  const agentRunStatusFilter = ref("all");
  const externalApiLogs = ref<ExternalApiLogView[]>([]);
  const notifications = ref<NotificationView[]>([]);
  const notificationTotal = ref(0);
  const unreadNotificationCount = ref(0);
  const criticalNotificationCount = ref(0);
  const notificationStatusFilter = ref("unread");
  const notificationSeverityFilter = ref("all");
  const databaseBackups = ref<BackupInfo[]>([]);
  const categoryCatalogShops = ref<CategoryCatalogShopSummary[]>([]);
  const categoryCache = ref<CategoryCacheView[]>([]);
  const categoryRelations = ref<CategoryRelationView[]>([]);
  const freightTemplates = ref<FreightTemplateView[]>([]);
  const selectedCategoryShopId = ref("shop-preview");
  const categoryKeyword = ref("");
  const categoryRuleCatId = ref("");
  const isTauriRuntime = "__TAURI_INTERNALS__" in window;
  const runtimeLabel = computed(() =>
    isTauriRuntime ? "Tauri 主控机" : "浏览器预览",
  );
  const selectedAgentSkill = computed(() => {
    return (
      agentSkills.value.find(
        (skill) => skill.name === selectedAgentSkillName.value,
      ) ||
      agentSkills.value[0] ||
      null
    );
  });
  const enabledAgentSkillCount = computed(
    () => agentSkills.value.filter((skill) => skill.enabled).length,
  );
  const readyAgentSkillCount = computed(() => {
    return agentSkills.value.filter(
      (skill) =>
        skill.file_status === "ready" && skill.runtime_status === "ready",
    ).length;
  });
  const selectedCategoryShop = computed(
    () =>
      shops.value.find((shop) => shop.id === selectedCategoryShopId.value) ||
      null,
  );
  const previewGroups = ref<ShopGroup[]>(createPreviewGroups());
  const previewShops = ref<ShopListItem[]>(createPreviewShops());
  const previewTaskRuns = ref<TaskRunView[]>(createPreviewTaskRuns());
  const previewCollectionTasks = ref<CollectionTaskView[]>(
    createPreviewCollectionTasks(),
  );
  const previewShipments = ref<ShipmentView[]>(createPreviewShipments());
  const previewPurchaseTasks = ref<PurchaseTaskView[]>(
    createPreviewPurchaseTasks(),
  );
  const previewAftersales = ref<AftersaleView[]>(createPreviewAftersales());
  const previewAftersaleRejectReasons = ref<AftersaleRejectReasonView[]>(
    createPreviewAftersaleRejectReasons(),
  );
  const previewGuaranteeOrders = ref<GuaranteeOrderView[]>(
    createPreviewGuaranteeOrders(),
  );
  const previewAftersaleEvidence = ref<AftersaleEvidenceView[]>(
    createPreviewAftersaleEvidence(),
  );
  const previewSupplierAftersaleFollowups = ref<
    SupplierAftersaleFollowupView[]
  >(createPreviewSupplierAftersaleFollowups());
  const previewProfitAdjustments = ref<
    Array<{
      order_id: string;
      kind: string;
      amount_cents: number;
    }>
  >(createPreviewProfitAdjustments());
  const previewPendingOrderCount = ref(0);
  const previewLastOrderSyncAt = ref<string | null>(null);
  const previewDatabaseBackups = ref<BackupInfo[]>(
    createPreviewDatabaseBackups(),
  );
  const previewExternalApiLogs = ref<ExternalApiLogView[]>(
    createPreviewExternalApiLogs(),
  );
  const previewNotifications = ref<NotificationView[]>(
    createPreviewNotifications(),
  );
  const previewCategoryCatalogShops = ref<CategoryCatalogShopSummary[]>(
    createPreviewCategoryCatalogShops(),
  );
  const previewCategoryCache = ref<CategoryCacheView[]>(
    createPreviewCategoryCache(),
  );
  const previewCategoryRelations = ref<CategoryRelationView[]>(
    createPreviewCategoryRelations(),
  );
  const previewFreightTemplates = ref<FreightTemplateView[]>(
    createPreviewFreightTemplates(),
  );

  const {
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
    openCollectionDetail,
    openPublishPricingDialog,
    openTestCollectDialog,
    paginatedCollectionTasks,
    refreshCollectionTasks,
    refreshTaobaoAccessLimitState,
    resumeCollectionTasks,
    retryAllFailedCollections,
    retryCollection,
    deleteTaobaoProfile,
    removeCollectionImage,
    resetAllPassedReviews,
    resetSelectedCollectionReview,
    reviewSingleCollectionTask,
    reviewSelectedCollectionTasks,
    runTestCollect,
    savePublishPricingStrategyFromForm,
    searchCollectionReviewCategories,
    selectCollectionExcelFile,
    selectedCollectionDetailImages,
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
    selectedCollectionMainImages,
    selectedCollectionSkuPreview,
    selectedCollectionTasks,
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
    confirmSelectedCollectionReview,
  } = useCollectionWorkflow({
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
  });

  const {
    previewCommand,
    buildPreviewJob,
    buildPreviewPriceJob,
    buildPreviewOrderProfitResult,
    summarizeOrderProfits,
    buildPreviewInventoryRisks,
    buildPreviewProductSalesAnalysis,
    summarizeProductSalesAnalysis,
    upsertPreviewShipment,
  } = createPreviewMode({
    aiProviderSettings,
    automationSettings,
    canSubmitAftersaleAction,
    currentJob,
    currentOrderPriceAdjustmentJob,
    currentPriceJob,
    deliverySettings,
    evidenceStatusLabel,
    evidenceTypeLabel,
    isActiveGuaranteeStatus,
    latestOrderPriceAdjustmentTaskId,
    latestPriceTaskId,
    latestTaskId,
    localApiConfig,
    publishPricingStrategy,
    previewAftersaleEvidence,
    previewAftersaleRejectReasons,
    previewAftersales,
    previewCategoryCache,
    previewCategoryCatalogShops,
    previewCategoryRelations,
    previewCollectionTasks,
    previewDatabaseBackups,
    previewExternalApiLogs,
    previewFreightTemplates,
    previewGroups,
    previewGuaranteeOrders,
    previewLastOrderSyncAt,
    previewNotifications,
    previewPendingOrderCount,
    previewProfitAdjustments,
    previewPurchaseTasks,
    previewShipments,
    previewShops,
    previewSupplierAftersaleFollowups,
    previewTaskRuns,
    queriedOrderPriceAdjustmentTaskId,
    queriedPriceTaskId,
    selectedCollectionTasks,
    supplierFollowupStatusLabel,
    supplierFollowupTypeLabel,
  });

  const groupForm = reactive({ name: "" });
  const shopForm = reactive({
    name: "",
    appid: "",
    app_secret: "",
    group_id: "group-default",
  });
  const shipmentForm = reactive({
    order_id: "",
    shop_id: "shop-preview",
    wechat_order_id: "",
    delivery_id: "SF",
    waybill_id: "SF1234567890",
    deliver_type: 1,
  });
  const purchaseShipmentForm = reactive({
    purchase_task_id: "",
    delivery_id: "SF",
    waybill_id: "SF1234567890",
    deliver_type: 1,
    estimated_cost: "",
  });
  const purchaseMappingForm = reactive({
    purchase_task_id: "",
    external_product_id: "",
    external_sku_id: "",
    source_url: "",
    supplier_name: "",
    supplier_product_id: "",
    estimated_cost: "",
    note: "",
  });
  const purchaseIssueForm = reactive({
    purchase_task_id: "",
    issue_type: "out_of_stock",
    note: "",
  });
  const orderProfitAdjustmentForm = reactive({
    order_id: "",
    kind: "purchase_freight",
    amount_cents: "",
    note: "",
  });
  const aftersaleResponsibilityForm = reactive({
    aftersale_id: "",
    responsibility_party: "supplier",
    supplier_compensation_cents: "",
    responsibility_note: "",
  });
  const aftersaleActionForm = reactive({
    aftersale_id: "",
    shop_id: "",
    address_id: "",
    accept_type: "",
    reject_reason_type: "1",
    reject_reason: "",
    note: "",
  });
  const guaranteeFollowupForm = reactive({
    guarantee_order_id: "",
    handling_status: "in_progress",
    responsibility_party: "unknown",
    supplier_compensation_cents: "",
    handling_note: "",
  });
  const evidenceForm = reactive({
    target_type: "aftersale",
    target_id: "",
    evidence_type: "supplier_proof",
    title: "",
    status: "draft",
    content_text: "",
    local_file_path: "",
    source_url: "",
  });
  const supplierFollowupForm = reactive({
    target_type: "aftersale",
    target_id: "",
    followup_type: "evidence_request",
    status: "waiting_supplier",
    purchase_task_id: "",
    supplier_name: "",
    note: "",
  });

  const deliveryCompanyOptions = computed(() => {
    const options = new Map<string, { value: string; label: string }>();
    for (const company of deliveryCompanies.value) {
      options.set(company.delivery_id, {
        value: company.delivery_id,
        label: company.delivery_name,
      });
    }
    for (const company of fallbackDeliveryCompanyOptions) {
      if (!options.has(company.value)) {
        options.set(company.value, company);
      }
    }
    return Array.from(options.values());
  });

  const aftersaleRejectReasonOptions = computed(() => {
    const shopId = aftersaleActionForm.shop_id.trim();
    const reasons = shopId
      ? aftersaleRejectReasons.value.filter(
          (reason) => reason.shop_id === shopId,
        )
      : aftersaleRejectReasons.value;
    return reasons.length > 0 ? reasons : aftersaleRejectReasons.value;
  });

  const publishPayload = ref(createDefaultPublishPayload());

  const priceUpdatePayload = ref(createDefaultPriceUpdatePayload());
  const orderPriceAdjustmentPayload = ref(
    createDefaultOrderPriceAdjustmentPayload(),
  );

  async function command<T>(
    name: string,
    args?: Record<string, unknown>,
  ): Promise<T> {
    if (isTauriRuntime) {
      return invoke<T>(name, args);
    }
    return previewCommand<T>(name, args);
  }

  const selectedGroupOptions = computed(() =>
    groups.value.map((group) => ({
      label: `${group.name} (${group.shop_count})`,
      value: group.id,
    })),
  );

  async function refreshAll() {
    loading.value = true;
    try {
      const [
        dashboardResult,
        groupsResult,
        shopsResult,
        taskRunsResult,
        deliverySettingsResult,
        deliveryCompaniesResult,
        automationSettingsResult,
        publishPricingStrategyResult,
        localApiConfigResult,
        aiProviderSettingsResult,
        agentSkillsResult,
        agentRunsResult,
        externalApiLogsResult,
        backupsResult,
      ] = await Promise.all([
        command<DashboardSummary>("get_dashboard"),
        command<ShopGroup[]>("list_shop_groups"),
        command<ShopListItem[]>("list_shops"),
        command<TaskRunView[]>("list_task_runs"),
        command<DeliverySettings>("get_delivery_settings"),
        command<DeliveryCompanyView[]>("list_delivery_companies"),
        command<OperationalAutomationSettings>("get_automation_settings"),
        command<PublishPricingStrategy>("get_publish_pricing_strategy"),
        command<LocalApiConfig>("get_local_api_config"),
        command<AiProviderSettings>("get_ai_provider_settings"),
        command<AgentSkillView[]>("list_agent_skills"),
        command<AgentRunView[]>("list_agent_runs", { limit: 80 }),
        command<ExternalApiLogView[]>("list_external_api_logs", { limit: 80 }),
        command<BackupInfo[]>("list_database_backups"),
      ]);
      dashboard.value = dashboardResult;
      groups.value = groupsResult;
      shops.value = shopsResult;
      taskRuns.value = taskRunsResult;
      deliverySettings.value = deliverySettingsResult;
      deliveryCompanies.value = deliveryCompaniesResult;
      automationSettings.value = automationSettingsResult;
      publishPricingStrategy.value = publishPricingStrategyResult;
      localApiConfig.value = localApiConfigResult;
      aiProviderSettings.value = aiProviderSettingsResult;
      syncAiProviderForm(aiProviderSettingsResult);
      syncAgentSkills(agentSkillsResult);
      agentRuns.value = agentRunsResult;
      externalApiLogs.value = externalApiLogsResult;
      databaseBackups.value = backupsResult;
      if (!shopForm.group_id && groups.value.length > 0) {
        shopForm.group_id = groups.value[0].id;
      }
      if (!shipmentForm.shop_id && shops.value.length > 0) {
        shipmentForm.shop_id = shops.value[0].id;
      }
      if (
        (!selectedCategoryShopId.value ||
          !shops.value.some(
            (shop) => shop.id === selectedCategoryShopId.value,
          )) &&
        shops.value.length > 0
      ) {
        selectedCategoryShopId.value = shops.value[0].id;
      }
      await Promise.all([
        refreshNotifications(),
        refreshPurchaseTasks(),
        refreshAftersales(),
        refreshAftersaleEvidence(),
        refreshSupplierAftersaleFollowups(),
        refreshAftersaleRejectReasons(),
        refreshGuaranteeOrders(),
        refreshDeliveryCompanies(),
        refreshDeliveryShipments(),
        refreshOrderProfits(),
        refreshOrderManagementItems(),
        refreshInventoryRisks(),
        refreshProductSalesAnalysis(),
        refreshProductManagementItems(),
        refreshCategoryCatalog(),
        refreshCollectionTasks(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      loading.value = false;
    }
  }

  async function refreshNotifications() {
    const result = await command<NotificationListResult>("list_notifications", {
      status: notificationStatusFilter.value,
      severity: notificationSeverityFilter.value,
      limit: 200,
    });
    notifications.value = result.items;
    notificationTotal.value = result.total;
    unreadNotificationCount.value = result.unread_count;
    criticalNotificationCount.value = result.critical_count;
  }

  function syncAiProviderForm(settings: AiProviderSettings) {
    aiProviderForm.enabled = settings.enabled;
    aiProviderForm.provider_type = settings.provider_type || "custom";
    aiProviderForm.custom_provider_id =
      settings.custom_provider_id || "wx-xd-custom";
    aiProviderForm.api = settings.api || "openai-completions";
    aiProviderForm.base_url = settings.base_url || "";
    aiProviderForm.model = settings.model || aiProviderForm.model;
    aiProviderForm.temperature = String(settings.temperature ?? 0.1);
    aiProviderForm.context_window = String(settings.context_window ?? 128000);
    aiProviderForm.max_tokens = String(settings.max_tokens ?? 4096);
    aiProviderForm.clear_api_key = false;
    aiProviderForm.api_key = "";
  }

  function syncAgentSkills(skills: AgentSkillView[]) {
    agentSkills.value = skills;
    if (
      !selectedAgentSkillName.value ||
      !skills.some((skill) => skill.name === selectedAgentSkillName.value)
    ) {
      selectedAgentSkillName.value = skills[0]?.name || "";
    }
  }

  async function refreshAgentSkills() {
    agentSkillsLoading.value = true;
    try {
      syncAgentSkills(await command<AgentSkillView[]>("list_agent_skills"));
    } catch (error) {
      ElMessage.error(`刷新 Agent 技能失败：${error}`);
    } finally {
      agentSkillsLoading.value = false;
    }
  }

  async function refreshAgentRuns() {
    agentRunsLoading.value = true;
    try {
      agentRuns.value = await command<AgentRunView[]>("list_agent_runs", {
        scene: agentRunSceneFilter.value,
        status: agentRunStatusFilter.value,
        limit: 80,
      });
    } catch (error) {
      ElMessage.error(`刷新 Agent 运行记录失败：${error}`);
    } finally {
      agentRunsLoading.value = false;
    }
  }

  async function saveAgentSkill(
    skill: AgentSkillView,
    enabled = skill.enabled,
  ) {
    agentSkillSaving.value = skill.name;
    try {
      const request: AgentSkillSettingsRequest = {
        name: skill.name,
        enabled,
        model: skill.model,
        temperature: skill.temperature,
      };
      const updated = await command<AgentSkillView>(
        "save_agent_skill_settings",
        { request },
      );
      agentSkills.value = agentSkills.value.map((item) =>
        item.name === updated.name ? updated : item,
      );
      selectedAgentSkillName.value = updated.name;
      ElMessage.success(enabled ? "技能已启用" : "技能已停用");
    } catch (error) {
      ElMessage.error(`保存技能失败：${error}`);
      await refreshAgentSkills();
    } finally {
      agentSkillSaving.value = "";
    }
  }

  async function testAgentSkill(skill: AgentSkillView) {
    if (!aiProviderCanTest.value) {
      ElMessage.warning(
        aiProviderRequiresApiKey.value
          ? "请先在 AI Agent 设置里启用并保存 API Key"
          : "请先在 AI Agent 设置里启用可用运行时并保存配置",
      );
      return;
    }
    agentSkillTesting.value = skill.name;
    try {
      const result = await command<AgentSkillTestResult>("test_agent_skill", {
        name: skill.name,
      });
      ElMessage.success(`技能试跑成功：${result.summary}`);
      await Promise.all([refreshAgentSkills(), refreshAgentRuns()]);
    } catch (error) {
      ElMessage.error(`技能试跑失败：${error}`);
      await Promise.all([refreshAgentSkills(), refreshAgentRuns()]);
    } finally {
      agentSkillTesting.value = "";
    }
  }

  async function refreshCategoryCatalog() {
    const result = await command<CategoryCatalogListResult>(
      "list_category_catalog",
      {
        shopId: selectedCategoryShopId.value,
        keyword: categoryKeyword.value.trim() || null,
        limit: 1000,
      },
    );
    categoryCatalogShops.value = result.shops;
    categoryCache.value = result.categories;
    categoryRelations.value = result.category_relations;
    freightTemplates.value = result.freight_templates;
  }

  async function refreshPurchaseTasks() {
    const result = await command<PurchaseTaskListResult>(
      "list_purchase_tasks",
      {
        status: purchaseStatusFilter.value,
        limit: 200,
      },
    );
    purchaseTasks.value = result.items;
    purchaseTaskTotal.value = result.total;
  }

  async function refreshAftersales() {
    const result = await command<AftersaleListResult>("list_aftersales", {
      status: aftersaleStatusFilter.value,
      limit: 200,
    });
    aftersales.value = result.items;
    aftersaleTotal.value = result.total;
  }

  async function refreshAftersaleEvidence() {
    const result = await command<AftersaleEvidenceListResult>(
      "list_aftersale_evidence",
      {
        targetType:
          evidenceTargetTypeFilter.value === "all"
            ? null
            : evidenceTargetTypeFilter.value,
        targetId: evidenceTargetIdFilter.value.trim() || null,
        status:
          evidenceStatusFilter.value === "all"
            ? null
            : evidenceStatusFilter.value,
        limit: 200,
      },
    );
    aftersaleEvidence.value = result.items;
    aftersaleEvidenceTotal.value = result.total;
  }

  async function refreshSupplierAftersaleFollowups() {
    const result = await command<SupplierAftersaleFollowupListResult>(
      "list_supplier_aftersale_followups",
      {
        targetType:
          supplierFollowupTargetTypeFilter.value === "all"
            ? null
            : supplierFollowupTargetTypeFilter.value,
        targetId: supplierFollowupTargetIdFilter.value.trim() || null,
        status:
          supplierFollowupStatusFilter.value === "all"
            ? null
            : supplierFollowupStatusFilter.value,
        limit: 200,
      },
    );
    supplierAftersaleFollowups.value = result.items;
    supplierAftersaleFollowupTotal.value = result.total;
  }

  async function refreshAftersaleRejectReasons() {
    aftersaleRejectReasons.value = await command<AftersaleRejectReasonView[]>(
      "list_aftersale_reject_reasons",
    );
  }

  async function refreshGuaranteeOrders() {
    const result = await command<GuaranteeOrderListResult>(
      "list_guarantee_orders",
      {
        status: guaranteeStatusFilter.value,
        limit: 200,
      },
    );
    guaranteeOrders.value = result.items;
    guaranteeOrderTotal.value = result.total;
  }

  async function refreshDeliveryCompanies() {
    deliveryCompanies.value = await command<DeliveryCompanyView[]>(
      "list_delivery_companies",
    );
  }

  async function refreshDeliveryShipments() {
    const result = await command<ShipmentListResult>(
      "list_delivery_shipments",
      {
        status: deliveryStatusFilter.value,
        limit: 200,
      },
    );
    deliveryShipments.value = result.items;
    deliveryShipmentTotal.value = result.total;
  }

  async function refreshOrderProfits() {
    const result = await command<OrderProfitListResult>(
      "list_order_profit_summaries",
      {
        status: orderProfitStatusFilter.value,
        limit: 200,
      },
    );
    orderProfits.value = result.items;
    orderProfitTotal.value = result.total;
    orderProfitTotals.value = result.totals;
  }

  async function refreshOrderManagementItems() {
    const result = await command<OrderManagementListResult>(
      "list_order_management_items",
      {
        status: orderManagementStatusFilter.value,
        shop_id: orderManagementShopFilter.value,
        keyword: orderManagementKeyword.value,
        limit: 200,
      },
    );
    orderManagementItems.value = result.items;
    orderManagementTotal.value = result.total;
  }

  async function refreshInventoryRisks() {
    const result = await command<InventoryRiskListResult>(
      "list_inventory_risks",
      {
        status: inventoryRiskStatusFilter.value,
        limit: 200,
      },
    );
    inventoryRisks.value = result.items;
    inventoryRiskTotal.value = result.total;
    inventoryRiskStats.value = {
      low_stock_count: result.low_stock_count,
      out_of_stock_count: result.out_of_stock_count,
      issue_count: result.issue_count,
    };
  }

  async function refreshProductSalesAnalysis() {
    const result = await command<ProductSalesAnalysisListResult>(
      "list_product_sales_analysis",
      {
        status: productSalesAnalysisStatusFilter.value,
        limit: 200,
      },
    );
    productSalesAnalysis.value = result.items;
    productSalesAnalysisTotal.value = result.total;
    productSalesAnalysisTotals.value = result.totals;
  }

  async function refreshProductManagementItems() {
    const result = await command<ProductManagementListResult>(
      "list_product_management_items",
      {
        status: productManagementStatusFilter.value,
        shop_id: productManagementShopFilter.value,
        keyword: productManagementKeyword.value,
        limit: 200,
      },
    );
    productManagementItems.value = result.items;
    productManagementTotal.value = result.total;
  }

  async function runInventoryRiskScan() {
    try {
      const result = await command<InventoryRiskScanResult>(
        "run_inventory_risk_scan_once",
      );
      ElMessage.success(
        `库存扫描完成：商品 ${result.scanned_products} 个，通知 ${result.notifications_created} 条`,
      );
      await Promise.all([
        refreshInventoryRisks(),
        refreshProductSalesAnalysis(),
        refreshNotifications(),
        refreshAll(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function createGroup() {
    if (!groupForm.name.trim()) {
      ElMessage.warning("先填写店铺组名称");
      return;
    }
    try {
      await command("create_shop_group", { name: groupForm.name });
      groupForm.name = "";
      ElMessage.success("店铺组已创建");
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function createShop() {
    if (!shopForm.name.trim() || !shopForm.appid.trim()) {
      ElMessage.warning("店铺名称和 appid 都要填写");
      return;
    }
    try {
      await command("create_shop", { request: { ...shopForm } });
      shopForm.name = "";
      shopForm.appid = "";
      shopForm.app_secret = "";
      ElMessage.success("店铺已加入店铺组");
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function verifyShop(shop: ShopListItem) {
    if (!shop.has_secret) {
      ElMessage.warning("这个店铺还没有保存 app_secret");
      return;
    }
    try {
      const result = await command<ShopCredentialCheck>(
        "verify_shop_credentials",
        {
          shopId: shop.id,
          forceRefresh: false,
        },
      );
      if (result.status === "active") {
        ElMessage.success(`凭证验证成功，到期时间：${result.expires_at}`);
      } else {
        ElMessage.error(
          `凭证验证失败：${result.errmsg || result.errcode || "未知错误"}`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function syncShopBasicInfo(shop: ShopListItem) {
    if (!shop.has_secret) {
      ElMessage.warning("这个店铺还没有保存 app_secret");
      return;
    }
    try {
      const result = await command<ShopBasicInfoSyncResult>(
        "sync_shop_basic_info",
        {
          shopId: shop.id,
        },
      );
      if (result.status === "active") {
        ElMessage.success(`店铺资料已同步：${result.nickname || shop.name}`);
      } else {
        ElMessage.error(
          `资料同步失败：${result.errmsg || result.errcode || "未知错误"}`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function checkShopQuota(shop: ShopListItem) {
    if (!shop.has_secret) {
      ElMessage.warning("这个店铺还没有保存 app_secret");
      return;
    }
    try {
      const result = await command<ApiQuotaCheckResult>(
        "check_shop_api_quota",
        {
          request: {
            shop_id: shop.id,
            cgi_path: "/channels/ec/basics/info/get",
          },
        },
      );
      if (result.errcode === null) {
        ElMessage.success(`额度剩余：${result.remain ?? "-"}`);
      } else {
        ElMessage.error(`额度查询失败：${result.errmsg || result.errcode}`);
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function syncSelectedShopCategoryCatalog() {
    const shop = selectedCategoryShop.value;
    if (!shop) {
      ElMessage.warning("请先选择店铺");
      return;
    }
    if (!shop.has_secret && isTauriRuntime) {
      ElMessage.warning("这个店铺还没有保存 app_secret");
      return;
    }
    try {
      const result = await command<CategoryCatalogSyncResult>(
        "sync_shop_category_catalog",
        {
          shopId: shop.id,
        },
      );
      if (result.failed_steps.length > 0) {
        ElMessage.warning(
          `类目同步部分完成：店铺类目节点 ${result.synced_categories}，生效权限 ${result.synced_category_relations}，运费模板 ${result.synced_freight_templates}`,
        );
      } else {
        ElMessage.success(
          `类目同步完成：店铺类目节点 ${result.synced_categories}，生效权限 ${result.synced_category_relations}，运费模板 ${result.synced_freight_templates}`,
        );
      }
      await Promise.all([refreshCategoryCatalog(), refreshAll()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function syncSelectedCategoryRules(catId?: number) {
    const shop = selectedCategoryShop.value;
    const targetCatId = catId ?? Number(categoryRuleCatId.value);
    if (!shop) {
      ElMessage.warning("请先选择店铺");
      return;
    }
    if (!Number.isFinite(targetCatId) || targetCatId <= 0) {
      ElMessage.warning("请输入有效 cat_id");
      return;
    }
    if (!shop.has_secret && isTauriRuntime) {
      ElMessage.warning("这个店铺还没有保存 app_secret");
      return;
    }
    try {
      categoryRuleCatId.value = String(targetCatId);
      const result = await command<CategoryRuleSyncResult>(
        "sync_category_rules",
        {
          shopId: shop.id,
          catId: targetCatId,
        },
      );
      if (result.failed_steps.length > 0) {
        ElMessage.warning(
          `规则同步部分完成：${result.failed_steps.join("；")}`,
        );
      } else {
        ElMessage.success(
          `规则同步完成：商品属性 ${result.product_attr_count}，销售属性 ${result.sale_attr_count}`,
        );
      }
      await Promise.all([refreshCategoryCatalog(), refreshAll()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function rotateLocalApiKey() {
    try {
      const result = await command<LocalApiKeyRotationResult>(
        "rotate_local_api_key",
      );
      rotatedLocalApiKey.value = result.api_key;
      await refreshAll();
      ElMessage.success("本地 HTTP API Key 已生成，旧 Key 已失效");
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function refreshExternalApiLogs() {
    try {
      externalApiLogs.value = await command<ExternalApiLogView[]>(
        "list_external_api_logs",
        { limit: 80 },
      );
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function saveAiProviderSettings() {
    const temperature = Number(aiProviderForm.temperature || "0.1");
    if (!Number.isFinite(temperature) || temperature < 0 || temperature > 1) {
      ElMessage.warning("temperature 必须在 0 到 1 之间");
      return;
    }
    const contextWindow = Number(aiProviderForm.context_window || "128000");
    const maxTokens = Number(aiProviderForm.max_tokens || "4096");
    if (!Number.isInteger(contextWindow) || contextWindow <= 0) {
      ElMessage.warning("context_window 必须是大于 0 的整数");
      return;
    }
    if (!Number.isInteger(maxTokens) || maxTokens <= 0) {
      ElMessage.warning("max_tokens 必须是大于 0 的整数");
      return;
    }
    if (
      aiProviderForm.enabled &&
      aiProviderForm.provider_type === "custom" &&
      !aiProviderForm.base_url.trim()
    ) {
      ElMessage.warning("启用 Custom provider 前必须填写 Base URL");
      return;
    }
    if (aiProviderForm.enabled && !aiProviderForm.model.trim()) {
      ElMessage.warning("启用 AI 前必须填写模型");
      return;
    }
    aiProviderSaving.value = true;
    try {
      const result = await command<AiProviderSettings>(
        "save_ai_provider_settings",
        {
          request: {
            enabled: aiProviderForm.enabled,
            provider_type: aiProviderForm.provider_type,
            custom_provider_id: aiProviderForm.custom_provider_id.trim(),
            api: aiProviderForm.api,
            base_url: aiProviderForm.base_url.trim(),
            model: aiProviderForm.model.trim(),
            temperature,
            context_window: contextWindow,
            max_tokens: maxTokens,
            api_key: aiProviderForm.api_key.trim() || null,
            clear_api_key: aiProviderForm.clear_api_key,
          },
        },
      );
      aiProviderSettings.value = result;
      syncAiProviderForm(result);
      ElMessage.success("AI provider 配置已保存");
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      aiProviderSaving.value = false;
    }
  }

  async function testAiProvider() {
    aiProviderTesting.value = true;
    try {
      const result = await command<AiProviderTestResult>("test_ai_provider");
      ElMessage.success(result.message);
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      aiProviderTesting.value = false;
    }
  }

  async function refreshDatabaseBackups() {
    try {
      databaseBackups.value = await command<BackupInfo[]>(
        "list_database_backups",
      );
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function createDatabaseBackup() {
    if (backupRunning.value) {
      return;
    }
    backupRunning.value = true;
    try {
      const result = await command<BackupCreateResult>(
        "create_database_backup",
      );
      databaseBackups.value = await command<BackupInfo[]>(
        "list_database_backups",
      );
      ElMessage.success(`备份已创建：${result.backup.file_name}`);
      if (isTauriRuntime) {
        await revealItemInDir(result.backup.file_path);
      }
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      backupRunning.value = false;
    }
  }

  async function restoreDatabaseBackup(backup: BackupInfo) {
    if (!backup.integrity_ok) {
      ElMessage.warning("这个备份未通过完整性校验，不能恢复");
      return;
    }
    try {
      await ElMessageBox.confirm(
        `恢复会覆盖当前本地数据库。系统会先自动创建回滚备份，再从 ${backup.file_name} 恢复。`,
        "确认恢复数据库",
        {
          confirmButtonText: "恢复",
          cancelButtonText: "取消",
          type: "warning",
        },
      );
    } catch {
      return;
    }
    backupRunning.value = true;
    try {
      const result = await command<BackupRestoreResult>(
        "restore_database_backup",
        {
          request: { file_path: backup.file_path },
        },
      );
      ElMessage.success(result.message);
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      backupRunning.value = false;
    }
  }

  async function revealBackup(backup: BackupInfo) {
    if (!isTauriRuntime) {
      ElMessage.info(backup.file_path);
      return;
    }
    await revealItemInDir(backup.file_path);
  }

  async function resetCollectionPublishWorkspace() {
    if (workspaceResetRunning.value) {
      return;
    }
    const confirmText = "确认清理采集和铺货数据";
    let inputValue = "";
    try {
      const result = await ElMessageBox.prompt(
        `这会清空采集任务、铺货任务、素材记录、自动补齐记录和相关通知/日志。系统会先自动备份，店铺、密钥、微信类目缓存、运费模板、订单、售后、采购和发货数据会保留。请输入“${confirmText}”继续。`,
        "重置采集/铺货工作区",
        {
          confirmButtonText: "确认清理",
          cancelButtonText: "取消",
          inputPlaceholder: confirmText,
          inputValidator: (value) =>
            value.trim() === confirmText || `请输入：${confirmText}`,
          type: "warning",
        },
      );
      inputValue = result.value;
    } catch {
      return;
    }

    workspaceResetRunning.value = true;
    try {
      const result = await command<CollectionPublishWorkspaceResetResult>(
        "reset_collection_publish_workspace",
        {
          request: { confirm_text: inputValue },
        },
      );
      currentJob.value = null;
      latestTaskId.value = "";
      queriedTaskId.value = "";
      selectedCollectionTasks.value = [];
      ElMessage.success(`${result.message} 备份：${result.backup.file_name}`);
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      workspaceResetRunning.value = false;
    }
  }

  async function savePublishPricingStrategy(strategy: PublishPricingStrategy) {
    publishPricingSaving.value = true;
    try {
      publishPricingStrategy.value = await command<PublishPricingStrategy>(
        "save_publish_pricing_strategy",
        {
          strategy,
        },
      );
      ElMessage.success("价格策略已保存");
      return true;
    } catch (error) {
      ElMessage.error(String(error));
      try {
        publishPricingStrategy.value = await command<PublishPricingStrategy>(
          "get_publish_pricing_strategy",
        );
      } catch {
        publishPricingStrategy.value = defaultPublishPricingStrategy();
      }
      return false;
    } finally {
      publishPricingSaving.value = false;
    }
  }

  async function saveAutomationSettings() {
    try {
      automationSettings.value = await command<OperationalAutomationSettings>(
        "set_automation_settings",
        {
          settings: automationSettings.value,
        },
      );
      ElMessage.success("自动推进设置已保存");
    } catch (error) {
      ElMessage.error(String(error));
      await refreshAll();
    }
  }

  async function runOperationalAutomationOnce() {
    if (automationRunning.value) {
      return;
    }
    automationRunning.value = true;
    try {
      const result = await command<OperationalAutomationRunResult>(
        "run_operational_automation_once",
      );
      lastAutomationResult.value = result;
      if (result.errors.length > 0) {
        ElMessage.warning(
          `自动推进完成，但 ${result.errors.length} 个步骤失败，请查看错误。`,
        );
      } else if (result.executed_steps.length === 0) {
        ElMessage.info("没有启用的自动推进步骤");
      } else {
        ElMessage.success(
          `自动推进完成：执行 ${result.executed_steps.length} 步，跳过 ${result.skipped_steps.length} 步`,
        );
      }
      await Promise.all([
        refreshAll(),
        queryJob(),
        queryPriceUpdateJob(),
        queryOrderPriceAdjustmentJob(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      automationRunning.value = false;
    }
  }

  async function createPublishJob() {
    let request: unknown;
    try {
      request = JSON.parse(publishPayload.value);
    } catch {
      ElMessage.error("铺货 JSON 格式不正确");
      return;
    }
    try {
      const result = await command<PublishJobCreated>(
        "create_external_publish_job",
        { request },
      );
      latestTaskId.value = result.task_id;
      queriedTaskId.value = result.task_id;
      selectedSection.value = "publish-tasks";
      ElMessage.success(`铺货任务已创建：${result.task_id}`);
      await Promise.all([refreshAll(), queryJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function queryJob() {
    if (!queriedTaskId.value.trim()) {
      return;
    }
    try {
      currentJob.value = await command<PublishJobView>("get_publish_job", {
        taskId: queriedTaskId.value.trim(),
      });
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function createPriceUpdateJob() {
    let request: unknown;
    try {
      request = JSON.parse(priceUpdatePayload.value);
    } catch {
      ElMessage.error("商品售价 JSON 格式不正确");
      return;
    }
    try {
      const result = await command<PriceUpdateJobCreated>(
        "create_price_update_job",
        { request },
      );
      latestPriceTaskId.value = result.task_id;
      queriedPriceTaskId.value = result.task_id;
      ElMessage.success(`商品售价调整任务已创建：${result.task_id}`);
      await Promise.all([refreshAll(), queryPriceUpdateJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function queryPriceUpdateJob() {
    if (!queriedPriceTaskId.value.trim()) {
      return;
    }
    try {
      currentPriceJob.value = await command<PriceUpdateJobView>(
        "get_price_update_job",
        {
          taskId: queriedPriceTaskId.value.trim(),
        },
      );
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function createOrderPriceAdjustmentJob() {
    let request: unknown;
    try {
      request = JSON.parse(orderPriceAdjustmentPayload.value);
    } catch {
      ElMessage.error("订单改价 JSON 格式不正确");
      return;
    }
    try {
      const result = await command<OrderPriceAdjustmentJobCreated>(
        "create_order_price_adjustment_job",
        { request },
      );
      latestOrderPriceAdjustmentTaskId.value = result.task_id;
      queriedOrderPriceAdjustmentTaskId.value = result.task_id;
      ElMessage.success(`订单改价任务已创建：${result.task_id}`);
      await Promise.all([refreshAll(), queryOrderPriceAdjustmentJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function queryOrderPriceAdjustmentJob() {
    if (!queriedOrderPriceAdjustmentTaskId.value.trim()) {
      return;
    }
    try {
      currentOrderPriceAdjustmentJob.value =
        await command<OrderPriceAdjustmentJobView>(
          "get_order_price_adjustment_job",
          {
            taskId: queriedOrderPriceAdjustmentTaskId.value.trim(),
          },
        );
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runPriceUpdatePrecheckOnce() {
    try {
      const result = await command<PriceUpdatePrecheckBatchResult>(
        "run_price_update_precheck_once",
        { limit: 50 },
      );
      if (result.processed_items === 0) {
        ElMessage.info("没有待校验的商品售价任务项");
      } else {
        ElMessage.success(
          `商品售价校验完成：处理 ${result.processed_items} 项，就绪 ${result.ready_items} 项，失败 ${result.failed_items} 项`,
        );
      }
      await Promise.all([refreshAll(), queryPriceUpdateJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runPriceUpdateSubmitOnce() {
    try {
      const result = await command<PriceUpdateSubmitBatchResult>(
        "run_price_update_submit_once",
        { limit: 20 },
      );
      if (result.processed_items === 0) {
        ElMessage.info("没有待提交 updateproduct 的商品售价任务项");
      } else {
        ElMessage.success(
          `商品售价提交完成：处理 ${result.processed_items} 项，已提交 ${result.submitted_items} 项，失败 ${result.failed_items} 项`,
        );
      }
      await Promise.all([refreshAll(), queryPriceUpdateJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runPriceUpdateConfirmOnce() {
    try {
      const result = await command<PriceUpdateConfirmBatchResult>(
        "run_price_update_confirm_once",
        { limit: 50 },
      );
      if (result.processed_items === 0) {
        ElMessage.info("没有待确认价格的商品售价任务项");
      } else {
        ElMessage.success(
          `商品售价确认完成：处理 ${result.processed_items} 项，已确认 ${result.confirmed_items} 项，待生效 ${result.pending_items} 项，失败 ${result.failed_items} 项`,
        );
      }
      await Promise.all([refreshAll(), queryPriceUpdateJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runOrderPriceAdjustmentOnce() {
    try {
      const result = await command<OrderPriceAdjustmentBatchResult>(
        "run_order_price_adjustment_once",
        { limit: 20 },
      );
      if (result.processed_items === 0) {
        ElMessage.info("没有待提交的订单改价任务");
      } else {
        ElMessage.success(
          `订单改价提交完成：处理 ${result.processed_items} 单，成功 ${result.success_items} 单，失败 ${result.failed_items} 单`,
        );
      }
      await Promise.all([refreshAll(), queryOrderPriceAdjustmentJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runOrderSyncOnce() {
    try {
      const result = await command<OrderSyncBatchResult>(
        "run_order_sync_once",
        {
          lookbackDays: 1,
          pageSize: 100,
        },
      );
      if (result.processed_shops === 0) {
        ElMessage.info("没有可同步订单的 active 店铺");
      } else {
        ElMessage.success(
          `订单同步完成：店铺 ${result.processed_shops} 个，待发货订单 ${result.synced_orders} 个，失败店铺 ${result.failed_shops} 个`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runUnpaidOrderSyncOnce() {
    try {
      const result = await command<OrderSyncBatchResult>(
        "run_order_sync_once",
        {
          lookbackDays: 1,
          pageSize: 100,
          orderStatus: 10,
        },
      );
      if (result.processed_shops === 0) {
        ElMessage.info("没有可同步订单的 active 店铺");
      } else {
        ElMessage.success(
          `待付款订单同步完成：店铺 ${result.processed_shops} 个，订单 ${result.synced_orders} 个，失败店铺 ${result.failed_shops} 个`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runOrderDetailSyncOnce() {
    try {
      const result = await command<OrderDetailSyncBatchResult>(
        "run_order_detail_sync_once",
        { limit: 50 },
      );
      if (result.processed_orders === 0) {
        ElMessage.info("没有待同步详情的订单");
      } else {
        ElMessage.success(
          `订单详情同步完成：订单 ${result.synced_orders} 个，订单项 ${result.created_items} 个，失败 ${result.failed_orders} 个`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runAftersaleSyncOnce() {
    try {
      const result = await command<AftersaleSyncBatchResult>(
        "run_aftersale_sync_once",
        {
          lookbackHours: 24,
          limit: 200,
        },
      );
      if (result.processed_shops === 0) {
        ElMessage.info("没有可同步售后的 active 店铺");
      } else {
        ElMessage.success(
          `售后同步完成：店铺 ${result.processed_shops} 个，售后单 ${result.synced_aftersales} 个，失败店铺 ${result.failed_shops} 个，失败售后 ${result.failed_aftersales} 个`,
        );
      }
      await Promise.all([
        refreshAll(),
        refreshAftersales(),
        refreshOrderProfits(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runGuaranteeSyncOnce() {
    try {
      const result = await command<GuaranteeSyncBatchResult>(
        "run_guarantee_sync_once",
        {
          lookbackHours: 24,
          limit: 200,
        },
      );
      if (result.processed_shops === 0) {
        ElMessage.info("没有可同步纠纷单的 active 店铺");
      } else {
        ElMessage.success(
          `纠纷单同步完成：店铺 ${result.processed_shops} 个，纠纷单 ${result.synced_guarantees} 个，失败店铺 ${result.failed_shops} 个，失败纠纷 ${result.failed_guarantees} 个`,
        );
      }
      await Promise.all([
        refreshAll(),
        refreshGuaranteeOrders(),
        refreshNotifications(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  function selectAftersaleResponsibility(row: AftersaleView) {
    aftersaleResponsibilityForm.aftersale_id = row.id;
    aftersaleResponsibilityForm.responsibility_party =
      row.responsibility_party || "supplier";
    aftersaleResponsibilityForm.supplier_compensation_cents =
      row.supplier_compensation_cents
        ? String(row.supplier_compensation_cents)
        : "";
    aftersaleResponsibilityForm.responsibility_note =
      row.responsibility_note || row.reason || "";
  }

  function selectAftersaleAction(row: AftersaleView) {
    aftersaleActionForm.aftersale_id = row.id;
    aftersaleActionForm.shop_id = row.shop_id;
    aftersaleActionForm.address_id = "";
    aftersaleActionForm.accept_type = "";
    aftersaleActionForm.reject_reason_type =
      aftersaleActionForm.reject_reason_type || "1";
    aftersaleActionForm.reject_reason = row.reason || "";
    aftersaleActionForm.note = row.reason || "";
  }

  function selectGuaranteeFollowup(row: GuaranteeOrderView) {
    guaranteeFollowupForm.guarantee_order_id = row.id;
    guaranteeFollowupForm.handling_status =
      row.handling_status || "in_progress";
    guaranteeFollowupForm.responsibility_party =
      row.responsibility_party || "unknown";
    guaranteeFollowupForm.supplier_compensation_cents =
      row.supplier_compensation_cents
        ? String(row.supplier_compensation_cents)
        : "";
    guaranteeFollowupForm.handling_note =
      row.handling_note || row.apply_reason || row.merchant_refuse_reason || "";
  }

  function selectEvidenceTarget(
    targetType: "aftersale" | "guarantee",
    row: AftersaleView | GuaranteeOrderView,
  ) {
    evidenceForm.target_type = targetType;
    evidenceForm.target_id = row.id;
    evidenceTargetTypeFilter.value = targetType;
    evidenceTargetIdFilter.value = row.id;
    if (targetType === "guarantee") {
      const guarantee = row as GuaranteeOrderView;
      evidenceForm.title = evidenceForm.title || "纠纷举证资料";
      evidenceForm.content_text =
        guarantee.apply_reason || guarantee.handling_note || "";
    } else {
      const aftersale = row as AftersaleView;
      evidenceForm.title = evidenceForm.title || "售后处理凭证";
      evidenceForm.content_text =
        aftersale.reason || aftersale.responsibility_note || "";
    }
    void refreshAftersaleEvidence();
    ElMessage.info("已切换到当前单据的凭证列表");
  }

  function clearEvidenceTargetFilter() {
    evidenceTargetIdFilter.value = "";
    void refreshAftersaleEvidence();
  }

  function selectSupplierFollowupTarget(
    targetType: "aftersale" | "guarantee",
    row: AftersaleView | GuaranteeOrderView,
  ) {
    supplierFollowupForm.target_type = targetType;
    supplierFollowupForm.target_id = row.id;
    supplierFollowupTargetTypeFilter.value = targetType;
    supplierFollowupTargetIdFilter.value = row.id;
    if (targetType === "guarantee") {
      const guarantee = row as GuaranteeOrderView;
      supplierFollowupForm.note =
        supplierFollowupForm.note ||
        guarantee.handling_note ||
        guarantee.apply_reason ||
        "";
    } else {
      const aftersale = row as AftersaleView;
      supplierFollowupForm.note =
        supplierFollowupForm.note ||
        aftersale.responsibility_note ||
        aftersale.reason ||
        "";
    }
    void refreshSupplierAftersaleFollowups();
    ElMessage.info("已切换到当前单据的供应商协同记录");
  }

  function clearSupplierFollowupTargetFilter() {
    supplierFollowupTargetIdFilter.value = "";
    void refreshSupplierAftersaleFollowups();
  }

  function applyAftersaleRejectReason(reasonType: string | number) {
    const type = Number(reasonType);
    if (!Number.isInteger(type) || type <= 0) {
      return;
    }
    const reason =
      aftersaleRejectReasonOptions.value.find(
        (item) => item.reject_reason_type === type,
      ) ||
      aftersaleRejectReasons.value.find(
        (item) => item.reject_reason_type === type,
      );
    if (!reason) {
      return;
    }
    aftersaleActionForm.reject_reason_type = String(reason.reject_reason_type);
    aftersaleActionForm.reject_reason = reason.reject_reason;
  }

  async function syncAftersaleRejectReasons() {
    const shopId =
      aftersaleActionForm.shop_id.trim() ||
      aftersales.value[0]?.shop_id ||
      shops.value[0]?.id ||
      "";
    if (!shopId) {
      ElMessage.warning("先添加店铺或选择一条售后单，再同步拒绝原因");
      return;
    }
    try {
      const result = await command<AftersaleRejectReasonSyncResult>(
        "sync_aftersale_reject_reasons",
        {
          shopId,
        },
      );
      if (result.failed_steps.length > 0) {
        ElMessage.warning(result.failed_steps.join("；"));
      } else {
        ElMessage.success(`售后拒绝原因已同步：${result.synced_reasons} 条`);
      }
      aftersaleActionForm.shop_id = shopId;
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function recordGuaranteeFollowup() {
    if (!guaranteeFollowupForm.guarantee_order_id.trim()) {
      ElMessage.warning("先选择或填写纠纷单 ID");
      return;
    }
    const compensation =
      guaranteeFollowupForm.supplier_compensation_cents.trim()
        ? Number(guaranteeFollowupForm.supplier_compensation_cents)
        : 0;
    if (
      !Number.isFinite(compensation) ||
      compensation < 0 ||
      !Number.isInteger(compensation)
    ) {
      ElMessage.warning("供应商赔付金额必须是非负整数，单位分");
      return;
    }
    if (
      compensation > 0 &&
      guaranteeFollowupForm.responsibility_party !== "supplier"
    ) {
      ElMessage.warning("只有供应商责任才能记录纠纷供应商赔付金额");
      return;
    }
    try {
      const result = await command<GuaranteeFollowupResult>(
        "record_guarantee_followup",
        {
          request: {
            guarantee_order_id: guaranteeFollowupForm.guarantee_order_id.trim(),
            handling_status: guaranteeFollowupForm.handling_status,
            responsibility_party:
              guaranteeFollowupForm.responsibility_party || null,
            handling_note: guaranteeFollowupForm.handling_note.trim() || null,
            supplier_compensation_cents: compensation,
          },
        },
      );
      ElMessage.success(result.message);
      await Promise.all([
        refreshGuaranteeOrders(),
        refreshOrderProfits(),
        refreshNotifications(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function recordAftersaleEvidence() {
    if (!evidenceForm.target_id.trim()) {
      ElMessage.warning("先选择或填写售后/纠纷目标 ID");
      return;
    }
    if (!evidenceForm.title.trim()) {
      ElMessage.warning("凭证标题不能为空");
      return;
    }
    if (
      !evidenceForm.content_text.trim() &&
      !evidenceForm.local_file_path.trim() &&
      !evidenceForm.source_url.trim()
    ) {
      ElMessage.warning("凭证说明、本地文件路径和来源链接至少填写一项");
      return;
    }
    try {
      const result = await command<AftersaleEvidenceRecordResult>(
        "record_aftersale_evidence",
        {
          request: {
            target_type: evidenceForm.target_type,
            target_id: evidenceForm.target_id.trim(),
            evidence_type: evidenceForm.evidence_type,
            title: evidenceForm.title.trim(),
            content_text: evidenceForm.content_text.trim() || null,
            local_file_path: evidenceForm.local_file_path.trim() || null,
            source_url: evidenceForm.source_url.trim() || null,
            status: evidenceForm.status,
          },
        },
      );
      ElMessage.success(result.message);
      evidenceForm.title = "";
      evidenceForm.content_text = "";
      evidenceForm.local_file_path = "";
      evidenceForm.source_url = "";
      await Promise.all([
        refreshAftersaleEvidence(),
        refreshAftersales(),
        refreshGuaranteeOrders(),
        refreshNotifications(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function recordSupplierAftersaleFollowup() {
    if (!supplierFollowupForm.target_id.trim()) {
      ElMessage.warning("先选择或填写售后/纠纷目标 ID");
      return;
    }
    if (!supplierFollowupForm.note.trim()) {
      ElMessage.warning("供应商协同备注不能为空");
      return;
    }
    try {
      const result = await command<SupplierAftersaleFollowupRecordResult>(
        "record_supplier_aftersale_followup",
        {
          request: {
            target_type: supplierFollowupForm.target_type,
            target_id: supplierFollowupForm.target_id.trim(),
            followup_type: supplierFollowupForm.followup_type,
            status: supplierFollowupForm.status,
            note: supplierFollowupForm.note.trim(),
            purchase_task_id:
              supplierFollowupForm.purchase_task_id.trim() || null,
            supplier_name: supplierFollowupForm.supplier_name.trim() || null,
          },
        },
      );
      ElMessage.success(result.message);
      supplierFollowupForm.note = "";
      await Promise.all([
        refreshSupplierAftersaleFollowups(),
        refreshNotifications(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function updateAftersaleEvidenceStatus(
    row: AftersaleEvidenceView,
    status: string,
  ) {
    try {
      const result = await command<AftersaleEvidenceStatusUpdateResult>(
        "update_aftersale_evidence_status",
        {
          request: {
            evidence_id: row.id,
            status,
          },
        },
      );
      ElMessage.success(result.message);
      await Promise.all([refreshAftersaleEvidence(), refreshNotifications()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function exportAftersaleEvidence(
    targetType?: "aftersale" | "guarantee",
    targetId?: string,
    targetLabel?: string,
  ) {
    try {
      const filteredTargetId = evidenceTargetIdFilter.value.trim() || null;
      const result = await command<AftersaleEvidenceExportResult>(
        "export_aftersale_evidence",
        {
          targetType:
            targetType ??
            (evidenceTargetTypeFilter.value === "all"
              ? null
              : evidenceTargetTypeFilter.value),
          targetId: targetId ?? filteredTargetId,
          status: targetId
            ? null
            : evidenceStatusFilter.value === "all"
              ? null
              : evidenceStatusFilter.value,
          format: evidenceExportFormat.value,
        },
      );
      evidenceExportPath.value = result.file_path;
      ElMessage.success(
        `${targetLabel ? `${targetLabel}：` : ""}已导出 ${result.exported_count} 条凭证资料`,
      );
      if (isTauriRuntime) {
        await revealItemInDir(result.file_path);
      }
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function recordAftersaleResponsibility() {
    if (!aftersaleResponsibilityForm.aftersale_id.trim()) {
      ElMessage.warning("先选择或填写售后单 ID");
      return;
    }
    const compensation =
      aftersaleResponsibilityForm.supplier_compensation_cents.trim()
        ? Number(aftersaleResponsibilityForm.supplier_compensation_cents)
        : 0;
    if (
      !Number.isFinite(compensation) ||
      compensation < 0 ||
      !Number.isInteger(compensation)
    ) {
      ElMessage.warning("供应商赔付金额必须是非负整数，单位分");
      return;
    }
    if (
      compensation > 0 &&
      aftersaleResponsibilityForm.responsibility_party !== "supplier"
    ) {
      ElMessage.warning("只有供应商责任才能记录供应商赔付金额");
      return;
    }
    try {
      const result = await command<AftersaleResponsibilityResult>(
        "record_aftersale_responsibility",
        {
          request: {
            aftersale_id: aftersaleResponsibilityForm.aftersale_id.trim(),
            responsibility_party:
              aftersaleResponsibilityForm.responsibility_party,
            responsibility_note:
              aftersaleResponsibilityForm.responsibility_note.trim() || null,
            supplier_compensation_cents: compensation,
          },
        },
      );
      ElMessage.success(result.message);
      await Promise.all([
        refreshAftersales(),
        refreshOrderProfits(),
        refreshNotifications(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function submitAftersaleAccept() {
    if (!aftersaleActionForm.aftersale_id.trim()) {
      ElMessage.warning("先选择或填写售后单 ID");
      return;
    }
    const acceptType = aftersaleActionForm.accept_type
      ? Number(aftersaleActionForm.accept_type)
      : null;
    if (
      acceptType !== null &&
      (![1, 2].includes(acceptType) || !Number.isInteger(acceptType))
    ) {
      ElMessage.warning("同意类型只能选择同意退货或同意退款");
      return;
    }
    try {
      const result = await command<AftersaleActionResult>("accept_aftersale", {
        request: {
          aftersale_id: aftersaleActionForm.aftersale_id.trim(),
          address_id: aftersaleActionForm.address_id.trim() || null,
          accept_type: acceptType,
          note: aftersaleActionForm.note.trim() || null,
        },
      });
      if (result.status === "success") {
        ElMessage.success(result.message);
      } else {
        ElMessage.warning(result.message);
      }
      await Promise.all([
        refreshAftersales(),
        refreshNotifications(),
        refreshDashboardOnly(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function submitAftersaleReject() {
    if (!aftersaleActionForm.aftersale_id.trim()) {
      ElMessage.warning("先选择或填写售后单 ID");
      return;
    }
    const rejectReasonType = Number(aftersaleActionForm.reject_reason_type);
    if (!Number.isInteger(rejectReasonType) || rejectReasonType <= 0) {
      ElMessage.warning("拒绝原因类型必须是正整数");
      return;
    }
    const selectedReason = aftersaleRejectReasonOptions.value.find(
      (item) => item.reject_reason_type === rejectReasonType,
    );
    try {
      const result = await command<AftersaleActionResult>("reject_aftersale", {
        request: {
          aftersale_id: aftersaleActionForm.aftersale_id.trim(),
          reject_reason_type: rejectReasonType,
          reject_reason:
            aftersaleActionForm.reject_reason.trim() ||
            selectedReason?.reject_reason ||
            null,
          note: aftersaleActionForm.note.trim() || null,
        },
      });
      if (result.status === "success") {
        ElMessage.success(result.message);
      } else {
        ElMessage.warning(result.message);
      }
      await Promise.all([
        refreshAftersales(),
        refreshNotifications(),
        refreshDashboardOnly(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runPurchaseTaskGenerationOnce() {
    try {
      const result = await command<PurchaseTaskBatchResult>(
        "run_purchase_task_generation_once",
        { limit: 100 },
      );
      if (result.processed_items === 0) {
        ElMessage.info("没有待生成采购任务的订单项");
      } else {
        ElMessage.success(
          `采购任务生成完成：处理 ${result.processed_items} 项，生成 ${result.created_tasks} 项，跳过 ${result.skipped_items} 项`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function exportPurchaseTasks() {
    try {
      const result = await command<PurchaseTaskExportResult>(
        "export_purchase_tasks",
        {
          status: purchaseStatusFilter.value,
        },
      );
      purchaseExportPath.value = result.file_path;
      ElMessage.success(`已导出 ${result.exported_count} 条采购任务`);
      if (isTauriRuntime) {
        await revealItemInDir(result.file_path);
      }
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function exportSupplierAgentTasks() {
    try {
      const result = await command<SupplierAgentExportResult>(
        "export_supplier_agent_tasks",
        {
          status: purchaseStatusFilter.value,
          format: supplierAgentExportFormat.value,
        },
      );
      supplierAgentExportPath.value = result.file_path;
      ElMessage.success(`已导出 ${result.exported_count} 条供应商 agent 任务`);
      await refreshAgentRuns();
      if (isTauriRuntime) {
        await revealItemInDir(result.file_path);
      }
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function fillSupplierAgentTemplate() {
    try {
      supplierAgentApplyText.value = await command<string>(
        "get_supplier_agent_result_template",
      );
      supplierAgentApplyResult.value = null;
      ElMessage.success("已填入供应商 agent 结果模板");
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function applySupplierAgentResults() {
    if (!supplierAgentApplyText.value.trim()) {
      ElMessage.warning("先粘贴供应商 agent 结果 JSON/JSONL");
      return;
    }
    try {
      const result = await command<SupplierAgentApplyResult>(
        "apply_supplier_agent_results",
        {
          request: {
            raw_results: supplierAgentApplyText.value,
            dry_run: supplierAgentDryRun.value,
            continue_on_error: supplierAgentContinueOnError.value,
          },
        },
      );
      supplierAgentApplyResult.value = result;
      if (result.failed > 0) {
        ElMessage.warning(
          `处理 ${result.processed} 条，失败 ${result.failed} 条`,
        );
        await refreshAgentRuns();
      } else if (result.dry_run) {
        ElMessage.success(`干跑通过 ${result.succeeded} 条`);
        await refreshAgentRuns();
      } else {
        ElMessage.success(`已写回 ${result.succeeded} 条供应商 agent 结果`);
        await refreshAll();
      }
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  function selectPurchaseTaskMapping(task: PurchaseTaskView) {
    purchaseMappingForm.purchase_task_id = task.id;
    purchaseMappingForm.external_product_id = task.external_product_id || "";
    purchaseMappingForm.external_sku_id = task.external_sku_id || "";
    purchaseMappingForm.source_url = task.source_url || "";
    purchaseMappingForm.supplier_name = task.supplier_name || "";
    purchaseMappingForm.supplier_product_id = task.supplier_product_id || "";
    purchaseMappingForm.estimated_cost =
      task.estimated_cost !== null && task.estimated_cost !== undefined
        ? String(task.estimated_cost)
        : "";
    purchaseMappingForm.note = task.error_summary || "";
  }

  async function resolvePurchaseTaskMapping() {
    if (!purchaseMappingForm.purchase_task_id.trim()) {
      ElMessage.warning("先选择或填写采购任务 ID");
      return;
    }
    if (
      !purchaseMappingForm.external_product_id.trim() ||
      !purchaseMappingForm.external_sku_id.trim()
    ) {
      ElMessage.warning("外部商品 ID 和外部 SKU 必填");
      return;
    }
    const estimatedCost = purchaseMappingForm.estimated_cost.trim()
      ? Number(purchaseMappingForm.estimated_cost)
      : null;
    if (
      estimatedCost !== null &&
      (!Number.isFinite(estimatedCost) || estimatedCost < 0)
    ) {
      ElMessage.warning("采购成本必须是非负数字");
      return;
    }
    try {
      const result = await command<PurchaseTaskMappingResult>(
        "resolve_purchase_task_mapping",
        {
          request: {
            purchase_task_id: purchaseMappingForm.purchase_task_id.trim(),
            external_product_id: purchaseMappingForm.external_product_id.trim(),
            external_sku_id: purchaseMappingForm.external_sku_id.trim(),
            source_url: purchaseMappingForm.source_url.trim() || null,
            supplier_name: purchaseMappingForm.supplier_name.trim() || null,
            supplier_product_id:
              purchaseMappingForm.supplier_product_id.trim() || null,
            estimated_cost: estimatedCost,
            note: purchaseMappingForm.note.trim() || null,
          },
        },
      );
      ElMessage.success(result.message);
      await Promise.all([
        refreshPurchaseTasks(),
        refreshNotifications(),
        refreshDashboardOnly(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  function selectPurchaseTaskShipment(task: PurchaseTaskView) {
    purchaseShipmentForm.purchase_task_id = task.id;
    if (task.supplier_delivery_id) {
      purchaseShipmentForm.delivery_id = task.supplier_delivery_id;
    }
    if (task.supplier_waybill_id) {
      purchaseShipmentForm.waybill_id = task.supplier_waybill_id;
    }
    if (task.supplier_deliver_type) {
      purchaseShipmentForm.deliver_type = task.supplier_deliver_type;
    }
    if (task.estimated_cost !== null && task.estimated_cost !== undefined) {
      purchaseShipmentForm.estimated_cost = String(task.estimated_cost);
    }
  }

  function selectPurchaseTaskIssue(task: PurchaseTaskView) {
    purchaseIssueForm.purchase_task_id = task.id;
    if (task.status === "supplier_price_changed") {
      purchaseIssueForm.issue_type = "price_changed";
    } else if (task.status === "supplier_cancelled") {
      purchaseIssueForm.issue_type = "supplier_cancelled";
    } else if (task.status === "supplier_quality_risk") {
      purchaseIssueForm.issue_type = "quality_risk";
    } else if (task.status === "supplier_exception") {
      purchaseIssueForm.issue_type = "other";
    } else {
      purchaseIssueForm.issue_type = "out_of_stock";
    }
    purchaseIssueForm.note = task.error_summary || "";
  }

  async function markPurchaseTaskIssue() {
    if (!purchaseIssueForm.purchase_task_id.trim()) {
      ElMessage.warning("先选择或填写采购任务 ID");
      return;
    }
    try {
      const result = await command<PurchaseTaskIssueResult>(
        "mark_purchase_task_issue",
        {
          request: {
            purchase_task_id: purchaseIssueForm.purchase_task_id.trim(),
            issue_type: purchaseIssueForm.issue_type,
            note: purchaseIssueForm.note.trim() || null,
          },
        },
      );
      ElMessage.success(result.message);
      await Promise.all([
        refreshPurchaseTasks(),
        refreshNotifications(),
        refreshDashboardOnly(),
      ]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  function selectOrderProfitAdjustment(row: OrderProfitView) {
    orderProfitAdjustmentForm.order_id = row.order_id;
  }

  async function recordOrderProfitAdjustment() {
    if (!orderProfitAdjustmentForm.order_id.trim()) {
      ElMessage.warning("先选择或填写订单 ID");
      return;
    }
    const amountCents = Number(orderProfitAdjustmentForm.amount_cents);
    if (!Number.isInteger(amountCents) || amountCents < 0) {
      ElMessage.warning("金额必须是非负整数，单位为分");
      return;
    }
    try {
      const result = await command<OrderProfitAdjustmentResult>(
        "record_order_profit_adjustment",
        {
          request: {
            order_id: orderProfitAdjustmentForm.order_id.trim(),
            kind: orderProfitAdjustmentForm.kind,
            amount_cents: amountCents,
            note: orderProfitAdjustmentForm.note.trim() || null,
          },
        },
      );
      ElMessage.success(result.message);
      orderProfitAdjustmentForm.amount_cents = "";
      orderProfitAdjustmentForm.note = "";
      await refreshOrderProfits();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function recordPurchaseTaskShipment() {
    if (!purchaseShipmentForm.purchase_task_id.trim()) {
      ElMessage.warning("先选择或填写采购任务 ID");
      return;
    }
    if (
      purchaseShipmentForm.deliver_type === 1 &&
      (!purchaseShipmentForm.delivery_id.trim() ||
        !purchaseShipmentForm.waybill_id.trim())
    ) {
      ElMessage.warning("自寄快递必须填写快递公司和快递单号");
      return;
    }
    const selectedCompany = deliveryCompanyOptions.value.find(
      (item) => item.value === purchaseShipmentForm.delivery_id,
    );
    const estimatedCost = purchaseShipmentForm.estimated_cost.trim()
      ? Number(purchaseShipmentForm.estimated_cost)
      : null;
    if (
      estimatedCost !== null &&
      (!Number.isFinite(estimatedCost) || estimatedCost < 0)
    ) {
      ElMessage.warning("采购成本必须是非负数字");
      return;
    }
    try {
      const result = await command<PurchaseTaskShipmentResult>(
        "record_purchase_task_shipment",
        {
          request: {
            purchase_task_id: purchaseShipmentForm.purchase_task_id.trim(),
            delivery_id:
              purchaseShipmentForm.deliver_type === 1
                ? purchaseShipmentForm.delivery_id
                : null,
            delivery_name:
              purchaseShipmentForm.deliver_type === 1
                ? selectedCompany?.label || null
                : null,
            waybill_id:
              purchaseShipmentForm.deliver_type === 1
                ? purchaseShipmentForm.waybill_id.trim()
                : null,
            deliver_type: purchaseShipmentForm.deliver_type,
            estimated_cost: estimatedCost,
          },
        },
      );
      ElMessage.success(result.message);
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function setAutoSendDelivery(value: boolean | string | number) {
    try {
      const enabled = Boolean(value);
      deliverySettings.value = await command<DeliverySettings>(
        "set_auto_send_delivery",
        { enabled },
      );
      ElMessage.success(enabled ? "自动微信发货已开启" : "自动微信发货已关闭");
      await refreshAll();
    } catch (error) {
      deliverySettings.value.auto_send_delivery =
        !deliverySettings.value.auto_send_delivery;
      ElMessage.error(String(error));
    }
  }

  async function syncDeliveryCompanies() {
    const shopId = shipmentForm.shop_id.trim() || shops.value[0]?.id || "";
    if (!shopId) {
      ElMessage.warning("先添加店铺，再同步快递公司");
      return;
    }
    try {
      const result = await command<DeliveryCompanySyncResult>(
        "sync_delivery_companies",
        {
          shopId,
          ewaybillOnly: false,
        },
      );
      if (result.failed_steps.length > 0) {
        ElMessage.warning(result.failed_steps.join("；"));
      } else {
        ElMessage.success(`快递公司已同步：${result.synced_companies} 家`);
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function recordOrderShipment() {
    const selectedCompany = deliveryCompanyOptions.value.find(
      (item) => item.value === shipmentForm.delivery_id,
    );
    if (
      !shipmentForm.order_id.trim() &&
      (!shipmentForm.shop_id.trim() || !shipmentForm.wechat_order_id.trim())
    ) {
      ElMessage.warning("填写本地订单 ID，或填写店铺 ID + 微信订单号");
      return;
    }
    if (
      shipmentForm.deliver_type === 1 &&
      (!shipmentForm.delivery_id.trim() || !shipmentForm.waybill_id.trim())
    ) {
      ElMessage.warning("自寄快递必须填写快递公司和快递单号");
      return;
    }
    try {
      const result = await command<ShipmentRecordResult>(
        "record_order_shipment",
        {
          request: {
            order_id: shipmentForm.order_id.trim() || null,
            shop_id: shipmentForm.shop_id.trim() || null,
            wechat_order_id: shipmentForm.wechat_order_id.trim() || null,
            delivery_id:
              shipmentForm.deliver_type === 1 ? shipmentForm.delivery_id : null,
            delivery_name:
              shipmentForm.deliver_type === 1
                ? selectedCompany?.label || null
                : null,
            waybill_id:
              shipmentForm.deliver_type === 1
                ? shipmentForm.waybill_id.trim()
                : null,
            deliver_type: shipmentForm.deliver_type,
          },
        },
      );
      ElMessage.success(result.message);
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function runDeliverySubmissionOnce() {
    try {
      const result = await command<DeliverySubmitBatchResult>(
        "run_delivery_submission_once",
        { limit: 20 },
      );
      if (result.processed_shipments === 0) {
        ElMessage.info(
          deliverySettings.value.auto_send_delivery
            ? "没有待提交微信发货的物流单"
            : "自动发货开关关闭，未提交微信发货",
        );
      } else {
        ElMessage.success(
          `微信发货完成：处理 ${result.processed_shipments} 单，成功 ${result.submitted_shipments} 单，失败 ${result.failed_shipments} 单`,
        );
      }
      await refreshAll();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function retryDeliveryShipment(shipment: ShipmentView) {
    try {
      const result = await command<ShipmentRetryResult>(
        "retry_delivery_shipment",
        {
          shipmentId: shipment.id,
        },
      );
      ElMessage.success(result.message);
      await refreshDeliveryShipments();
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function resumePublishFailuresOnce() {
    if (publishRetryRunning.value) {
      return;
    }
    publishRetryRunning.value = true;
    try {
      const result = await command<PublishPipelineRunResult>(
        "run_publish_pipeline_once",
      );
      const stepResults = [
        result.publish_precheck,
        result.publish_attribute_fill,
        result.publish_category_precheck,
        result.publish_asset_upload,
        result.publish_submit,
        result.publish_status_sync,
        result.publish_listing,
      ].filter(Boolean) as Array<{
        processed_items?: number;
        failed_items?: number;
      }>;
      const processedItems = stepResults.reduce(
        (sum, stepResult) => sum + Number(stepResult.processed_items || 0),
        0,
      );
      const failedItems = stepResults.reduce(
        (sum, stepResult) => sum + Number(stepResult.failed_items || 0),
        0,
      );
      if (result.errors.length > 0) {
        ElMessage.warning(
          `铺货推进完成，但 ${result.errors.length} 个步骤异常，请看当前任务异常原因`,
        );
      } else if (processedItems === 0) {
        ElMessage.info("没有待推进的铺货任务项");
      } else if (failedItems > 0) {
        ElMessage.warning(
          `已推进 ${processedItems} 项，其中 ${failedItems} 项进入异常`,
        );
      } else {
        ElMessage.success(`已推进 ${processedItems} 个铺货任务项`);
      }
      await Promise.all([refreshAll(), queryJob()]);
    } catch (error) {
      ElMessage.error(String(error));
    } finally {
      publishRetryRunning.value = false;
    }
  }

  async function openTask(task: TaskRunView) {
    if (task.task_type.startsWith("aftersales.")) {
      selectedSection.value = "exceptions";
      await Promise.all([refreshAftersales(), refreshGuaranteeOrders()]);
      return;
    }
    if (task.task_type.startsWith("inventory.")) {
      selectedSection.value = "analytics";
      await refreshInventoryRisks();
      return;
    }
    if (task.task_type.startsWith("price.")) {
      queriedPriceTaskId.value = task.id;
      selectedSection.value = "price";
      await queryPriceUpdateJob();
      return;
    }
    if (task.task_type === "orders.change_order_price") {
      queriedOrderPriceAdjustmentTaskId.value = task.id;
      selectedSection.value = "price";
      await queryOrderPriceAdjustmentJob();
      return;
    }
    if (
      task.task_type.startsWith("orders.") ||
      task.task_type.startsWith("procurement.") ||
      task.task_type.startsWith("delivery.")
    ) {
      selectedSection.value = "procurement";
      await Promise.all([refreshPurchaseTasks(), refreshDeliveryShipments()]);
      return;
    }
    if (task.task_type.startsWith("collection.")) {
      selectedSection.value = "publish";
      await refreshCollectionTasks();
      return;
    }
    if (task.task_type.startsWith("publish.")) {
      selectedSection.value = "publish-tasks";
      if (currentJob.value?.id === task.id) {
        closeCurrentPublishJob();
        return;
      }
      queriedTaskId.value = task.id;
      await queryJob();
      return;
    }
    queriedTaskId.value = task.id;
    selectedSection.value = "publish-tasks";
    await queryJob();
  }

  function closeCurrentPublishJob() {
    currentJob.value = null;
    queriedTaskId.value = "";
  }

  function statusType(status: string) {
    return statusTone[status] ?? "info";
  }

  function notificationSeverityType(severity: string) {
    if (severity === "critical") {
      return "danger";
    }
    if (severity === "warning") {
      return "warning";
    }
    return "info";
  }

  function notificationSeverityLabel(severity: string) {
    const labels: Record<string, string> = {
      critical: "严重",
      warning: "提醒",
      info: "信息",
    };
    return labels[severity] ?? severity;
  }

  function notificationSourceLabel(sourceType: string) {
    const labels: Record<string, string> = {
      publish_item: "铺货",
      purchase_mapping: "采购映射",
      purchase_mapping_resolved: "采购映射",
      purchase_issue: "采购异常",
      inventory_risk: "库存风控",
      delivery_order: "履约",
      delivery_shipment: "发货",
      aftersale: "售后",
      aftersale_sync: "售后同步",
      aftersale_evidence: "售后凭证",
      aftersale_responsibility: "售后归因",
      aftersale_action: "售后处理",
      guarantee_order: "纠纷单",
      guarantee_sync: "纠纷同步",
      guarantee_followup: "纠纷跟进",
      supplier_aftersale_followup: "供应商售后协同",
      order_detail_sync: "订单同步",
    };
    return labels[sourceType] ?? sourceType;
  }

  function aftersaleResponsibilityLabel(party: string | null) {
    if (!party) {
      return "未归因";
    }
    return (
      aftersaleResponsibilityOptions.find((item) => item.value === party)
        ?.label || party
    );
  }

  function guaranteeHandlingStatusLabel(status: string | null) {
    if (!status) {
      return "未跟进";
    }
    return (
      guaranteeHandlingStatusOptions.find((item) => item.value === status)
        ?.label || status
    );
  }

  function evidenceTypeLabel(type: string) {
    return (
      evidenceTypeOptions.find((item) => item.value === type)?.label || type
    );
  }

  function evidenceStatusLabel(status: string) {
    return (
      evidenceStatusOptions.find((item) => item.value === status)?.label ||
      status
    );
  }

  function supplierFollowupTypeLabel(type: string) {
    return (
      supplierFollowupTypeOptions.find((item) => item.value === type)?.label ||
      type
    );
  }

  function supplierFollowupStatusLabel(status: string) {
    return (
      supplierFollowupStatusOptions.find((item) => item.value === status)
        ?.label || status
    );
  }

  function aftersaleActionLabel(action: string | null) {
    if (!action) {
      return "未处理";
    }
    const labels: Record<string, string> = {
      accept: "同意",
      reject: "拒绝",
    };
    return labels[action] ?? action;
  }

  function aftersaleActionStatusLabel(status: string | null) {
    if (!status) {
      return "未提交";
    }
    const labels: Record<string, string> = {
      success: "已提交",
      failed: "失败",
    };
    return labels[status] ?? status;
  }

  function canSubmitAftersaleAction(row: AftersaleView) {
    return !aftersaleTerminalStatuses.includes(row.status);
  }

  function isActiveGuaranteeStatus(status: string) {
    return ![
      "STATUS_NO_NEED_PAY",
      "STATUS_PAY_SUCC",
      "STATUS_USER_CANCEL",
      "sync_failed",
    ].includes(status);
  }

  function aftersaleRejectReasonLabel(reason: AftersaleRejectReasonView) {
    return `${reason.reject_reason_type} · ${reason.reject_reason_type_text}`;
  }

  async function markNotificationRead(notification: NotificationView) {
    try {
      const result = await command<NotificationMarkResult>(
        "mark_notification_read",
        {
          notificationId: notification.id,
        },
      );
      if (result.updated_count > 0) {
        ElMessage.success(result.message);
      } else {
        ElMessage.info(result.message);
      }
      await Promise.all([refreshNotifications(), refreshDashboardOnly()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function markAllNotificationsRead() {
    try {
      const result = await command<NotificationMarkResult>(
        "mark_all_notifications_read",
      );
      ElMessage.success(result.message);
      await Promise.all([refreshNotifications(), refreshDashboardOnly()]);
    } catch (error) {
      ElMessage.error(String(error));
    }
  }

  async function refreshDashboardOnly() {
    dashboard.value = await command<DashboardSummary>("get_dashboard");
  }

  async function openNotification(notification: NotificationView) {
    if (notification.source_type === "publish_item") {
      selectedSection.value = "publish-tasks";
      if (notification.data_json) {
        try {
          const data = JSON.parse(notification.data_json) as {
            job_id?: unknown;
          };
          if (typeof data.job_id === "string" && data.job_id.trim()) {
            queriedTaskId.value = data.job_id;
          }
        } catch {
          // 通知摘要异常不阻断页面跳转，任务页仍可手工查询。
        }
      }
      await queryJob();
      return;
    }
    if (notification.source_type === "purchase_mapping") {
      selectedSection.value = "procurement";
      purchaseStatusFilter.value = "needs_mapping";
      await refreshPurchaseTasks();
      return;
    }
    if (notification.source_type === "purchase_issue") {
      selectedSection.value = "procurement";
      purchaseStatusFilter.value = "all";
      await refreshPurchaseTasks();
      return;
    }
    if (notification.source_type === "inventory_risk") {
      selectedSection.value = "analytics";
      inventoryRiskStatusFilter.value = "all";
      await refreshInventoryRisks();
      return;
    }
    if (
      notification.source_type === "delivery_order" ||
      notification.source_type === "delivery_shipment"
    ) {
      selectedSection.value = "procurement";
      deliveryStatusFilter.value = "all";
      await refreshDeliveryShipments();
      return;
    }
    if (
      notification.source_type === "aftersale" ||
      notification.source_type === "aftersale_sync" ||
      notification.source_type === "aftersale_evidence" ||
      notification.source_type === "aftersale_responsibility" ||
      notification.source_type === "aftersale_action" ||
      notification.source_type === "guarantee_order" ||
      notification.source_type === "guarantee_sync" ||
      notification.source_type === "guarantee_followup"
    ) {
      selectedSection.value = "exceptions";
      if (notification.source_type === "guarantee_sync") {
        guaranteeStatusFilter.value = "sync_failed";
      } else if (
        notification.source_type === "guarantee_order" ||
        notification.source_type === "guarantee_followup"
      ) {
        guaranteeStatusFilter.value = "active";
      } else {
        aftersaleStatusFilter.value =
          notification.source_type === "aftersale_sync"
            ? "sync_failed"
            : "active";
      }
      await Promise.all([
        refreshAftersales(),
        refreshGuaranteeOrders(),
        refreshAftersaleEvidence(),
      ]);
      return;
    }
    selectedSection.value = "workbench";
    await refreshAll();
  }

  function profitStatusLabel(status: string) {
    const labels: Record<string, string> = {
      missing_purchase_task: "缺采购任务",
      missing_cost: "缺成本",
      loss: "亏损",
      profitable: "有毛利",
    };
    return labels[status] ?? status;
  }

  function inventoryRiskLabel(status: string) {
    return (
      inventoryRiskStatusOptions.find((item) => item.value === status)?.label ||
      status
    );
  }

  function productSalesStatusLabel(status: string) {
    return (
      productSalesStatusOptions.find((item) => item.value === status)?.label ||
      status
    );
  }

  function productManagementStatusLabel(status: string) {
    return (
      productManagementStatusOptions.find((item) => item.value === status)
        ?.label || status
    );
  }

  function orderManagementStatusLabel(status: string) {
    return (
      orderManagementStatusOptions.find((item) => item.value === status)
        ?.label || status
    );
  }

  function purchaseManagementStatusLabel(status: string) {
    const labels: Record<string, string> = {
      none: "未生成",
      missing_purchase_task: "缺采购任务",
      needs_mapping: "缺映射",
      purchase_issue: "采购异常",
      pending_purchase: "待采购",
      missing_cost: "缺成本",
      supplier_shipped: "供应商已发",
      completed: "已完成",
    };
    return labels[status] ?? status;
  }

  function shipmentStatusLabel(status: string) {
    const labels: Record<string, string> = {
      none: "未生成",
      pending: "待处理",
      waiting_confirmation: "待确认",
      ready_to_send: "待提交",
      send_failed: "发货失败",
      wechat_shipped: "已发货",
    };
    return labels[status] ?? status;
  }

  onMounted(refreshAll);

  return {
    command,
    aftersaleActionForm,
    aftersaleActionLabel,
    aftersaleActionStatusLabel,
    aftersaleEvidence,
    aftersaleEvidenceTotal,
    aftersaleRejectReasonLabel,
    aftersaleRejectReasonOptions,
    aftersaleRejectReasons,
    aftersaleResponsibilityForm,
    aftersaleResponsibilityLabel,
    aftersaleResponsibilityOptions,
    aftersales,
    aftersaleStatusFilter,
    aftersaleTerminalStatuses,
    aftersaleTotal,
    agentSkillSaving,
    agentRuns,
    agentRunSceneFilter,
    agentRunStatusFilter,
    agentRunsLoading,
    agentSkills,
    agentSkillsLoading,
    agentSkillTesting,
    aiProviderApiKeyPlaceholder,
    aiProviderApiOptions,
    aiProviderBaseUrlPlaceholder,
    aiProviderCanTest,
    aiProviderForm,
    aiProviderFormIsCustom,
    aiProviderFormRequiresApiKey,
    aiProviderFormRequiresBaseUrl,
    aiProviderFormRuntimeLabel,
    aiProviderModelPlaceholder,
    aiProviderOptions,
    aiProviderRequiresApiKey,
    aiProviderRuntimeLabel,
    aiProviderSaving,
    aiProviderSettings,
    aiProviderTesting,
    applyAftersaleRejectReason,
    applySupplierAgentResults,
    automationRunning,
    automationSettings,
    backupRunning,
    workspaceResetRunning,
    buildPreviewInventoryRisks,
    buildPreviewJob,
    buildPreviewOrderProfitResult,
    buildPreviewPriceJob,
    buildPreviewProductSalesAnalysis,
    canSubmitAftersaleAction,
    categoryCache,
    categoryCatalogShops,
    categoryRelations,
    categoryKeyword,
    categoryRuleCatId,
    checkShopQuota,
    checkTaobaoLoginState,
    closeCurrentPublishJob,
    clearCollectionHistory,
    clearTaobaoAccessLimitState,
    clearEvidenceTargetFilter,
    clearSupplierFollowupTargetFilter,
    canSelectCollectionTask,
    collectionPublishedShopNames,
    collectionProductSummary,
    collectionPublishJobText,
    collectionPublishing,
    collectionPublishTargetShopIds,
    collectionReviewCategoryKeyword,
    collectionReviewCategorySearching,
    collectionReviewConfirming,
    collectionReviewing,
    collectionReviewStatusLabel,
    collectionReviewStatusType,
    collectionTargetShopSelectionRequired,
    collectionAccessLimitChecking,
    collectionAccessLimitState,
    collectionCheckingLogin,
    collectionDetailVisible,
    collectionFilePath,
    collectionFileName,
    collectionImporting,
    collectionImportVisible,
    collectionImageSrc,
    collectionImageUpdating,
    collectionLoggingIn,
    collectionTaskPage,
    collectionTaskPageSize,
    collectionTaskPageSizeOptions,
    collectionTasks,
    collectionTaskStatusLabel,
    collectionTaskTotal,
    collectionTesting,
    createPublishJobFromSelectedCollections,
    confirmSelectedCollectionReview,
    publishPricingDialogVisible,
    publishPricingForm,
    publishPricingSaving,
    publishPricingStrategy,
    publishPricingSummary,
    computeSalePriceCents,
    clearCollectionExcelFile,
    createDatabaseBackup,
    createGroup,
    createOrderPriceAdjustmentJob,
    createPriceUpdateJob,
    createPublishJob,
    createShop,
    criticalNotificationCount,
    currentJob,
    currentOrderPriceAdjustmentJob,
    currentPriceJob,
    dashboard,
    databaseBackups,
    defaultAutomationSettings,
    deliveryCompanies,
    deliveryCompanyOptions,
    deliverySettings,
    deliveryShipments,
    deliveryShipmentTotal,
    deliveryStatusFilter,
    enabledAgentSkillCount,
    evidenceExportFormat,
    evidenceExportPath,
    evidenceForm,
    evidenceStatusFilter,
    evidenceStatusLabel,
    evidenceStatusOptions,
    evidenceTargetIdFilter,
    evidenceTargetTypeFilter,
    evidenceTargetTypeOptions,
    evidenceTypeLabel,
    evidenceTypeOptions,
    exportAftersaleEvidence,
    exportPurchaseTasks,
    exportSupplierAgentTasks,
    externalApiLogs,
    fallbackDeliveryCompanyOptions,
    fillSupplierAgentTemplate,
    formatBytes,
    formatCents,
    formatDateTime,
    formatSignedCents,
    formatUnixTime,
    freightTemplates,
    groupForm,
    groups,
    guaranteeFollowupForm,
    guaranteeHandlingStatusLabel,
    guaranteeHandlingStatusOptions,
    guaranteeOrders,
    guaranteeOrderTotal,
    guaranteeStatusFilter,
    handleCollectionTaskPageChange,
    handleCollectionTaskPageSizeChange,
    inventoryRiskLabel,
    inventoryRisks,
    inventoryRiskStats,
    inventoryRiskStatusFilter,
    inventoryRiskStatusOptions,
    inventoryRiskTotal,
    isActiveGuaranteeStatus,
    isCollectionTaskSelected,
    isCurrentCollectionPageAllSelected,
    isCurrentCollectionPageIndeterminate,
    isTauriRuntime,
    lastAutomationResult,
    latestPriceTaskId,
    latestOrderPriceAdjustmentTaskId,
    latestTaskId,
    loading,
    localApiConfig,
    markAllNotificationsRead,
    markNotificationRead,
    markPurchaseTaskIssue,
    notifications,
    notificationSeverityFilter,
    notificationSeverityLabel,
    notificationSeverityType,
    notificationSourceLabel,
    notificationStatusFilter,
    notificationTotal,
    openNotification,
    openCollectionDetail,
    openPublishPricingDialog,
    openTask,
    openTestCollectDialog,
    orderProfitAdjustmentForm,
    orderManagementItems,
    orderManagementKeyword,
    orderManagementShopFilter,
    orderManagementStatusFilter,
    orderManagementStatusLabel,
    orderManagementStatusOptions,
    orderManagementTotal,
    orderProfits,
    orderProfitStatusFilter,
    orderProfitTotal,
    orderProfitTotals,
    paginatedCollectionTasks,
    previewAftersaleEvidence,
    previewAftersaleRejectReasons,
    previewAftersales,
    previewCategoryCache,
    previewCategoryCatalogShops,
    previewCategoryRelations,
    previewDatabaseBackups,
    previewExternalApiLogs,
    previewFreightTemplates,
    previewGroups,
    previewGuaranteeOrders,
    previewLastOrderSyncAt,
    previewNotifications,
    previewPendingOrderCount,
    previewProfitAdjustments,
    previewPurchaseTasks,
    previewShipments,
    previewShops,
    previewSupplierAftersaleFollowups,
    previewTaskRuns,
    priceUpdatePayload,
    orderPriceAdjustmentPayload,
    productSalesAnalysis,
    productSalesAnalysisStatusFilter,
    productSalesAnalysisTotal,
    productSalesAnalysisTotals,
    productSalesStatusLabel,
    productSalesStatusOptions,
    productManagementItems,
    productManagementKeyword,
    productManagementShopFilter,
    productManagementStatusFilter,
    productManagementStatusLabel,
    productManagementStatusOptions,
    productManagementTotal,
    profitAdjustmentKindOptions,
    profitStatusLabel,
    publishPayload,
    publishRetryRunning,
    purchaseExportPath,
    purchaseIssueForm,
    purchaseIssueTypeOptions,
    purchaseMappingForm,
    purchaseShipmentForm,
    purchaseStatusFilter,
    purchaseTasks,
    purchaseTaskTotal,
    queriedPriceTaskId,
    queriedOrderPriceAdjustmentTaskId,
    queriedTaskId,
    queryJob,
    queryOrderPriceAdjustmentJob,
    queryPriceUpdateJob,
    recordAftersaleEvidence,
    recordAftersaleResponsibility,
    recordGuaranteeFollowup,
    recordOrderProfitAdjustment,
    recordOrderShipment,
    recordPurchaseTaskShipment,
    recordSupplierAftersaleFollowup,
    resetCollectionPublishWorkspace,
    refreshAftersaleEvidence,
    refreshAftersaleRejectReasons,
    refreshAftersales,
    refreshAll,
    refreshAgentRuns,
    refreshAgentSkills,
    refreshCategoryCatalog,
    refreshCollectionTasks,
    refreshDashboardOnly,
    refreshDatabaseBackups,
    refreshDeliveryCompanies,
    refreshDeliveryShipments,
    refreshExternalApiLogs,
    refreshGuaranteeOrders,
    refreshInventoryRisks,
    refreshNotifications,
    refreshOrderManagementItems,
    refreshOrderProfits,
    refreshProductManagementItems,
    refreshProductSalesAnalysis,
    refreshPurchaseTasks,
    refreshSupplierAftersaleFollowups,
    refreshTaobaoAccessLimitState,
    readyAgentSkillCount,
    resolvePurchaseTaskMapping,
    restoreDatabaseBackup,
    resumeCollectionTasks,
    resumePublishFailuresOnce,
    retryAllFailedCollections,
    retryCollection,
    deleteTaobaoProfile,
    removeCollectionImage,
    resetAllPassedReviews,
    resetSelectedCollectionReview,
    reviewSingleCollectionTask,
    reviewSelectedCollectionTasks,
    retryDeliveryShipment,
    revealBackup,
    rotatedLocalApiKey,
    rotateLocalApiKey,
    runAftersaleSyncOnce,
    runDeliverySubmissionOnce,
    runGuaranteeSyncOnce,
    runInventoryRiskScan,
    runOperationalAutomationOnce,
    runOrderDetailSyncOnce,
    runOrderPriceAdjustmentOnce,
    runOrderSyncOnce,
    runUnpaidOrderSyncOnce,
    runPriceUpdateConfirmOnce,
    runPriceUpdatePrecheckOnce,
    runPriceUpdateSubmitOnce,
    runPurchaseTaskGenerationOnce,
    runTestCollect,
    runtimeLabel,
    saveAgentSkill,
    saveAiProviderSettings,
    saveAutomationSettings,
    savePublishPricingStrategyFromForm,
    searchCollectionReviewCategories,
    selectAftersaleAction,
    selectAftersaleResponsibility,
    selectedAgentSkill,
    selectedAgentSkillName,
    selectedCollectionDetailImages,
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
    selectedCollectionMainImages,
    selectedCollectionSkuPreview,
    selectedCollectionTasks,
    selectCollectionExcelFile,
    selectedCategoryShop,
    selectedCategoryShopId,
    selectedGroupOptions,
    selectedSection,
    selectEvidenceTarget,
    selectGuaranteeFollowup,
    selectOrderProfitAdjustment,
    selectPurchaseTaskIssue,
    selectPurchaseTaskMapping,
    selectPurchaseTaskShipment,
    selectSupplierFollowupTarget,
    setAutoSendDelivery,
    shipmentForm,
    shopForm,
    shops,
    startCollectionPolling,
    startExcelImport,
    statusType,
    shipmentStatusLabel,
    setCollectionTaskSelected,
    submitAftersaleAccept,
    submitAftersaleReject,
    summarizeOrderProfits,
    summarizeProductSalesAnalysis,
    purchaseManagementStatusLabel,
    supplierAftersaleFollowups,
    supplierAftersaleFollowupTotal,
    supplierAgentApplyResult,
    supplierAgentApplyText,
    supplierAgentContinueOnError,
    supplierAgentDryRun,
    supplierAgentExportFormat,
    supplierAgentExportPath,
    supplierFollowupForm,
    supplierFollowupStatusFilter,
    supplierFollowupStatusLabel,
    supplierFollowupStatusOptions,
    supplierFollowupTargetIdFilter,
    supplierFollowupTargetTypeFilter,
    supplierFollowupTypeLabel,
    supplierFollowupTypeOptions,
    syncAftersaleRejectReasons,
    syncAiProviderForm,
    syncDeliveryCompanies,
    syncSelectedCategoryRules,
    syncSelectedShopCategoryCatalog,
    syncShopBasicInfo,
    taskRuns,
    testAgentSkill,
    testAiProvider,
    testCollectHeaded,
    testCollectResult,
    testCollectUrl,
    testCollectVisible,
    toggleCurrentCollectionPageSelection,
    triggerTaobaoLogin,
    unreadNotificationCount,
    updateAftersaleEvidenceStatus,
    uploadCollectionImage,
    upsertPreviewShipment,
    verifyShop,
  };
}

export type WxXdAppContext = ReturnType<typeof useWxXdApp>;
