use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub pending_order_count: i64,
    pub abnormal_shop_count: i64,
    pub failed_publish_product_count: i64,
    pub unread_notification_count: i64,
    pub running_task_count: i64,
    pub controller_status: String,
    pub database_path: String,
    pub now_shanghai: String,
    pub last_order_sync_at: Option<String>,
    pub last_publish_summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShopGroup {
    pub id: String,
    pub name: String,
    pub status: String,
    pub shop_count: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateShopRequest {
    pub name: String,
    pub appid: String,
    pub app_secret: Option<String>,
    pub group_id: String,
}

#[derive(Debug, Serialize)]
pub struct Shop {
    pub id: String,
    pub name: String,
    pub appid: String,
    pub status: String,
    pub group_id: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ShopListItem {
    pub id: String,
    pub name: String,
    pub appid: String,
    pub status: String,
    pub group_id: String,
    pub group_name: String,
    pub has_secret: bool,
    pub token_expires_at: Option<String>,
    pub wechat_nickname: Option<String>,
    pub wechat_status: Option<String>,
    pub last_health_check_at: Option<String>,
    pub last_quota_remain: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ShopCredentialCheck {
    pub shop_id: String,
    pub status: String,
    pub expires_at: Option<String>,
    pub errcode: Option<i64>,
    pub errmsg: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShopBasicInfoSyncResult {
    pub shop_id: String,
    pub status: String,
    pub nickname: Option<String>,
    pub wechat_status: Option<String>,
    pub errcode: Option<i64>,
    pub errmsg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiQuotaCheckRequest {
    pub shop_id: String,
    pub cgi_path: String,
}

#[derive(Debug, Serialize)]
pub struct ApiQuotaCheckResult {
    pub shop_id: String,
    pub cgi_path: String,
    pub daily_limit: Option<i64>,
    pub used: Option<i64>,
    pub remain: Option<i64>,
    pub rate_call_count: Option<i64>,
    pub rate_refresh_second: Option<i64>,
    pub errcode: Option<i64>,
    pub errmsg: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryCatalogListResult {
    pub shops: Vec<CategoryCatalogShopSummary>,
    pub categories: Vec<CategoryCacheView>,
    pub category_relations: Vec<CategoryRelationView>,
    pub freight_templates: Vec<FreightTemplateView>,
}

#[derive(Debug, Serialize)]
pub struct CategoryCatalogShopSummary {
    pub shop_id: String,
    pub shop_name: String,
    pub category_count: i64,
    pub detail_count: i64,
    pub product_rule_count: i64,
    pub delivery_rule_count: i64,
    pub category_relation_count: i64,
    pub active_category_relation_count: i64,
    pub freight_template_count: i64,
    pub last_category_sync_at: Option<String>,
    pub last_relation_sync_at: Option<String>,
    pub last_rule_sync_at: Option<String>,
    pub last_freight_sync_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryCacheView {
    pub shop_id: String,
    pub shop_name: String,
    pub cat_id: i64,
    pub parent_cat_id: Option<i64>,
    pub level: Option<i64>,
    pub name: String,
    pub product_attr_count: i64,
    pub sale_attr_count: i64,
    pub product_qua_count: i64,
    pub has_detail: bool,
    pub has_product_rule: bool,
    pub has_delivery_rule: bool,
    pub is_available_for_shop: bool,
    pub synced_at: String,
    pub detail_synced_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryRelationView {
    pub shop_id: String,
    pub shop_name: String,
    pub cat_id: i64,
    pub category_name: Option<String>,
    pub status: i64,
    pub uneffective_reason: Option<String>,
    pub effective_time: Option<i64>,
    pub uneffective_time: Option<i64>,
    pub qua_id: Option<i64>,
    pub synced_at: String,
}

#[derive(Debug, Serialize)]
pub struct FreightTemplateView {
    pub shop_id: String,
    pub shop_name: String,
    pub template_id: String,
    pub synced_at: String,
}

#[derive(Debug, Serialize)]
pub struct CategoryCatalogSyncResult {
    pub task_id: String,
    pub shop_id: String,
    pub synced_categories: i64,
    pub synced_category_relations: i64,
    pub synced_freight_templates: i64,
    pub failed_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryRuleSyncResult {
    pub task_id: String,
    pub shop_id: String,
    pub cat_id: i64,
    pub synced_detail: bool,
    pub synced_product_rule: bool,
    pub synced_delivery_rule: bool,
    pub product_attr_count: i64,
    pub sale_attr_count: i64,
    pub product_qua_count: i64,
    pub failed_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentSkillView {
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub runtime: String,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub skill_path: String,
    pub schema_path: String,
    pub checksum: String,
    pub file_status: String,
    pub runtime_status: String,
    pub last_test_status: Option<String>,
    pub last_test_summary: Option<String>,
    pub last_test_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AgentSkillSettingsRequest {
    pub name: String,
    pub enabled: bool,
    pub model: Option<String>,
    pub temperature: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AgentSkillTestResult {
    pub name: String,
    pub status: String,
    pub summary: String,
    pub checked_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExternalPublishJobRequest {
    pub request_id: String,
    #[serde(default)]
    pub target_shop_group_ids: Vec<String>,
    #[serde(default)]
    pub target_shop_ids: Vec<String>,
    pub products: Vec<ExternalProductInput>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CollectionPublishRequest {
    #[serde(default)]
    pub collection_task_ids: Vec<String>,
    #[serde(default)]
    pub target_shop_ids: Vec<String>,
    pub pricing_strategy: Option<PublishPricingStrategy>,
}

#[derive(Debug, Deserialize)]
pub struct CollectionReviewRunRequest {
    #[serde(default)]
    pub task_ids: Vec<String>,
    #[serde(default)]
    pub target_shop_ids: Vec<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CollectionReviewConfirmRequest {
    pub task_id: String,
    pub title: Option<String>,
    pub category_ids: Option<Vec<i64>>,
    pub category_path: Option<String>,
    #[serde(default)]
    pub target_shop_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CollectionImageUploadRequest {
    pub task_id: String,
    pub kind: String,
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CollectionImageRemoveRequest {
    pub task_id: String,
    pub kind: String,
    pub image_url: String,
}

#[derive(Debug, Serialize)]
pub struct CollectionReviewBatchResult {
    pub processed_items: i64,
    pub passed_items: i64,
    pub needs_review_items: i64,
    pub blocked_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollectionReviewCategoryCandidate {
    pub category_ids: Vec<i64>,
    pub category_path: String,
    pub score: i64,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PublishPricingStrategy {
    pub sale_price_markup_rate: f64,
    pub sale_price_fixed_cents: i64,
    pub sale_price_floor_cents: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalProductInput {
    pub external_product_id: String,
    pub title: String,
    pub source_url: String,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub detail_images: Vec<String>,
    pub skus: Vec<ExternalSkuInput>,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub category_hint: Option<String>,
    pub brand_hint: Option<String>,
    pub weight_gram: Option<i64>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalSkuInput {
    pub external_sku_id: String,
    #[serde(default)]
    pub specs: serde_json::Value,
    pub cost_price: f64,
    pub stock: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PriceUpdateJobRequest {
    pub request_id: String,
    #[serde(default)]
    pub target_shop_group_ids: Vec<String>,
    #[serde(default)]
    pub target_shop_ids: Vec<String>,
    pub products: Vec<PriceUpdateProductInput>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PriceUpdateProductInput {
    pub external_product_id: String,
    pub target_price_cents: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OrderPriceAdjustmentJobRequest {
    pub request_id: String,
    pub orders: Vec<OrderPriceAdjustmentOrderInput>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OrderPriceAdjustmentOrderInput {
    pub shop_id: String,
    pub wechat_order_id: String,
    #[serde(default)]
    pub change_express: bool,
    pub express_fee_cents: Option<i64>,
    #[serde(default)]
    pub lines: Vec<OrderPriceAdjustmentLineInput>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrderPriceAdjustmentLineInput {
    pub product_id: String,
    pub sku_id: String,
    pub change_price_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct PublishJobCreated {
    pub task_id: String,
    pub status: String,
    pub accepted_product_count: usize,
    pub target_shop_count: usize,
}

#[derive(Debug, Serialize)]
pub struct PriceUpdateJobCreated {
    pub task_id: String,
    pub status: String,
    pub accepted_product_count: usize,
    pub target_shop_count: usize,
}

#[derive(Debug, Serialize)]
pub struct OrderPriceAdjustmentJobCreated {
    pub task_id: String,
    pub status: String,
    pub accepted_order_count: usize,
}

#[derive(Debug, Serialize)]
pub struct TaskRunView {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub progress: i64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub pending_count: i64,
    pub ready_count: i64,
    pub failed_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AgentRunView {
    pub id: String,
    pub skill_name: String,
    pub skill_version: String,
    pub scene: String,
    pub source_type: String,
    pub source_id: String,
    pub shop_id: Option<String>,
    pub status: String,
    pub provider_type: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub input_summary: String,
    pub decision: Option<String>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AgentRunEventView {
    pub id: String,
    pub run_id: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    pub data_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OperationalAutomationSettings {
    pub order_sync_enabled: bool,
    pub order_detail_sync_enabled: bool,
    pub aftersale_sync_enabled: bool,
    pub purchase_task_enabled: bool,
    pub delivery_submission_enabled: bool,
    pub publish_precheck_enabled: bool,
    pub publish_attribute_fill_enabled: bool,
    pub publish_category_precheck_enabled: bool,
    pub publish_asset_upload_enabled: bool,
    pub publish_submit_enabled: bool,
    pub publish_status_sync_enabled: bool,
    pub publish_listing_enabled: bool,
    pub price_confirm_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct OperationalAutomationRunResult {
    pub executed_steps: Vec<String>,
    pub skipped_steps: Vec<String>,
    pub errors: Vec<AutomationStepError>,
    pub order_sync: Option<OrderSyncBatchResult>,
    pub order_detail_sync: Option<OrderDetailSyncBatchResult>,
    pub aftersale_sync: Option<AftersaleSyncBatchResult>,
    pub purchase_task_generation: Option<PurchaseTaskBatchResult>,
    pub delivery_submission: Option<DeliverySubmitBatchResult>,
    pub publish_precheck: Option<PublishTaskBatchResult>,
    pub publish_attribute_fill: Option<PublishAttributeFillBatchResult>,
    pub publish_category_precheck: Option<PublishCategoryPrecheckBatchResult>,
    pub publish_asset_upload: Option<AssetUploadBatchResult>,
    pub publish_submit: Option<ProductSubmitBatchResult>,
    pub publish_status_sync: Option<ProductStatusSyncBatchResult>,
    pub publish_listing: Option<ProductListingBatchResult>,
    pub price_confirm: Option<PriceUpdateConfirmBatchResult>,
}

#[derive(Debug, Serialize)]
pub struct PublishPipelineRunResult {
    pub executed_steps: Vec<String>,
    pub skipped_steps: Vec<String>,
    pub errors: Vec<AutomationStepError>,
    pub publish_precheck: Option<PublishTaskBatchResult>,
    pub publish_attribute_fill: Option<PublishAttributeFillBatchResult>,
    pub publish_category_precheck: Option<PublishCategoryPrecheckBatchResult>,
    pub publish_asset_upload: Option<AssetUploadBatchResult>,
    pub publish_submit: Option<ProductSubmitBatchResult>,
    pub publish_status_sync: Option<ProductStatusSyncBatchResult>,
    pub publish_listing: Option<ProductListingBatchResult>,
}

#[derive(Debug, Serialize)]
pub struct AutomationStepError {
    pub step: String,
    pub error: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BackupInfo {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: String,
    pub integrity_ok: bool,
    pub integrity_message: String,
}

#[derive(Debug, Serialize)]
pub struct BackupCreateResult {
    pub backup: BackupInfo,
}

#[derive(Debug, Deserialize)]
pub struct BackupRestoreRequest {
    pub file_path: String,
}

#[derive(Debug, Serialize)]
pub struct BackupRestoreResult {
    pub restored_from: BackupInfo,
    pub rollback_backup: BackupInfo,
    pub integrity_ok: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CollectionPublishWorkspaceResetRequest {
    pub confirm_text: String,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceResetTableCount {
    pub name: String,
    pub before: i64,
    pub after: i64,
}

#[derive(Debug, Serialize)]
pub struct CollectionPublishWorkspaceResetResult {
    pub backup: BackupInfo,
    pub integrity_ok: bool,
    pub integrity_message: String,
    pub counts: Vec<WorkspaceResetTableCount>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ExternalApiLogView {
    pub id: String,
    pub method: String,
    pub path: String,
    pub status: String,
    pub status_code: i64,
    pub error_code: Option<String>,
    pub request_summary: Option<String>,
    pub response_summary: Option<String>,
    pub duration_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationListResult {
    pub items: Vec<NotificationView>,
    pub total: i64,
    pub unread_count: i64,
    pub critical_count: i64,
}

#[derive(Debug, Serialize)]
pub struct NotificationView {
    pub id: String,
    pub severity: String,
    pub source_type: String,
    pub source_id: String,
    pub shop_id: Option<String>,
    pub shop_name: Option<String>,
    pub title: String,
    pub body: String,
    pub status: String,
    pub data_json: Option<String>,
    pub read_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationMarkResult {
    pub updated_count: i64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AiProviderSettings {
    pub enabled: bool,
    pub provider_type: String,
    pub custom_provider_id: String,
    pub api: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub context_window: i64,
    pub max_tokens: i64,
    pub has_api_key: bool,
    pub api_key_hint: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AiProviderSettingsRequest {
    pub enabled: bool,
    pub provider_type: String,
    pub custom_provider_id: Option<String>,
    pub api: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f64>,
    pub context_window: Option<i64>,
    pub max_tokens: Option<i64>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

#[derive(Debug, Serialize)]
pub struct AiProviderTestResult {
    pub status: String,
    pub provider_type: String,
    pub model: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PublishTaskBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub ready_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct PublishAttributeFillBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub auto_filled_items: i64,
    pub suggestion_only_items: i64,
    pub failed_items: i64,
    pub generated_suggestions: i64,
}

#[derive(Debug, Serialize)]
pub struct PublishCategoryPrecheckBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub passed_items: i64,
    pub failed_items: i64,
    pub skipped_items: i64,
}

#[derive(Debug, Serialize)]
pub struct AssetUploadBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub uploaded_assets: i64,
    pub reused_assets: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductSubmitBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub submitted_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductStatusSyncBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub success_items: i64,
    pub pending_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductListingBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub listing_submitted_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct PriceUpdatePrecheckBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub ready_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct PriceUpdateSubmitBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub submitted_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct PriceUpdateConfirmBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub confirmed_items: i64,
    pub pending_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderPriceAdjustmentBatchResult {
    pub processed_jobs: i64,
    pub processed_items: i64,
    pub success_items: i64,
    pub failed_items: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderSyncBatchResult {
    pub task_id: String,
    pub processed_shops: i64,
    pub synced_orders: i64,
    pub failed_shops: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderDetailSyncBatchResult {
    pub task_id: String,
    pub processed_orders: i64,
    pub synced_orders: i64,
    pub created_items: i64,
    pub failed_orders: i64,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskBatchResult {
    pub task_id: String,
    pub processed_items: i64,
    pub created_tasks: i64,
    pub skipped_items: i64,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskListResult {
    pub items: Vec<PurchaseTaskView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskView {
    pub id: String,
    pub order_id: String,
    pub wechat_order_id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub status: String,
    pub external_product_id: Option<String>,
    pub external_sku_id: Option<String>,
    pub source_url: Option<String>,
    pub title: Option<String>,
    pub quantity: i64,
    pub sale_price: Option<i64>,
    pub real_price: Option<i64>,
    pub estimated_revenue: Option<i64>,
    pub estimated_cost: Option<f64>,
    pub estimated_profit: Option<f64>,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub supplier_delivery_id: Option<String>,
    pub supplier_delivery_name: Option<String>,
    pub supplier_waybill_id: Option<String>,
    pub supplier_deliver_type: Option<i64>,
    pub supplier_shipped_at: Option<String>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskExportResult {
    pub file_path: String,
    pub exported_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SupplierAgentExportResult {
    pub file_path: String,
    pub exported_count: i64,
    pub format: String,
    pub sensitive_fields: String,
}

#[derive(Debug, Deserialize)]
pub struct SupplierAgentApplyRequest {
    pub raw_results: String,
    pub dry_run: bool,
    pub continue_on_error: bool,
}

#[derive(Debug, Serialize)]
pub struct SupplierAgentApplyResult {
    pub dry_run: bool,
    pub processed: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub results: Vec<SupplierAgentApplyItemResult>,
}

#[derive(Debug, Serialize)]
pub struct SupplierAgentApplyItemResult {
    pub index: i64,
    pub purchase_task_id: Option<String>,
    pub action: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub response: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PurchaseTaskMappingRequest {
    pub purchase_task_id: String,
    pub external_product_id: String,
    pub external_sku_id: String,
    pub source_url: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub estimated_cost: Option<f64>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskMappingResult {
    pub purchase_task_id: String,
    pub order_id: String,
    pub status: String,
    pub external_product_id: String,
    pub external_sku_id: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct PurchaseTaskIssueRequest {
    pub purchase_task_id: String,
    pub issue_type: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskIssueResult {
    pub purchase_task_id: String,
    pub order_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct PurchaseTaskShipmentRequest {
    pub purchase_task_id: String,
    pub delivery_id: Option<String>,
    pub delivery_name: Option<String>,
    pub waybill_id: Option<String>,
    pub deliver_type: Option<i64>,
    pub estimated_cost: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PurchaseTaskShipmentResult {
    pub purchase_task_id: String,
    pub order_id: String,
    pub purchase_status: String,
    pub shipment_id: Option<String>,
    pub shipment_status: Option<String>,
    pub auto_send_enabled: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct OrderProfitListResult {
    pub items: Vec<OrderProfitView>,
    pub total: i64,
    pub totals: OrderProfitTotals,
}

#[derive(Debug, Serialize)]
pub struct InventoryRiskView {
    pub external_product_id: String,
    pub title: String,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub total_stock: i64,
    pub reserved_quantity: i64,
    pub available_stock: i64,
    pub active_shop_count: i64,
    pub pending_purchase_quantity: i64,
    pub supplier_issue_count: i64,
    pub risk_status: String,
    pub recommendation: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct InventoryRiskListResult {
    pub items: Vec<InventoryRiskView>,
    pub total: i64,
    pub low_stock_count: i64,
    pub out_of_stock_count: i64,
    pub issue_count: i64,
}

#[derive(Debug, Serialize)]
pub struct InventoryRiskScanResult {
    pub task_id: String,
    pub scanned_products: i64,
    pub low_stock_products: i64,
    pub out_of_stock_products: i64,
    pub issue_products: i64,
    pub notifications_created: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductSalesAnalysisListResult {
    pub items: Vec<ProductSalesAnalysisView>,
    pub total: i64,
    pub totals: ProductSalesAnalysisTotals,
}

#[derive(Debug, Serialize)]
pub struct ProductManagementListResult {
    pub items: Vec<ProductManagementView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductManagementView {
    pub external_product_id: String,
    pub title: String,
    pub source_url: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub management_status: String,
    pub publish_status: Option<String>,
    pub publish_error_summary: Option<String>,
    pub active_shop_count: i64,
    pub shop_count: i64,
    pub order_count: i64,
    pub units_sold: i64,
    pub revenue_cents: i64,
    pub purchase_task_count: i64,
    pub missing_cost_count: i64,
    pub purchase_cost_cents: i64,
    pub related_aftersale_count: i64,
    pub related_refund_cents: i64,
    pub total_stock: i64,
    pub available_stock: i64,
    pub inventory_risk_status: String,
    pub operation_status: String,
    pub recommendation: String,
    pub last_order_at: Option<String>,
    pub updated_at: String,
    pub shops: Vec<ProductManagementShopView>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProductManagementShopView {
    pub shop_id: String,
    pub shop_name: String,
    pub status: String,
    pub wechat_product_id: Option<String>,
    pub wechat_status: Option<i64>,
    pub wechat_edit_status: Option<i64>,
    pub current_price_cents: Option<i64>,
    pub last_status_sync_at: Option<String>,
    pub last_price_update_at: Option<String>,
    pub audit_summary: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ShopProductListResult {
    pub items: Vec<WechatShopProductView>,
    pub total: i64,
}

/// 缓存的微信小店真实商品（列表项），数据来自 sync_shop_products。
#[derive(Debug, Serialize, Clone)]
pub struct WechatShopProductView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub wechat_product_id: String,
    pub out_product_id: Option<String>,
    pub title: String,
    pub head_img: Option<String>,
    pub status: i64,
    pub edit_status: Option<i64>,
    pub min_price_cents: Option<i64>,
    pub cat_id: Option<i64>,
    pub total_stock: i64,
    pub sku_count: i64,
    pub audit_summary: Option<String>,
    pub synced_at: String,
    pub updated_at: String,
}

/// 缓存的商品 SKU。
#[derive(Debug, Serialize, Clone)]
pub struct WechatShopProductSkuView {
    pub sku_id: String,
    pub out_sku_id: Option<String>,
    pub sku_code: Option<String>,
    pub sale_price_cents: Option<i64>,
    pub stock_num: Option<i64>,
    pub sku_attrs: Option<String>,
    pub thumb_img: Option<String>,
}

/// 商品详情（缓存的商品列表项 + SKU 列表）。
#[derive(Debug, Serialize)]
pub struct WechatShopProductDetailView {
    #[serde(flatten)]
    pub product: WechatShopProductView,
    pub skus: Vec<WechatShopProductSkuView>,
}

/// 同步微信商品的结果统计。
#[derive(Debug, Serialize)]
pub struct SyncShopProductsResult {
    pub synced_count: i64,
    pub total_num: i64,
    pub failed_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductSalesAnalysisTotals {
    pub product_count: i64,
    pub sold_product_count: i64,
    pub total_units_sold: i64,
    pub revenue_cents: i64,
    pub purchase_cost_cents: i64,
    pub gross_profit_cents: i64,
    pub missing_cost_product_count: i64,
    pub scale_candidate_count: i64,
    pub risk_product_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductSalesAnalysisView {
    pub external_product_id: String,
    pub title: String,
    pub supplier_name: Option<String>,
    pub active_shop_count: i64,
    pub order_count: i64,
    pub units_sold: i64,
    pub revenue_cents: i64,
    pub purchase_task_count: i64,
    pub missing_cost_count: i64,
    pub purchase_cost_cents: i64,
    pub related_aftersale_count: i64,
    pub related_refund_cents: i64,
    pub total_stock: i64,
    pub available_stock: i64,
    pub inventory_risk_status: String,
    pub operation_status: String,
    pub recommendation: String,
    pub last_order_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct OrderProfitTotals {
    pub order_count: i64,
    pub revenue_cents: i64,
    pub purchase_cost_cents: i64,
    pub purchase_freight_cents: i64,
    pub refund_cents: i64,
    pub aftersale_compensation_cents: i64,
    pub other_cost_cents: i64,
    pub other_income_cents: i64,
    pub estimated_profit_cents: i64,
    pub actual_profit_cents: i64,
    pub unknown_actual_order_count: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderProfitView {
    pub order_id: String,
    pub wechat_order_id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub order_status: String,
    pub item_count: i64,
    pub quantity: i64,
    pub revenue_cents: i64,
    pub purchase_task_count: i64,
    pub missing_cost_count: i64,
    pub purchase_cost_cents: i64,
    pub purchase_freight_cents: i64,
    pub refund_cents: i64,
    pub aftersale_compensation_cents: i64,
    pub other_cost_cents: i64,
    pub other_income_cents: i64,
    pub estimated_profit_cents: i64,
    pub actual_profit_cents: Option<i64>,
    pub profit_status: String,
    pub detail_synced_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrderProfitAdjustmentRequest {
    pub order_id: String,
    pub kind: String,
    pub amount_cents: i64,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrderProfitAdjustmentResult {
    pub adjustment_id: String,
    pub order_id: String,
    pub kind: String,
    pub amount_cents: i64,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct OrderManagementListResult {
    pub items: Vec<OrderManagementView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct OrderManagementView {
    pub order_id: String,
    pub wechat_order_id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub wechat_status: Option<i64>,
    pub order_status: String,
    pub management_status: String,
    pub item_count: i64,
    pub quantity: i64,
    pub revenue_cents: i64,
    pub purchase_task_count: i64,
    pub missing_cost_count: i64,
    pub purchase_status: String,
    pub shipment_count: i64,
    pub shipment_status: String,
    pub active_aftersale_count: i64,
    pub profit_status: String,
    pub estimated_profit_cents: i64,
    pub actual_profit_cents: Option<i64>,
    pub detail_synced_at: Option<String>,
    pub detail_error: Option<String>,
    pub synced_at: Option<String>,
    pub order_created_at: Option<i64>,
    pub order_updated_at: Option<i64>,
    pub updated_at: Option<String>,
    pub items: Vec<OrderManagementItemView>,
}

#[derive(Debug, Serialize, Clone)]
pub struct OrderManagementItemView {
    pub id: String,
    pub wechat_product_id: Option<String>,
    pub wechat_sku_id: Option<String>,
    pub external_product_id: Option<String>,
    pub external_sku_id: Option<String>,
    pub title: Option<String>,
    pub quantity: i64,
    pub sale_price: Option<i64>,
    pub real_price: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AftersaleSyncBatchResult {
    pub task_id: String,
    pub processed_shops: i64,
    pub synced_aftersales: i64,
    pub failed_shops: i64,
    pub failed_aftersales: i64,
}

#[derive(Debug, Serialize)]
pub struct AftersaleListResult {
    pub items: Vec<AftersaleView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct AftersaleView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub order_id: Option<String>,
    pub wechat_order_id: Option<String>,
    pub wechat_aftersale_id: String,
    pub status: String,
    pub aftersale_type: Option<String>,
    pub reason: Option<String>,
    pub refund_amount_cents: Option<i64>,
    pub responsibility_party: Option<String>,
    pub responsibility_note: Option<String>,
    pub supplier_compensation_cents: i64,
    pub handled_at: Option<String>,
    pub last_action: Option<String>,
    pub last_action_status: Option<String>,
    pub last_action_error: Option<String>,
    pub last_action_note: Option<String>,
    pub last_action_at: Option<String>,
    pub evidence_count: i64,
    pub synced_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AftersaleResponsibilityRequest {
    pub aftersale_id: String,
    pub responsibility_party: String,
    pub responsibility_note: Option<String>,
    pub supplier_compensation_cents: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AftersaleResponsibilityResult {
    pub aftersale_id: String,
    pub responsibility_party: String,
    pub supplier_compensation_cents: i64,
    pub profit_adjustment_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AftersaleEvidenceListResult {
    pub items: Vec<AftersaleEvidenceView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct AftersaleEvidenceView {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub external_target_id: String,
    pub evidence_type: String,
    pub evidence_type_text: String,
    pub title: String,
    pub content_text: Option<String>,
    pub local_file_path: Option<String>,
    pub source_url: Option<String>,
    pub status: String,
    pub status_text: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AftersaleEvidenceRecordRequest {
    pub target_type: String,
    pub target_id: String,
    pub evidence_type: String,
    pub title: String,
    pub content_text: Option<String>,
    pub local_file_path: Option<String>,
    pub source_url: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AftersaleEvidenceRecordResult {
    pub evidence_id: String,
    pub target_type: String,
    pub target_id: String,
    pub evidence_type: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AftersaleEvidenceStatusUpdateRequest {
    pub evidence_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AftersaleEvidenceStatusUpdateResult {
    pub evidence_id: String,
    pub status: String,
    pub status_text: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AftersaleEvidenceExportResult {
    pub file_path: String,
    pub exported_count: i64,
    pub format: String,
    pub sensitive_fields: String,
}

#[derive(Debug, Serialize)]
pub struct SupplierAftersaleFollowupListResult {
    pub items: Vec<SupplierAftersaleFollowupView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct SupplierAftersaleFollowupView {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub external_target_id: String,
    pub purchase_task_id: Option<String>,
    pub supplier_name: Option<String>,
    pub followup_type: String,
    pub followup_type_text: String,
    pub status: String,
    pub status_text: String,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SupplierAftersaleFollowupRecordRequest {
    pub target_type: String,
    pub target_id: String,
    pub followup_type: String,
    pub status: String,
    pub note: String,
    pub purchase_task_id: Option<String>,
    pub supplier_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SupplierAftersaleFollowupRecordResult {
    pub followup_id: String,
    pub target_type: String,
    pub target_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AftersaleAcceptRequest {
    pub aftersale_id: String,
    pub address_id: Option<String>,
    pub accept_type: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AftersaleRejectRequest {
    pub aftersale_id: String,
    pub reject_reason_type: i64,
    pub reject_reason: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AftersaleActionResult {
    pub aftersale_id: String,
    pub wechat_aftersale_id: String,
    pub action: String,
    pub status: String,
    pub errcode: Option<i64>,
    pub errmsg: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct AftersaleRejectReasonView {
    pub shop_id: String,
    pub reject_reason_type: i64,
    pub reject_reason_type_text: String,
    pub reject_reason: String,
    pub reject_scene: Option<i64>,
    pub reject_scene_text: String,
    pub synced_at: String,
}

#[derive(Debug, Serialize)]
pub struct AftersaleRejectReasonSyncResult {
    pub task_id: String,
    pub shop_id: String,
    pub synced_reasons: i64,
    pub failed_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GuaranteeSyncBatchResult {
    pub task_id: String,
    pub processed_shops: i64,
    pub synced_guarantees: i64,
    pub failed_shops: i64,
    pub failed_guarantees: i64,
}

#[derive(Debug, Serialize)]
pub struct GuaranteeOrderListResult {
    pub items: Vec<GuaranteeOrderView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct GuaranteeOrderView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub order_id: Option<String>,
    pub wechat_order_id: Option<String>,
    pub guarantee_order_id: String,
    pub guarantee_type: Option<i64>,
    pub guarantee_type_text: String,
    pub status: String,
    pub status_text: String,
    pub apply_reason: Option<String>,
    pub pay_amount_cents: Option<i64>,
    pub merchant_refuse_reason: Option<String>,
    pub handling_status: Option<String>,
    pub handling_note: Option<String>,
    pub responsibility_party: Option<String>,
    pub supplier_compensation_cents: i64,
    pub handled_at: Option<String>,
    pub created_time: Option<i64>,
    pub updated_time_unix: Option<i64>,
    pub expire_time: Option<i64>,
    pub complete_time: Option<i64>,
    pub evidence_count: i64,
    pub synced_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GuaranteeFollowupRequest {
    pub guarantee_order_id: String,
    pub handling_status: String,
    pub responsibility_party: Option<String>,
    pub handling_note: Option<String>,
    pub supplier_compensation_cents: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct GuaranteeFollowupResult {
    pub guarantee_order_id: String,
    pub handling_status: String,
    pub responsibility_party: Option<String>,
    pub supplier_compensation_cents: i64,
    pub profit_adjustment_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ShipmentRecordRequest {
    pub order_id: Option<String>,
    pub shop_id: Option<String>,
    pub wechat_order_id: Option<String>,
    pub delivery_id: Option<String>,
    pub delivery_name: Option<String>,
    pub waybill_id: Option<String>,
    pub deliver_type: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ShipmentRecordResult {
    pub shipment_id: String,
    pub order_id: String,
    pub status: String,
    pub auto_send_enabled: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ShipmentListResult {
    pub items: Vec<ShipmentView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct ShipmentView {
    pub id: String,
    pub order_id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub wechat_order_id: String,
    pub delivery_id: Option<String>,
    pub delivery_name: Option<String>,
    pub waybill_id: Option<String>,
    pub deliver_type: i64,
    pub status: String,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub submitted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ShipmentRetryResult {
    pub shipment_id: String,
    pub status: String,
    pub auto_send_enabled: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DeliverySettings {
    pub auto_send_delivery: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeliveryCompanyView {
    pub shop_id: String,
    pub delivery_id: String,
    pub delivery_name: String,
    pub synced_at: String,
}

#[derive(Debug, Serialize)]
pub struct DeliveryCompanySyncResult {
    pub task_id: String,
    pub shop_id: String,
    pub synced_companies: i64,
    pub failed_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LocalApiConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub has_api_key: bool,
    pub api_key_hint: Option<String>,
    pub auth_header: String,
}

#[derive(Debug, Serialize)]
pub struct LocalApiKeyRotationResult {
    pub api_key: String,
    pub key_hint: String,
    pub base_url: String,
    pub warning: String,
}

#[derive(Debug, Serialize)]
pub struct DeliverySubmitBatchResult {
    pub task_id: String,
    pub processed_shipments: i64,
    pub submitted_shipments: i64,
    pub failed_shipments: i64,
}

#[derive(Debug, Serialize)]
pub struct PublishJobView {
    pub id: String,
    pub request_id: String,
    pub status: String,
    pub accepted_product_count: i64,
    pub target_shop_count: i64,
    pub created_at: String,
    pub products: Vec<PublishProductView>,
}

#[derive(Debug, Serialize)]
pub struct PriceUpdateJobView {
    pub id: String,
    pub request_id: String,
    pub status: String,
    pub accepted_product_count: i64,
    pub target_shop_count: i64,
    pub created_at: String,
    pub items: Vec<PriceUpdateItemView>,
}

#[derive(Debug, Serialize)]
pub struct PriceUpdateItemView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub external_product_id: String,
    pub target_price_cents: i64,
    pub status: String,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub wechat_product_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct OrderPriceAdjustmentJobView {
    pub id: String,
    pub request_id: String,
    pub status: String,
    pub accepted_order_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<OrderPriceAdjustmentItemView>,
}

#[derive(Debug, Serialize)]
pub struct OrderPriceAdjustmentItemView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub order_id: Option<String>,
    pub wechat_order_id: String,
    pub wechat_status: Option<i64>,
    pub change_order_infos: Vec<OrderPriceAdjustmentLineInput>,
    pub change_express: bool,
    pub express_fee_cents: Option<i64>,
    pub note: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PublishProductView {
    pub external_product_id: String,
    pub title: String,
    pub status: String,
    pub success_count: i64,
    pub failed_count: i64,
    pub pending_count: i64,
    pub error_summary: Option<String>,
    pub items: Vec<PublishJobItemView>,
}

#[derive(Debug, Serialize)]
pub struct PublishJobItemView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    pub status: String,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CollectionTaskView {
    pub id: String,
    pub title: String,
    pub source_url: String,
    pub category_path: String,
    pub target_shop_ids: Vec<String>,
    pub status: String,
    pub error_summary: Option<String>,
    pub collected_data: Option<String>,
    pub review_status: String,
    pub review_summary: Option<String>,
    pub reviewed_data: Option<String>,
    pub review_result_json: Option<String>,
    pub reviewed_at: Option<String>,
    pub published_shop_ids: Vec<String>,
    pub publish_job_ids: Vec<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 统一流水线商品视图（前端工作台一行 = 一条流水线）。后端直接吐出，前端零适配层。
#[derive(Debug, Serialize)]
pub struct PipelineProductView {
    pub id: String,
    pub external_product_id: Option<String>,
    pub title: String,
    pub source_url: String,
    pub category_path: String,
    /// 对外 6 态：pending_collect / collecting / need_confirm / publishing / listed / error
    pub status: String,
    /// none / need_confirm / error —— 驱动前端红黄点
    pub attention: String,
    pub progress_text: Option<String>,
    pub error_code: Option<String>,
    pub error_reason: Option<String>,
    pub total_shops: i64,
    pub listed_shops: i64,
    pub failed_shops: i64,
    pub pending_shops: i64,
    pub can_retry: bool,
    pub can_confirm: bool,
    /// 待确认子类型（CATEGORY/ATTR/IMAGE/SHOP_SETTING/REVIEW/GENERIC），决定前端弹哪种确认 UI
    pub confirm_kind: Option<String>,
    pub updated_at: String,
    pub shops: Vec<PipelineShopTargetView>,
}

/// 流水线商品在单个目标店的推进视图（主行展开）。
#[derive(Debug, Serialize)]
pub struct PipelineShopTargetView {
    pub id: String,
    pub shop_id: String,
    pub shop_name: String,
    /// 人话状态：已上架 / 类目预检中 / 等待微信审核 / 失败原因 …
    pub status_text: String,
    pub error_code: Option<String>,
    pub error_reason: Option<String>,
    pub can_retry: bool,
    pub wechat_product_id: Option<String>,
    pub audit_summary: Option<String>,
    pub updated_at: String,
}
