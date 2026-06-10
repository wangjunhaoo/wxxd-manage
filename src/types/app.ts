export type DashboardSummary = {
  pending_order_count: number;
  abnormal_shop_count: number;
  failed_publish_product_count: number;
  unread_notification_count: number;
  running_task_count: number;
  controller_status: string;
  database_path: string;
  now_shanghai: string;
  last_order_sync_at: string | null;
  last_publish_summary: string | null;
};

export type ShopGroup = {
  id: string;
  name: string;
  status: string;
  shop_count: number;
  created_at: string;
};

export type ShopListItem = {
  id: string;
  name: string;
  appid: string;
  status: string;
  group_id: string;
  group_name: string;
  has_secret: boolean;
  token_expires_at: string | null;
  wechat_nickname: string | null;
  wechat_status: string | null;
  last_health_check_at: string | null;
  last_quota_remain: number | null;
  created_at: string;
};

export type ShopCredentialCheck = {
  shop_id: string;
  status: string;
  expires_at: string | null;
  errcode: number | null;
  errmsg: string | null;
};

export type ShopBasicInfoSyncResult = {
  shop_id: string;
  status: string;
  nickname: string | null;
  wechat_status: string | null;
  errcode: number | null;
  errmsg: string | null;
};

export type ApiQuotaCheckResult = {
  shop_id: string;
  cgi_path: string;
  daily_limit: number | null;
  used: number | null;
  remain: number | null;
  rate_call_count: number | null;
  rate_refresh_second: number | null;
  errcode: number | null;
  errmsg: string | null;
};

export type CategoryCatalogShopSummary = {
  shop_id: string;
  shop_name: string;
  category_count: number;
  detail_count: number;
  product_rule_count: number;
  delivery_rule_count: number;
  category_relation_count: number;
  active_category_relation_count: number;
  freight_template_count: number;
  last_category_sync_at: string | null;
  last_relation_sync_at: string | null;
  last_rule_sync_at: string | null;
  last_freight_sync_at: string | null;
};

export type CategoryCacheView = {
  shop_id: string;
  shop_name: string;
  cat_id: number;
  parent_cat_id: number | null;
  level: number | null;
  name: string;
  product_attr_count: number;
  sale_attr_count: number;
  product_qua_count: number;
  has_detail: boolean;
  has_product_rule: boolean;
  has_delivery_rule: boolean;
  is_available_for_shop: boolean;
  synced_at: string;
  detail_synced_at: string | null;
};

export type CategoryRelationView = {
  shop_id: string;
  shop_name: string;
  cat_id: number;
  category_name: string | null;
  status: number;
  uneffective_reason: string | null;
  effective_time: number | null;
  uneffective_time: number | null;
  qua_id: number | null;
  synced_at: string;
};

export type FreightTemplateView = {
  shop_id: string;
  shop_name: string;
  template_id: string;
  template_name: string | null;
  synced_at: string;
  is_default: boolean;
};

export type CategoryCatalogListResult = {
  shops: CategoryCatalogShopSummary[];
  categories: CategoryCacheView[];
  category_relations: CategoryRelationView[];
  freight_templates: FreightTemplateView[];
};

export type CategoryCatalogSyncResult = {
  task_id: string;
  shop_id: string;
  synced_categories: number;
  synced_category_relations: number;
  synced_freight_templates: number;
  failed_steps: string[];
};

export type CategoryRuleSyncResult = {
  task_id: string;
  shop_id: string;
  cat_id: number;
  synced_detail: boolean;
  synced_product_rule: boolean;
  synced_delivery_rule: boolean;
  product_attr_count: number;
  sale_attr_count: number;
  product_qua_count: number;
  failed_steps: string[];
};

export type CategoryDetailPrewarmResult = {
  task_id: string;
  shop_id: string;
  total: number;
  synced: number;
  skipped: number;
  failed_count: number;
  failed_cats: number[];
};

export type PublishJobCreated = {
  task_id: string;
  status: string;
  accepted_product_count: number;
  target_shop_count: number;
};

export type TaskRunView = {
  id: string;
  task_type: string;
  status: string;
  progress: number;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
  pending_count: number;
  ready_count: number;
  failed_count: number;
};

export type AgentRunView = {
  id: string;
  skill_name: string;
  skill_version: string;
  scene: string;
  source_type: string;
  source_id: string;
  shop_id: string | null;
  status: string;
  provider_type: string | null;
  model: string | null;
  temperature: number | null;
  input_summary: string;
  decision: string | null;
  error_code: string | null;
  error_summary: string | null;
  started_at: string | null;
  finished_at: string | null;
  duration_ms: number | null;
  created_at: string;
};

export type AgentRunEventView = {
  id: string;
  run_id: string;
  event_type: string;
  level: string;
  message: string;
  data_json: string | null;
  created_at: string;
};

export type PublishTaskBatchResult = {
  processed_jobs: number;
  processed_items: number;
  ready_items: number;
  failed_items: number;
};

export type PublishAttributeFillBatchResult = {
  processed_jobs: number;
  processed_items: number;
  auto_filled_items: number;
  suggestion_only_items: number;
  failed_items: number;
  generated_suggestions: number;
};

export type PublishCategoryPrecheckBatchResult = {
  processed_jobs: number;
  processed_items: number;
  passed_items: number;
  failed_items: number;
  skipped_items: number;
};

export type AssetUploadBatchResult = {
  processed_jobs: number;
  processed_items: number;
  uploaded_assets: number;
  reused_assets: number;
  failed_items: number;
};

export type ProductSubmitBatchResult = {
  processed_jobs: number;
  processed_items: number;
  submitted_items: number;
  failed_items: number;
};

export type ProductStatusSyncBatchResult = {
  processed_jobs: number;
  processed_items: number;
  success_items: number;
  pending_items: number;
  failed_items: number;
};

export type ProductListingBatchResult = {
  processed_jobs: number;
  processed_items: number;
  listing_submitted_items: number;
  failed_items: number;
};

export type PublishPipelineRunResult = {
  executed_steps: string[];
  skipped_steps: string[];
  errors: AutomationStepError[];
  publish_precheck: PublishTaskBatchResult | null;
  publish_attribute_fill: PublishAttributeFillBatchResult | null;
  publish_category_precheck: PublishCategoryPrecheckBatchResult | null;
  publish_asset_upload: AssetUploadBatchResult | null;
  publish_submit: ProductSubmitBatchResult | null;
  publish_status_sync: ProductStatusSyncBatchResult | null;
  publish_listing: ProductListingBatchResult | null;
};

export type PriceUpdateJobCreated = {
  task_id: string;
  status: string;
  accepted_product_count: number;
  target_shop_count: number;
};

export type OrderPriceAdjustmentJobCreated = {
  task_id: string;
  status: string;
  accepted_order_count: number;
};

export type PriceUpdatePrecheckBatchResult = {
  processed_jobs: number;
  processed_items: number;
  ready_items: number;
  failed_items: number;
};

export type PriceUpdateSubmitBatchResult = {
  processed_jobs: number;
  processed_items: number;
  submitted_items: number;
  failed_items: number;
};

export type PriceUpdateConfirmBatchResult = {
  processed_jobs: number;
  processed_items: number;
  confirmed_items: number;
  pending_items: number;
  failed_items: number;
};

export type OrderPriceAdjustmentBatchResult = {
  processed_jobs: number;
  processed_items: number;
  success_items: number;
  failed_items: number;
};

export type OrderSyncBatchResult = {
  task_id: string;
  processed_shops: number;
  synced_orders: number;
  failed_shops: number;
};

export type OrderDetailSyncBatchResult = {
  task_id: string;
  processed_orders: number;
  synced_orders: number;
  created_items: number;
  failed_orders: number;
};

export type AftersaleSyncBatchResult = {
  task_id: string;
  processed_shops: number;
  synced_aftersales: number;
  failed_shops: number;
  failed_aftersales: number;
};

export type AftersaleView = {
  id: string;
  shop_id: string;
  shop_name: string;
  order_id: string | null;
  wechat_order_id: string | null;
  wechat_aftersale_id: string;
  status: string;
  aftersale_type: string | null;
  reason: string | null;
  refund_amount_cents: number | null;
  responsibility_party: string | null;
  responsibility_note: string | null;
  supplier_compensation_cents: number;
  handled_at: string | null;
  last_action: string | null;
  last_action_status: string | null;
  last_action_error: string | null;
  last_action_note: string | null;
  last_action_at: string | null;
  evidence_count: number;
  synced_at: string;
  updated_at: string;
};

export type AftersaleListResult = {
  items: AftersaleView[];
  total: number;
};

export type AftersaleResponsibilityResult = {
  aftersale_id: string;
  responsibility_party: string;
  supplier_compensation_cents: number;
  profit_adjustment_id: string | null;
  message: string;
};

export type AftersaleActionResult = {
  aftersale_id: string;
  wechat_aftersale_id: string;
  action: string;
  status: string;
  errcode: number | null;
  errmsg: string | null;
  message: string;
};

export type AftersaleRejectReasonView = {
  shop_id: string;
  reject_reason_type: number;
  reject_reason_type_text: string;
  reject_reason: string;
  reject_scene: number | null;
  reject_scene_text: string;
  synced_at: string;
};

export type AftersaleRejectReasonSyncResult = {
  task_id: string;
  shop_id: string;
  synced_reasons: number;
  failed_steps: string[];
};

export type GuaranteeSyncBatchResult = {
  task_id: string;
  processed_shops: number;
  synced_guarantees: number;
  failed_shops: number;
  failed_guarantees: number;
};

export type GuaranteeOrderView = {
  id: string;
  shop_id: string;
  shop_name: string;
  order_id: string | null;
  wechat_order_id: string | null;
  guarantee_order_id: string;
  guarantee_type: number | null;
  guarantee_type_text: string;
  status: string;
  status_text: string;
  apply_reason: string | null;
  pay_amount_cents: number | null;
  merchant_refuse_reason: string | null;
  handling_status: string | null;
  handling_note: string | null;
  responsibility_party: string | null;
  supplier_compensation_cents: number;
  handled_at: string | null;
  created_time: number | null;
  updated_time_unix: number | null;
  expire_time: number | null;
  complete_time: number | null;
  evidence_count: number;
  synced_at: string;
  updated_at: string;
};

export type GuaranteeOrderListResult = {
  items: GuaranteeOrderView[];
  total: number;
};

export type GuaranteeFollowupResult = {
  guarantee_order_id: string;
  handling_status: string;
  responsibility_party: string | null;
  supplier_compensation_cents: number;
  profit_adjustment_id: string | null;
  message: string;
};

export type AftersaleEvidenceView = {
  id: string;
  target_type: string;
  target_id: string;
  shop_id: string;
  shop_name: string;
  external_target_id: string;
  evidence_type: string;
  evidence_type_text: string;
  title: string;
  content_text: string | null;
  local_file_path: string | null;
  source_url: string | null;
  status: string;
  status_text: string;
  created_at: string;
  updated_at: string;
};

export type AftersaleEvidenceListResult = {
  items: AftersaleEvidenceView[];
  total: number;
};

export type AftersaleEvidenceRecordResult = {
  evidence_id: string;
  target_type: string;
  target_id: string;
  evidence_type: string;
  status: string;
  message: string;
};

export type AftersaleEvidenceStatusUpdateResult = {
  evidence_id: string;
  status: string;
  status_text: string;
  message: string;
};

export type AftersaleEvidenceExportResult = {
  file_path: string;
  exported_count: number;
  format: string;
  sensitive_fields: string;
};

export type SupplierAftersaleFollowupView = {
  id: string;
  target_type: string;
  target_id: string;
  shop_id: string;
  shop_name: string;
  external_target_id: string;
  purchase_task_id: string | null;
  supplier_name: string | null;
  followup_type: string;
  followup_type_text: string;
  status: string;
  status_text: string;
  note: string;
  created_at: string;
  updated_at: string;
};

export type SupplierAftersaleFollowupListResult = {
  items: SupplierAftersaleFollowupView[];
  total: number;
};

export type SupplierAftersaleFollowupRecordResult = {
  followup_id: string;
  target_type: string;
  target_id: string;
  status: string;
  message: string;
};

export type PurchaseTaskBatchResult = {
  task_id: string;
  processed_items: number;
  created_tasks: number;
  skipped_items: number;
};

export type PurchaseTaskView = {
  id: string;
  order_id: string;
  wechat_order_id: string;
  shop_id: string;
  shop_name: string;
  status: string;
  external_product_id: string | null;
  external_sku_id: string | null;
  source_url: string | null;
  title: string | null;
  quantity: number;
  sale_price: number | null;
  real_price: number | null;
  estimated_revenue: number | null;
  estimated_cost: number | null;
  estimated_profit: number | null;
  supplier_name: string | null;
  supplier_product_id: string | null;
  supplier_delivery_id: string | null;
  supplier_delivery_name: string | null;
  supplier_waybill_id: string | null;
  supplier_deliver_type: number | null;
  supplier_shipped_at: string | null;
  error_summary: string | null;
  created_at: string;
  updated_at: string;
};

export type PurchaseTaskListResult = {
  items: PurchaseTaskView[];
  total: number;
};

export type PurchaseTaskExportResult = {
  file_path: string;
  exported_count: number;
};

export type SupplierAgentExportResult = {
  file_path: string;
  exported_count: number;
  format: string;
  sensitive_fields: string;
};

export type SupplierAgentApplyItemResult = {
  index: number;
  purchase_task_id: string | null;
  action: string | null;
  status: string;
  error: string | null;
  response: unknown | null;
};

export type SupplierAgentApplyResult = {
  dry_run: boolean;
  processed: number;
  succeeded: number;
  failed: number;
  results: SupplierAgentApplyItemResult[];
};

export type PurchaseTaskMappingResult = {
  purchase_task_id: string;
  order_id: string;
  status: string;
  external_product_id: string;
  external_sku_id: string;
  message: string;
};

export type PurchaseTaskIssueResult = {
  purchase_task_id: string;
  order_id: string;
  status: string;
  message: string;
};

export type PurchaseTaskShipmentResult = {
  purchase_task_id: string;
  order_id: string;
  purchase_status: string;
  shipment_id: string | null;
  shipment_status: string | null;
  auto_send_enabled: boolean;
  message: string;
};

export type OrderProfitView = {
  order_id: string;
  wechat_order_id: string;
  shop_id: string;
  shop_name: string;
  order_status: string;
  item_count: number;
  quantity: number;
  revenue_cents: number;
  purchase_task_count: number;
  missing_cost_count: number;
  purchase_cost_cents: number;
  purchase_freight_cents: number;
  refund_cents: number;
  aftersale_compensation_cents: number;
  other_cost_cents: number;
  other_income_cents: number;
  estimated_profit_cents: number;
  actual_profit_cents: number | null;
  profit_status: string;
  detail_synced_at: string | null;
  updated_at: string | null;
};

export type OrderProfitTotals = {
  order_count: number;
  revenue_cents: number;
  purchase_cost_cents: number;
  purchase_freight_cents: number;
  refund_cents: number;
  aftersale_compensation_cents: number;
  other_cost_cents: number;
  other_income_cents: number;
  estimated_profit_cents: number;
  actual_profit_cents: number;
  unknown_actual_order_count: number;
};

export type OrderProfitListResult = {
  items: OrderProfitView[];
  total: number;
  totals: OrderProfitTotals;
};

export type InventoryRiskView = {
  external_product_id: string;
  title: string;
  supplier_name: string | null;
  supplier_product_id: string | null;
  total_stock: number;
  reserved_quantity: number;
  available_stock: number;
  active_shop_count: number;
  pending_purchase_quantity: number;
  supplier_issue_count: number;
  risk_status: string;
  recommendation: string;
  updated_at: string;
};

export type InventoryRiskListResult = {
  items: InventoryRiskView[];
  total: number;
  low_stock_count: number;
  out_of_stock_count: number;
  issue_count: number;
};

export type InventoryRiskScanResult = {
  task_id: string;
  scanned_products: number;
  low_stock_products: number;
  out_of_stock_products: number;
  issue_products: number;
  notifications_created: number;
};

export type ProductSalesAnalysisView = {
  external_product_id: string;
  title: string;
  supplier_name: string | null;
  active_shop_count: number;
  order_count: number;
  units_sold: number;
  revenue_cents: number;
  purchase_task_count: number;
  missing_cost_count: number;
  purchase_cost_cents: number;
  related_aftersale_count: number;
  related_refund_cents: number;
  total_stock: number;
  available_stock: number;
  inventory_risk_status: string;
  operation_status: string;
  recommendation: string;
  last_order_at: string | null;
  updated_at: string;
};

export type ProductSalesAnalysisTotals = {
  product_count: number;
  sold_product_count: number;
  total_units_sold: number;
  revenue_cents: number;
  purchase_cost_cents: number;
  gross_profit_cents: number;
  missing_cost_product_count: number;
  scale_candidate_count: number;
  risk_product_count: number;
};

export type ProductSalesAnalysisListResult = {
  items: ProductSalesAnalysisView[];
  total: number;
  totals: ProductSalesAnalysisTotals;
};

export type ProductManagementShopView = {
  shop_id: string;
  shop_name: string;
  status: string;
  wechat_product_id: string | null;
  wechat_status: number | null;
  wechat_edit_status: number | null;
  current_price_cents: number | null;
  last_status_sync_at: string | null;
  last_price_update_at: string | null;
  audit_summary: string | null;
  source_url: string | null;
};

export type ProductManagementView = {
  external_product_id: string;
  title: string;
  source_url: string | null;
  supplier_name: string | null;
  supplier_product_id: string | null;
  management_status: string;
  publish_status: string | null;
  publish_error_summary: string | null;
  active_shop_count: number;
  shop_count: number;
  order_count: number;
  units_sold: number;
  revenue_cents: number;
  purchase_task_count: number;
  missing_cost_count: number;
  purchase_cost_cents: number;
  related_aftersale_count: number;
  related_refund_cents: number;
  total_stock: number;
  available_stock: number;
  inventory_risk_status: string;
  operation_status: string;
  recommendation: string;
  last_order_at: string | null;
  updated_at: string;
  shops: ProductManagementShopView[];
};

export type ProductManagementListResult = {
  items: ProductManagementView[];
  total: number;
};

export type OrderProfitAdjustmentResult = {
  adjustment_id: string;
  order_id: string;
  kind: string;
  amount_cents: number;
  message: string;
};

export type OrderManagementItemView = {
  id: string;
  wechat_product_id: string | null;
  wechat_sku_id: string | null;
  external_product_id: string | null;
  external_sku_id: string | null;
  title: string | null;
  quantity: number;
  sale_price: number | null;
  real_price: number | null;
};

export type OrderManagementView = {
  order_id: string;
  wechat_order_id: string;
  shop_id: string;
  shop_name: string;
  wechat_status: number | null;
  order_status: string;
  management_status: string;
  item_count: number;
  quantity: number;
  revenue_cents: number;
  purchase_task_count: number;
  missing_cost_count: number;
  purchase_status: string;
  shipment_count: number;
  shipment_status: string;
  active_aftersale_count: number;
  profit_status: string;
  estimated_profit_cents: number;
  actual_profit_cents: number | null;
  detail_synced_at: string | null;
  detail_error: string | null;
  synced_at: string | null;
  order_created_at: number | null;
  order_updated_at: number | null;
  updated_at: string | null;
  items: OrderManagementItemView[];
};

export type OrderManagementListResult = {
  items: OrderManagementView[];
  total: number;
};

export type DeliverySettings = {
  auto_send_delivery: boolean;
};

export type DeliveryCompanyView = {
  shop_id: string;
  delivery_id: string;
  delivery_name: string;
  synced_at: string;
};

export type DeliveryCompanySyncResult = {
  task_id: string;
  shop_id: string;
  synced_companies: number;
  failed_steps: string[];
};

export type LocalApiConfig = {
  enabled: boolean;
  host: string;
  port: number;
  base_url: string;
  has_api_key: boolean;
  api_key_hint: string | null;
  auth_header: string;
};

export type LocalApiKeyRotationResult = {
  api_key: string;
  key_hint: string;
  base_url: string;
  warning: string;
};

export type AiProviderSettings = {
  enabled: boolean;
  provider_type: string;
  custom_provider_id: string;
  api: string;
  base_url: string;
  model: string;
  temperature: number;
  context_window: number;
  max_tokens: number;
  has_api_key: boolean;
  api_key_hint: string | null;
  updated_at: string | null;
};

export type AiProviderTestResult = {
  status: string;
  provider_type: string;
  model: string;
  message: string;
};

export type AgentSkillView = {
  name: string;
  version: string;
  description: string;
  enabled: boolean;
  runtime: string;
  model: string | null;
  temperature: number | null;
  skill_path: string;
  schema_path: string;
  checksum: string;
  file_status: string;
  runtime_status: string;
  last_test_status: string | null;
  last_test_summary: string | null;
  last_test_at: string | null;
  updated_at: string | null;
};

export type AgentSkillSettingsRequest = {
  name: string;
  enabled: boolean;
  model?: string | null;
  temperature?: number | null;
};

export type AgentSkillTestResult = {
  name: string;
  status: string;
  summary: string;
  checked_at: string;
};

export type ExternalApiLogView = {
  id: string;
  method: string;
  path: string;
  status: string;
  status_code: number;
  error_code: string | null;
  request_summary: string | null;
  response_summary: string | null;
  duration_ms: number;
  created_at: string;
};

export type NotificationView = {
  id: string;
  severity: string;
  source_type: string;
  source_id: string;
  shop_id: string | null;
  shop_name: string | null;
  title: string;
  body: string;
  status: string;
  data_json: string | null;
  read_at: string | null;
  created_at: string;
  updated_at: string;
};

export type NotificationListResult = {
  items: NotificationView[];
  total: number;
  unread_count: number;
  critical_count: number;
};

export type NotificationMarkResult = {
  updated_count: number;
  message: string;
};

export type BackupInfo = {
  id: string;
  file_name: string;
  file_path: string;
  size_bytes: number;
  sha256: string;
  created_at: string;
  integrity_ok: boolean;
  integrity_message: string;
};

export type BackupCreateResult = {
  backup: BackupInfo;
};

export type BackupRestoreResult = {
  restored_from: BackupInfo;
  rollback_backup: BackupInfo;
  integrity_ok: boolean;
  message: string;
};

export type WorkspaceResetTableCount = {
  name: string;
  before: number;
  after: number;
};

export type CollectionPublishWorkspaceResetResult = {
  backup: BackupInfo;
  integrity_ok: boolean;
  integrity_message: string;
  counts: WorkspaceResetTableCount[];
  message: string;
};

export type ShipmentRecordResult = {
  shipment_id: string;
  order_id: string;
  status: string;
  auto_send_enabled: boolean;
  message: string;
};

export type ShipmentView = {
  id: string;
  order_id: string;
  shop_id: string;
  shop_name: string;
  wechat_order_id: string;
  delivery_id: string | null;
  delivery_name: string | null;
  waybill_id: string | null;
  deliver_type: number;
  status: string;
  error_code: string | null;
  error_summary: string | null;
  submitted_at: string | null;
  created_at: string;
  updated_at: string;
};

export type ShipmentListResult = {
  items: ShipmentView[];
  total: number;
};

export type ShipmentRetryResult = {
  shipment_id: string;
  status: string;
  auto_send_enabled: boolean;
  message: string;
};

export type DeliverySubmitBatchResult = {
  task_id: string;
  processed_shipments: number;
  submitted_shipments: number;
  failed_shipments: number;
};

export type OperationalAutomationSettings = {
  order_sync_enabled: boolean;
  order_detail_sync_enabled: boolean;
  aftersale_sync_enabled: boolean;
  purchase_task_enabled: boolean;
  delivery_submission_enabled: boolean;
  publish_precheck_enabled: boolean;
  publish_attribute_fill_enabled: boolean;
  publish_category_precheck_enabled: boolean;
  publish_asset_upload_enabled: boolean;
  publish_submit_enabled: boolean;
  publish_status_sync_enabled: boolean;
  publish_listing_enabled: boolean;
  price_confirm_enabled: boolean;
};

export type AutomationStepError = {
  step: string;
  error: string;
};

export type OperationalAutomationRunResult = {
  executed_steps: string[];
  skipped_steps: string[];
  errors: AutomationStepError[];
  order_sync: OrderSyncBatchResult | null;
  order_detail_sync: OrderDetailSyncBatchResult | null;
  aftersale_sync: AftersaleSyncBatchResult | null;
  purchase_task_generation: PurchaseTaskBatchResult | null;
  delivery_submission: DeliverySubmitBatchResult | null;
  publish_precheck: PublishTaskBatchResult | null;
  publish_attribute_fill: PublishAttributeFillBatchResult | null;
  publish_category_precheck: PublishCategoryPrecheckBatchResult | null;
  publish_asset_upload: AssetUploadBatchResult | null;
  publish_submit: ProductSubmitBatchResult | null;
  publish_status_sync: ProductStatusSyncBatchResult | null;
  publish_listing: ProductListingBatchResult | null;
  price_confirm: PriceUpdateConfirmBatchResult | null;
};

export type PublishJobView = {
  id: string;
  request_id: string;
  status: string;
  accepted_product_count: number;
  target_shop_count: number;
  created_at: string;
  products: Array<{
    external_product_id: string;
    title: string;
    status: string;
    success_count: number;
    failed_count: number;
    pending_count: number;
    error_summary: string | null;
    items: Array<{
      id: string;
      shop_id: string;
      shop_name: string;
      status: string;
      error_code: string | null;
      error_summary: string | null;
      created_at: string;
    }>;
  }>;
};

export type PublishJobProductView = PublishJobView["products"][number];
export type PublishJobItemRow = PublishJobProductView["items"][number];

export type PriceUpdateJobView = {
  id: string;
  request_id: string;
  status: string;
  accepted_product_count: number;
  target_shop_count: number;
  created_at: string;
  items: Array<{
    id: string;
    shop_id: string;
    shop_name: string;
    external_product_id: string;
    target_price_cents: number;
    status: string;
    error_code: string | null;
    error_summary: string | null;
    wechat_product_id: string | null;
    created_at: string;
    updated_at: string;
  }>;
};

export type OrderPriceAdjustmentLine = {
  product_id: string;
  sku_id: string;
  change_price_cents: number;
};

export type OrderPriceAdjustmentJobView = {
  id: string;
  request_id: string;
  status: string;
  accepted_order_count: number;
  created_at: string;
  updated_at: string;
  items: Array<{
    id: string;
    shop_id: string;
    shop_name: string;
    order_id: string | null;
    wechat_order_id: string;
    wechat_status: number | null;
    change_order_infos: OrderPriceAdjustmentLine[];
    change_express: boolean;
    express_fee_cents: number | null;
    note: string | null;
    status: string;
    error_code: string | null;
    error_summary: string | null;
    created_at: string;
    updated_at: string;
    submitted_at: string | null;
  }>;
};

// 采集与导入相关状态
export type CollectionTaskView = {
  id: string;
  title: string;
  source_url: string;
  category_path: string;
  target_shop_ids: string[];
  status: "pending" | "running" | "success" | "failed";
  error_summary: string | null;
  collected_data: string | null;
  review_status: "pending" | "passed" | "needs_review" | "blocked" | "failed";
  review_summary: string | null;
  reviewed_data: string | null;
  review_result_json: string | null;
  reviewed_at: string | null;
  published_shop_ids: string[];
  publish_job_ids: string[];
  published_at: string | null;
  created_at: string;
  updated_at: string;
};

// 统一流水线商品视图（后端 list_pipeline_products 返回，字段 snake_case）
export type PipelineShopTargetView = {
  id: string;
  shop_id: string;
  shop_name: string;
  status_text: string;
  error_code: string | null;
  error_reason: string | null;
  can_retry: boolean;
  wechat_product_id: string | null;
  audit_summary: string | null;
  updated_at: string;
};

export type PipelineProductView = {
  id: string;
  external_product_id: string | null;
  title: string;
  source_url: string;
  category_path: string;
  status:
    | "pending_collect"
    | "collecting"
    | "collected"
    | "need_confirm"
    | "publishing"
    | "listed"
    | "error";
  attention: "none" | "need_confirm" | "error";
  progress_text: string | null;
  error_code: string | null;
  error_reason: string | null;
  total_shops: number;
  listed_shops: number;
  failed_shops: number;
  pending_shops: number;
  can_retry: boolean;
  can_confirm: boolean;
  confirm_kind: string | null;
  updated_at: string;
  shops: PipelineShopTargetView[];
};

// 采集明细（后端 get_pipeline_product_detail，点商品标题按需拉取，不在列表里）
export type PipelineProductDetailSku = {
  external_sku_id: string;
  specs: Record<string, unknown> | null;
  cost_price: number;
  stock: number;
};

export type PipelineProductDetailView = {
  id: string;
  external_product_id: string | null;
  title: string;
  source_url: string;
  category_path: string;
  images: string[];
  detail_images: string[];
  supplier_name: string | null;
  brand_hint: string | null;
  category_hint: string | null;
  weight_gram: number | null;
  item_params: Record<string, unknown> | null;
  skus: PipelineProductDetailSku[];
};

// 微信小店真实商品（后端 list_cached_shop_products / get_cached_shop_product_detail / sync_shop_products，字段 snake_case）
export type WechatShopProductSkuView = {
  sku_id: string;
  out_sku_id: string | null;
  sku_code: string | null;
  sale_price_cents: number | null;
  stock_num: number | null;
  sku_attrs: string | null;
  thumb_img: string | null;
};

export type WechatShopProductView = {
  id: string;
  shop_id: string;
  shop_name: string;
  wechat_product_id: string;
  out_product_id: string | null;
  title: string;
  head_img: string | null;
  status: number;
  edit_status: number | null;
  min_price_cents: number | null;
  cat_id: number | null;
  total_stock: number;
  sku_count: number;
  audit_summary: string | null;
  synced_at: string;
  updated_at: string;
};

export type WechatShopProductDetailView = WechatShopProductView & {
  skus: WechatShopProductSkuView[];
};

export type ShopProductListResult = {
  items: WechatShopProductView[];
  total: number;
};

export type SyncShopProductsResult = {
  synced_count: number;
  total_num: number;
  failed_count: number;
};

export type CleanupOrphanDraftsResult = {
  shop_id: string;
  draft_total: number;
  deleted: number;
  listed_promoted: number;
  remaining_after: number;
};

export type PublishPricingStrategy = {
  sale_price_markup_rate: number;
  sale_price_fixed_cents: number;
  sale_price_floor_cents: number;
};

export type CollectionPublishRequest = {
  collection_task_ids: string[];
  target_shop_ids: string[];
  pricing_strategy?: PublishPricingStrategy;
};

export type CollectionReviewRunRequest = {
  task_ids?: string[];
  target_shop_ids?: string[];
  limit?: number;
};

export type CollectionReviewBatchResult = {
  processed_items: number;
  passed_items: number;
  needs_review_items: number;
  blocked_items: number;
  failed_items: number;
};

export type CollectionReviewConfirmRequest = {
  task_id: string;
  title?: string;
  category_ids?: number[];
  category_path?: string;
  target_shop_ids?: string[];
};

export type CollectionImageUploadRequest = {
  task_id: string;
  kind: "main" | "detail";
  file_path: string;
};

export type CollectionImageRemoveRequest = {
  task_id: string;
  kind: "main" | "detail";
  image_url: string;
};
