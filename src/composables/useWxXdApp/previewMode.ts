import type { Ref } from "../../runtime/reactive";
import {
  defaultAutomationSettings,
  defaultPublishPricingStrategy,
  fallbackDeliveryCompanyOptions,
} from "./constants";
import type {
  AftersaleSyncBatchResult,
  AftersaleEvidenceView,
  AftersaleRejectReasonView,
  AiProviderSettings,
  AgentRunView,
  AgentSkillSettingsRequest,
  AgentSkillTestResult,
  AgentSkillView,
  AftersaleView,
  AssetUploadBatchResult,
  BackupInfo,
  CollectionPublishRequest,
  CollectionTaskView,
  CategoryCacheView,
  CategoryCatalogShopSummary,
  CategoryRelationView,
  DeliverySettings,
  ExternalApiLogView,
  FreightTemplateView,
  DeliverySubmitBatchResult,
  GuaranteeOrderView,
  InventoryRiskView,
  LocalApiConfig,
  OperationalAutomationRunResult,
  NotificationView,
  OperationalAutomationSettings,
  OrderDetailSyncBatchResult,
  OrderManagementView,
  OrderPriceAdjustmentJobView,
  OrderProfitListResult,
  OrderProfitTotals,
  OrderProfitView,
  OrderSyncBatchResult,
  PriceUpdateConfirmBatchResult,
  PriceUpdateJobView,
  ProductListingBatchResult,
  ProductManagementView,
  ProductSalesAnalysisTotals,
  ProductSalesAnalysisView,
  ProductStatusSyncBatchResult,
  ProductSubmitBatchResult,
  PublishAttributeFillBatchResult,
  PublishCategoryPrecheckBatchResult,
  PublishJobView,
  PublishPipelineRunResult,
  PublishPricingStrategy,
  PurchaseTaskView,
  PublishTaskBatchResult,
  PurchaseTaskBatchResult,
  ShipmentView,
  ShopGroup,
  ShopListItem,
  SupplierAftersaleFollowupView,
  TaskRunView,
} from "../../types/app";

interface PreviewModeDeps {
  aiProviderSettings: Ref<AiProviderSettings>;
  automationSettings: Ref<OperationalAutomationSettings>;
  currentJob: Ref<PublishJobView | null>;
  currentOrderPriceAdjustmentJob: Ref<OrderPriceAdjustmentJobView | null>;
  currentPriceJob: Ref<PriceUpdateJobView | null>;
  deliverySettings: Ref<DeliverySettings>;
  latestOrderPriceAdjustmentTaskId: Ref<string>;
  latestPriceTaskId: Ref<string>;
  latestTaskId: Ref<string>;
  localApiConfig: Ref<LocalApiConfig>;
  publishPricingStrategy: Ref<PublishPricingStrategy>;
  previewAftersaleEvidence: Ref<AftersaleEvidenceView[]>;
  previewAftersaleRejectReasons: Ref<AftersaleRejectReasonView[]>;
  previewAftersales: Ref<AftersaleView[]>;
  previewCategoryCache: Ref<CategoryCacheView[]>;
  previewCategoryCatalogShops: Ref<CategoryCatalogShopSummary[]>;
  previewCategoryRelations: Ref<CategoryRelationView[]>;
  previewCollectionTasks: Ref<CollectionTaskView[]>;
  previewDatabaseBackups: Ref<BackupInfo[]>;
  previewExternalApiLogs: Ref<ExternalApiLogView[]>;
  previewFreightTemplates: Ref<FreightTemplateView[]>;
  previewGroups: Ref<ShopGroup[]>;
  previewGuaranteeOrders: Ref<GuaranteeOrderView[]>;
  previewLastOrderSyncAt: Ref<string | null>;
  previewNotifications: Ref<NotificationView[]>;
  previewPendingOrderCount: Ref<number>;
  previewProfitAdjustments: Ref<
    Array<{ order_id: string; kind: string; amount_cents: number }>
  >;
  previewPurchaseTasks: Ref<PurchaseTaskView[]>;
  previewShipments: Ref<ShipmentView[]>;
  previewShops: Ref<ShopListItem[]>;
  previewSupplierAftersaleFollowups: Ref<SupplierAftersaleFollowupView[]>;
  previewTaskRuns: Ref<TaskRunView[]>;
  queriedOrderPriceAdjustmentTaskId: Ref<string>;
  queriedPriceTaskId: Ref<string>;
  selectedCollectionTasks: Ref<CollectionTaskView[]>;
  canSubmitAftersaleAction: (row: AftersaleView) => boolean;
  evidenceStatusLabel: (status: string) => string;
  evidenceTypeLabel: (type: string) => string;
  isActiveGuaranteeStatus: (status: string) => boolean;
  supplierFollowupStatusLabel: (status: string) => string;
  supplierFollowupTypeLabel: (type: string) => string;
}

export function createPreviewMode(deps: PreviewModeDeps) {
  const {
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
  } = deps;
  let previewAgentSkills: AgentSkillView[] = [
    {
      name: "wx-xd-product-review",
      version: "1.0.0",
      description:
        "审查采集商品，清洗标题、识别不能用于微信小店铺货的图片，并从本地微信类目候选中选择类目。",
      enabled: true,
      runtime: "pi_coding_agent",
      model: null,
      temperature: null,
      skill_path: "agent-skills/wx-xd-product-review/SKILL.md",
      schema_path: "agent-skills/wx-xd-product-review/output_schema.json",
      checksum: "preview",
      file_status: "ready",
      runtime_status: "ready",
      last_test_status: null,
      last_test_summary: null,
      last_test_at: null,
      updated_at: null,
    },
    {
      name: "wx-xd-attribute-suggestion",
      version: "1.0.0",
      description: "基于微信类目详情、商品资料和 SKU 规格生成必填属性候选值。",
      enabled: true,
      runtime: "pi_coding_agent",
      model: null,
      temperature: null,
      skill_path: "agent-skills/wx-xd-attribute-suggestion/SKILL.md",
      schema_path: "agent-skills/wx-xd-attribute-suggestion/output_schema.json",
      checksum: "preview",
      file_status: "ready",
      runtime_status: "ready",
      last_test_status: null,
      last_test_summary: null,
      last_test_at: null,
      updated_at: null,
    },
  ];
  let previewAgentRuns: AgentRunView[] = [
    {
      id: "agent-run-preview-1",
      skill_name: "wx-xd-product-review",
      skill_version: "1.0.0",
      scene: "collection_review",
      source_type: "collection_task",
      source_id: "collection-preview-1",
      shop_id: "shop-preview",
      status: "needs_review",
      provider_type: "custom",
      model: "preview-model",
      temperature: 0.1,
      input_summary:
        "采集审查 task_id=collection-preview-1 主图=5 详情图=8 类目候选=3",
      decision: "needs_review",
      error_code: null,
      error_summary: null,
      started_at: "2026-05-22T00:20:00+08:00",
      finished_at: "2026-05-22T00:20:03+08:00",
      duration_ms: 3120,
      created_at: "2026-05-22T00:20:00+08:00",
    },
    {
      id: "agent-run-preview-2",
      skill_name: "wx-xd-supplier-bridge",
      skill_version: "1.0.0",
      scene: "supplier_bridge",
      source_type: "purchase_task_export",
      source_id: "all",
      shop_id: null,
      status: "succeeded",
      provider_type: null,
      model: null,
      temperature: null,
      input_summary: "供应商 Agent 导出 status=all format=jsonl count=3",
      decision: "success",
      error_code: null,
      error_summary: null,
      started_at: "2026-05-22T00:18:00+08:00",
      finished_at: "2026-05-22T00:18:00+08:00",
      duration_ms: 24,
      created_at: "2026-05-22T00:18:00+08:00",
    },
  ];

  async function previewCommand<T>(
    name: string,
    args?: Record<string, unknown>,
  ): Promise<T> {
    if (name === "get_dashboard") {
      return {
        pending_order_count: previewPendingOrderCount.value,
        abnormal_shop_count: 1,
        failed_publish_product_count:
          currentJob.value?.products.filter(
            (product) => product.failed_count > 0,
          ).length ?? 0,
        unread_notification_count: previewNotifications.value.filter(
          (item) => item.status === "unread",
        ).length,
        running_task_count: previewTaskRuns.value.filter((task) =>
          ["pending", "queued", "running"].includes(task.status),
        ).length,
        controller_status: "浏览器预览模式",
        database_path: "浏览器预览不访问本地 SQLite",
        now_shanghai: "2026-05-22T00:00:00+08:00",
        last_order_sync_at: previewLastOrderSyncAt.value,
        last_publish_summary: latestTaskId.value || null,
      } as T;
    }
    if (name === "list_shop_groups") {
      return previewGroups.value as T;
    }
    if (name === "list_shops") {
      return previewShops.value as T;
    }
    if (name === "create_shop_group") {
      const id = `group-preview-${Date.now()}`;
      previewGroups.value.push({
        id,
        name: String(args?.name || "预览店铺组"),
        status: "active",
        shop_count: 0,
        created_at: "2026-05-22T00:00:00+08:00",
      });
      return previewGroups.value[previewGroups.value.length - 1] as T;
    }
    if (name === "create_shop") {
      const request = args?.request as
        | {
            name?: string;
            appid?: string;
            app_secret?: string;
            group_id?: string;
          }
        | undefined;
      const group = previewGroups.value.find(
        (item) => item.id === request?.group_id,
      );
      if (group) {
        group.shop_count += 1;
      }
      const shop = {
        id: `shop-preview-${Date.now()}`,
        name: String(request?.name || "预览店铺"),
        appid: String(request?.appid || "wx-preview"),
        status: request?.app_secret ? "not_verified" : "missing_secret",
        group_id: String(request?.group_id || "group-default"),
        group_name: group?.name || "默认店铺组",
        has_secret: Boolean(request?.app_secret),
        token_expires_at: null,
        wechat_nickname: null,
        wechat_status: null,
        last_health_check_at: null,
        last_quota_remain: null,
        created_at: "2026-05-22T00:00:00+08:00",
      };
      previewShops.value.unshift(shop);
      return shop as T;
    }
    if (name === "verify_shop_credentials") {
      const shopId = String(args?.shopId || "");
      const shop = previewShops.value.find((item) => item.id === shopId);
      if (shop) {
        shop.status = shop.has_secret ? "active" : "missing_secret";
        shop.token_expires_at = shop.has_secret
          ? "2026-05-22T02:00:00+08:00"
          : null;
      }
      return {
        shop_id: shopId,
        status: shop?.status || "missing_secret",
        expires_at: shop?.token_expires_at || null,
        errcode: null,
        errmsg: null,
      } as T;
    }
    if (name === "sync_shop_basic_info") {
      const shopId = String(args?.shopId || "");
      const shop = previewShops.value.find((item) => item.id === shopId);
      if (shop) {
        shop.status = "active";
        shop.wechat_nickname = `${shop.name}资料`;
        shop.wechat_status = "open_finished";
        shop.last_health_check_at = "2026-05-22T02:05:00+08:00";
      }
      return {
        shop_id: shopId,
        status: "active",
        nickname: shop?.wechat_nickname || null,
        wechat_status: shop?.wechat_status || null,
        errcode: null,
        errmsg: null,
      } as T;
    }
    if (name === "check_shop_api_quota") {
      const request = args?.request as
        | { shop_id?: string; cgi_path?: string }
        | undefined;
      const shop = previewShops.value.find(
        (item) => item.id === request?.shop_id,
      );
      if (shop) {
        shop.last_quota_remain = 499990;
      }
      return {
        shop_id: request?.shop_id || "",
        cgi_path: request?.cgi_path || "/channels/ec/basics/info/get",
        daily_limit: 500000,
        used: 10,
        remain: 499990,
        rate_call_count: 10000,
        rate_refresh_second: 60,
        errcode: null,
        errmsg: null,
      } as T;
    }
    if (name === "list_category_catalog") {
      const shopId = String(args?.shopId || "");
      const keyword = String(args?.keyword || "").trim();
      const shopCategories = previewCategoryCache.value.filter(
        (item) => !shopId || item.shop_id === shopId,
      );
      const categories = (
        keyword
          ? withPreviewCategoryContext(shopCategories, keyword)
          : shopCategories
      )
        .slice()
        .sort(comparePreviewCategories);
      const categoryRelations = previewCategoryRelations.value.filter(
        (item) => {
          if (shopId && item.shop_id !== shopId) {
            return false;
          }
          if (!keyword) {
            return true;
          }
          return (
            String(item.cat_id).includes(keyword) ||
            (item.category_name || "").includes(keyword) ||
            (item.uneffective_reason || "").includes(keyword)
          );
        },
      );
      const freightTemplates = previewFreightTemplates.value.filter(
        (item) => !shopId || item.shop_id === shopId,
      );
      return {
        shops: previewCategoryCatalogShops.value,
        categories,
        category_relations: categoryRelations,
        freight_templates: freightTemplates,
      } as T;
    }
    if (name === "sync_shop_category_catalog") {
      const shopId = String(args?.shopId || "shop-preview");
      const shop = previewShops.value.find((item) => item.id === shopId);
      const summary = previewCategoryCatalogShops.value.find(
        (item) => item.shop_id === shopId,
      );
      if (summary) {
        summary.last_category_sync_at = "2026-05-22T00:22:00+08:00";
        summary.last_freight_sync_at = "2026-05-22T00:22:00+08:00";
      }
      previewTaskRuns.value.unshift({
        id: `catalog-sync-preview-${Date.now()}`,
        task_type: "catalog.sync_category_rules",
        status: "success",
        progress: 100,
        created_at: "2026-05-22T00:22:00+08:00",
        started_at: "2026-05-22T00:22:00+08:00",
        finished_at: "2026-05-22T00:22:01+08:00",
        pending_count: 0,
        ready_count: 0,
        failed_count: 0,
      });
      return {
        task_id: `catalog-sync-preview-${Date.now()}`,
        shop_id: shop?.id || shopId,
        synced_categories: previewCategoryCache.value.filter(
          (item) => item.shop_id === shopId,
        ).length,
        synced_category_relations: previewCategoryRelations.value.filter(
          (item) => item.shop_id === shopId && item.status === 1,
        ).length,
        synced_freight_templates: previewFreightTemplates.value.filter(
          (item) => item.shop_id === shopId,
        ).length,
        failed_steps: [],
      } as T;
    }
    if (name === "sync_category_rules") {
      const shopId = String(args?.shopId || "shop-preview");
      const catId = Number(args?.catId || 1000102);
      const category = previewCategoryCache.value.find(
        (item) => item.shop_id === shopId && item.cat_id === catId,
      );
      if (category) {
        category.has_detail = true;
        category.has_product_rule = true;
        category.has_delivery_rule = true;
        category.product_attr_count = Math.max(category.product_attr_count, 6);
        category.sale_attr_count = Math.max(category.sale_attr_count, 2);
        category.detail_synced_at = "2026-05-22T00:23:00+08:00";
      }
      return {
        task_id: `category-rule-sync-preview-${Date.now()}`,
        shop_id: shopId,
        cat_id: catId,
        synced_detail: true,
        synced_product_rule: true,
        synced_delivery_rule: true,
        product_attr_count: category?.product_attr_count || 6,
        sale_attr_count: category?.sale_attr_count || 2,
        product_qua_count: category?.product_qua_count || 0,
        failed_steps: [],
      } as T;
    }
    if (name === "get_collection_tasks") {
      return previewCollectionTasks.value as T;
    }
    if (name === "import_excel_for_collection") {
      const nextId = `col-preview-${Date.now()}`;
      previewCollectionTasks.value.unshift({
        id: nextId,
        title: "预览导入商品",
        source_url: "https://item.taobao.com/item.htm?id=preview",
        category_path: "服饰内衣 > 女装 > 预览类目",
        target_shop_ids: [],
        status: "success",
        error_summary: null,
        collected_data: JSON.stringify({
          external_product_id: `tb-preview-${Date.now()}`,
          title: "预览导入商品",
          source_url: "https://item.taobao.com/item.htm?id=preview",
          images: [
            "https://example.com/head-1.jpg",
            "https://example.com/head-2.jpg",
            "https://example.com/head-3.jpg",
          ],
          detail_images: ["https://example.com/detail-1.jpg"],
          skus: [
            {
              external_sku_id: "sku-1",
              specs: { 颜色: "默认" },
              cost_price: 35,
              stock: 50,
            },
          ],
          supplier_name: "淘宝采集",
          supplier_product_id: "preview",
          category_hint: "服饰内衣 > 女装 > 预览类目",
          brand_hint: "无品牌",
          weight_gram: 300,
          metadata: {},
        }),
        review_status: "pending",
        review_summary: null,
        reviewed_data: null,
        review_result_json: null,
        reviewed_at: null,
        published_shop_ids: [],
        publish_job_ids: [],
        published_at: null,
        created_at: "2026-05-22T00:30:00+08:00",
        updated_at: "2026-05-22T00:30:10+08:00",
      });
      return 1 as T;
    }
    if (name === "retry_collection_task") {
      const task = previewCollectionTasks.value.find(
        (item) => item.id === args?.taskId,
      );
      if (task) {
        task.status = "success";
        task.error_summary = null;
        task.updated_at = "2026-05-22T00:31:00+08:00";
      }
      return undefined as T;
    }
    if (name === "run_collection_review_once") {
      const request = args?.request as { task_ids?: string[] } | undefined;
      const ids =
        request?.task_ids ||
        previewCollectionTasks.value.map((task) => task.id);
      let passed = 0;
      previewCollectionTasks.value.forEach((task) => {
        if (!ids.includes(task.id) || task.status !== "success") {
          return;
        }
        task.review_status = "passed";
        task.review_summary = "预览审查通过";
        task.reviewed_data = task.collected_data;
        task.review_result_json = JSON.stringify({
          status: "passed",
          summary: "预览审查通过",
          issues: [],
        });
        task.reviewed_at = "2026-05-22T00:31:30+08:00";
        passed += 1;
      });
      return {
        processed_items: passed,
        passed_items: passed,
        needs_review_items: 0,
        blocked_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "confirm_collection_review") {
      const request = args?.request as { task_id?: string } | undefined;
      const task = previewCollectionTasks.value.find(
        (item) => item.id === request?.task_id,
      );
      if (!task) {
        throw new Error("采集任务不存在");
      }
      task.review_status = "passed";
      task.review_summary = "人工确认审查通过";
      task.reviewed_data = task.reviewed_data || task.collected_data;
      task.reviewed_at = "2026-05-22T00:31:45+08:00";
      return task as T;
    }
    if (name === "reset_collection_review") {
      const task = previewCollectionTasks.value.find(
        (item) => item.id === args?.taskId,
      );
      if (!task) {
        throw new Error("采集任务不存在");
      }
      task.review_status = "pending";
      task.review_summary = null;
      task.reviewed_data = null;
      task.review_result_json = null;
      task.reviewed_at = null;
      return task as T;
    }
    if (name === "resume_collection_tasks") {
      return previewCollectionTasks.value.filter(
        (item) => item.status === "pending" || item.status === "running",
      ).length as T;
    }
    if (name === "clear_collection_tasks") {
      previewCollectionTasks.value = [];
      selectedCollectionTasks.value = [];
      return undefined as T;
    }
    if (name === "reset_collection_publish_workspace") {
      const backup: BackupInfo = {
        id: `wx-xd-manual-before-reset-preview-${Date.now()}`,
        file_name: `wx-xd-manual-before-reset-preview-${Date.now()}.sqlite`,
        file_path: `/tmp/wx-xd-backups/wx-xd-manual-before-reset-preview-${Date.now()}.sqlite`,
        size_bytes: 5242880,
        sha256: "preview-sha256",
        created_at: "2026-05-22T00:15:00+08:00",
        integrity_ok: true,
        integrity_message: "ok",
      };
      previewDatabaseBackups.value.unshift(backup);
      previewCollectionTasks.value = [];
      selectedCollectionTasks.value = [];
      currentJob.value = null;
      previewNotifications.value = previewNotifications.value.filter(
        (item) => item.source_type !== "publish_item",
      );
      previewTaskRuns.value = previewTaskRuns.value.filter(
        (item) =>
          !item.task_type.startsWith("publish.") &&
          !item.task_type.startsWith("collection."),
      );
      return {
        backup,
        integrity_ok: true,
        integrity_message: "ok",
        counts: [],
        message: "采集/铺货工作区已清理；基础配置已保留。",
      } as T;
    }
    if (name === "create_publish_job_from_collection_tasks") {
      const request = args?.request as CollectionPublishRequest | undefined;
      const taskIds = request?.collection_task_ids || [];
      const shopIds = request?.target_shop_ids || [];
      if (taskIds.length === 0 || shopIds.length === 0) {
        throw new Error("请选择采集结果和目标微信小店");
      }
      const taskId = `pub_preview_${Date.now()}`;
      latestTaskId.value = taskId;
      currentJob.value = buildPreviewJob(taskId);
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "publish.create_external_job",
        status: "queued",
        progress: 0,
        created_at: "2026-05-22T00:32:00+08:00",
        started_at: null,
        finished_at: null,
        pending_count: taskIds.length * shopIds.length,
        ready_count: 0,
        failed_count: 0,
      });
      previewCollectionTasks.value.forEach((task) => {
        if (!taskIds.includes(task.id)) {
          return;
        }
        task.published_shop_ids = Array.from(
          new Set([...task.published_shop_ids, ...shopIds]),
        );
        task.publish_job_ids = Array.from(
          new Set([...task.publish_job_ids, taskId]),
        );
        task.published_at = "2026-05-22T00:32:00+08:00";
        task.updated_at = "2026-05-22T00:32:00+08:00";
      });
      return {
        task_id: taskId,
        status: "queued",
        accepted_product_count: taskIds.length,
        target_shop_count: shopIds.length,
      } as T;
    }
    if (name === "create_external_publish_job") {
      const taskId = `pub_preview_${Date.now()}`;
      latestTaskId.value = taskId;
      currentJob.value = buildPreviewJob(taskId);
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "publish.create_external_job",
        status: "queued",
        progress: 0,
        created_at: "2026-05-22T00:00:00+08:00",
        started_at: null,
        finished_at: null,
        pending_count: 1,
        ready_count: 0,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        status: "queued",
        accepted_product_count: 1,
        target_shop_count: 1,
      } as T;
    }
    if (name === "get_publish_job") {
      return (currentJob.value ||
        buildPreviewJob(String(args?.taskId || "pub_preview"))) as T;
    }
    if (name === "create_order_price_adjustment_job") {
      const taskId = `order_price_preview_${Date.now()}`;
      latestOrderPriceAdjustmentTaskId.value = taskId;
      queriedOrderPriceAdjustmentTaskId.value = taskId;
      currentOrderPriceAdjustmentJob.value =
        buildPreviewOrderPriceAdjustmentJob(taskId);
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "orders.change_order_price",
        status: "queued",
        progress: 0,
        created_at: "2026-05-22T00:20:00+08:00",
        started_at: null,
        finished_at: null,
        pending_count: 1,
        ready_count: 0,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        status: "queued",
        accepted_order_count: 1,
      } as T;
    }
    if (name === "get_order_price_adjustment_job") {
      return (currentOrderPriceAdjustmentJob.value ||
        buildPreviewOrderPriceAdjustmentJob(
          String(args?.taskId || "order_price_preview"),
        )) as T;
    }
    if (name === "create_price_update_job") {
      const taskId = `price_preview_${Date.now()}`;
      latestPriceTaskId.value = taskId;
      queriedPriceTaskId.value = taskId;
      currentPriceJob.value = buildPreviewPriceJob(taskId);
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "price.create_update_job",
        status: "queued",
        progress: 0,
        created_at: "2026-05-22T00:11:00+08:00",
        started_at: null,
        finished_at: null,
        pending_count: 1,
        ready_count: 0,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        status: "queued",
        accepted_product_count: 1,
        target_shop_count: 1,
      } as T;
    }
    if (name === "get_price_update_job") {
      return (currentPriceJob.value ||
        buildPreviewPriceJob(String(args?.taskId || "price_preview"))) as T;
    }
    if (name === "list_task_runs") {
      return previewTaskRuns.value as T;
    }
    if (name === "get_local_api_config") {
      return localApiConfig.value as T;
    }
    if (name === "get_ai_provider_settings") {
      return aiProviderSettings.value as T;
    }
    if (name === "save_ai_provider_settings") {
      const request = args?.request as
        | {
            enabled?: boolean;
            provider_type?: string;
            custom_provider_id?: string;
            api?: string;
            base_url?: string;
            model?: string;
            temperature?: number;
            context_window?: number;
            max_tokens?: number;
            api_key?: string;
            clear_api_key?: boolean;
          }
        | undefined;
      aiProviderSettings.value = {
        enabled: Boolean(request?.enabled),
        provider_type: request?.provider_type || "custom",
        custom_provider_id: request?.custom_provider_id || "wx-xd-custom",
        api: request?.api || "openai-completions",
        base_url: request?.base_url || "",
        model: request?.model || "",
        temperature: request?.temperature ?? 0.1,
        context_window: request?.context_window ?? 128000,
        max_tokens: request?.max_tokens ?? 4096,
        has_api_key:
          Boolean(request?.api_key) ||
          (aiProviderSettings.value.has_api_key && !request?.clear_api_key),
        api_key_hint: request?.api_key
          ? `指纹 ${request.api_key.slice(-8)}`
          : request?.clear_api_key
            ? null
            : aiProviderSettings.value.api_key_hint,
        updated_at: "2026-05-22T00:26:00+08:00",
      };
      return aiProviderSettings.value as T;
    }
    if (name === "test_ai_provider") {
      if (!aiProviderSettings.value.enabled) {
        throw new Error("AI Agent 未启用");
      }
      return {
        status: "success",
        provider_type: aiProviderSettings.value.provider_type,
        model: aiProviderSettings.value.model,
        message: "pi-coding-agent 连通成功：预览模式",
      } as T;
    }
    if (name === "list_agent_skills") {
      return previewAgentSkills as T;
    }
    if (name === "list_agent_runs") {
      const scene = String(args?.scene || "all");
      const status = String(args?.status || "all");
      const limit = Number(args?.limit || 80);
      return previewAgentRuns
        .filter(
          (run) =>
            (scene === "all" || run.scene === scene) &&
            (status === "all" || run.status === status),
        )
        .slice(0, limit) as T;
    }
    if (name === "save_agent_skill_settings") {
      const request = args?.request as AgentSkillSettingsRequest | undefined;
      previewAgentSkills = previewAgentSkills.map((skill) => {
        if (skill.name !== request?.name) return skill;
        return {
          ...skill,
          enabled: Boolean(request.enabled),
          model: request.model || null,
          temperature: request.temperature ?? null,
          updated_at: "2026-05-22T00:26:00+08:00",
        };
      });
      return previewAgentSkills.find(
        (skill) => skill.name === request?.name,
      ) as T;
    }
    if (name === "test_agent_skill") {
      const skillName = String(args?.name || "wx-xd-product-review");
      if (!aiProviderSettings.value.enabled) {
        throw new Error("AI Agent 未启用");
      }
      const result: AgentSkillTestResult = {
        name: skillName,
        status: "success",
        summary: "预览模式技能试跑成功",
        checked_at: "2026-05-22T00:26:00+08:00",
      };
      previewAgentSkills = previewAgentSkills.map((skill) =>
        skill.name === skillName
          ? {
              ...skill,
              last_test_status: result.status,
              last_test_summary: result.summary,
              last_test_at: result.checked_at,
            }
          : skill,
      );
      return result as T;
    }
    if (name === "list_external_api_logs") {
      return previewExternalApiLogs.value as T;
    }
    if (name === "list_notifications") {
      const status = String(args?.status || "all");
      const severity = String(args?.severity || "all");
      const items = previewNotifications.value.filter((item) => {
        const statusMatched = status === "all" || item.status === status;
        const severityMatched =
          severity === "all" || item.severity === severity;
        return statusMatched && severityMatched;
      });
      return {
        items,
        total: items.length,
        unread_count: previewNotifications.value.filter(
          (item) => item.status === "unread",
        ).length,
        critical_count: previewNotifications.value.filter(
          (item) => item.status === "unread" && item.severity === "critical",
        ).length,
      } as T;
    }
    if (name === "mark_notification_read") {
      const notificationId = String(args?.notificationId || "");
      const notification = previewNotifications.value.find(
        (item) => item.id === notificationId,
      );
      if (!notification) {
        throw new Error("通知不存在");
      }
      const updated = notification.status === "read" ? 0 : 1;
      notification.status = "read";
      notification.read_at =
        notification.read_at || "2026-05-22T00:25:00+08:00";
      notification.updated_at = "2026-05-22T00:25:00+08:00";
      return {
        updated_count: updated,
        message: updated > 0 ? "通知已标记已读" : "通知已经是已读状态",
      } as T;
    }
    if (name === "mark_all_notifications_read") {
      let updated = 0;
      previewNotifications.value.forEach((notification) => {
        if (notification.status !== "read") {
          updated += 1;
          notification.status = "read";
          notification.read_at = "2026-05-22T00:25:00+08:00";
          notification.updated_at = "2026-05-22T00:25:00+08:00";
        }
      });
      return {
        updated_count: updated,
        message: `已标记 ${updated} 条通知为已读`,
      } as T;
    }
    if (name === "list_database_backups") {
      return previewDatabaseBackups.value as T;
    }
    if (name === "create_database_backup") {
      const backup: BackupInfo = {
        id: `wx-xd-manual-preview-${Date.now()}`,
        file_name: `wx-xd-manual-preview-${Date.now()}.sqlite`,
        file_path: `/tmp/wx-xd-backups/wx-xd-manual-preview-${Date.now()}.sqlite`,
        size_bytes: 5242880,
        sha256: "preview-sha256",
        created_at: "2026-05-22T00:15:00+08:00",
        integrity_ok: true,
        integrity_message: "ok",
      };
      previewDatabaseBackups.value.unshift(backup);
      return { backup } as T;
    }
    if (name === "restore_database_backup") {
      const request = args?.request as { file_path?: string } | undefined;
      const restored = previewDatabaseBackups.value.find(
        (item) => item.file_path === request?.file_path,
      );
      if (!restored) {
        throw new Error("备份文件不存在");
      }
      const rollback: BackupInfo = {
        id: `wx-xd-pre-restore-preview-${Date.now()}`,
        file_name: `wx-xd-pre-restore-preview-${Date.now()}.sqlite`,
        file_path: `/tmp/wx-xd-backups/wx-xd-pre-restore-preview-${Date.now()}.sqlite`,
        size_bytes: 5242880,
        sha256: "preview-rollback-sha256",
        created_at: "2026-05-22T00:16:00+08:00",
        integrity_ok: true,
        integrity_message: "ok",
      };
      previewDatabaseBackups.value.unshift(rollback);
      return {
        restored_from: restored,
        rollback_backup: rollback,
        integrity_ok: true,
        message:
          "数据库已恢复并通过完整性校验，建议重启应用以确保所有页面读取最新连接。",
      } as T;
    }
    if (name === "rotate_local_api_key") {
      const apiKey = `preview_${Math.random().toString(36).slice(2)}_${Date.now()}`;
      localApiConfig.value = {
        ...localApiConfig.value,
        has_api_key: true,
        api_key_hint: `末尾 ${apiKey.slice(-6)}`,
      };
      return {
        api_key: apiKey,
        key_hint: localApiConfig.value.api_key_hint,
        base_url: localApiConfig.value.base_url,
        warning: "预览模式生成的是示例 Key。",
      } as T;
    }
    if (name === "get_delivery_settings") {
      return deliverySettings.value as T;
    }
    if (name === "list_delivery_companies") {
      const shopId = String(args?.shopId || "shop-preview");
      return fallbackDeliveryCompanyOptions.map((company) => ({
        shop_id: shopId,
        delivery_id: company.value,
        delivery_name: company.label,
        synced_at: "2026-05-22T00:30:00+08:00",
      })) as T;
    }
    if (name === "sync_delivery_companies") {
      const shopId = String(args?.shopId || "shop-preview");
      const taskId = `delivery_company_sync_preview_${Date.now()}`;
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "delivery.sync_company_list",
        status: "success",
        progress: 100,
        pending_count: 0,
        ready_count: fallbackDeliveryCompanyOptions.length,
        failed_count: 0,
        started_at: "2026-05-22T00:30:00+08:00",
        finished_at: "2026-05-22T00:30:00+08:00",
        created_at: "2026-05-22T00:30:00+08:00",
      });
      return {
        task_id: taskId,
        shop_id: shopId,
        synced_companies: fallbackDeliveryCompanyOptions.length,
        failed_steps: [],
      } as T;
    }
    if (name === "list_aftersale_reject_reasons") {
      const shopId = String(args?.shopId || "");
      const rejectScene =
        args?.rejectScene === undefined || args.rejectScene === null
          ? null
          : Number(args.rejectScene);
      return previewAftersaleRejectReasons.value
        .filter((reason) => !shopId || reason.shop_id === shopId)
        .filter(
          (reason) =>
            rejectScene === null || reason.reject_scene === rejectScene,
        ) as T;
    }
    if (name === "sync_aftersale_reject_reasons") {
      const shopId = String(args?.shopId || "shop-preview");
      const taskId = `aftersale_reject_reason_sync_preview_${Date.now()}`;
      const now = "2026-05-22T00:24:00+08:00";
      previewAftersaleRejectReasons.value =
        previewAftersaleRejectReasons.value.map((reason) =>
          reason.shop_id === shopId ? { ...reason, synced_at: now } : reason,
        );
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "aftersales.sync_reject_reasons",
        status: "success",
        progress: 100,
        pending_count: 0,
        ready_count: previewAftersaleRejectReasons.value.filter(
          (reason) => reason.shop_id === shopId,
        ).length,
        failed_count: 0,
        started_at: now,
        finished_at: now,
        created_at: now,
      });
      return {
        task_id: taskId,
        shop_id: shopId,
        synced_reasons: previewAftersaleRejectReasons.value.filter(
          (reason) => reason.shop_id === shopId,
        ).length,
        failed_steps: [],
      } as T;
    }
    if (name === "get_automation_settings") {
      return automationSettings.value as T;
    }
    if (name === "set_automation_settings") {
      automationSettings.value = {
        ...defaultAutomationSettings(),
        ...(args?.settings as
          | Partial<OperationalAutomationSettings>
          | undefined),
      };
      return automationSettings.value as T;
    }
    if (name === "get_publish_pricing_strategy") {
      return publishPricingStrategy.value as T;
    }
    if (name === "save_publish_pricing_strategy") {
      publishPricingStrategy.value = {
        ...defaultPublishPricingStrategy(),
        ...(args?.strategy as Partial<PublishPricingStrategy> | undefined),
      };
      return publishPricingStrategy.value as T;
    }
    if (name === "run_publish_pipeline_once") {
      const result: PublishPipelineRunResult = {
        executed_steps: [],
        skipped_steps: [],
        errors: [],
        publish_precheck: null,
        publish_attribute_fill: null,
        publish_category_precheck: null,
        publish_asset_upload: null,
        publish_submit: null,
        publish_status_sync: null,
        publish_listing: null,
      };
      const runStep = async <R>(
        step: string,
        commandName: string,
        commandArgs: Record<string, unknown> | undefined,
        assign: (stepResult: R) => void,
      ) => {
        try {
          const stepResult = await previewCommand<R>(commandName, commandArgs);
          result.executed_steps.push(step);
          assign(stepResult);
        } catch (error) {
          result.errors.push({ step, error: String(error) });
        }
      };
      await runStep<PublishTaskBatchResult>(
        "publish.precheck_products",
        "run_publish_tasks_once",
        { limit: 50 },
        (stepResult) => {
          result.publish_precheck = stepResult;
        },
      );
      await runStep<PublishAttributeFillBatchResult>(
        "publish.fill_required_attributes",
        "run_publish_attribute_fill_once",
        { limit: 50 },
        (stepResult) => {
          result.publish_attribute_fill = stepResult;
        },
      );
      await runStep<PublishCategoryPrecheckBatchResult>(
        "publish.category_precheck",
        "run_publish_category_prechecks_once",
        { limit: 20 },
        (stepResult) => {
          result.publish_category_precheck = stepResult;
        },
      );
      await runStep<AssetUploadBatchResult>(
        "publish.upload_assets",
        "run_publish_asset_uploads_once",
        { limit: 10 },
        (stepResult) => {
          result.publish_asset_upload = stepResult;
        },
      );
      await runStep<ProductSubmitBatchResult>(
        "publish.submit_products",
        "run_publish_submits_once",
        { limit: 10 },
        (stepResult) => {
          result.publish_submit = stepResult;
        },
      );
      await runStep<ProductStatusSyncBatchResult>(
        "publish.sync_status",
        "run_publish_status_sync_once",
        { limit: 20 },
        (stepResult) => {
          result.publish_status_sync = stepResult;
        },
      );
      await runStep<ProductListingBatchResult>(
        "publish.listing_products",
        "run_publish_listing_once",
        { limit: 10 },
        (stepResult) => {
          result.publish_listing = stepResult;
        },
      );
      return result as T;
    }
    if (name === "run_operational_automation_once") {
      const result: OperationalAutomationRunResult = {
        executed_steps: [],
        skipped_steps: [],
        errors: [],
        order_sync: null,
        order_detail_sync: null,
        aftersale_sync: null,
        purchase_task_generation: null,
        delivery_submission: null,
        publish_precheck: null,
        publish_attribute_fill: null,
        publish_category_precheck: null,
        publish_asset_upload: null,
        publish_submit: null,
        publish_status_sync: null,
        publish_listing: null,
        price_confirm: null,
      };
      const runStep = async <R>(
        enabled: boolean,
        step: string,
        commandName: string,
        commandArgs: Record<string, unknown> | undefined,
        assign: (stepResult: R) => void,
      ) => {
        if (!enabled) {
          result.skipped_steps.push(step);
          return;
        }
        try {
          const stepResult = await previewCommand<R>(commandName, commandArgs);
          result.executed_steps.push(step);
          assign(stepResult);
        } catch (error) {
          result.errors.push({ step, error: String(error) });
        }
      };
      await runStep<OrderSyncBatchResult>(
        automationSettings.value.order_sync_enabled,
        "orders.sync_shop_orders",
        "run_order_sync_once",
        { lookbackDays: 1, pageSize: 100 },
        (stepResult) => {
          result.order_sync = stepResult;
        },
      );
      await runStep<OrderDetailSyncBatchResult>(
        automationSettings.value.order_detail_sync_enabled,
        "orders.sync_order_details",
        "run_order_detail_sync_once",
        { limit: 50 },
        (stepResult) => {
          result.order_detail_sync = stepResult;
        },
      );
      await runStep<AftersaleSyncBatchResult>(
        automationSettings.value.aftersale_sync_enabled,
        "aftersales.sync_shop_aftersales",
        "run_aftersale_sync_once",
        { lookbackHours: 24, limit: 200 },
        (stepResult) => {
          result.aftersale_sync = stepResult;
        },
      );
      await runStep<PurchaseTaskBatchResult>(
        automationSettings.value.purchase_task_enabled,
        "procurement.create_purchase_tasks",
        "run_purchase_task_generation_once",
        { limit: 100 },
        (stepResult) => {
          result.purchase_task_generation = stepResult;
        },
      );
      await runStep<DeliverySubmitBatchResult>(
        automationSettings.value.delivery_submission_enabled,
        "delivery.submit_wechat_shipment",
        "run_delivery_submission_once",
        { limit: 20 },
        (stepResult) => {
          result.delivery_submission = stepResult;
        },
      );
      await runStep<PublishTaskBatchResult>(
        automationSettings.value.publish_precheck_enabled,
        "publish.precheck_products",
        "run_publish_tasks_once",
        { limit: 50 },
        (stepResult) => {
          result.publish_precheck = stepResult;
        },
      );
      await runStep<PublishAttributeFillBatchResult>(
        automationSettings.value.publish_attribute_fill_enabled,
        "publish.fill_required_attributes",
        "run_publish_attribute_fill_once",
        { limit: 50 },
        (stepResult) => {
          result.publish_attribute_fill = stepResult;
        },
      );
      await runStep<PublishCategoryPrecheckBatchResult>(
        automationSettings.value.publish_category_precheck_enabled,
        "publish.category_precheck",
        "run_publish_category_prechecks_once",
        { limit: 20 },
        (stepResult) => {
          result.publish_category_precheck = stepResult;
        },
      );
      await runStep<AssetUploadBatchResult>(
        automationSettings.value.publish_asset_upload_enabled,
        "publish.upload_assets",
        "run_publish_asset_uploads_once",
        { limit: 10 },
        (stepResult) => {
          result.publish_asset_upload = stepResult;
        },
      );
      await runStep<ProductSubmitBatchResult>(
        automationSettings.value.publish_submit_enabled,
        "publish.submit_products",
        "run_publish_submits_once",
        { limit: 10 },
        (stepResult) => {
          result.publish_submit = stepResult;
        },
      );
      await runStep<ProductStatusSyncBatchResult>(
        automationSettings.value.publish_status_sync_enabled,
        "publish.sync_status",
        "run_publish_status_sync_once",
        { limit: 20 },
        (stepResult) => {
          result.publish_status_sync = stepResult;
        },
      );
      await runStep<ProductListingBatchResult>(
        automationSettings.value.publish_listing_enabled,
        "publish.listing_products",
        "run_publish_listing_once",
        { limit: 10 },
        (stepResult) => {
          result.publish_listing = stepResult;
        },
      );
      await runStep<PriceUpdateConfirmBatchResult>(
        automationSettings.value.price_confirm_enabled,
        "price.confirm",
        "run_price_update_confirm_once",
        { limit: 50 },
        (stepResult) => {
          result.price_confirm = stepResult;
        },
      );
      return result as T;
    }
    if (name === "set_auto_send_delivery") {
      deliverySettings.value.auto_send_delivery = Boolean(args?.enabled);
      if (deliverySettings.value.auto_send_delivery) {
        previewShipments.value.forEach((shipment) => {
          if (shipment.status === "waiting_confirmation") {
            shipment.status = "ready_to_send";
            shipment.updated_at = "2026-05-22T00:09:30+08:00";
          }
        });
      }
      return deliverySettings.value as T;
    }
    if (name === "list_delivery_shipments") {
      const status = String(args?.status || "all");
      const items =
        status === "all"
          ? previewShipments.value
          : previewShipments.value.filter((item) => item.status === status);
      return {
        items,
        total: items.length,
      } as T;
    }
    if (name === "retry_delivery_shipment") {
      const shipmentId = String(args?.shipmentId || "");
      const shipment = previewShipments.value.find(
        (item) => item.id === shipmentId,
      );
      if (!shipment) {
        throw new Error("物流单不存在");
      }
      if (shipment.status === "wechat_shipped") {
        throw new Error("已发货成功的物流单不能重试");
      }
      shipment.status = deliverySettings.value.auto_send_delivery
        ? "ready_to_send"
        : "waiting_confirmation";
      shipment.error_code = null;
      shipment.error_summary = null;
      shipment.updated_at = "2026-05-22T00:09:40+08:00";
      return {
        shipment_id: shipment.id,
        status: shipment.status,
        auto_send_enabled: deliverySettings.value.auto_send_delivery,
        message: deliverySettings.value.auto_send_delivery
          ? "物流单已重新进入待提交微信发货队列"
          : "自动发货开关关闭，物流单已回到待确认状态",
      } as T;
    }
    if (name === "list_purchase_tasks") {
      const status = String(args?.status || "all");
      const items =
        status === "all"
          ? previewPurchaseTasks.value
          : previewPurchaseTasks.value.filter((item) => item.status === status);
      return {
        items,
        total: items.length,
      } as T;
    }
    if (name === "resolve_purchase_task_mapping") {
      const request = args?.request as
        | {
            purchase_task_id?: string;
            external_product_id?: string;
            external_sku_id?: string;
            source_url?: string | null;
            supplier_name?: string | null;
            supplier_product_id?: string | null;
            estimated_cost?: number | null;
            note?: string | null;
          }
        | undefined;
      const task = previewPurchaseTasks.value.find(
        (item) => item.id === request?.purchase_task_id,
      );
      const externalProductId = request?.external_product_id?.trim() || "";
      const externalSkuId = request?.external_sku_id?.trim() || "";
      if (!task || !externalProductId || !externalSkuId) {
        throw new Error("采购映射参数不完整");
      }
      if (
        task.status !== "needs_mapping" &&
        task.status !== "pending_purchase"
      ) {
        throw new Error("只有待映射或待采购任务允许补齐映射");
      }
      task.status = "pending_purchase";
      task.external_product_id = externalProductId;
      task.external_sku_id = externalSkuId;
      task.source_url = request?.source_url?.trim() || task.source_url;
      task.supplier_name = request?.supplier_name?.trim() || task.supplier_name;
      task.supplier_product_id =
        request?.supplier_product_id?.trim() || task.supplier_product_id;
      task.estimated_cost = request?.estimated_cost ?? task.estimated_cost;
      task.estimated_profit =
        task.estimated_cost === null || task.estimated_cost === undefined
          ? null
          : (task.estimated_revenue || 0) / 100 - task.estimated_cost;
      task.error_summary = null;
      task.updated_at = "2026-05-22T00:22:00+08:00";
      previewNotifications.value.forEach((notification) => {
        if (
          notification.source_type === "purchase_mapping" &&
          notification.status === "unread"
        ) {
          notification.status = "read";
          notification.read_at = "2026-05-22T00:22:00+08:00";
          notification.updated_at = "2026-05-22T00:22:00+08:00";
        }
      });
      previewNotifications.value.unshift({
        id: `notification-preview-purchase-mapping-resolved-${Date.now()}`,
        severity: "info",
        source_type: "purchase_mapping_resolved",
        source_id: task.id,
        shop_id: task.shop_id,
        shop_name: task.shop_name,
        title: "采购任务映射已补齐",
        body: `订单 ${task.wechat_order_id} 的采购任务已补齐外部商品映射，重新进入待采购。`,
        status: "unread",
        data_json: JSON.stringify({
          purchase_task_id: task.id,
          order_id: task.order_id,
          has_source_url: Boolean(request?.source_url?.trim()),
          has_note: Boolean(request?.note?.trim()),
        }),
        read_at: null,
        created_at: "2026-05-22T00:22:00+08:00",
        updated_at: "2026-05-22T00:22:00+08:00",
      });
      return {
        purchase_task_id: task.id,
        order_id: task.order_id,
        status: task.status,
        external_product_id: externalProductId,
        external_sku_id: externalSkuId,
        message: "采购任务映射已补齐，已重新进入待采购",
      } as T;
    }
    if (name === "list_inventory_risks") {
      const status = String(args?.status || "all");
      const allItems = buildPreviewInventoryRisks();
      const items =
        status === "all"
          ? allItems
          : allItems.filter((item) => item.risk_status === status);
      return {
        items,
        total: items.length,
        low_stock_count: items.filter(
          (item) =>
            item.risk_status === "low_stock" ||
            item.risk_status === "stock_pressure",
        ).length,
        out_of_stock_count: items.filter(
          (item) => item.risk_status === "out_of_stock",
        ).length,
        issue_count: items.filter(
          (item) => item.risk_status === "supplier_issue",
        ).length,
      } as T;
    }
    if (name === "list_product_sales_analysis") {
      const status = String(args?.status || "all");
      const allItems = buildPreviewProductSalesAnalysis();
      const items =
        status === "all"
          ? allItems
          : allItems.filter(
              (item) =>
                item.operation_status === status ||
                item.inventory_risk_status === status,
            );
      return {
        items,
        total: items.length,
        totals: summarizeProductSalesAnalysis(items),
      } as T;
    }
    if (name === "list_product_management_items") {
      const status = String(args?.status || "all");
      const keyword = String(args?.keyword || "")
        .trim()
        .toLowerCase();
      const allItems =
        buildPreviewProductSalesAnalysis().map<ProductManagementView>(
          (item) => {
            const managementStatus =
              item.inventory_risk_status !== "healthy" &&
              item.inventory_risk_status !== "not_listed"
                ? "inventory_risk"
                : item.active_shop_count <= 0
                  ? "not_listed"
                  : item.order_count > 0
                    ? "listed_sold"
                    : "listed_unsold";
            return {
              ...item,
              source_url: "https://example.com/source",
              supplier_product_id: null,
              management_status: managementStatus,
              publish_status: null,
              publish_error_summary: null,
              shop_count: item.active_shop_count,
              shops: [],
            };
          },
        );
      const items = allItems.filter(
        (item) =>
          (status === "all" ||
            item.management_status === status ||
            item.inventory_risk_status === status ||
            item.operation_status === status) &&
          (!keyword ||
            item.external_product_id.toLowerCase().includes(keyword) ||
            item.title.toLowerCase().includes(keyword)),
      );
      return { items, total: items.length } as T;
    }
    if (name === "run_inventory_risk_scan_once") {
      const items = buildPreviewInventoryRisks();
      const riskyItems = items.filter(
        (item) =>
          item.risk_status !== "healthy" && item.risk_status !== "not_listed",
      );
      riskyItems.forEach((item) => {
        previewNotifications.value.unshift({
          id: `notification-preview-inventory-${item.external_product_id}-${Date.now()}`,
          severity:
            item.risk_status === "out_of_stock" ? "critical" : "warning",
          source_type: "inventory_risk",
          source_id: item.external_product_id,
          shop_id: null,
          shop_name: null,
          title: `库存风控：${item.title}`,
          body: `${item.recommendation}；可用库存 ${item.available_stock}，采购占用 ${item.reserved_quantity}。`,
          status: "unread",
          data_json: JSON.stringify({
            external_product_id: item.external_product_id,
            risk_status: item.risk_status,
          }),
          read_at: null,
          created_at: "2026-05-22T00:35:00+08:00",
          updated_at: "2026-05-22T00:35:00+08:00",
        });
      });
      const taskId = `inventory_risk_scan_preview_${Date.now()}`;
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "inventory.scan_risks",
        status: "success",
        progress: 100,
        pending_count: 0,
        ready_count: items.length,
        failed_count: 0,
        started_at: "2026-05-22T00:35:00+08:00",
        finished_at: "2026-05-22T00:35:00+08:00",
        created_at: "2026-05-22T00:35:00+08:00",
      });
      return {
        task_id: taskId,
        scanned_products: items.length,
        low_stock_products: items.filter(
          (item) =>
            item.risk_status === "low_stock" ||
            item.risk_status === "stock_pressure",
        ).length,
        out_of_stock_products: items.filter(
          (item) => item.risk_status === "out_of_stock",
        ).length,
        issue_products: items.filter(
          (item) => item.risk_status === "supplier_issue",
        ).length,
        notifications_created: riskyItems.length,
      } as T;
    }
    if (name === "mark_purchase_task_issue") {
      const request = args?.request as
        | {
            purchase_task_id?: string;
            issue_type?: string;
            note?: string | null;
          }
        | undefined;
      const task = previewPurchaseTasks.value.find(
        (item) => item.id === request?.purchase_task_id,
      );
      if (!task || !request?.issue_type) {
        throw new Error("采购异常参数不完整");
      }
      const mapping: Record<string, { status: string; label: string }> = {
        out_of_stock: { status: "supplier_out_of_stock", label: "供应商缺货" },
        price_changed: {
          status: "supplier_price_changed",
          label: "供应商涨价",
        },
        supplier_cancelled: {
          status: "supplier_cancelled",
          label: "供应商取消",
        },
        quality_risk: {
          status: "supplier_quality_risk",
          label: "供应商质量风险",
        },
        other: { status: "supplier_exception", label: "供应商其他异常" },
      };
      const issue = mapping[request.issue_type] || mapping.other;
      task.status = issue.status;
      task.error_summary = request.note
        ? `${issue.label}：${request.note}`
        : issue.label;
      task.updated_at = "2026-05-22T00:21:00+08:00";
      previewNotifications.value.unshift({
        id: `notification-preview-purchase-issue-${Date.now()}`,
        severity: "warning",
        source_type: "purchase_issue",
        source_id: task.id,
        shop_id: task.shop_id,
        shop_name: task.shop_name,
        title: "采购任务供应商异常",
        body: `订单 ${task.wechat_order_id} 的采购任务已标记为${issue.label}，需人工处理。`,
        status: "unread",
        data_json: JSON.stringify({
          purchase_task_id: task.id,
          order_id: task.order_id,
          status: issue.status,
        }),
        read_at: null,
        created_at: "2026-05-22T00:21:00+08:00",
        updated_at: "2026-05-22T00:21:00+08:00",
      });
      return {
        purchase_task_id: task.id,
        order_id: task.order_id,
        status: task.status,
        message: `${issue.label}已记录，订单已进入异常待处理`,
      } as T;
    }
    if (name === "list_order_profit_summaries") {
      return buildPreviewOrderProfitResult(String(args?.status || "all")) as T;
    }
    if (name === "list_order_management_items") {
      const status = String(args?.status || "all");
      const keyword = String(args?.keyword || "")
        .trim()
        .toLowerCase();
      const allItems = buildPreviewOrderProfitResult(
        "all",
      ).items.map<OrderManagementView>((item) => {
        const managementStatus = !item.detail_synced_at
          ? "needs_detail"
          : item.purchase_task_count <= 0 || item.missing_cost_count > 0
            ? "needs_purchase"
            : item.order_status === "pending_shipment"
              ? "needs_shipment"
              : item.order_status;
        return {
          order_id: item.order_id,
          wechat_order_id: item.wechat_order_id,
          shop_id: item.shop_id,
          shop_name: item.shop_name,
          wechat_status: null,
          order_status: item.order_status,
          management_status: managementStatus,
          item_count: item.item_count,
          quantity: item.quantity,
          revenue_cents: item.revenue_cents,
          purchase_task_count: item.purchase_task_count,
          missing_cost_count: item.missing_cost_count,
          purchase_status:
            item.purchase_task_count <= 0
              ? "missing_purchase_task"
              : item.missing_cost_count > 0
                ? "missing_cost"
                : "completed",
          shipment_count: item.order_status === "wechat_shipped" ? 1 : 0,
          shipment_status:
            item.order_status === "wechat_shipped" ? "wechat_shipped" : "none",
          active_aftersale_count:
            item.order_status === "aftersale_active" ? 1 : 0,
          profit_status: item.profit_status,
          estimated_profit_cents: item.estimated_profit_cents,
          actual_profit_cents: item.actual_profit_cents,
          detail_synced_at: item.detail_synced_at,
          detail_error: null,
          synced_at: item.updated_at,
          order_created_at: null,
          order_updated_at: null,
          updated_at: item.updated_at,
          items: [],
        };
      });
      const items = allItems.filter(
        (item) =>
          (status === "all" ||
            item.management_status === status ||
            item.order_status === status ||
            item.purchase_status === status ||
            item.profit_status === status) &&
          (!keyword ||
            item.order_id.toLowerCase().includes(keyword) ||
            item.wechat_order_id.toLowerCase().includes(keyword) ||
            item.shop_name.toLowerCase().includes(keyword)),
      );
      return { items, total: items.length } as T;
    }
    if (name === "list_aftersales") {
      const status = String(args?.status || "all");
      const items =
        status === "all"
          ? previewAftersales.value
          : status === "active"
            ? previewAftersales.value.filter((item) =>
                canSubmitAftersaleAction(item),
              )
            : previewAftersales.value.filter((item) => item.status === status);
      return {
        items,
        total: items.length,
      } as T;
    }
    if (name === "list_aftersale_evidence") {
      const targetType = args?.targetType ? String(args.targetType) : "all";
      const targetId = args?.targetId ? String(args.targetId) : "";
      const status = args?.status ? String(args.status) : "all";
      const items = previewAftersaleEvidence.value.filter(
        (item) =>
          (targetType === "all" || item.target_type === targetType) &&
          (!targetId ||
            item.target_id === targetId ||
            item.external_target_id === targetId) &&
          (status === "all" || item.status === status),
      );
      return {
        items,
        total: items.length,
      } as T;
    }
    if (name === "list_guarantee_orders") {
      const status = String(args?.status || "all");
      const items =
        status === "all"
          ? previewGuaranteeOrders.value
          : status === "active"
            ? previewGuaranteeOrders.value.filter((item) =>
                isActiveGuaranteeStatus(item.status),
              )
            : previewGuaranteeOrders.value.filter(
                (item) => item.status === status,
              );
      return {
        items,
        total: items.length,
      } as T;
    }
    if (name === "record_aftersale_evidence") {
      const request = args?.request as
        | {
            target_type?: string;
            target_id?: string;
            evidence_type?: string;
            title?: string;
            content_text?: string | null;
            local_file_path?: string | null;
            source_url?: string | null;
            status?: string | null;
          }
        | undefined;
      if (
        !request?.target_type ||
        !request.target_id ||
        !request.evidence_type ||
        !request.title
      ) {
        throw new Error("凭证资料参数不完整");
      }
      const target =
        request.target_type === "guarantee"
          ? previewGuaranteeOrders.value.find(
              (item) =>
                item.id === request.target_id ||
                item.guarantee_order_id === request.target_id,
            )
          : previewAftersales.value.find(
              (item) =>
                item.id === request.target_id ||
                item.wechat_aftersale_id === request.target_id,
            );
      if (!target) {
        throw new Error("凭证目标不存在");
      }
      const externalTargetId =
        request.target_type === "guarantee"
          ? (target as GuaranteeOrderView).guarantee_order_id
          : (target as AftersaleView).wechat_aftersale_id;
      const now = "2026-05-22T00:34:00+08:00";
      const evidenceType = request.evidence_type;
      const status = request.status || "draft";
      const evidenceId = `evidence-preview-${Date.now()}`;
      previewAftersaleEvidence.value.unshift({
        id: evidenceId,
        target_type: request.target_type,
        target_id: target.id,
        shop_id: target.shop_id,
        shop_name: target.shop_name,
        external_target_id: externalTargetId,
        evidence_type: evidenceType,
        evidence_type_text: evidenceTypeLabel(evidenceType),
        title: request.title,
        content_text: request.content_text || null,
        local_file_path: request.local_file_path || null,
        source_url: request.source_url || null,
        status,
        status_text: evidenceStatusLabel(status),
        created_at: now,
        updated_at: now,
      });
      target.evidence_count += 1;
      previewNotifications.value.unshift({
        id: `notification-preview-evidence-${Date.now()}`,
        severity: "info",
        source_type: "aftersale_evidence",
        source_id: evidenceId,
        shop_id: target.shop_id,
        shop_name: target.shop_name,
        title:
          request.target_type === "guarantee"
            ? "纠纷凭证资料已记录"
            : "售后凭证资料已记录",
        body: `${request.target_type === "guarantee" ? "纠纷单" : "售后单"} ${externalTargetId} 已记录本地凭证资料：${request.title}。`,
        status: "unread",
        data_json: JSON.stringify({
          evidence_id: evidenceId,
          target_type: request.target_type,
          external_target_id: externalTargetId,
          evidence_type: evidenceType,
          status,
          has_content_text: Boolean(request.content_text),
          has_local_file_path: Boolean(request.local_file_path),
          has_source_url: Boolean(request.source_url),
        }),
        read_at: null,
        created_at: now,
        updated_at: now,
      });
      return {
        evidence_id: evidenceId,
        target_type: request.target_type,
        target_id: target.id,
        evidence_type: evidenceType,
        status,
        message: "本地凭证资料已记录，尚未上传微信或提交平台处理",
      } as T;
    }
    if (name === "update_aftersale_evidence_status") {
      const request = args?.request as
        | {
            evidence_id?: string;
            status?: string;
          }
        | undefined;
      const evidence = previewAftersaleEvidence.value.find(
        (item) => item.id === request?.evidence_id,
      );
      if (!evidence || !request?.status) {
        throw new Error("凭证状态参数不完整");
      }
      const now = "2026-05-22T00:36:00+08:00";
      evidence.status = request.status;
      evidence.status_text = evidenceStatusLabel(request.status);
      evidence.updated_at = now;
      previewNotifications.value.unshift({
        id: `notification-preview-evidence-status-${Date.now()}`,
        severity: "info",
        source_type: "aftersale_evidence",
        source_id: evidence.id,
        shop_id: evidence.shop_id,
        shop_name: evidence.shop_name,
        title: "本地凭证状态已更新",
        body: `${evidence.target_type === "guarantee" ? "纠纷单" : "售后单"} ${evidence.external_target_id} 的本地凭证「${evidence.title}」已标记为${evidence.status_text}。`,
        status: "unread",
        data_json: JSON.stringify({
          evidence_id: evidence.id,
          target_type: evidence.target_type,
          target_id: evidence.target_id,
          external_target_id: evidence.external_target_id,
          status: evidence.status,
        }),
        read_at: null,
        created_at: now,
        updated_at: now,
      });
      return {
        evidence_id: evidence.id,
        status: evidence.status,
        status_text: evidence.status_text,
        message: "本地凭证状态已更新，尚未上传微信或提交平台处理",
      } as T;
    }
    if (name === "export_aftersale_evidence") {
      const targetType = args?.targetType ? String(args.targetType) : "all";
      const targetId = args?.targetId ? String(args.targetId) : "";
      const status = args?.status ? String(args.status) : "all";
      const exportFormat = String(args?.format || "md");
      const exportedCount = previewAftersaleEvidence.value.filter(
        (item) =>
          (targetType === "all" || item.target_type === targetType) &&
          (!targetId ||
            item.target_id === targetId ||
            item.external_target_id === targetId) &&
          (status === "all" || item.status === status),
      ).length;
      return {
        file_path: `/tmp/wx-xd-preview-aftersale-evidence.${exportFormat === "md" ? "md" : exportFormat}`,
        exported_count: exportedCount,
        format: exportFormat,
        sensitive_fields: "recipient_info_and_file_contents_excluded",
      } as T;
    }
    if (name === "list_supplier_aftersale_followups") {
      const targetType = args?.targetType ? String(args.targetType) : "all";
      const targetId = args?.targetId ? String(args.targetId) : "";
      const status = args?.status ? String(args.status) : "all";
      const items = previewSupplierAftersaleFollowups.value.filter(
        (item) =>
          (targetType === "all" || item.target_type === targetType) &&
          (!targetId ||
            item.target_id === targetId ||
            item.external_target_id === targetId) &&
          (status === "all" || item.status === status),
      );
      return {
        items,
        total: items.length,
      } as T;
    }
    if (name === "record_supplier_aftersale_followup") {
      const request = args?.request as
        | {
            target_type?: string;
            target_id?: string;
            followup_type?: string;
            status?: string;
            note?: string;
            purchase_task_id?: string | null;
            supplier_name?: string | null;
          }
        | undefined;
      if (
        !request?.target_type ||
        !request.target_id ||
        !request.followup_type ||
        !request.status ||
        !request.note
      ) {
        throw new Error("供应商协同参数不完整");
      }
      const target =
        request.target_type === "guarantee"
          ? previewGuaranteeOrders.value.find(
              (item) =>
                item.id === request.target_id ||
                item.guarantee_order_id === request.target_id,
            )
          : previewAftersales.value.find(
              (item) =>
                item.id === request.target_id ||
                item.wechat_aftersale_id === request.target_id,
            );
      if (!target) {
        throw new Error("协同目标不存在");
      }
      const externalTargetId =
        request.target_type === "guarantee"
          ? (target as GuaranteeOrderView).guarantee_order_id
          : (target as AftersaleView).wechat_aftersale_id;
      const now = "2026-05-22T00:37:00+08:00";
      const followupId = `supplier-followup-preview-${Date.now()}`;
      previewSupplierAftersaleFollowups.value.unshift({
        id: followupId,
        target_type: request.target_type,
        target_id: target.id,
        shop_id: target.shop_id,
        shop_name: target.shop_name,
        external_target_id: externalTargetId,
        purchase_task_id: request.purchase_task_id || null,
        supplier_name: request.supplier_name || null,
        followup_type: request.followup_type,
        followup_type_text: supplierFollowupTypeLabel(request.followup_type),
        status: request.status,
        status_text: supplierFollowupStatusLabel(request.status),
        note: request.note,
        created_at: now,
        updated_at: now,
      });
      previewNotifications.value.unshift({
        id: `notification-preview-supplier-followup-${Date.now()}`,
        severity: "info",
        source_type: "supplier_aftersale_followup",
        source_id: followupId,
        shop_id: target.shop_id,
        shop_name: target.shop_name,
        title: "供应商售后协同已记录",
        body: `${request.target_type === "guarantee" ? "纠纷单" : "售后单"} ${externalTargetId} 已记录供应商协同：${supplierFollowupTypeLabel(request.followup_type)}。`,
        status: "unread",
        data_json: JSON.stringify({
          followup_id: followupId,
          target_type: request.target_type,
          external_target_id: externalTargetId,
          followup_type: request.followup_type,
          status: request.status,
          purchase_task_id_present: Boolean(request.purchase_task_id),
          supplier_name_present: Boolean(request.supplier_name),
        }),
        read_at: null,
        created_at: now,
        updated_at: now,
      });
      return {
        followup_id: followupId,
        target_type: request.target_type,
        target_id: target.id,
        status: request.status,
        message:
          "供应商售后协同已记录，只保存本地脱敏备注，不调用供应商平台或微信处理接口",
      } as T;
    }
    if (name === "record_guarantee_followup") {
      const request = args?.request as
        | {
            guarantee_order_id?: string;
            handling_status?: string;
            responsibility_party?: string | null;
            handling_note?: string | null;
            supplier_compensation_cents?: number | null;
          }
        | undefined;
      const guarantee = previewGuaranteeOrders.value.find(
        (item) =>
          item.id === request?.guarantee_order_id ||
          item.guarantee_order_id === request?.guarantee_order_id,
      );
      if (!guarantee || !request?.handling_status) {
        throw new Error("纠纷跟进参数不完整");
      }
      const compensation = request.supplier_compensation_cents || 0;
      if (compensation > 0 && request.responsibility_party !== "supplier") {
        throw new Error("只有供应商责任才能记录纠纷供应商赔付金额");
      }
      const now = "2026-05-22T00:32:00+08:00";
      guarantee.handling_status = request.handling_status;
      guarantee.responsibility_party = request.responsibility_party || null;
      guarantee.handling_note = request.handling_note || null;
      guarantee.supplier_compensation_cents = compensation;
      guarantee.handled_at = now;
      guarantee.updated_at = now;
      const profitAdjustmentId =
        compensation > 0 && guarantee.order_id
          ? `profit-adj-guarantee-preview-${Date.now()}`
          : null;
      if (profitAdjustmentId && guarantee.order_id) {
        previewProfitAdjustments.value.push({
          order_id: guarantee.order_id,
          kind: "other_income",
          amount_cents: compensation,
        });
      }
      previewNotifications.value.unshift({
        id: `notification-preview-guarantee-followup-${Date.now()}`,
        severity:
          request.handling_status === "resolved" ||
          request.handling_status === "ignored"
            ? "info"
            : "warning",
        source_type: "guarantee_followup",
        source_id: guarantee.id,
        shop_id: guarantee.shop_id,
        shop_name: guarantee.shop_name,
        title: "纠纷单跟进已更新",
        body: `纠纷单 ${guarantee.guarantee_order_id} 已更新本地跟进状态。`,
        status: "unread",
        data_json: JSON.stringify({
          guarantee_order_id: guarantee.guarantee_order_id,
          handling_status: request.handling_status,
          responsibility_party: request.responsibility_party || null,
          has_note: Boolean(request.handling_note),
          supplier_compensation_cents: compensation,
        }),
        read_at: null,
        created_at: now,
        updated_at: now,
      });
      return {
        guarantee_order_id: guarantee.guarantee_order_id,
        handling_status: guarantee.handling_status,
        responsibility_party: guarantee.responsibility_party,
        supplier_compensation_cents: compensation,
        profit_adjustment_id: profitAdjustmentId,
        message: profitAdjustmentId
          ? "纠纷跟进已记录，供应商赔付已计入订单利润回款"
          : "纠纷跟进已记录",
      } as T;
    }
    if (name === "record_aftersale_responsibility") {
      const request = args?.request as
        | {
            aftersale_id?: string;
            responsibility_party?: string;
            responsibility_note?: string | null;
            supplier_compensation_cents?: number | null;
          }
        | undefined;
      const aftersale = previewAftersales.value.find(
        (item) =>
          item.id === request?.aftersale_id ||
          item.wechat_aftersale_id === request?.aftersale_id,
      );
      if (!aftersale || !request?.responsibility_party) {
        throw new Error("售后责任归因参数不完整");
      }
      const compensation = request.supplier_compensation_cents || 0;
      aftersale.responsibility_party = request.responsibility_party;
      aftersale.responsibility_note = request.responsibility_note || null;
      aftersale.supplier_compensation_cents = compensation;
      aftersale.handled_at = "2026-05-22T00:20:00+08:00";
      aftersale.updated_at = "2026-05-22T00:20:00+08:00";
      const profitAdjustmentId =
        compensation > 0 && aftersale.order_id
          ? `profit-adj-aftersale-preview-${Date.now()}`
          : null;
      if (profitAdjustmentId && aftersale.order_id) {
        previewProfitAdjustments.value.push({
          order_id: aftersale.order_id,
          kind: "other_income",
          amount_cents: compensation,
        });
      }
      return {
        aftersale_id: aftersale.id,
        responsibility_party: aftersale.responsibility_party,
        supplier_compensation_cents: compensation,
        profit_adjustment_id: profitAdjustmentId,
        message: profitAdjustmentId
          ? "售后责任已记录，供应商赔付已计入订单利润回款"
          : "售后责任已记录",
      } as T;
    }
    if (name === "accept_aftersale") {
      const request = args?.request as
        | {
            aftersale_id?: string;
            address_id?: string | null;
            accept_type?: number | null;
            note?: string | null;
          }
        | undefined;
      const aftersale = previewAftersales.value.find(
        (item) =>
          item.id === request?.aftersale_id ||
          item.wechat_aftersale_id === request?.aftersale_id,
      );
      if (!aftersale || !request?.aftersale_id) {
        throw new Error("售后同意参数不完整");
      }
      if (!canSubmitAftersaleAction(aftersale)) {
        throw new Error("当前售后状态不允许提交处理动作");
      }
      const acceptType =
        request.accept_type === undefined || request.accept_type === null
          ? null
          : Number(request.accept_type);
      if (acceptType !== null && ![1, 2].includes(acceptType)) {
        throw new Error("同意类型只能为 1 或 2");
      }
      const now = "2026-05-22T00:24:00+08:00";
      aftersale.last_action = "accept";
      aftersale.last_action_status = "success";
      aftersale.last_action_error = null;
      aftersale.last_action_note = request.note || null;
      aftersale.last_action_at = now;
      aftersale.updated_at = now;
      previewNotifications.value.unshift({
        id: `notification-preview-aftersale-action-${Date.now()}`,
        severity: "info",
        source_type: "aftersale_action",
        source_id: aftersale.id,
        shop_id: aftersale.shop_id,
        shop_name: aftersale.shop_name,
        title: "售后同意已提交",
        body: `售后单 ${aftersale.wechat_aftersale_id} 已提交同意处理，等待下次同步确认平台状态。`,
        status: "unread",
        data_json: JSON.stringify({
          aftersale_id: aftersale.id,
          action: "accept",
          accept_type: acceptType,
        }),
        read_at: null,
        created_at: now,
        updated_at: now,
      });
      return {
        aftersale_id: aftersale.id,
        wechat_aftersale_id: aftersale.wechat_aftersale_id,
        action: "accept",
        status: "success",
        errcode: null,
        errmsg: null,
        message: "售后同意已提交，等待下次同步确认平台状态",
      } as T;
    }
    if (name === "reject_aftersale") {
      const request = args?.request as
        | {
            aftersale_id?: string;
            reject_reason_type?: number;
            reject_reason?: string | null;
            note?: string | null;
          }
        | undefined;
      const aftersale = previewAftersales.value.find(
        (item) =>
          item.id === request?.aftersale_id ||
          item.wechat_aftersale_id === request?.aftersale_id,
      );
      const rejectReasonType = Number(request?.reject_reason_type);
      if (
        !aftersale ||
        !request?.aftersale_id ||
        !Number.isInteger(rejectReasonType) ||
        rejectReasonType <= 0
      ) {
        throw new Error("售后拒绝参数不完整");
      }
      if (!canSubmitAftersaleAction(aftersale)) {
        throw new Error("当前售后状态不允许提交处理动作");
      }
      const now = "2026-05-22T00:24:00+08:00";
      aftersale.last_action = "reject";
      aftersale.last_action_status = "success";
      aftersale.last_action_error = null;
      aftersale.last_action_note =
        request.note || request.reject_reason || null;
      aftersale.last_action_at = now;
      aftersale.updated_at = now;
      previewNotifications.value.unshift({
        id: `notification-preview-aftersale-action-${Date.now()}`,
        severity: "warning",
        source_type: "aftersale_action",
        source_id: aftersale.id,
        shop_id: aftersale.shop_id,
        shop_name: aftersale.shop_name,
        title: "售后拒绝已提交",
        body: `售后单 ${aftersale.wechat_aftersale_id} 已提交拒绝处理，等待下次同步确认平台状态。`,
        status: "unread",
        data_json: JSON.stringify({
          aftersale_id: aftersale.id,
          action: "reject",
          reject_reason_type: rejectReasonType,
        }),
        read_at: null,
        created_at: now,
        updated_at: now,
      });
      return {
        aftersale_id: aftersale.id,
        wechat_aftersale_id: aftersale.wechat_aftersale_id,
        action: "reject",
        status: "success",
        errcode: null,
        errmsg: null,
        message: "售后拒绝已提交，等待下次同步确认平台状态",
      } as T;
    }
    if (name === "record_order_profit_adjustment") {
      const request = args?.request as
        | {
            order_id?: string;
            kind?: string;
            amount_cents?: number;
          }
        | undefined;
      if (
        !request?.order_id ||
        !request.kind ||
        request.amount_cents === undefined
      ) {
        throw new Error("利润调整参数不完整");
      }
      previewProfitAdjustments.value.push({
        order_id: request.order_id,
        kind: request.kind,
        amount_cents: request.amount_cents,
      });
      return {
        adjustment_id: `profit-adj-preview-${Date.now()}`,
        order_id: request.order_id,
        kind: request.kind,
        amount_cents: request.amount_cents,
        message: "利润调整项已记录",
      } as T;
    }
    if (name === "export_purchase_tasks") {
      const status = String(args?.status || "all");
      const exportedCount =
        status === "all"
          ? previewPurchaseTasks.value.length
          : previewPurchaseTasks.value.filter((item) => item.status === status)
              .length;
      return {
        file_path: "/tmp/wx-xd-preview-purchase-tasks.csv",
        exported_count: exportedCount,
      } as T;
    }
    if (name === "export_supplier_agent_tasks") {
      const status = String(args?.status || "all");
      const exportFormat = String(args?.format || "jsonl");
      const exportedCount =
        status === "all"
          ? previewPurchaseTasks.value.length
          : previewPurchaseTasks.value.filter((item) => item.status === status)
              .length;
      return {
        file_path: `/tmp/wx-xd-preview-supplier-agent.${exportFormat === "md" ? "md" : exportFormat}`,
        exported_count: exportedCount,
        format: exportFormat,
        sensitive_fields: "excluded",
      } as T;
    }
    if (name === "get_supplier_agent_result_template") {
      return ([
        JSON.stringify({
          purchase_task_id: "purchase-task-id",
          action: "shipment",
          delivery_id: "SF",
          delivery_name: "顺丰速运",
          waybill_id: "SF1234567890",
          deliver_type: 1,
          estimated_cost: 18.8,
        }),
        JSON.stringify({
          purchase_task_id: "purchase-task-id",
          action: "issue",
          issue_type: "out_of_stock",
          note: "supplier reported no stock",
        }),
        JSON.stringify({
          purchase_task_id: "purchase-task-id",
          action: "mapping",
          external_product_id: "external-product-id",
          external_sku_id: "external-sku-id",
          supplier_name: "supplier",
          supplier_product_id: "supplier-product-id",
          estimated_cost: 18.8,
          note: "operator confirmed mapping",
        }),
      ].join("\n") + "\n") as T;
    }
    if (name === "apply_supplier_agent_results") {
      const request = args?.request as
        | { raw_results?: string; dry_run?: boolean }
        | undefined;
      const lines = (request?.raw_results || "")
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean);
      const results = lines.map((line, index) => {
        const parsed = JSON.parse(line);
        return {
          index: index + 1,
          purchase_task_id: parsed.purchase_task_id || null,
          action: parsed.action || null,
          status: request?.dry_run ? "dry_run" : "success",
          error: null,
          response: null,
        };
      });
      return {
        dry_run: Boolean(request?.dry_run),
        processed: results.length,
        succeeded: results.length,
        failed: 0,
        results,
      } as T;
    }
    if (name === "record_purchase_task_shipment") {
      const request = args?.request as
        | {
            purchase_task_id?: string;
            delivery_id?: string;
            delivery_name?: string;
            waybill_id?: string;
            deliver_type?: number;
            estimated_cost?: number | null;
          }
        | undefined;
      const task = previewPurchaseTasks.value.find(
        (item) => item.id === request?.purchase_task_id,
      );
      if (!task) {
        throw new Error("采购任务不存在");
      }
      task.status = "supplier_shipped";
      task.supplier_delivery_id = request?.delivery_id || null;
      task.supplier_delivery_name = request?.delivery_name || null;
      task.supplier_waybill_id = request?.waybill_id || null;
      task.supplier_deliver_type = request?.deliver_type || 1;
      task.supplier_shipped_at = "2026-05-22T00:10:00+08:00";
      task.estimated_cost = request?.estimated_cost ?? task.estimated_cost;
      task.estimated_profit =
        task.estimated_cost === null || task.estimated_cost === undefined
          ? null
          : (task.estimated_revenue || 0) / 100 - task.estimated_cost;
      task.error_summary = null;
      task.updated_at = "2026-05-22T00:10:00+08:00";
      upsertPreviewShipment({
        id: `shipment-preview-${task.order_id}`,
        order_id: task.order_id,
        shop_id: task.shop_id,
        shop_name: task.shop_name,
        wechat_order_id: task.wechat_order_id,
        delivery_id: request?.delivery_id || null,
        delivery_name: request?.delivery_name || null,
        waybill_id: request?.waybill_id || null,
        deliver_type: request?.deliver_type || 1,
        status: deliverySettings.value.auto_send_delivery
          ? "ready_to_send"
          : "waiting_confirmation",
        error_code: null,
        error_summary: null,
        submitted_at: null,
        created_at: "2026-05-22T00:10:00+08:00",
        updated_at: "2026-05-22T00:10:00+08:00",
      });
      return {
        purchase_task_id: task.id,
        order_id: task.order_id,
        purchase_status: task.status,
        shipment_id: `shipment-preview-${task.order_id}`,
        shipment_status: deliverySettings.value.auto_send_delivery
          ? "ready_to_send"
          : "waiting_confirmation",
        auto_send_enabled: deliverySettings.value.auto_send_delivery,
        message: deliverySettings.value.auto_send_delivery
          ? "采购物流已回填，等待提交微信发货"
          : "采购物流已回填，自动发货开关关闭，当前仅进入待确认发货",
      } as T;
    }
    if (name === "run_order_sync_once") {
      const taskId = `order_sync_preview_${Date.now()}`;
      previewPendingOrderCount.value = 2;
      previewLastOrderSyncAt.value = "2026-05-22T00:06:00+08:00";
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "orders.sync_shop_orders",
        status: "success",
        progress: 100,
        created_at: "2026-05-22T00:06:00+08:00",
        started_at: "2026-05-22T00:06:00+08:00",
        finished_at: "2026-05-22T00:06:03+08:00",
        pending_count: 0,
        ready_count: 0,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        processed_shops: 1,
        synced_orders: 2,
        failed_shops: 0,
      } as T;
    }
    if (name === "run_order_detail_sync_once") {
      const taskId = `order_detail_preview_${Date.now()}`;
      previewPendingOrderCount.value = 2;
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "orders.sync_order_details",
        status: "success",
        progress: 100,
        created_at: "2026-05-22T00:07:00+08:00",
        started_at: "2026-05-22T00:07:00+08:00",
        finished_at: "2026-05-22T00:07:02+08:00",
        pending_count: 0,
        ready_count: 2,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        processed_orders: 2,
        synced_orders: 2,
        created_items: 2,
        failed_orders: 0,
      } as T;
    }
    if (name === "run_aftersale_sync_once") {
      const taskId = `aftersale_sync_preview_${Date.now()}`;
      const activeExists = previewAftersales.value.some(
        (item) => item.status === "MERCHANT_PROCESSING",
      );
      if (!activeExists) {
        previewAftersales.value.unshift({
          id: `aftersale-preview-${Date.now()}`,
          shop_id: "shop-preview",
          shop_name: "预览店铺",
          order_id: "order-preview-1",
          wechat_order_id: "420000000001",
          wechat_aftersale_id: `after-sale-${Date.now()}`,
          status: "MERCHANT_PROCESSING",
          aftersale_type: "refund",
          reason: "买家申请退款，等待商家处理",
          refund_amount_cents: 2990,
          responsibility_party: null,
          responsibility_note: null,
          supplier_compensation_cents: 0,
          handled_at: null,
          last_action: null,
          last_action_status: null,
          last_action_error: null,
          last_action_note: null,
          last_action_at: null,
          evidence_count: 0,
          synced_at: "2026-05-22T00:18:00+08:00",
          updated_at: "2026-05-22T00:18:00+08:00",
        });
      }
      previewPendingOrderCount.value = Math.max(
        previewPendingOrderCount.value,
        1,
      );
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "aftersales.sync_shop_aftersales",
        status: "partial_success",
        progress: 100,
        created_at: "2026-05-22T00:18:00+08:00",
        started_at: "2026-05-22T00:18:00+08:00",
        finished_at: "2026-05-22T00:18:02+08:00",
        pending_count: 0,
        ready_count: previewAftersales.value.filter(
          (item) => item.status === "MERCHANT_PROCESSING",
        ).length,
        failed_count: previewAftersales.value.filter(
          (item) => item.status === "sync_failed",
        ).length,
      });
      return {
        task_id: taskId,
        processed_shops: 1,
        synced_aftersales: previewAftersales.value.length - 1,
        failed_shops: 0,
        failed_aftersales: 1,
      } as T;
    }
    if (name === "run_guarantee_sync_once") {
      const taskId = `guarantee_sync_preview_${Date.now()}`;
      const activeExists = previewGuaranteeOrders.value.some(
        (item) => item.status === "STATUS_WAIT_MERCHANT_PROOF",
      );
      if (!activeExists) {
        previewGuaranteeOrders.value.unshift({
          id: `guarantee-preview-${Date.now()}`,
          shop_id: "shop-preview",
          shop_name: "预览店铺",
          order_id: "order-preview-1",
          wechat_order_id: "420000000001",
          guarantee_order_id: `200000${Date.now()}`,
          guarantee_type: 2,
          guarantee_type_text: "坏损包退",
          status: "STATUS_WAIT_MERCHANT_PROOF",
          status_text: "等待商家举证",
          apply_reason: "买家反馈商品破损，需要商家补充凭证",
          pay_amount_cents: 2990,
          merchant_refuse_reason: null,
          handling_status: "pending",
          handling_note: null,
          responsibility_party: null,
          supplier_compensation_cents: 0,
          handled_at: null,
          created_time: 1779428400,
          updated_time_unix: 1779429000,
          expire_time: 1779514800,
          complete_time: null,
          evidence_count: 0,
          synced_at: "2026-05-22T00:25:00+08:00",
          updated_at: "2026-05-22T00:25:00+08:00",
        });
      }
      previewPendingOrderCount.value = Math.max(
        previewPendingOrderCount.value,
        1,
      );
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "aftersales.sync_guarantee_orders",
        status: "success",
        progress: 100,
        created_at: "2026-05-22T00:25:00+08:00",
        started_at: "2026-05-22T00:25:00+08:00",
        finished_at: "2026-05-22T00:25:05+08:00",
        pending_count: 0,
        ready_count: previewGuaranteeOrders.value.filter((item) =>
          isActiveGuaranteeStatus(item.status),
        ).length,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        processed_shops: 1,
        synced_guarantees: previewGuaranteeOrders.value.length,
        failed_shops: 0,
        failed_guarantees: 0,
      } as T;
    }
    if (name === "run_purchase_task_generation_once") {
      const taskId = `purchase_task_preview_${Date.now()}`;
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "procurement.create_purchase_tasks",
        status: "success",
        progress: 100,
        created_at: "2026-05-22T00:08:00+08:00",
        started_at: "2026-05-22T00:08:00+08:00",
        finished_at: "2026-05-22T00:08:01+08:00",
        pending_count: 0,
        ready_count: 2,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        processed_items: 2,
        created_tasks: 2,
        skipped_items: 0,
      } as T;
    }
    if (name === "record_order_shipment") {
      const request = args?.request as
        | {
            order_id?: string;
            shop_id?: string;
            wechat_order_id?: string;
            delivery_id?: string;
            delivery_name?: string;
            waybill_id?: string;
            deliver_type?: number;
          }
        | undefined;
      const orderId = request?.order_id || "order-preview";
      const shipmentId = `shipment-preview-${orderId}`;
      upsertPreviewShipment({
        id: shipmentId,
        order_id: orderId,
        shop_id: request?.shop_id || "shop-preview",
        shop_name:
          previewShops.value.find((shop) => shop.id === request?.shop_id)
            ?.name || "预览店铺",
        wechat_order_id: request?.wechat_order_id || "420000000001",
        delivery_id: request?.delivery_id || null,
        delivery_name: request?.delivery_name || null,
        waybill_id: request?.waybill_id || null,
        deliver_type: request?.deliver_type || 1,
        status: deliverySettings.value.auto_send_delivery
          ? "ready_to_send"
          : "waiting_confirmation",
        error_code: null,
        error_summary: null,
        submitted_at: null,
        created_at: "2026-05-22T00:09:00+08:00",
        updated_at: "2026-05-22T00:09:00+08:00",
      });
      return {
        shipment_id: shipmentId,
        order_id: orderId,
        status: deliverySettings.value.auto_send_delivery
          ? "ready_to_send"
          : "waiting_confirmation",
        auto_send_enabled: deliverySettings.value.auto_send_delivery,
        message: deliverySettings.value.auto_send_delivery
          ? "物流已回填，等待提交微信发货"
          : "物流已回填，自动发货开关关闭，当前仅进入待确认发货",
      } as T;
    }
    if (name === "run_delivery_submission_once") {
      const taskId = `delivery_submit_preview_${Date.now()}`;
      if (!deliverySettings.value.auto_send_delivery) {
        previewTaskRuns.value.unshift({
          id: taskId,
          task_type: "delivery.submit_wechat_shipment",
          status: "success",
          progress: 100,
          created_at: "2026-05-22T00:09:00+08:00",
          started_at: "2026-05-22T00:09:00+08:00",
          finished_at: "2026-05-22T00:09:00+08:00",
          pending_count: 0,
          ready_count: 0,
          failed_count: 0,
        });
        return {
          task_id: taskId,
          processed_shipments: 0,
          submitted_shipments: 0,
          failed_shipments: 0,
        } as T;
      }
      const readyShipments = previewShipments.value.filter(
        (shipment) => shipment.status === "ready_to_send",
      );
      readyShipments.forEach((shipment) => {
        shipment.status = "wechat_shipped";
        shipment.submitted_at = "2026-05-22T00:09:02+08:00";
        shipment.updated_at = "2026-05-22T00:09:02+08:00";
        shipment.error_code = null;
        shipment.error_summary = null;
      });
      previewPendingOrderCount.value = 0;
      previewTaskRuns.value.unshift({
        id: taskId,
        task_type: "delivery.submit_wechat_shipment",
        status: "success",
        progress: 100,
        created_at: "2026-05-22T00:09:00+08:00",
        started_at: "2026-05-22T00:09:00+08:00",
        finished_at: "2026-05-22T00:09:02+08:00",
        pending_count: 0,
        ready_count: 1,
        failed_count: 0,
      });
      return {
        task_id: taskId,
        processed_shipments: readyShipments.length,
        submitted_shipments: readyShipments.length,
        failed_shipments: 0,
      } as T;
    }
    if (name === "run_publish_tasks_once") {
      const task = previewTaskRuns.value.find((item) =>
        ["pending", "queued", "running"].includes(item.status),
      );
      if (task) {
        task.status = "ready_to_publish";
        task.progress = 100;
        task.started_at = "2026-05-22T00:01:00+08:00";
        task.finished_at = "2026-05-22T00:01:05+08:00";
        task.pending_count = 0;
        task.ready_count = 1;
        if (currentJob.value?.id === task.id) {
          currentJob.value.status = "ready_to_publish";
          currentJob.value.products.forEach((product) => {
            product.status = "ready_to_publish";
            product.pending_count = 1;
            product.items.forEach((item) => {
              item.status = "ready_to_publish";
              item.error_summary = "本地前置校验通过，等待素材上传与微信发品";
            });
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          ready_items: 1,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        ready_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_publish_category_prechecks_once") {
      const task = previewTaskRuns.value.find(
        (item) => item.status === "ready_to_publish",
      );
      if (task) {
        task.status = "ready_to_publish";
        task.progress = 35;
        task.ready_count = 1;
        if (currentJob.value?.id === task.id) {
          currentJob.value.status = "ready_to_publish";
          currentJob.value.products.forEach((product) => {
            product.status = "ready_to_publish";
            product.items.forEach((item) => {
              item.status = "category_prechecked";
              item.error_summary = "微信类目预检通过，等待素材上传";
            });
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          passed_items: 1,
          failed_items: 0,
          skipped_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        passed_items: 0,
        failed_items: 0,
        skipped_items: 0,
      } as T;
    }
    if (name === "run_publish_attribute_fill_once") {
      const task = previewTaskRuns.value.find(
        (item) => currentJob.value?.id === item.id,
      );
      const attrFailedItems =
        currentJob.value?.products
          .flatMap((product) => product.items)
          .filter(
            (item) =>
              item.status === "failed" &&
              item.error_code === "CATEGORY_ATTRS_NEED_AI_FILL",
          ) || [];
      if (task && attrFailedItems.length > 0 && currentJob.value) {
        task.status = "ready_to_publish";
        task.progress = 35;
        task.ready_count = attrFailedItems.length;
        task.failed_count = 0;
        currentJob.value.status = "ready_to_publish";
        currentJob.value.products.forEach((product) => {
          product.status = "ready_to_publish";
          product.failed_count = 0;
          product.pending_count = attrFailedItems.length;
          product.items.forEach((item) => {
            if (
              item.status === "failed" &&
              item.error_code === "CATEGORY_ATTRS_NEED_AI_FILL"
            ) {
              item.status = "ready_to_publish";
              item.error_code = null;
              item.error_summary =
                "已自动补齐必填属性，等待重新执行微信类目预检";
            }
          });
        });
        return {
          processed_jobs: 1,
          processed_items: attrFailedItems.length,
          auto_filled_items: attrFailedItems.length,
          suggestion_only_items: 0,
          failed_items: 0,
          generated_suggestions: attrFailedItems.length,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        auto_filled_items: 0,
        suggestion_only_items: 0,
        failed_items: 0,
        generated_suggestions: 0,
      } as T;
    }
    if (name === "run_publish_ai_attribute_suggestions_once") {
      const task = previewTaskRuns.value.find(
        (item) => currentJob.value?.id === item.id,
      );
      const attrFailedItems =
        currentJob.value?.products
          .flatMap((product) => product.items)
          .filter(
            (item) =>
              item.status === "failed" &&
              item.error_code === "CATEGORY_ATTRS_NEED_AI_FILL",
          ) || [];
      if (task && attrFailedItems.length > 0) {
        return {
          processed_jobs: 1,
          processed_items: attrFailedItems.length,
          auto_filled_items:
            aiProviderSettings.value.enabled &&
            aiProviderSettings.value.has_api_key
              ? attrFailedItems.length
              : 0,
          suggestion_only_items:
            aiProviderSettings.value.enabled &&
            aiProviderSettings.value.has_api_key
              ? 0
              : attrFailedItems.length,
          failed_items: 0,
          generated_suggestions: attrFailedItems.length,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        auto_filled_items: 0,
        suggestion_only_items: 0,
        failed_items: 0,
        generated_suggestions: 0,
      } as T;
    }
    if (name === "run_order_price_adjustment_once") {
      const task = previewTaskRuns.value.find(
        (item) =>
          item.task_type === "orders.change_order_price" &&
          ["pending", "queued", "running"].includes(item.status),
      );
      if (task) {
        task.status = "success";
        task.progress = 100;
        task.pending_count = 0;
        task.finished_at = "2026-05-22T00:21:00+08:00";
        if (currentOrderPriceAdjustmentJob.value?.id === task.id) {
          currentOrderPriceAdjustmentJob.value.status = "success";
          currentOrderPriceAdjustmentJob.value.updated_at =
            "2026-05-22T00:21:00+08:00";
          currentOrderPriceAdjustmentJob.value.items.forEach((item) => {
            item.status = "success";
            item.error_code = null;
            item.error_summary = "微信已接受未付款订单改价";
            item.updated_at = "2026-05-22T00:21:00+08:00";
            item.submitted_at = "2026-05-22T00:21:00+08:00";
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          success_items: 1,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        success_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_price_update_precheck_once") {
      const task = previewTaskRuns.value.find(
        (item) =>
          item.task_type === "price.create_update_job" &&
          ["pending", "queued", "running"].includes(item.status),
      );
      if (task) {
        task.status = "ready_to_update";
        task.progress = 50;
        task.pending_count = 0;
        task.ready_count = 1;
        task.started_at = "2026-05-22T00:12:00+08:00";
        if (currentPriceJob.value?.id === task.id) {
          currentPriceJob.value.status = "ready_to_update";
          currentPriceJob.value.items.forEach((item) => {
            item.status = "ready_to_update";
            item.error_code = null;
            item.error_summary =
              "本地商品售价校验通过，等待提交微信 updateproduct";
            item.updated_at = "2026-05-22T00:12:00+08:00";
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          ready_items: 1,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        ready_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_price_update_submit_once") {
      const task = previewTaskRuns.value.find(
        (item) =>
          item.task_type === "price.create_update_job" &&
          item.status === "ready_to_update",
      );
      if (task) {
        task.status = "submitted";
        task.progress = 75;
        task.ready_count = 0;
        task.pending_count = 1;
        if (currentPriceJob.value?.id === task.id) {
          currentPriceJob.value.status = "submitted";
          currentPriceJob.value.items.forEach((item) => {
            item.status = "submitted";
            item.error_code = null;
            item.error_summary =
              "微信 updateproduct 已提交，等待商品状态同步确认价格";
            item.updated_at = "2026-05-22T00:13:00+08:00";
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          submitted_items: 1,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        submitted_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_price_update_confirm_once") {
      const task = previewTaskRuns.value.find(
        (item) =>
          item.task_type === "price.create_update_job" &&
          ["submitted", "audit_pending"].includes(item.status),
      );
      if (task) {
        task.status = "success";
        task.progress = 100;
        task.ready_count = 0;
        task.pending_count = 0;
        task.finished_at = "2026-05-22T00:14:00+08:00";
        if (currentPriceJob.value?.id === task.id) {
          currentPriceJob.value.status = "success";
          currentPriceJob.value.items.forEach((item) => {
            item.status = "success";
            item.error_code = null;
            item.error_summary =
              "线上 product.skus[].sale_price 已全部匹配目标价";
            item.updated_at = "2026-05-22T00:14:00+08:00";
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          confirmed_items: 1,
          pending_items: 0,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        confirmed_items: 0,
        pending_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_publish_asset_uploads_once") {
      const task = previewTaskRuns.value.find(
        (item) => item.status === "ready_to_publish",
      );
      if (task) {
        task.status = "assets_ready";
        task.progress = 100;
        task.ready_count = 1;
        if (currentJob.value?.id === task.id) {
          currentJob.value.status = "assets_ready";
          currentJob.value.products.forEach((product) => {
            product.status = "assets_ready";
            product.items.forEach((item) => {
              item.status = "assets_ready";
              item.error_summary = "微信素材上传完成，等待 addproduct";
            });
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          uploaded_assets: 4,
          reused_assets: 0,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        uploaded_assets: 0,
        reused_assets: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_publish_submits_once") {
      const task = previewTaskRuns.value.find(
        (item) => item.status === "assets_ready",
      );
      if (task) {
        task.status = "submitted";
        task.progress = 100;
        task.pending_count = 1;
        task.ready_count = 0;
        if (currentJob.value?.id === task.id) {
          currentJob.value.status = "submitted";
          currentJob.value.products.forEach((product) => {
            product.status = "submitted";
            product.items.forEach((item) => {
              item.status = "submitted";
              item.error_summary = "微信 addproduct 已提交，等待审核状态同步";
            });
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          submitted_items: 1,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        submitted_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_publish_status_sync_once") {
      const task = previewTaskRuns.value.find((item) =>
        ["submitted", "audit_pending"].includes(item.status),
      );
      if (task) {
        const wasSubmitted = task.status === "submitted";
        task.status = wasSubmitted ? "audit_passed" : "success";
        task.progress = wasSubmitted ? 90 : 100;
        task.pending_count = wasSubmitted ? 0 : 0;
        task.ready_count = 0;
        task.finished_at = wasSubmitted ? null : "2026-05-22T00:05:00+08:00";
        if (currentJob.value?.id === task.id) {
          currentJob.value.status = wasSubmitted ? "audit_passed" : "success";
          currentJob.value.products.forEach((product) => {
            product.status = wasSubmitted ? "audit_passed" : "success";
            product.success_count = wasSubmitted ? 0 : 1;
            product.pending_count = 0;
            product.items.forEach((item) => {
              item.status = wasSubmitted ? "audit_passed" : "success";
              item.error_code = null;
              item.error_summary = wasSubmitted
                ? "微信商品状态：4 审核成功；编辑状态：4 审核成功。审核已通过，后续可进入上架"
                : "微信商品状态：5 已上架；编辑状态：4 审核成功。商品已上架";
            });
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          success_items: 1,
          pending_items: 0,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        success_items: 0,
        pending_items: 0,
        failed_items: 0,
      } as T;
    }
    if (name === "run_publish_listing_once") {
      const task = previewTaskRuns.value.find(
        (item) => item.status === "audit_passed",
      );
      if (task) {
        task.status = "audit_pending";
        task.progress = 80;
        task.pending_count = 1;
        task.ready_count = 0;
        task.finished_at = null;
        if (currentJob.value?.id === task.id) {
          currentJob.value.status = "audit_pending";
          currentJob.value.products.forEach((product) => {
            product.status = "audit_pending";
            product.success_count = 0;
            product.pending_count = 1;
            product.items.forEach((item) => {
              item.status = "audit_pending";
              item.error_code = null;
              item.error_summary =
                "微信 listingproduct 已提交，等待 getproduct 确认已上架";
            });
          });
        }
        return {
          processed_jobs: 1,
          processed_items: 1,
          listing_submitted_items: 1,
          failed_items: 0,
        } as T;
      }
      return {
        processed_jobs: 0,
        processed_items: 0,
        listing_submitted_items: 0,
        failed_items: 0,
      } as T;
    }
    throw new Error(`浏览器预览暂不支持命令：${name}`);
  }

  function buildPreviewJob(taskId: string): PublishJobView {
    return {
      id: taskId,
      request_id: "preview-request",
      status: "queued",
      accepted_product_count: 1,
      target_shop_count: 1,
      created_at: "2026-05-22T00:00:00+08:00",
      products: [
        {
          external_product_id: "demo-1688-10001",
          title: "夏季薄款防晒衣女",
          status: "queued",
          success_count: 0,
          failed_count: 0,
          pending_count: 1,
          error_summary: null,
          items: [
            {
              id: "item-preview",
              shop_id: "shop-preview",
              shop_name: "预览店铺",
              status: "pending",
              error_code: null,
              error_summary: null,
              created_at: "2026-05-22T00:00:00+08:00",
            },
          ],
        },
      ],
    };
  }

  function buildPreviewPriceJob(taskId: string): PriceUpdateJobView {
    return {
      id: taskId,
      request_id: "preview-price-request",
      status: "queued",
      accepted_product_count: 1,
      target_shop_count: 1,
      created_at: "2026-05-22T00:11:00+08:00",
      items: [
        {
          id: "price-item-preview",
          shop_id: "shop-preview",
          shop_name: "预览店铺",
          external_product_id: "demo-1688-10001",
          target_price_cents: 3290,
          status: "pending",
          error_code: null,
          error_summary: null,
          wechat_product_id: "1234567890",
          created_at: "2026-05-22T00:11:00+08:00",
          updated_at: "2026-05-22T00:11:00+08:00",
        },
      ],
    };
  }

  function buildPreviewOrderPriceAdjustmentJob(
    taskId: string,
  ): OrderPriceAdjustmentJobView {
    return {
      id: taskId,
      request_id: "preview-order-price-request",
      status: "queued",
      accepted_order_count: 1,
      created_at: "2026-05-22T00:20:00+08:00",
      updated_at: "2026-05-22T00:20:00+08:00",
      items: [
        {
          id: "order-price-item-preview",
          shop_id: "shop-preview",
          shop_name: "预览店铺",
          order_id: "order-shop-preview-3704612354559743232",
          wechat_order_id: "3704612354559743232",
          wechat_status: 10,
          change_order_infos: [
            {
              product_id: "1234567890",
              sku_id: "5678901234",
              change_price_cents: 300,
            },
          ],
          change_express: true,
          express_fee_cents: 0,
          note: "未付款订单人工让利",
          status: "pending",
          error_code: null,
          error_summary: null,
          created_at: "2026-05-22T00:20:00+08:00",
          updated_at: "2026-05-22T00:20:00+08:00",
          submitted_at: null,
        },
      ],
    };
  }

  function buildPreviewOrderProfitResult(
    status: string,
  ): OrderProfitListResult {
    const grouped = new Map<string, PurchaseTaskView[]>();
    previewPurchaseTasks.value.forEach((task) => {
      const rows = grouped.get(task.order_id) || [];
      rows.push(task);
      grouped.set(task.order_id, rows);
    });
    const items = Array.from(grouped.entries()).map(([orderId, tasks]) => {
      const first = tasks[0];
      const revenue = tasks.reduce(
        (sum, task) => sum + (task.estimated_revenue || 0),
        0,
      );
      const purchaseCost = tasks.reduce(
        (sum, task) => sum + Math.round((task.estimated_cost || 0) * 100),
        0,
      );
      const missingCost = tasks.filter(
        (task) =>
          task.estimated_cost === null || task.estimated_cost === undefined,
      ).length;
      const adjustment = (kind: string) =>
        previewProfitAdjustments.value
          .filter((item) => item.order_id === orderId && item.kind === kind)
          .reduce((sum, item) => sum + item.amount_cents, 0);
      const purchaseFreight = adjustment("purchase_freight");
      const refund = adjustment("refund");
      const aftersaleCompensation = adjustment("aftersale_compensation");
      const otherCost = adjustment("other_cost");
      const otherIncome = adjustment("other_income");
      const estimatedProfit =
        revenue +
        otherIncome -
        purchaseCost -
        purchaseFreight -
        refund -
        aftersaleCompensation -
        otherCost;
      const actualProfit = missingCost === 0 ? estimatedProfit : null;
      const profitStatus =
        tasks.length === 0
          ? "missing_purchase_task"
          : missingCost > 0
            ? "missing_cost"
            : estimatedProfit < 0
              ? "loss"
              : "profitable";
      return {
        order_id: orderId,
        wechat_order_id: first.wechat_order_id,
        shop_id: first.shop_id,
        shop_name: first.shop_name,
        order_status:
          first.status === "supplier_shipped"
            ? "supplier_shipped"
            : "pending_purchase",
        item_count: tasks.length,
        quantity: tasks.reduce((sum, task) => sum + task.quantity, 0),
        revenue_cents: revenue,
        purchase_task_count: tasks.length,
        missing_cost_count: missingCost,
        purchase_cost_cents: purchaseCost,
        purchase_freight_cents: purchaseFreight,
        refund_cents: refund,
        aftersale_compensation_cents: aftersaleCompensation,
        other_cost_cents: otherCost,
        other_income_cents: otherIncome,
        estimated_profit_cents: estimatedProfit,
        actual_profit_cents: actualProfit,
        profit_status: profitStatus,
        detail_synced_at: "2026-05-22T00:07:02+08:00",
        updated_at: first.updated_at,
      };
    });
    const filtered =
      status === "all"
        ? items
        : items.filter(
            (item) =>
              item.profit_status === status || item.order_status === status,
          );
    return {
      items: filtered,
      total: filtered.length,
      totals: summarizeOrderProfits(filtered),
    };
  }

  function summarizeOrderProfits(items: OrderProfitView[]): OrderProfitTotals {
    return {
      order_count: items.length,
      revenue_cents: items.reduce((sum, item) => sum + item.revenue_cents, 0),
      purchase_cost_cents: items.reduce(
        (sum, item) => sum + item.purchase_cost_cents,
        0,
      ),
      purchase_freight_cents: items.reduce(
        (sum, item) => sum + item.purchase_freight_cents,
        0,
      ),
      refund_cents: items.reduce((sum, item) => sum + item.refund_cents, 0),
      aftersale_compensation_cents: items.reduce(
        (sum, item) => sum + item.aftersale_compensation_cents,
        0,
      ),
      other_cost_cents: items.reduce(
        (sum, item) => sum + item.other_cost_cents,
        0,
      ),
      other_income_cents: items.reduce(
        (sum, item) => sum + item.other_income_cents,
        0,
      ),
      estimated_profit_cents: items.reduce(
        (sum, item) => sum + item.estimated_profit_cents,
        0,
      ),
      actual_profit_cents: items.reduce(
        (sum, item) => sum + (item.actual_profit_cents || 0),
        0,
      ),
      unknown_actual_order_count: items.filter(
        (item) => item.actual_profit_cents === null,
      ).length,
    };
  }

  function buildPreviewInventoryRisks(): InventoryRiskView[] {
    const reserved = previewPurchaseTasks.value
      .filter((task) =>
        [
          "pending_purchase",
          "supplier_shipped",
          "wechat_shipped",
          "completed",
        ].includes(task.status),
      )
      .reduce((sum, task) => sum + task.quantity, 0);
    const pending = previewPurchaseTasks.value
      .filter((task) => task.status === "pending_purchase")
      .reduce((sum, task) => sum + task.quantity, 0);
    const issueCount = previewPurchaseTasks.value.filter(
      (task) =>
        task.status.startsWith("supplier_") &&
        task.status !== "supplier_shipped",
    ).length;
    const totalStock = 12;
    const available = totalStock - reserved;
    const riskStatus =
      issueCount > 0
        ? "supplier_issue"
        : available <= 0
          ? "out_of_stock"
          : pending > available
            ? "stock_pressure"
            : available <= 5
              ? "low_stock"
              : "healthy";
    const recommendation: Record<string, string> = {
      supplier_issue:
        "存在供应商异常采购任务，先人工处理供应商缺货、涨价或取消",
      out_of_stock: "货源库存已不足，建议暂停继续铺货并评估下架或换源",
      stock_pressure: "待采购数量超过可用库存，建议优先补货或减少铺货范围",
      low_stock: "可用库存偏低，建议补库存或降低新店铺铺货节奏",
      healthy: "库存风险正常，可继续观察真实订单和售后表现",
    };
    return [
      {
        external_product_id: "demo-1688-10001",
        title: "夏季薄款防晒衣女",
        supplier_name: "示例供应商",
        supplier_product_id: "10001",
        total_stock: totalStock,
        reserved_quantity: reserved,
        available_stock: available,
        active_shop_count: 1,
        pending_purchase_quantity: pending,
        supplier_issue_count: issueCount,
        risk_status: riskStatus,
        recommendation: recommendation[riskStatus],
        updated_at: "2026-05-22T00:35:00+08:00",
      },
    ];
  }

  function buildPreviewProductSalesAnalysis(): ProductSalesAnalysisView[] {
    const inventory = buildPreviewInventoryRisks()[0];
    const productTasks = previewPurchaseTasks.value.filter(
      (task) => task.external_product_id === inventory.external_product_id,
    );
    const orderIds = new Set(productTasks.map((task) => task.order_id));
    const unitsSold = productTasks.reduce(
      (sum, task) => sum + task.quantity,
      0,
    );
    const revenue = productTasks.reduce(
      (sum, task) => sum + (task.estimated_revenue || 0),
      0,
    );
    const purchaseCost = productTasks.reduce(
      (sum, task) => sum + Math.round((task.estimated_cost || 0) * 100),
      0,
    );
    const missingCost = productTasks.filter(
      (task) =>
        task.estimated_cost === null || task.estimated_cost === undefined,
    ).length;
    const relatedAftersales = previewAftersales.value.filter(
      (item) => item.order_id && orderIds.has(item.order_id),
    );
    const relatedRefund = relatedAftersales.reduce(
      (sum, item) => sum + (item.refund_amount_cents || 0),
      0,
    );
    const grossProfit = revenue - purchaseCost - relatedRefund;
    const operationStatus =
      inventory.risk_status !== "healthy" &&
      inventory.risk_status !== "not_listed"
        ? "stock_risk"
        : unitsSold > 0 &&
            productTasks.length > 0 &&
            missingCost === 0 &&
            grossProfit < 0
          ? "margin_risk"
          : unitsSold > 0 && relatedAftersales.length > 0
            ? "aftersale_watch"
            : unitsSold >= 3 && grossProfit > 0 && inventory.available_stock > 5
              ? "scale_candidate"
              : inventory.active_shop_count > 0 && unitsSold === 0
                ? "no_sales"
                : inventory.active_shop_count === 0
                  ? "not_listed"
                  : unitsSold > 0
                    ? "steady"
                    : "observe";
    const recommendation: Record<string, string> = {
      stock_risk: "库存或供应商风险已触发，先处理补货、换源或暂停继续铺货",
      margin_risk: "已有真实订单但毛利为负，优先核对成本并进入价格调整",
      aftersale_watch: "已有订单关联售后，先观察原因和责任方再扩大铺货",
      scale_candidate: "真实订单、毛利和库存都较健康，可优先追加店铺或测试放量",
      no_sales: "已铺货但暂无真实订单，建议检查价格、标题、素材或下架节奏",
      not_listed: "尚未形成有效店铺商品，可进入铺货候选池",
      steady: "已有真实订单，继续观察毛利、库存和售后变化",
      observe: "数据不足，先保持观察，不做自动动销动作",
    };
    return [
      {
        external_product_id: inventory.external_product_id,
        title: inventory.title,
        supplier_name: inventory.supplier_name,
        active_shop_count: inventory.active_shop_count,
        order_count: orderIds.size,
        units_sold: unitsSold,
        revenue_cents: revenue,
        purchase_task_count: productTasks.length,
        missing_cost_count: missingCost,
        purchase_cost_cents: purchaseCost,
        related_aftersale_count: relatedAftersales.length,
        related_refund_cents: relatedRefund,
        total_stock: inventory.total_stock,
        available_stock: inventory.available_stock,
        inventory_risk_status: inventory.risk_status,
        operation_status: operationStatus,
        recommendation: recommendation[operationStatus],
        last_order_at: productTasks[0]?.updated_at || null,
        updated_at: inventory.updated_at,
      },
    ];
  }

  function summarizeProductSalesAnalysis(
    items: ProductSalesAnalysisView[],
  ): ProductSalesAnalysisTotals {
    return {
      product_count: items.length,
      sold_product_count: items.filter((item) => item.units_sold > 0).length,
      total_units_sold: items.reduce((sum, item) => sum + item.units_sold, 0),
      revenue_cents: items.reduce((sum, item) => sum + item.revenue_cents, 0),
      purchase_cost_cents: items.reduce(
        (sum, item) => sum + item.purchase_cost_cents,
        0,
      ),
      gross_profit_cents: items.reduce(
        (sum, item) =>
          sum +
          item.revenue_cents -
          item.purchase_cost_cents -
          item.related_refund_cents,
        0,
      ),
      missing_cost_product_count: items.filter(
        (item) => item.missing_cost_count > 0,
      ).length,
      scale_candidate_count: items.filter(
        (item) => item.operation_status === "scale_candidate",
      ).length,
      risk_product_count: items.filter((item) =>
        ["stock_risk", "margin_risk", "aftersale_watch"].includes(
          item.operation_status,
        ),
      ).length,
    };
  }

  function upsertPreviewShipment(next: ShipmentView) {
    const index = previewShipments.value.findIndex(
      (shipment) => shipment.id === next.id,
    );
    if (index >= 0) {
      previewShipments.value[index] = {
        ...previewShipments.value[index],
        ...next,
      };
    } else {
      previewShipments.value.unshift(next);
    }
  }

  return {
    previewCommand,
    buildPreviewJob,
    buildPreviewPriceJob,
    buildPreviewOrderProfitResult,
    summarizeOrderProfits,
    buildPreviewInventoryRisks,
    buildPreviewProductSalesAnalysis,
    summarizeProductSalesAnalysis,
    upsertPreviewShipment,
  };
}

function withPreviewCategoryContext(
  items: CategoryCacheView[],
  keyword: string,
) {
  const byKey = new Map(items.map((item) => [previewCategoryKey(item), item]));
  const childrenByParent = new Map<string, CategoryCacheView[]>();
  const selectedKeys = new Set<string>();

  for (const item of items) {
    if (!item.parent_cat_id) {
      continue;
    }
    const parentKey = `${item.shop_id}:${item.parent_cat_id}`;
    const children = childrenByParent.get(parentKey) || [];
    children.push(item);
    childrenByParent.set(parentKey, children);
  }

  const addAncestors = (item: CategoryCacheView) => {
    let current: CategoryCacheView | undefined = item;
    while (current) {
      selectedKeys.add(previewCategoryKey(current));
      current = current.parent_cat_id
        ? byKey.get(`${current.shop_id}:${current.parent_cat_id}`)
        : undefined;
    }
  };

  const addDescendants = (item: CategoryCacheView) => {
    selectedKeys.add(previewCategoryKey(item));
    for (const child of childrenByParent.get(previewCategoryKey(item)) || []) {
      addDescendants(child);
    }
  };

  for (const item of items) {
    if (
      !item.name.includes(keyword) &&
      !String(item.cat_id).includes(keyword)
    ) {
      continue;
    }
    addAncestors(item);
    addDescendants(item);
  }

  return items.filter((item) => selectedKeys.has(previewCategoryKey(item)));
}

function previewCategoryKey(item: CategoryCacheView) {
  return `${item.shop_id}:${item.cat_id}`;
}

function comparePreviewCategories(
  left: CategoryCacheView,
  right: CategoryCacheView,
) {
  const levelDiff = (left.level ?? 0) - (right.level ?? 0);
  if (levelDiff !== 0) {
    return levelDiff;
  }
  return (
    left.name.localeCompare(right.name, "zh-Hans-CN") ||
    left.cat_id - right.cat_id
  );
}
