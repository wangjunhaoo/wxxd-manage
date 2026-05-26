<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ElMessage, ElMessageBox } from "element-plus";
import { Bell, CircleCheck, CircleClose, Plus, Refresh, Search, UploadFilled } from "@element-plus/icons-vue";

type DashboardSummary = {
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

type ShopGroup = {
  id: string;
  name: string;
  status: string;
  shop_count: number;
  created_at: string;
};

type ShopListItem = {
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

type ShopCredentialCheck = {
  shop_id: string;
  status: string;
  expires_at: string | null;
  errcode: number | null;
  errmsg: string | null;
};

type ShopBasicInfoSyncResult = {
  shop_id: string;
  status: string;
  nickname: string | null;
  wechat_status: string | null;
  errcode: number | null;
  errmsg: string | null;
};

type ApiQuotaCheckResult = {
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

type CategoryCatalogShopSummary = {
  shop_id: string;
  shop_name: string;
  category_count: number;
  detail_count: number;
  product_rule_count: number;
  delivery_rule_count: number;
  freight_template_count: number;
  last_category_sync_at: string | null;
  last_rule_sync_at: string | null;
  last_freight_sync_at: string | null;
};

type CategoryCacheView = {
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
  synced_at: string;
  detail_synced_at: string | null;
};

type FreightTemplateView = {
  shop_id: string;
  shop_name: string;
  template_id: string;
  synced_at: string;
};

type CategoryCatalogListResult = {
  shops: CategoryCatalogShopSummary[];
  categories: CategoryCacheView[];
  freight_templates: FreightTemplateView[];
};

type CategoryCatalogSyncResult = {
  task_id: string;
  shop_id: string;
  synced_categories: number;
  synced_freight_templates: number;
  failed_steps: string[];
};

type CategoryRuleSyncResult = {
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

type PublishJobCreated = {
  task_id: string;
  status: string;
  accepted_product_count: number;
  target_shop_count: number;
};

type TaskRunView = {
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

type PublishTaskBatchResult = {
  processed_jobs: number;
  processed_items: number;
  ready_items: number;
  failed_items: number;
};

type PublishAttributeFillBatchResult = {
  processed_jobs: number;
  processed_items: number;
  auto_filled_items: number;
  suggestion_only_items: number;
  failed_items: number;
  generated_suggestions: number;
};

type PublishAttributeSuggestionSkuValue = {
  sku_index: number;
  value: string;
};

type PublishAttributeSuggestionView = {
  id: string;
  item_id: string;
  job_id: string;
  product_row_id: string;
  shop_id: string;
  shop_name: string;
  external_product_id: string;
  title: string;
  attr_kind: string;
  attr_key: string;
  suggested_value: string | null;
  sku_values: PublishAttributeSuggestionSkuValue[];
  confidence: number;
  source: string;
  applied: boolean;
  allowed_values: string[];
  reason: string | null;
  updated_at: string;
};

type PublishAttributeSuggestionListResult = {
  items: PublishAttributeSuggestionView[];
  total: number;
  pending_count: number;
  applied_count: number;
};

type PublishAttributeSuggestionApplyResult = {
  processed_suggestions: number;
  processed_items: number;
  applied_suggestions: number;
  updated_items: number;
  failed_suggestions: number;
  message: string;
};

type PublishCategoryPrecheckBatchResult = {
  processed_jobs: number;
  processed_items: number;
  passed_items: number;
  failed_items: number;
  skipped_items: number;
};

type AssetUploadBatchResult = {
  processed_jobs: number;
  processed_items: number;
  uploaded_assets: number;
  reused_assets: number;
  failed_items: number;
};

type ProductSubmitBatchResult = {
  processed_jobs: number;
  processed_items: number;
  submitted_items: number;
  failed_items: number;
};

type ProductStatusSyncBatchResult = {
  processed_jobs: number;
  processed_items: number;
  success_items: number;
  pending_items: number;
  failed_items: number;
};

type ProductListingBatchResult = {
  processed_jobs: number;
  processed_items: number;
  listing_submitted_items: number;
  failed_items: number;
};

type PriceUpdateJobCreated = {
  task_id: string;
  status: string;
  accepted_product_count: number;
  target_shop_count: number;
};

type PriceUpdatePrecheckBatchResult = {
  processed_jobs: number;
  processed_items: number;
  ready_items: number;
  failed_items: number;
};

type PriceUpdateSubmitBatchResult = {
  processed_jobs: number;
  processed_items: number;
  submitted_items: number;
  failed_items: number;
};

type PriceUpdateConfirmBatchResult = {
  processed_jobs: number;
  processed_items: number;
  confirmed_items: number;
  pending_items: number;
  failed_items: number;
};

type OrderSyncBatchResult = {
  task_id: string;
  processed_shops: number;
  synced_orders: number;
  failed_shops: number;
};

type OrderDetailSyncBatchResult = {
  task_id: string;
  processed_orders: number;
  synced_orders: number;
  created_items: number;
  failed_orders: number;
};

type AftersaleSyncBatchResult = {
  task_id: string;
  processed_shops: number;
  synced_aftersales: number;
  failed_shops: number;
  failed_aftersales: number;
};

type AftersaleView = {
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

type AftersaleListResult = {
  items: AftersaleView[];
  total: number;
};

type AftersaleResponsibilityResult = {
  aftersale_id: string;
  responsibility_party: string;
  supplier_compensation_cents: number;
  profit_adjustment_id: string | null;
  message: string;
};

type AftersaleActionResult = {
  aftersale_id: string;
  wechat_aftersale_id: string;
  action: string;
  status: string;
  errcode: number | null;
  errmsg: string | null;
  message: string;
};

type AftersaleRejectReasonView = {
  shop_id: string;
  reject_reason_type: number;
  reject_reason_type_text: string;
  reject_reason: string;
  reject_scene: number | null;
  reject_scene_text: string;
  synced_at: string;
};

type AftersaleRejectReasonSyncResult = {
  task_id: string;
  shop_id: string;
  synced_reasons: number;
  failed_steps: string[];
};

type GuaranteeSyncBatchResult = {
  task_id: string;
  processed_shops: number;
  synced_guarantees: number;
  failed_shops: number;
  failed_guarantees: number;
};

type GuaranteeOrderView = {
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

type GuaranteeOrderListResult = {
  items: GuaranteeOrderView[];
  total: number;
};

type GuaranteeFollowupResult = {
  guarantee_order_id: string;
  handling_status: string;
  responsibility_party: string | null;
  supplier_compensation_cents: number;
  profit_adjustment_id: string | null;
  message: string;
};

type AftersaleEvidenceView = {
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

type AftersaleEvidenceListResult = {
  items: AftersaleEvidenceView[];
  total: number;
};

type AftersaleEvidenceRecordResult = {
  evidence_id: string;
  target_type: string;
  target_id: string;
  evidence_type: string;
  status: string;
  message: string;
};

type AftersaleEvidenceStatusUpdateResult = {
  evidence_id: string;
  status: string;
  status_text: string;
  message: string;
};

type AftersaleEvidenceExportResult = {
  file_path: string;
  exported_count: number;
  format: string;
  sensitive_fields: string;
};

type SupplierAftersaleFollowupView = {
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

type SupplierAftersaleFollowupListResult = {
  items: SupplierAftersaleFollowupView[];
  total: number;
};

type SupplierAftersaleFollowupRecordResult = {
  followup_id: string;
  target_type: string;
  target_id: string;
  status: string;
  message: string;
};

type PurchaseTaskBatchResult = {
  task_id: string;
  processed_items: number;
  created_tasks: number;
  skipped_items: number;
};

type PurchaseTaskView = {
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

type PurchaseTaskListResult = {
  items: PurchaseTaskView[];
  total: number;
};

type PurchaseTaskExportResult = {
  file_path: string;
  exported_count: number;
};

type SupplierAgentExportResult = {
  file_path: string;
  exported_count: number;
  format: string;
  sensitive_fields: string;
};

type SupplierAgentApplyItemResult = {
  index: number;
  purchase_task_id: string | null;
  action: string | null;
  status: string;
  error: string | null;
  response: unknown | null;
};

type SupplierAgentApplyResult = {
  dry_run: boolean;
  processed: number;
  succeeded: number;
  failed: number;
  results: SupplierAgentApplyItemResult[];
};

type PurchaseTaskMappingResult = {
  purchase_task_id: string;
  order_id: string;
  status: string;
  external_product_id: string;
  external_sku_id: string;
  message: string;
};

type PurchaseTaskIssueResult = {
  purchase_task_id: string;
  order_id: string;
  status: string;
  message: string;
};

type PurchaseTaskShipmentResult = {
  purchase_task_id: string;
  order_id: string;
  purchase_status: string;
  shipment_id: string | null;
  shipment_status: string | null;
  auto_send_enabled: boolean;
  message: string;
};

type OrderProfitView = {
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

type OrderProfitTotals = {
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

type OrderProfitListResult = {
  items: OrderProfitView[];
  total: number;
  totals: OrderProfitTotals;
};

type InventoryRiskView = {
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

type InventoryRiskListResult = {
  items: InventoryRiskView[];
  total: number;
  low_stock_count: number;
  out_of_stock_count: number;
  issue_count: number;
};

type InventoryRiskScanResult = {
  task_id: string;
  scanned_products: number;
  low_stock_products: number;
  out_of_stock_products: number;
  issue_products: number;
  notifications_created: number;
};

type ProductSalesAnalysisView = {
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

type ProductSalesAnalysisTotals = {
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

type ProductSalesAnalysisListResult = {
  items: ProductSalesAnalysisView[];
  total: number;
  totals: ProductSalesAnalysisTotals;
};

type OrderProfitAdjustmentResult = {
  adjustment_id: string;
  order_id: string;
  kind: string;
  amount_cents: number;
  message: string;
};

type DeliverySettings = {
  auto_send_delivery: boolean;
};

type DeliveryCompanyView = {
  shop_id: string;
  delivery_id: string;
  delivery_name: string;
  synced_at: string;
};

type DeliveryCompanySyncResult = {
  task_id: string;
  shop_id: string;
  synced_companies: number;
  failed_steps: string[];
};

type LocalApiConfig = {
  enabled: boolean;
  host: string;
  port: number;
  base_url: string;
  has_api_key: boolean;
  api_key_hint: string | null;
  auth_header: string;
};

type LocalApiKeyRotationResult = {
  api_key: string;
  key_hint: string;
  base_url: string;
  warning: string;
};

type AiProviderSettings = {
  enabled: boolean;
  provider_type: string;
  base_url: string;
  model: string;
  temperature: number;
  has_api_key: boolean;
  api_key_hint: string | null;
  updated_at: string | null;
};

type AiProviderTestResult = {
  status: string;
  provider_type: string;
  model: string;
  message: string;
};

type ExternalApiLogView = {
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

type NotificationView = {
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

type NotificationListResult = {
  items: NotificationView[];
  total: number;
  unread_count: number;
  critical_count: number;
};

type NotificationMarkResult = {
  updated_count: number;
  message: string;
};

type BackupInfo = {
  id: string;
  file_name: string;
  file_path: string;
  size_bytes: number;
  sha256: string;
  created_at: string;
  integrity_ok: boolean;
  integrity_message: string;
};

type BackupCreateResult = {
  backup: BackupInfo;
};

type BackupRestoreResult = {
  restored_from: BackupInfo;
  rollback_backup: BackupInfo;
  integrity_ok: boolean;
  message: string;
};

type ShipmentRecordResult = {
  shipment_id: string;
  order_id: string;
  status: string;
  auto_send_enabled: boolean;
  message: string;
};

type ShipmentView = {
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

type ShipmentListResult = {
  items: ShipmentView[];
  total: number;
};

type ShipmentRetryResult = {
  shipment_id: string;
  status: string;
  auto_send_enabled: boolean;
  message: string;
};

type DeliverySubmitBatchResult = {
  task_id: string;
  processed_shipments: number;
  submitted_shipments: number;
  failed_shipments: number;
};

type OperationalAutomationSettings = {
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

type AutomationStepError = {
  step: string;
  error: string;
};

type OperationalAutomationRunResult = {
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

type PublishJobView = {
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

type PublishJobProductView = PublishJobView["products"][number];
type PublishJobItemRow = PublishJobProductView["items"][number];

type PriceUpdateJobView = {
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

const defaultAutomationSettings = (): OperationalAutomationSettings => ({
  order_sync_enabled: true,
  order_detail_sync_enabled: true,
  aftersale_sync_enabled: true,
  purchase_task_enabled: true,
  delivery_submission_enabled: true,
  publish_precheck_enabled: true,
  publish_attribute_fill_enabled: true,
  publish_category_precheck_enabled: true,
  publish_asset_upload_enabled: true,
  publish_submit_enabled: true,
  publish_status_sync_enabled: true,
  publish_listing_enabled: true,
  price_confirm_enabled: true,
});

const dashboard = ref<DashboardSummary | null>(null);
const groups = ref<ShopGroup[]>([]);
const shops = ref<ShopListItem[]>([]);
const selectedSection = ref("overview");
const loading = ref(false);
const backupRunning = ref(false);
const latestTaskId = ref("");
const queriedTaskId = ref("");
const currentJob = ref<PublishJobView | null>(null);
const latestPriceTaskId = ref("");
const queriedPriceTaskId = ref("");
const currentPriceJob = ref<PriceUpdateJobView | null>(null);
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
const automationSettings = ref<OperationalAutomationSettings>(defaultAutomationSettings());
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
const aiProviderSettings = ref<AiProviderSettings>({
  enabled: false,
  provider_type: "openai_compatible",
  base_url: "",
  model: "",
  temperature: 0.1,
  has_api_key: false,
  api_key_hint: null,
  updated_at: null,
});
const aiProviderForm = reactive({
  enabled: false,
  provider_type: "openai_compatible",
  base_url: "https://api.openai.com/v1",
  model: "",
  temperature: "0.1",
  api_key: "",
  clear_api_key: false,
});
const aiProviderSaving = ref(false);
const aiProviderTesting = ref(false);
const attributeSuggestions = ref<PublishAttributeSuggestionView[]>([]);
const attributeSuggestionTotal = ref(0);
const pendingAttributeSuggestionCount = ref(0);
const appliedAttributeSuggestionCount = ref(0);
const attributeSuggestionStatusFilter = ref("pending");
const selectedAttributeSuggestions = ref<PublishAttributeSuggestionView[]>([]);
const attributeSuggestionApplying = ref(false);
const jobAttributeSuggestions = ref<PublishAttributeSuggestionView[]>([]);
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
const freightTemplates = ref<FreightTemplateView[]>([]);
const selectedCategoryShopId = ref("shop-preview");
const categoryKeyword = ref("");
const categoryRuleCatId = ref("");
const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const runtimeLabel = computed(() => isTauriRuntime ? "Tauri 主控机" : "浏览器预览");
const selectedCategoryShop = computed(() => shops.value.find((shop) => shop.id === selectedCategoryShopId.value) || null);
const previewGroups = ref<ShopGroup[]>([
  {
    id: "group-default",
    name: "默认店铺组",
    status: "active",
    shop_count: 1,
    created_at: "2026-05-22T00:00:00+08:00",
  },
]);
const previewShops = ref<ShopListItem[]>([
  {
    id: "shop-preview",
    name: "预览店铺",
    appid: "wx-preview",
    status: "not_verified",
    group_id: "group-default",
    group_name: "默认店铺组",
    has_secret: false,
    token_expires_at: null,
    wechat_nickname: null,
    wechat_status: null,
    last_health_check_at: null,
    last_quota_remain: null,
    created_at: "2026-05-22T00:00:00+08:00",
  },
]);
const previewTaskRuns = ref<TaskRunView[]>([]);
const previewShipments = ref<ShipmentView[]>([
  {
    id: "shipment-preview-failed",
    order_id: "order-preview-failed",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    wechat_order_id: "420000000099",
    delivery_id: "SF",
    delivery_name: "顺丰",
    waybill_id: "SF0000000001",
    deliver_type: 1,
    status: "send_failed",
    error_code: "DELIVERY_PAYLOAD_INVALID",
    error_summary: "订单项缺少微信 product_id 或 sku_id，无法提交发货",
    submitted_at: null,
    created_at: "2026-05-22T00:09:00+08:00",
    updated_at: "2026-05-22T00:09:20+08:00",
  },
]);
const previewPurchaseTasks = ref<PurchaseTaskView[]>([
  {
    id: "purchase-preview-1",
    order_id: "order-preview-1",
    wechat_order_id: "420000000001",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    status: "pending_purchase",
    external_product_id: "demo-1688-10001",
    external_sku_id: "black-m",
    source_url: "https://detail.1688.com/offer/10001.html",
    title: "夏季薄款防晒衣女",
    quantity: 1,
    sale_price: 2990,
    real_price: 2990,
    estimated_revenue: 2990,
    estimated_cost: null,
    estimated_profit: null,
    supplier_name: "1688 示例供货商",
    supplier_product_id: "ali-demo-10001",
    supplier_delivery_id: null,
    supplier_delivery_name: null,
    supplier_waybill_id: null,
    supplier_deliver_type: null,
    supplier_shipped_at: null,
    error_summary: null,
    created_at: "2026-05-22T00:08:00+08:00",
    updated_at: "2026-05-22T00:08:00+08:00",
  },
  {
    id: "purchase-preview-2",
    order_id: "order-preview-2",
    wechat_order_id: "420000000002",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    status: "needs_mapping",
    external_product_id: null,
    external_sku_id: null,
    source_url: null,
    title: "待映射商品",
    quantity: 1,
    sale_price: 1990,
    real_price: 1990,
    estimated_revenue: 1990,
    estimated_cost: null,
    estimated_profit: null,
    supplier_name: null,
    supplier_product_id: null,
    supplier_delivery_id: null,
    supplier_delivery_name: null,
    supplier_waybill_id: null,
    supplier_deliver_type: null,
    supplier_shipped_at: null,
    error_summary: "缺少外部商品映射",
    created_at: "2026-05-22T00:08:00+08:00",
    updated_at: "2026-05-22T00:08:00+08:00",
  },
]);
const previewAftersales = ref<AftersaleView[]>([
  {
    id: "aftersale-preview-1",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    order_id: "order-preview-1",
    wechat_order_id: "420000000001",
    wechat_aftersale_id: "after-sale-420000000001",
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
  },
  {
    id: "aftersale-preview-failed",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    order_id: null,
    wechat_order_id: "420000000088",
    wechat_aftersale_id: "after-sale-failed",
    status: "sync_failed",
    aftersale_type: null,
    reason: "微信售后详情失败：订单不存在或权限不足",
    refund_amount_cents: null,
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
    synced_at: "2026-05-22T00:18:10+08:00",
    updated_at: "2026-05-22T00:18:10+08:00",
  },
]);
const previewAftersaleRejectReasons = ref<AftersaleRejectReasonView[]>([
  {
    shop_id: "shop-preview",
    reject_reason_type: 6,
    reject_reason_type_text: "买家误操作/已协商取消申请",
    reject_reason: "已与买家沟通确认，本次售后先取消处理。",
    reject_scene: 1,
    reject_scene_text: "拒绝仅退款",
    synced_at: "2026-05-22T00:24:00+08:00",
  },
  {
    shop_id: "shop-preview",
    reject_reason_type: 4,
    reject_reason_type_text: "包裹在买家发起售后时已完成发货/揽收",
    reject_reason: "商品已完成发货或揽收，建议买家收到货后按实际情况重新发起售后。",
    reject_scene: 1,
    reject_scene_text: "拒绝仅退款",
    synced_at: "2026-05-22T00:24:00+08:00",
  },
  {
    shop_id: "shop-preview",
    reject_reason_type: 1,
    reject_reason_type_text: "已在约定时间发货且物流运输正常",
    reject_reason: "本店已在约定时间内完成发货，目前物流正常转运。",
    reject_scene: 1,
    reject_scene_text: "拒绝仅退款",
    synced_at: "2026-05-22T00:24:00+08:00",
  },
]);
const previewGuaranteeOrders = ref<GuaranteeOrderView[]>([
  {
    id: "guarantee-preview-1",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    order_id: "order-preview-1",
    wechat_order_id: "420000000001",
    guarantee_order_id: "2000001077270153",
    guarantee_type: 2,
    guarantee_type_text: "坏损包退",
    status: "STATUS_WAIT_MERCHANT_PROOF",
    status_text: "等待商家举证",
    apply_reason: "买家反馈商品破损，需要商家补充凭证",
    pay_amount_cents: 2990,
    merchant_refuse_reason: null,
    handling_status: "pending",
    handling_note: "等待供应商提供发货前质检截图",
    responsibility_party: "unknown",
    supplier_compensation_cents: 0,
    handled_at: null,
    created_time: 1779428400,
    updated_time_unix: 1779429000,
    expire_time: 1779514800,
    complete_time: null,
    evidence_count: 1,
    synced_at: "2026-05-22T00:25:00+08:00",
    updated_at: "2026-05-22T00:25:00+08:00",
  },
  {
    id: "guarantee-preview-closed",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    order_id: null,
    wechat_order_id: "420000000099",
    guarantee_order_id: "2000001077270999",
    guarantee_type: 1,
    guarantee_type_text: "假一赔三/四",
    status: "STATUS_USER_CANCEL",
    status_text: "用户取消申请",
    apply_reason: "用户取消纠纷申请",
    pay_amount_cents: 0,
    merchant_refuse_reason: null,
    handling_status: "ignored",
    handling_note: "买家已取消，无需继续处理",
    responsibility_party: "customer",
    supplier_compensation_cents: 0,
    handled_at: "2026-05-22T00:30:00+08:00",
    created_time: 1779342000,
    updated_time_unix: 1779345600,
    expire_time: null,
    complete_time: 1779345600,
    evidence_count: 0,
    synced_at: "2026-05-22T00:25:00+08:00",
    updated_at: "2026-05-22T00:25:00+08:00",
  },
]);
const previewAftersaleEvidence = ref<AftersaleEvidenceView[]>([
  {
    id: "evidence-preview-1",
    target_type: "guarantee",
    target_id: "guarantee-preview-1",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    external_target_id: "2000001077270153",
    evidence_type: "supplier_proof",
    evidence_type_text: "供应商凭证",
    title: "供应商发货前质检截图",
    content_text: "已向供应商索要包装破损前后的质检截图，待人工核对后再决定是否提交平台。",
    local_file_path: "/Users/wangjunhao/Pictures/wx-xd-demo-proof.jpg",
    source_url: null,
    status: "draft",
    status_text: "草稿",
    created_at: "2026-05-22T00:34:00+08:00",
    updated_at: "2026-05-22T00:34:00+08:00",
  },
]);
const previewSupplierAftersaleFollowups = ref<SupplierAftersaleFollowupView[]>([
  {
    id: "supplier-followup-preview-1",
    target_type: "guarantee",
    target_id: "guarantee-preview-1",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    external_target_id: "2000001077270153",
    purchase_task_id: "purchase-preview-1",
    supplier_name: "示例供应商",
    followup_type: "evidence_request",
    followup_type_text: "索要凭证",
    status: "waiting_supplier",
    status_text: "等供应商",
    note: "已向供应商索要破损前质检截图和打包视频，只记录脱敏沟通摘要。",
    created_at: "2026-05-22T00:35:00+08:00",
    updated_at: "2026-05-22T00:35:00+08:00",
  },
]);
const previewProfitAdjustments = ref<Array<{
  order_id: string;
  kind: string;
  amount_cents: number;
}>>([
  { order_id: "order-preview-1", kind: "purchase_freight", amount_cents: 600 },
]);
const previewPendingOrderCount = ref(0);
const previewLastOrderSyncAt = ref<string | null>(null);
const previewDatabaseBackups = ref<BackupInfo[]>([]);
const previewExternalApiLogs = ref<ExternalApiLogView[]>([
  {
    id: "external-api-log-preview",
    method: "POST",
    path: "/api/publish-jobs",
    status: "success",
    status_code: 200,
    error_code: null,
    request_summary: "request_id=req-preview products=1 target_groups=1 target_shops=0",
    response_summary: "ok",
    duration_ms: 38,
    created_at: "2026-05-22T00:17:00+08:00",
  },
]);
const previewAttributeSuggestions = ref<PublishAttributeSuggestionView[]>([
  {
    id: "attr-suggestion-preview-1",
    item_id: "item-preview",
    job_id: "pub_preview_attr",
    product_row_id: "product-preview-1",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    external_product_id: "demo-1688-10001",
    title: "夏季薄款防晒衣女",
    attr_kind: "product",
    attr_key: "材质",
    suggested_value: "聚酯纤维",
    sku_values: [],
    confidence: 82,
    source: "ai_provider:gpt-4.1-mini",
    applied: false,
    allowed_values: ["聚酯纤维", "棉", "锦纶"],
    reason: "标题和货源规格中多次出现防晒衣常见面料，建议人工确认后采纳。",
    updated_at: "2026-05-22T00:27:00+08:00",
  },
  {
    id: "attr-suggestion-preview-2",
    item_id: "item-preview",
    job_id: "pub_preview_attr",
    product_row_id: "product-preview-1",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    external_product_id: "demo-1688-10001",
    title: "夏季薄款防晒衣女",
    attr_kind: "sale",
    attr_key: "颜色",
    suggested_value: "按 SKU 映射：黑色",
    sku_values: [{ sku_index: 0, value: "黑色" }],
    confidence: 92,
    source: "sku_attr_synonym",
    applied: true,
    allowed_values: ["黑色", "白色", "灰色"],
    reason: "已从 SKU 规格同义词自动确认。",
    updated_at: "2026-05-22T00:26:00+08:00",
  },
]);
const previewNotifications = ref<NotificationView[]>([
  {
    id: "notification-preview-publish",
    severity: "critical",
    source_type: "publish_item",
    source_id: "item-preview",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    title: "铺货任务失败",
    body: "店铺 预览店铺 铺货外部商品 demo-1688-10001 失败：缺少商品必填属性：材质（CATEGORY_ATTRS_NEED_AI_FILL）",
    status: "unread",
    data_json: "{\"external_product_id\":\"demo-1688-10001\"}",
    read_at: null,
    created_at: "2026-05-22T00:24:00+08:00",
    updated_at: "2026-05-22T00:24:00+08:00",
  },
  {
    id: "notification-preview-mapping",
    severity: "warning",
    source_type: "purchase_mapping",
    source_id: "order-item-preview-2",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    title: "采购任务缺少外部商品映射",
    body: "订单 order-preview-2 的订单项缺少外部商品 ID 或外部 SKU，需人工补齐映射后再继续采购。",
    status: "unread",
    data_json: "{\"order_id\":\"order-preview-2\"}",
    read_at: null,
    created_at: "2026-05-22T00:23:00+08:00",
    updated_at: "2026-05-22T00:23:00+08:00",
  },
  {
    id: "notification-preview-aftersale",
    severity: "warning",
    source_type: "aftersale",
    source_id: "after-sale-420000000001",
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    title: "微信售后待处理",
    body: "售后单 after-sale-420000000001 当前状态为 MERCHANT_PROCESSING，需人工判断是否同意、拒绝或补充凭证。",
    status: "read",
    data_json: "{\"wechat_order_id\":\"420000000001\"}",
    read_at: "2026-05-22T00:25:00+08:00",
    created_at: "2026-05-22T00:18:00+08:00",
    updated_at: "2026-05-22T00:25:00+08:00",
  },
]);
const previewCategoryCatalogShops = ref<CategoryCatalogShopSummary[]>([
  {
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    category_count: 3,
    detail_count: 1,
    product_rule_count: 1,
    delivery_rule_count: 1,
    freight_template_count: 1,
    last_category_sync_at: "2026-05-22T00:20:00+08:00",
    last_rule_sync_at: "2026-05-22T00:21:00+08:00",
    last_freight_sync_at: "2026-05-22T00:20:00+08:00",
  },
]);
const previewCategoryCache = ref<CategoryCacheView[]>([
  {
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    cat_id: 1000102,
    parent_cat_id: 1000101,
    level: 3,
    name: "防晒衣",
    product_attr_count: 6,
    sale_attr_count: 2,
    product_qua_count: 0,
    has_detail: true,
    has_product_rule: true,
    has_delivery_rule: true,
    synced_at: "2026-05-22T00:20:00+08:00",
    detail_synced_at: "2026-05-22T00:21:00+08:00",
  },
  {
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    cat_id: 1000101,
    parent_cat_id: 1000001,
    level: 2,
    name: "女装",
    product_attr_count: 0,
    sale_attr_count: 0,
    product_qua_count: 0,
    has_detail: false,
    has_product_rule: false,
    has_delivery_rule: false,
    synced_at: "2026-05-22T00:20:00+08:00",
    detail_synced_at: null,
  },
]);
const previewFreightTemplates = ref<FreightTemplateView[]>([
  {
    shop_id: "shop-preview",
    shop_name: "预览店铺",
    template_id: "template-preview-1001",
    synced_at: "2026-05-22T00:20:00+08:00",
  },
]);

const groupForm = reactive({ name: "" });
const shopForm = reactive({ name: "", appid: "", app_secret: "", group_id: "group-default" });
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

const fallbackDeliveryCompanyOptions = [
  { value: "SF", label: "顺丰" },
  { value: "ZTO", label: "中通" },
  { value: "YTO", label: "圆通" },
  { value: "YUNDA", label: "韵达" },
  { value: "JTSD", label: "极兔" },
  { value: "STO", label: "申通" },
  { value: "JD", label: "京东" },
  { value: "EMS", label: "中国邮政" },
  { value: "OTHER", label: "其他" },
];

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

const purchaseIssueTypeOptions = [
  { value: "out_of_stock", label: "供应商缺货" },
  { value: "price_changed", label: "供应商涨价" },
  { value: "supplier_cancelled", label: "供应商取消" },
  { value: "quality_risk", label: "质量风险" },
  { value: "other", label: "其他异常" },
];

const inventoryRiskStatusOptions = [
  { value: "all", label: "全部状态" },
  { value: "out_of_stock", label: "断货" },
  { value: "supplier_issue", label: "供应商异常" },
  { value: "stock_pressure", label: "库存压力" },
  { value: "low_stock", label: "低库存" },
  { value: "not_listed", label: "未铺货" },
  { value: "healthy", label: "正常" },
];

const productSalesStatusOptions = [
  { value: "all", label: "全部状态" },
  { value: "scale_candidate", label: "可放量" },
  { value: "stock_risk", label: "库存风险" },
  { value: "margin_risk", label: "毛利风险" },
  { value: "aftersale_watch", label: "售后观察" },
  { value: "no_sales", label: "未动销" },
  { value: "not_listed", label: "未铺货" },
  { value: "steady", label: "稳定观察" },
  { value: "observe", label: "数据不足" },
];

const profitAdjustmentKindOptions = [
  { value: "purchase_freight", label: "采购运费" },
  { value: "refund", label: "退款" },
  { value: "aftersale_compensation", label: "售后赔付" },
  { value: "other_cost", label: "其他成本" },
  { value: "other_income", label: "其他收入" },
];

const aftersaleResponsibilityOptions = [
  { value: "supplier", label: "供应商责任" },
  { value: "merchant", label: "本店责任" },
  { value: "customer", label: "买家原因" },
  { value: "platform", label: "平台原因" },
  { value: "unknown", label: "待确认" },
];

const guaranteeHandlingStatusOptions = [
  { value: "pending", label: "待跟进" },
  { value: "in_progress", label: "跟进中" },
  { value: "waiting_supplier", label: "等供应商" },
  { value: "evidence_ready", label: "凭证已整理" },
  { value: "resolved", label: "已处理" },
  { value: "ignored", label: "无需处理" },
];

const evidenceTargetTypeOptions = [
  { value: "aftersale", label: "售后单" },
  { value: "guarantee", label: "纠纷单" },
];

const evidenceTypeOptions = [
  { value: "image", label: "图片" },
  { value: "text", label: "文字说明" },
  { value: "chat_record", label: "沟通记录" },
  { value: "logistics", label: "物流凭证" },
  { value: "supplier_proof", label: "供应商凭证" },
  { value: "quality_check", label: "质检凭证" },
  { value: "other", label: "其他" },
];

const evidenceStatusOptions = [
  { value: "draft", label: "草稿" },
  { value: "ready", label: "已整理" },
  { value: "used", label: "已使用" },
  { value: "archived", label: "已归档" },
];

const supplierFollowupTypeOptions = [
  { value: "contact", label: "联系供应商" },
  { value: "evidence_request", label: "索要凭证" },
  { value: "evidence_received", label: "收到凭证" },
  { value: "compensation", label: "赔付沟通" },
  { value: "return_refund", label: "退货退款" },
  { value: "other", label: "其他协同" },
];

const supplierFollowupStatusOptions = [
  { value: "pending", label: "待处理" },
  { value: "contacted", label: "已联系" },
  { value: "waiting_supplier", label: "等供应商" },
  { value: "evidence_ready", label: "凭证已备" },
  { value: "compensation_pending", label: "赔付待确认" },
  { value: "closed", label: "已关闭" },
];

const aftersaleTerminalStatuses = [
  "MERCHANT_REFUND_SUCCESS",
  "MERCHANT_RETURN_SUCCESS",
  "USER_CANCELD",
  "USER_CANCELLED",
  "RETURN_CLOSED",
  "sync_failed",
];

const aftersaleRejectReasonOptions = computed(() => {
  const shopId = aftersaleActionForm.shop_id.trim();
  const reasons = shopId
    ? aftersaleRejectReasons.value.filter((reason) => reason.shop_id === shopId)
    : aftersaleRejectReasons.value;
  return reasons.length > 0 ? reasons : aftersaleRejectReasons.value;
});

const publishPayload = ref(JSON.stringify({
  request_id: `req-${Date.now()}`,
  target_shop_group_ids: ["group-default"],
  products: [
    {
      external_product_id: "demo-1688-10001",
      title: "夏季薄款防晒衣女",
      source_url: "https://example.com/products/10001",
      images: [
        "https://example.com/images/1.jpg",
        "https://example.com/images/2.jpg",
        "https://example.com/images/3.jpg"
      ],
      detail_images: ["https://example.com/detail/1.jpg"],
      supplier_name: "示例供应商",
      supplier_product_id: "10001",
      category_hint: "女装/防晒衣",
      brand_hint: "无品牌",
      weight_gram: 500,
      skus: [
        {
          external_sku_id: "black-m",
          specs: { "颜色": "黑色", "尺码": "M" },
          cost_price: 12.5,
          stock: 100
        }
      ],
      metadata: {
        wechat_category_ids: [1000001, 1000101, 1000102],
        wechat_attrs: [
          { attr_key: "材质", attr_value: "聚酯纤维" }
        ],
        freight_template_id: "replace-with-shop-freight-template-id",
        extra_service: {
          seven_day_return: 1,
          freight_insurance: 0
        },
        sale_price_markup_rate: 1.8,
        sale_price_fixed_cents: 500
      }
    }
  ]
}, null, 2));

const priceUpdatePayload = ref(JSON.stringify({
  request_id: `price-${Date.now()}`,
  target_shop_group_ids: ["group-default"],
  products: [
    {
      external_product_id: "demo-1688-10001",
      target_price_cents: 3290,
      reason: "测试批量改价"
    }
  ]
}, null, 2));

const statusTone: Record<string, string> = {
  active: "success",
  not_verified: "warning",
  missing_secret: "warning",
  auth_failed: "danger",
  api_failed: "danger",
  queued: "info",
  pending: "info",
  running: "primary",
  prechecking: "primary",
  category_prechecking: "primary",
  asset_uploading: "primary",
  listing: "primary",
  audit_pending: "primary",
  audit_passed: "success",
  assets_ready: "success",
  publishing: "primary",
  submitted: "primary",
  ready_to_publish: "success",
  category_prechecked: "success",
  ready_to_update: "success",
  success: "success",
  failed: "danger",
  partial_success: "warning",
  pending_purchase: "warning",
  needs_mapping: "warning",
  supplier_shipped: "warning",
  supplier_out_of_stock: "danger",
  supplier_price_changed: "warning",
  supplier_cancelled: "danger",
  supplier_quality_risk: "danger",
  supplier_exception: "danger",
  waiting_confirmation: "warning",
  ready_to_send: "primary",
  wechat_shipped: "success",
  send_failed: "danger",
  aftersale_active: "danger",
  MERCHANT_PROCESSING: "warning",
  MERCHANT_REFUND_SUCCESS: "success",
  MERCHANT_RETURN_SUCCESS: "success",
  MERCHANT_FAIL: "danger",
  USER_CANCELD: "info",
  USER_CANCELLED: "info",
  RETURN_CLOSED: "info",
  draft: "info",
  ready: "success",
  used: "primary",
  archived: "info",
  contacted: "primary",
  waiting_supplier: "warning",
  evidence_ready: "success",
  compensation_pending: "warning",
  closed: "info",
  STATUS_WAIT_MERCHANT_HANDLE: "danger",
  STATUS_WAIT_MERCHANT_PROOF: "danger",
  STATUS_WAIT_BOTH_PROOF: "danger",
  STATUS_WAIT_PLATFORM_HANDLE: "warning",
  STATUS_WAIT_USER_CONFIRM: "warning",
  STATUS_PAY_BLOCK: "danger",
  STATUS_PAY_FAIL: "danger",
  STATUS_PAYING: "primary",
  STATUS_PAY_SUCC: "success",
  STATUS_NO_NEED_PAY: "success",
  STATUS_USER_CANCEL: "info",
  sync_failed: "danger",
  exception: "danger",
  missing_purchase_task: "warning",
  missing_cost: "warning",
  loss: "danger",
  profitable: "success",
  out_of_stock: "danger",
  supplier_issue: "danger",
  stock_pressure: "warning",
  low_stock: "warning",
  not_listed: "info",
  healthy: "success",
  scale_candidate: "success",
  stock_risk: "danger",
  margin_risk: "danger",
  aftersale_watch: "warning",
  no_sales: "warning",
  steady: "success",
  observe: "info",
  unread: "warning",
  read: "info",
};

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime) {
    return invoke<T>(name, args);
  }
  return previewCommand<T>(name, args);
}

async function previewCommand<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (name === "get_dashboard") {
    return {
      pending_order_count: previewPendingOrderCount.value,
      abnormal_shop_count: 1,
      failed_publish_product_count: currentJob.value?.products.filter((product) => product.failed_count > 0).length ?? 0,
      unread_notification_count: previewNotifications.value.filter((item) => item.status === "unread").length,
      running_task_count: previewTaskRuns.value.filter((task) => ["pending", "queued", "running"].includes(task.status)).length,
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
    const request = args?.request as { name?: string; appid?: string; app_secret?: string; group_id?: string } | undefined;
    const group = previewGroups.value.find((item) => item.id === request?.group_id);
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
      shop.token_expires_at = shop.has_secret ? "2026-05-22T02:00:00+08:00" : null;
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
    const request = args?.request as { shop_id?: string; cgi_path?: string } | undefined;
    const shop = previewShops.value.find((item) => item.id === request?.shop_id);
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
    const categories = previewCategoryCache.value.filter((item) => {
      const shopMatched = !shopId || item.shop_id === shopId;
      const keywordMatched = !keyword || item.name.includes(keyword) || String(item.cat_id).includes(keyword);
      return shopMatched && keywordMatched;
    });
    const freightTemplates = previewFreightTemplates.value.filter((item) => !shopId || item.shop_id === shopId);
    return {
      shops: previewCategoryCatalogShops.value,
      categories,
      freight_templates: freightTemplates,
    } as T;
  }
  if (name === "sync_shop_category_catalog") {
    const shopId = String(args?.shopId || "shop-preview");
    const shop = previewShops.value.find((item) => item.id === shopId);
    const summary = previewCategoryCatalogShops.value.find((item) => item.shop_id === shopId);
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
      synced_categories: previewCategoryCache.value.filter((item) => item.shop_id === shopId).length,
      synced_freight_templates: previewFreightTemplates.value.filter((item) => item.shop_id === shopId).length,
      failed_steps: [],
    } as T;
  }
  if (name === "sync_category_rules") {
    const shopId = String(args?.shopId || "shop-preview");
    const catId = Number(args?.catId || 1000102);
    const category = previewCategoryCache.value.find((item) => item.shop_id === shopId && item.cat_id === catId);
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
  if (name === "create_external_publish_job") {
    const taskId = `pub_preview_${Date.now()}`;
    latestTaskId.value = taskId;
    currentJob.value = buildPreviewJob(taskId);
    previewAttributeSuggestions.value.forEach((suggestion) => {
      suggestion.job_id = taskId;
    });
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
    return (currentJob.value || buildPreviewJob(String(args?.taskId || "pub_preview"))) as T;
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
    return (currentPriceJob.value || buildPreviewPriceJob(String(args?.taskId || "price_preview"))) as T;
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
    const request = args?.request as {
      enabled?: boolean;
      provider_type?: string;
      base_url?: string;
      model?: string;
      temperature?: number;
      api_key?: string;
      clear_api_key?: boolean;
    } | undefined;
    aiProviderSettings.value = {
      enabled: Boolean(request?.enabled),
      provider_type: request?.provider_type || "openai_compatible",
      base_url: request?.base_url || "",
      model: request?.model || "",
      temperature: request?.temperature ?? 0.1,
      has_api_key: Boolean(request?.api_key) || (aiProviderSettings.value.has_api_key && !request?.clear_api_key),
      api_key_hint: request?.api_key ? `指纹 ${request.api_key.slice(-8)}` : (request?.clear_api_key ? null : aiProviderSettings.value.api_key_hint),
      updated_at: "2026-05-22T00:26:00+08:00",
    };
    return aiProviderSettings.value as T;
  }
  if (name === "test_ai_provider") {
    if (!aiProviderSettings.value.enabled || !aiProviderSettings.value.has_api_key) {
      throw new Error("AI provider 未启用或缺少 API Key");
    }
    return {
      status: "success",
      provider_type: aiProviderSettings.value.provider_type,
      model: aiProviderSettings.value.model,
      message: "AI provider 连通成功：预览模式",
    } as T;
  }
  if (name === "list_publish_attribute_suggestions") {
    const status = String(args?.status || "pending");
    const jobId = String(args?.jobId || "");
    const itemId = String(args?.itemId || "");
    const items = previewAttributeSuggestions.value.filter((item) => {
      if (status === "all") {
        return (!jobId || item.job_id === jobId) && (!itemId || item.item_id === itemId);
      }
      if (status === "applied") {
        return item.applied && (!jobId || item.job_id === jobId) && (!itemId || item.item_id === itemId);
      }
      return !item.applied && (!jobId || item.job_id === jobId) && (!itemId || item.item_id === itemId);
    });
    const scoped = previewAttributeSuggestions.value.filter((item) => {
      return (!jobId || item.job_id === jobId) && (!itemId || item.item_id === itemId);
    });
    return {
      items,
      total: items.length,
      pending_count: scoped.filter((item) => !item.applied).length,
      applied_count: scoped.filter((item) => item.applied).length,
    } as T;
  }
  if (name === "apply_publish_attribute_suggestions") {
    const request = args?.request as { suggestion_ids?: string[] } | undefined;
    const ids = new Set(request?.suggestion_ids || []);
    let applied = 0;
    previewAttributeSuggestions.value.forEach((suggestion) => {
      if (ids.has(suggestion.id) && !suggestion.applied) {
        suggestion.applied = true;
        suggestion.updated_at = "2026-05-22T00:28:00+08:00";
        applied += 1;
      }
    });
    if (currentJob.value) {
      currentJob.value.products.forEach((product) => {
        product.items.forEach((item) => {
          if (item.id === "item-preview") {
            item.status = "ready_to_publish";
            item.error_code = null;
            item.error_summary = "已人工采纳属性建议，等待重新执行微信类目预检";
          }
        });
      });
    }
    return {
      processed_suggestions: ids.size,
      processed_items: applied > 0 ? 1 : 0,
      applied_suggestions: applied,
      updated_items: applied > 0 ? 1 : 0,
      failed_suggestions: Math.max(0, ids.size - applied),
      message: `已采纳 ${applied} 条建议`,
    } as T;
  }
  if (name === "list_external_api_logs") {
    return previewExternalApiLogs.value as T;
  }
  if (name === "list_notifications") {
    const status = String(args?.status || "all");
    const severity = String(args?.severity || "all");
    const items = previewNotifications.value.filter((item) => {
      const statusMatched = status === "all" || item.status === status;
      const severityMatched = severity === "all" || item.severity === severity;
      return statusMatched && severityMatched;
    });
    return {
      items,
      total: items.length,
      unread_count: previewNotifications.value.filter((item) => item.status === "unread").length,
      critical_count: previewNotifications.value.filter((item) => item.status === "unread" && item.severity === "critical").length,
    } as T;
  }
  if (name === "mark_notification_read") {
    const notificationId = String(args?.notificationId || "");
    const notification = previewNotifications.value.find((item) => item.id === notificationId);
    if (!notification) {
      throw new Error("通知不存在");
    }
    const updated = notification.status === "read" ? 0 : 1;
    notification.status = "read";
    notification.read_at = notification.read_at || "2026-05-22T00:25:00+08:00";
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
    const restored = previewDatabaseBackups.value.find((item) => item.file_path === request?.file_path);
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
      message: "数据库已恢复并通过完整性校验，建议重启应用以确保所有页面读取最新连接。",
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
    const rejectScene = args?.rejectScene === undefined || args.rejectScene === null
      ? null
      : Number(args.rejectScene);
    return previewAftersaleRejectReasons.value
      .filter((reason) => !shopId || reason.shop_id === shopId)
      .filter((reason) => rejectScene === null || reason.reject_scene === rejectScene) as T;
  }
  if (name === "sync_aftersale_reject_reasons") {
    const shopId = String(args?.shopId || "shop-preview");
    const taskId = `aftersale_reject_reason_sync_preview_${Date.now()}`;
    const now = "2026-05-22T00:24:00+08:00";
    previewAftersaleRejectReasons.value = previewAftersaleRejectReasons.value.map((reason) => (
      reason.shop_id === shopId ? { ...reason, synced_at: now } : reason
    ));
    previewTaskRuns.value.unshift({
      id: taskId,
      task_type: "aftersales.sync_reject_reasons",
      status: "success",
      progress: 100,
      pending_count: 0,
      ready_count: previewAftersaleRejectReasons.value.filter((reason) => reason.shop_id === shopId).length,
      failed_count: 0,
      started_at: now,
      finished_at: now,
      created_at: now,
    });
    return {
      task_id: taskId,
      shop_id: shopId,
      synced_reasons: previewAftersaleRejectReasons.value.filter((reason) => reason.shop_id === shopId).length,
      failed_steps: [],
    } as T;
  }
  if (name === "get_automation_settings") {
    return automationSettings.value as T;
  }
  if (name === "set_automation_settings") {
    automationSettings.value = {
      ...defaultAutomationSettings(),
      ...(args?.settings as Partial<OperationalAutomationSettings> | undefined),
    };
    return automationSettings.value as T;
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
    const runStep = async <R,>(
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
      (stepResult) => { result.order_sync = stepResult; },
    );
    await runStep<OrderDetailSyncBatchResult>(
      automationSettings.value.order_detail_sync_enabled,
      "orders.sync_order_details",
      "run_order_detail_sync_once",
      { limit: 50 },
      (stepResult) => { result.order_detail_sync = stepResult; },
    );
    await runStep<AftersaleSyncBatchResult>(
      automationSettings.value.aftersale_sync_enabled,
      "aftersales.sync_shop_aftersales",
      "run_aftersale_sync_once",
      { lookbackHours: 24, limit: 200 },
      (stepResult) => { result.aftersale_sync = stepResult; },
    );
    await runStep<PurchaseTaskBatchResult>(
      automationSettings.value.purchase_task_enabled,
      "procurement.create_purchase_tasks",
      "run_purchase_task_generation_once",
      { limit: 100 },
      (stepResult) => { result.purchase_task_generation = stepResult; },
    );
    await runStep<DeliverySubmitBatchResult>(
      automationSettings.value.delivery_submission_enabled,
      "delivery.submit_wechat_shipment",
      "run_delivery_submission_once",
      { limit: 20 },
      (stepResult) => { result.delivery_submission = stepResult; },
    );
    await runStep<PublishTaskBatchResult>(
      automationSettings.value.publish_precheck_enabled,
      "publish.precheck_products",
      "run_publish_tasks_once",
      { limit: 50 },
      (stepResult) => { result.publish_precheck = stepResult; },
    );
    await runStep<PublishAttributeFillBatchResult>(
      automationSettings.value.publish_attribute_fill_enabled,
      "publish.fill_required_attributes",
      "run_publish_attribute_fill_once",
      { limit: 50 },
      (stepResult) => { result.publish_attribute_fill = stepResult; },
    );
    await runStep<PublishCategoryPrecheckBatchResult>(
      automationSettings.value.publish_category_precheck_enabled,
      "publish.category_precheck",
      "run_publish_category_prechecks_once",
      { limit: 20 },
      (stepResult) => { result.publish_category_precheck = stepResult; },
    );
    await runStep<AssetUploadBatchResult>(
      automationSettings.value.publish_asset_upload_enabled,
      "publish.upload_assets",
      "run_publish_asset_uploads_once",
      { limit: 10 },
      (stepResult) => { result.publish_asset_upload = stepResult; },
    );
    await runStep<ProductSubmitBatchResult>(
      automationSettings.value.publish_submit_enabled,
      "publish.submit_products",
      "run_publish_submits_once",
      { limit: 10 },
      (stepResult) => { result.publish_submit = stepResult; },
    );
    await runStep<ProductStatusSyncBatchResult>(
      automationSettings.value.publish_status_sync_enabled,
      "publish.sync_status",
      "run_publish_status_sync_once",
      { limit: 20 },
      (stepResult) => { result.publish_status_sync = stepResult; },
    );
    await runStep<ProductListingBatchResult>(
      automationSettings.value.publish_listing_enabled,
      "publish.listing_products",
      "run_publish_listing_once",
      { limit: 10 },
      (stepResult) => { result.publish_listing = stepResult; },
    );
    await runStep<PriceUpdateConfirmBatchResult>(
      automationSettings.value.price_confirm_enabled,
      "price.confirm",
      "run_price_update_confirm_once",
      { limit: 50 },
      (stepResult) => { result.price_confirm = stepResult; },
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
    const items = status === "all"
      ? previewShipments.value
      : previewShipments.value.filter((item) => item.status === status);
    return {
      items,
      total: items.length,
    } as T;
  }
  if (name === "retry_delivery_shipment") {
    const shipmentId = String(args?.shipmentId || "");
    const shipment = previewShipments.value.find((item) => item.id === shipmentId);
    if (!shipment) {
      throw new Error("物流单不存在");
    }
    if (shipment.status === "wechat_shipped") {
      throw new Error("已发货成功的物流单不能重试");
    }
    shipment.status = deliverySettings.value.auto_send_delivery ? "ready_to_send" : "waiting_confirmation";
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
    const items = status === "all"
      ? previewPurchaseTasks.value
      : previewPurchaseTasks.value.filter((item) => item.status === status);
    return {
      items,
      total: items.length,
    } as T;
  }
  if (name === "resolve_purchase_task_mapping") {
    const request = args?.request as {
      purchase_task_id?: string;
      external_product_id?: string;
      external_sku_id?: string;
      source_url?: string | null;
      supplier_name?: string | null;
      supplier_product_id?: string | null;
      estimated_cost?: number | null;
      note?: string | null;
    } | undefined;
    const task = previewPurchaseTasks.value.find((item) => item.id === request?.purchase_task_id);
    const externalProductId = request?.external_product_id?.trim() || "";
    const externalSkuId = request?.external_sku_id?.trim() || "";
    if (!task || !externalProductId || !externalSkuId) {
      throw new Error("采购映射参数不完整");
    }
    if (task.status !== "needs_mapping" && task.status !== "pending_purchase") {
      throw new Error("只有待映射或待采购任务允许补齐映射");
    }
    task.status = "pending_purchase";
    task.external_product_id = externalProductId;
    task.external_sku_id = externalSkuId;
    task.source_url = request?.source_url?.trim() || task.source_url;
    task.supplier_name = request?.supplier_name?.trim() || task.supplier_name;
    task.supplier_product_id = request?.supplier_product_id?.trim() || task.supplier_product_id;
    task.estimated_cost = request?.estimated_cost ?? task.estimated_cost;
    task.estimated_profit = task.estimated_cost === null || task.estimated_cost === undefined
      ? null
      : ((task.estimated_revenue || 0) / 100) - task.estimated_cost;
    task.error_summary = null;
    task.updated_at = "2026-05-22T00:22:00+08:00";
    previewNotifications.value.forEach((notification) => {
      if (notification.source_type === "purchase_mapping" && notification.status === "unread") {
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
    const items = status === "all"
      ? allItems
      : allItems.filter((item) => item.risk_status === status);
    return {
      items,
      total: items.length,
      low_stock_count: items.filter((item) => item.risk_status === "low_stock" || item.risk_status === "stock_pressure").length,
      out_of_stock_count: items.filter((item) => item.risk_status === "out_of_stock").length,
      issue_count: items.filter((item) => item.risk_status === "supplier_issue").length,
    } as T;
  }
  if (name === "list_product_sales_analysis") {
    const status = String(args?.status || "all");
    const allItems = buildPreviewProductSalesAnalysis();
    const items = status === "all"
      ? allItems
      : allItems.filter((item) => item.operation_status === status || item.inventory_risk_status === status);
    return {
      items,
      total: items.length,
      totals: summarizeProductSalesAnalysis(items),
    } as T;
  }
  if (name === "run_inventory_risk_scan_once") {
    const items = buildPreviewInventoryRisks();
    const riskyItems = items.filter((item) => item.risk_status !== "healthy" && item.risk_status !== "not_listed");
    riskyItems.forEach((item) => {
      previewNotifications.value.unshift({
        id: `notification-preview-inventory-${item.external_product_id}-${Date.now()}`,
        severity: item.risk_status === "out_of_stock" ? "critical" : "warning",
        source_type: "inventory_risk",
        source_id: item.external_product_id,
        shop_id: null,
        shop_name: null,
        title: `库存风控：${item.title}`,
        body: `${item.recommendation}；可用库存 ${item.available_stock}，采购占用 ${item.reserved_quantity}。`,
        status: "unread",
        data_json: JSON.stringify({ external_product_id: item.external_product_id, risk_status: item.risk_status }),
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
      low_stock_products: items.filter((item) => item.risk_status === "low_stock" || item.risk_status === "stock_pressure").length,
      out_of_stock_products: items.filter((item) => item.risk_status === "out_of_stock").length,
      issue_products: items.filter((item) => item.risk_status === "supplier_issue").length,
      notifications_created: riskyItems.length,
    } as T;
  }
  if (name === "mark_purchase_task_issue") {
    const request = args?.request as {
      purchase_task_id?: string;
      issue_type?: string;
      note?: string | null;
    } | undefined;
    const task = previewPurchaseTasks.value.find((item) => item.id === request?.purchase_task_id);
    if (!task || !request?.issue_type) {
      throw new Error("采购异常参数不完整");
    }
    const mapping: Record<string, { status: string; label: string }> = {
      out_of_stock: { status: "supplier_out_of_stock", label: "供应商缺货" },
      price_changed: { status: "supplier_price_changed", label: "供应商涨价" },
      supplier_cancelled: { status: "supplier_cancelled", label: "供应商取消" },
      quality_risk: { status: "supplier_quality_risk", label: "供应商质量风险" },
      other: { status: "supplier_exception", label: "供应商其他异常" },
    };
    const issue = mapping[request.issue_type] || mapping.other;
    task.status = issue.status;
    task.error_summary = request.note ? `${issue.label}：${request.note}` : issue.label;
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
      data_json: JSON.stringify({ purchase_task_id: task.id, order_id: task.order_id, status: issue.status }),
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
  if (name === "list_aftersales") {
    const status = String(args?.status || "all");
    const items = status === "all"
      ? previewAftersales.value
      : status === "active"
        ? previewAftersales.value.filter((item) => canSubmitAftersaleAction(item))
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
    const items = previewAftersaleEvidence.value.filter((item) => (
      (targetType === "all" || item.target_type === targetType)
      && (!targetId || item.target_id === targetId || item.external_target_id === targetId)
      && (status === "all" || item.status === status)
    ));
    return {
      items,
      total: items.length,
    } as T;
  }
  if (name === "list_guarantee_orders") {
    const status = String(args?.status || "all");
    const items = status === "all"
      ? previewGuaranteeOrders.value
      : status === "active"
        ? previewGuaranteeOrders.value.filter((item) => isActiveGuaranteeStatus(item.status))
        : previewGuaranteeOrders.value.filter((item) => item.status === status);
    return {
      items,
      total: items.length,
    } as T;
  }
  if (name === "record_aftersale_evidence") {
    const request = args?.request as {
      target_type?: string;
      target_id?: string;
      evidence_type?: string;
      title?: string;
      content_text?: string | null;
      local_file_path?: string | null;
      source_url?: string | null;
      status?: string | null;
    } | undefined;
    if (!request?.target_type || !request.target_id || !request.evidence_type || !request.title) {
      throw new Error("凭证资料参数不完整");
    }
    const target = request.target_type === "guarantee"
      ? previewGuaranteeOrders.value.find((item) => item.id === request.target_id || item.guarantee_order_id === request.target_id)
      : previewAftersales.value.find((item) => item.id === request.target_id || item.wechat_aftersale_id === request.target_id);
    if (!target) {
      throw new Error("凭证目标不存在");
    }
    const externalTargetId = request.target_type === "guarantee"
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
      title: request.target_type === "guarantee" ? "纠纷凭证资料已记录" : "售后凭证资料已记录",
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
    const request = args?.request as {
      evidence_id?: string;
      status?: string;
    } | undefined;
    const evidence = previewAftersaleEvidence.value.find((item) => item.id === request?.evidence_id);
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
    const exportedCount = previewAftersaleEvidence.value.filter((item) => (
      (targetType === "all" || item.target_type === targetType)
      && (!targetId || item.target_id === targetId || item.external_target_id === targetId)
      && (status === "all" || item.status === status)
    )).length;
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
    const items = previewSupplierAftersaleFollowups.value.filter((item) => (
      (targetType === "all" || item.target_type === targetType)
      && (!targetId || item.target_id === targetId || item.external_target_id === targetId)
      && (status === "all" || item.status === status)
    ));
    return {
      items,
      total: items.length,
    } as T;
  }
  if (name === "record_supplier_aftersale_followup") {
    const request = args?.request as {
      target_type?: string;
      target_id?: string;
      followup_type?: string;
      status?: string;
      note?: string;
      purchase_task_id?: string | null;
      supplier_name?: string | null;
    } | undefined;
    if (!request?.target_type || !request.target_id || !request.followup_type || !request.status || !request.note) {
      throw new Error("供应商协同参数不完整");
    }
    const target = request.target_type === "guarantee"
      ? previewGuaranteeOrders.value.find((item) => item.id === request.target_id || item.guarantee_order_id === request.target_id)
      : previewAftersales.value.find((item) => item.id === request.target_id || item.wechat_aftersale_id === request.target_id);
    if (!target) {
      throw new Error("协同目标不存在");
    }
    const externalTargetId = request.target_type === "guarantee"
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
      message: "供应商售后协同已记录，只保存本地脱敏备注，不调用供应商平台或微信处理接口",
    } as T;
  }
  if (name === "record_guarantee_followup") {
    const request = args?.request as {
      guarantee_order_id?: string;
      handling_status?: string;
      responsibility_party?: string | null;
      handling_note?: string | null;
      supplier_compensation_cents?: number | null;
    } | undefined;
    const guarantee = previewGuaranteeOrders.value.find((item) => (
      item.id === request?.guarantee_order_id || item.guarantee_order_id === request?.guarantee_order_id
    ));
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
    const profitAdjustmentId = compensation > 0 && guarantee.order_id
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
      severity: request.handling_status === "resolved" || request.handling_status === "ignored" ? "info" : "warning",
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
    const request = args?.request as {
      aftersale_id?: string;
      responsibility_party?: string;
      responsibility_note?: string | null;
      supplier_compensation_cents?: number | null;
    } | undefined;
    const aftersale = previewAftersales.value.find((item) => (
      item.id === request?.aftersale_id || item.wechat_aftersale_id === request?.aftersale_id
    ));
    if (!aftersale || !request?.responsibility_party) {
      throw new Error("售后责任归因参数不完整");
    }
    const compensation = request.supplier_compensation_cents || 0;
    aftersale.responsibility_party = request.responsibility_party;
    aftersale.responsibility_note = request.responsibility_note || null;
    aftersale.supplier_compensation_cents = compensation;
    aftersale.handled_at = "2026-05-22T00:20:00+08:00";
    aftersale.updated_at = "2026-05-22T00:20:00+08:00";
    const profitAdjustmentId = compensation > 0 && aftersale.order_id
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
    const request = args?.request as {
      aftersale_id?: string;
      address_id?: string | null;
      accept_type?: number | null;
      note?: string | null;
    } | undefined;
    const aftersale = previewAftersales.value.find((item) => (
      item.id === request?.aftersale_id || item.wechat_aftersale_id === request?.aftersale_id
    ));
    if (!aftersale || !request?.aftersale_id) {
      throw new Error("售后同意参数不完整");
    }
    if (!canSubmitAftersaleAction(aftersale)) {
      throw new Error("当前售后状态不允许提交处理动作");
    }
    const acceptType = request.accept_type === undefined || request.accept_type === null
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
      data_json: JSON.stringify({ aftersale_id: aftersale.id, action: "accept", accept_type: acceptType }),
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
    const request = args?.request as {
      aftersale_id?: string;
      reject_reason_type?: number;
      reject_reason?: string | null;
      note?: string | null;
    } | undefined;
    const aftersale = previewAftersales.value.find((item) => (
      item.id === request?.aftersale_id || item.wechat_aftersale_id === request?.aftersale_id
    ));
    const rejectReasonType = Number(request?.reject_reason_type);
    if (!aftersale || !request?.aftersale_id || !Number.isInteger(rejectReasonType) || rejectReasonType <= 0) {
      throw new Error("售后拒绝参数不完整");
    }
    if (!canSubmitAftersaleAction(aftersale)) {
      throw new Error("当前售后状态不允许提交处理动作");
    }
    const now = "2026-05-22T00:24:00+08:00";
    aftersale.last_action = "reject";
    aftersale.last_action_status = "success";
    aftersale.last_action_error = null;
    aftersale.last_action_note = request.note || request.reject_reason || null;
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
      data_json: JSON.stringify({ aftersale_id: aftersale.id, action: "reject", reject_reason_type: rejectReasonType }),
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
    const request = args?.request as {
      order_id?: string;
      kind?: string;
      amount_cents?: number;
    } | undefined;
    if (!request?.order_id || !request.kind || request.amount_cents === undefined) {
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
    const exportedCount = status === "all"
      ? previewPurchaseTasks.value.length
      : previewPurchaseTasks.value.filter((item) => item.status === status).length;
    return {
      file_path: "/tmp/wx-xd-preview-purchase-tasks.csv",
      exported_count: exportedCount,
    } as T;
  }
  if (name === "export_supplier_agent_tasks") {
    const status = String(args?.status || "all");
    const exportFormat = String(args?.format || "jsonl");
    const exportedCount = status === "all"
      ? previewPurchaseTasks.value.length
      : previewPurchaseTasks.value.filter((item) => item.status === status).length;
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
    const request = args?.request as { raw_results?: string; dry_run?: boolean } | undefined;
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
    const request = args?.request as {
      purchase_task_id?: string;
      delivery_id?: string;
      delivery_name?: string;
      waybill_id?: string;
      deliver_type?: number;
      estimated_cost?: number | null;
    } | undefined;
    const task = previewPurchaseTasks.value.find((item) => item.id === request?.purchase_task_id);
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
    task.estimated_profit = task.estimated_cost === null || task.estimated_cost === undefined
      ? null
      : ((task.estimated_revenue || 0) / 100) - task.estimated_cost;
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
      status: deliverySettings.value.auto_send_delivery ? "ready_to_send" : "waiting_confirmation",
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
      shipment_status: deliverySettings.value.auto_send_delivery ? "ready_to_send" : "waiting_confirmation",
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
    const activeExists = previewAftersales.value.some((item) => item.status === "MERCHANT_PROCESSING");
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
    previewPendingOrderCount.value = Math.max(previewPendingOrderCount.value, 1);
    previewTaskRuns.value.unshift({
      id: taskId,
      task_type: "aftersales.sync_shop_aftersales",
      status: "partial_success",
      progress: 100,
      created_at: "2026-05-22T00:18:00+08:00",
      started_at: "2026-05-22T00:18:00+08:00",
      finished_at: "2026-05-22T00:18:02+08:00",
      pending_count: 0,
      ready_count: previewAftersales.value.filter((item) => item.status === "MERCHANT_PROCESSING").length,
      failed_count: previewAftersales.value.filter((item) => item.status === "sync_failed").length,
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
    const activeExists = previewGuaranteeOrders.value.some((item) => item.status === "STATUS_WAIT_MERCHANT_PROOF");
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
    previewPendingOrderCount.value = Math.max(previewPendingOrderCount.value, 1);
    previewTaskRuns.value.unshift({
      id: taskId,
      task_type: "aftersales.sync_guarantee_orders",
      status: "success",
      progress: 100,
      created_at: "2026-05-22T00:25:00+08:00",
      started_at: "2026-05-22T00:25:00+08:00",
      finished_at: "2026-05-22T00:25:05+08:00",
      pending_count: 0,
      ready_count: previewGuaranteeOrders.value.filter((item) => isActiveGuaranteeStatus(item.status)).length,
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
    const request = args?.request as {
      order_id?: string;
      shop_id?: string;
      wechat_order_id?: string;
      delivery_id?: string;
      delivery_name?: string;
      waybill_id?: string;
      deliver_type?: number;
    } | undefined;
    const orderId = request?.order_id || "order-preview";
    const shipmentId = `shipment-preview-${orderId}`;
    upsertPreviewShipment({
      id: shipmentId,
      order_id: orderId,
      shop_id: request?.shop_id || "shop-preview",
      shop_name: previewShops.value.find((shop) => shop.id === request?.shop_id)?.name || "预览店铺",
      wechat_order_id: request?.wechat_order_id || "420000000001",
      delivery_id: request?.delivery_id || null,
      delivery_name: request?.delivery_name || null,
      waybill_id: request?.waybill_id || null,
      deliver_type: request?.deliver_type || 1,
      status: deliverySettings.value.auto_send_delivery ? "ready_to_send" : "waiting_confirmation",
      error_code: null,
      error_summary: null,
      submitted_at: null,
      created_at: "2026-05-22T00:09:00+08:00",
      updated_at: "2026-05-22T00:09:00+08:00",
    });
    return {
      shipment_id: shipmentId,
      order_id: orderId,
      status: deliverySettings.value.auto_send_delivery ? "ready_to_send" : "waiting_confirmation",
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
    const readyShipments = previewShipments.value.filter((shipment) => shipment.status === "ready_to_send");
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
    const task = previewTaskRuns.value.find((item) => ["pending", "queued", "running"].includes(item.status));
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
    const task = previewTaskRuns.value.find((item) => item.status === "ready_to_publish");
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
    const task = previewTaskRuns.value.find((item) => currentJob.value?.id === item.id);
    const attrFailedItems = currentJob.value?.products.flatMap((product) => product.items)
      .filter((item) => item.status === "failed" && item.error_code === "CATEGORY_ATTRS_NEED_AI_FILL") || [];
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
          if (item.status === "failed" && item.error_code === "CATEGORY_ATTRS_NEED_AI_FILL") {
            item.status = "ready_to_publish";
            item.error_code = null;
            item.error_summary = "已自动补齐必填属性，等待重新执行微信类目预检";
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
    const task = previewTaskRuns.value.find((item) => currentJob.value?.id === item.id);
    const attrFailedItems = currentJob.value?.products.flatMap((product) => product.items)
      .filter((item) => item.status === "failed" && item.error_code === "CATEGORY_ATTRS_NEED_AI_FILL") || [];
    if (task && attrFailedItems.length > 0) {
      return {
        processed_jobs: 1,
        processed_items: attrFailedItems.length,
        auto_filled_items: aiProviderSettings.value.enabled && aiProviderSettings.value.has_api_key ? attrFailedItems.length : 0,
        suggestion_only_items: aiProviderSettings.value.enabled && aiProviderSettings.value.has_api_key ? 0 : attrFailedItems.length,
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
  if (name === "run_price_update_precheck_once") {
    const task = previewTaskRuns.value.find((item) => item.task_type === "price.create_update_job" && ["pending", "queued", "running"].includes(item.status));
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
          item.error_summary = "本地改价校验通过，等待提交微信 updateproduct";
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
    const task = previewTaskRuns.value.find((item) => item.task_type === "price.create_update_job" && item.status === "ready_to_update");
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
          item.error_summary = "微信 updateproduct 已提交，等待商品状态同步确认价格";
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
    const task = previewTaskRuns.value.find((item) => item.task_type === "price.create_update_job" && ["submitted", "audit_pending"].includes(item.status));
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
          item.error_summary = "线上 product.skus[].sale_price 已全部匹配目标价";
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
    const task = previewTaskRuns.value.find((item) => item.status === "ready_to_publish");
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
    const task = previewTaskRuns.value.find((item) => item.status === "assets_ready");
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
    const task = previewTaskRuns.value.find((item) => ["submitted", "audit_pending"].includes(item.status));
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
    const task = previewTaskRuns.value.find((item) => item.status === "audit_passed");
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
            item.error_summary = "微信 listingproduct 已提交，等待 getproduct 确认已上架";
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

function buildPreviewOrderProfitResult(status: string): OrderProfitListResult {
  const grouped = new Map<string, PurchaseTaskView[]>();
  previewPurchaseTasks.value.forEach((task) => {
    const rows = grouped.get(task.order_id) || [];
    rows.push(task);
    grouped.set(task.order_id, rows);
  });
  const items = Array.from(grouped.entries()).map(([orderId, tasks]) => {
    const first = tasks[0];
    const revenue = tasks.reduce((sum, task) => sum + (task.estimated_revenue || 0), 0);
    const purchaseCost = tasks.reduce((sum, task) => sum + Math.round((task.estimated_cost || 0) * 100), 0);
    const missingCost = tasks.filter((task) => task.estimated_cost === null || task.estimated_cost === undefined).length;
    const adjustment = (kind: string) => previewProfitAdjustments.value
      .filter((item) => item.order_id === orderId && item.kind === kind)
      .reduce((sum, item) => sum + item.amount_cents, 0);
    const purchaseFreight = adjustment("purchase_freight");
    const refund = adjustment("refund");
    const aftersaleCompensation = adjustment("aftersale_compensation");
    const otherCost = adjustment("other_cost");
    const otherIncome = adjustment("other_income");
    const estimatedProfit = revenue + otherIncome - purchaseCost - purchaseFreight - refund - aftersaleCompensation - otherCost;
    const actualProfit = missingCost === 0 ? estimatedProfit : null;
    const profitStatus = tasks.length === 0
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
      order_status: first.status === "supplier_shipped" ? "supplier_shipped" : "pending_purchase",
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
  const filtered = status === "all"
    ? items
    : items.filter((item) => item.profit_status === status || item.order_status === status);
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
    purchase_cost_cents: items.reduce((sum, item) => sum + item.purchase_cost_cents, 0),
    purchase_freight_cents: items.reduce((sum, item) => sum + item.purchase_freight_cents, 0),
    refund_cents: items.reduce((sum, item) => sum + item.refund_cents, 0),
    aftersale_compensation_cents: items.reduce((sum, item) => sum + item.aftersale_compensation_cents, 0),
    other_cost_cents: items.reduce((sum, item) => sum + item.other_cost_cents, 0),
    other_income_cents: items.reduce((sum, item) => sum + item.other_income_cents, 0),
    estimated_profit_cents: items.reduce((sum, item) => sum + item.estimated_profit_cents, 0),
    actual_profit_cents: items.reduce((sum, item) => sum + (item.actual_profit_cents || 0), 0),
    unknown_actual_order_count: items.filter((item) => item.actual_profit_cents === null).length,
  };
}

function buildPreviewInventoryRisks(): InventoryRiskView[] {
  const reserved = previewPurchaseTasks.value
    .filter((task) => ["pending_purchase", "supplier_shipped", "wechat_shipped", "completed"].includes(task.status))
    .reduce((sum, task) => sum + task.quantity, 0);
  const pending = previewPurchaseTasks.value
    .filter((task) => task.status === "pending_purchase")
    .reduce((sum, task) => sum + task.quantity, 0);
  const issueCount = previewPurchaseTasks.value
    .filter((task) => task.status.startsWith("supplier_") && task.status !== "supplier_shipped")
    .length;
  const totalStock = 12;
  const available = totalStock - reserved;
  const riskStatus = issueCount > 0
    ? "supplier_issue"
    : available <= 0
      ? "out_of_stock"
      : pending > available
        ? "stock_pressure"
        : available <= 5
          ? "low_stock"
          : "healthy";
  const recommendation: Record<string, string> = {
    supplier_issue: "存在供应商异常采购任务，先人工处理供应商缺货、涨价或取消",
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
  const productTasks = previewPurchaseTasks.value.filter((task) => task.external_product_id === inventory.external_product_id);
  const orderIds = new Set(productTasks.map((task) => task.order_id));
  const unitsSold = productTasks.reduce((sum, task) => sum + task.quantity, 0);
  const revenue = productTasks.reduce((sum, task) => sum + (task.estimated_revenue || 0), 0);
  const purchaseCost = productTasks.reduce((sum, task) => sum + Math.round((task.estimated_cost || 0) * 100), 0);
  const missingCost = productTasks.filter((task) => task.estimated_cost === null || task.estimated_cost === undefined).length;
  const relatedAftersales = previewAftersales.value.filter((item) => item.order_id && orderIds.has(item.order_id));
  const relatedRefund = relatedAftersales.reduce((sum, item) => sum + (item.refund_amount_cents || 0), 0);
  const grossProfit = revenue - purchaseCost - relatedRefund;
  const operationStatus = inventory.risk_status !== "healthy" && inventory.risk_status !== "not_listed"
    ? "stock_risk"
    : unitsSold > 0 && productTasks.length > 0 && missingCost === 0 && grossProfit < 0
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
    margin_risk: "已有真实订单但毛利为负，优先核对成本并进入批量改价",
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

function summarizeProductSalesAnalysis(items: ProductSalesAnalysisView[]): ProductSalesAnalysisTotals {
  return {
    product_count: items.length,
    sold_product_count: items.filter((item) => item.units_sold > 0).length,
    total_units_sold: items.reduce((sum, item) => sum + item.units_sold, 0),
    revenue_cents: items.reduce((sum, item) => sum + item.revenue_cents, 0),
    purchase_cost_cents: items.reduce((sum, item) => sum + item.purchase_cost_cents, 0),
    gross_profit_cents: items.reduce((sum, item) => sum + item.revenue_cents - item.purchase_cost_cents - item.related_refund_cents, 0),
    missing_cost_product_count: items.filter((item) => item.missing_cost_count > 0).length,
    scale_candidate_count: items.filter((item) => item.operation_status === "scale_candidate").length,
    risk_product_count: items.filter((item) => ["stock_risk", "margin_risk", "aftersale_watch"].includes(item.operation_status)).length,
  };
}

const selectedGroupOptions = computed(() => groups.value.map((group) => ({
  label: `${group.name} (${group.shop_count})`,
  value: group.id,
})));

function upsertPreviewShipment(next: ShipmentView) {
  const index = previewShipments.value.findIndex((shipment) => shipment.id === next.id);
  if (index >= 0) {
    previewShipments.value[index] = { ...previewShipments.value[index], ...next };
  } else {
    previewShipments.value.unshift(next);
  }
}

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
      localApiConfigResult,
      aiProviderSettingsResult,
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
      command<LocalApiConfig>("get_local_api_config"),
      command<AiProviderSettings>("get_ai_provider_settings"),
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
    localApiConfig.value = localApiConfigResult;
    aiProviderSettings.value = aiProviderSettingsResult;
    syncAiProviderForm(aiProviderSettingsResult);
    externalApiLogs.value = externalApiLogsResult;
    databaseBackups.value = backupsResult;
    if (!shopForm.group_id && groups.value.length > 0) {
      shopForm.group_id = groups.value[0].id;
    }
    if (!shipmentForm.shop_id && shops.value.length > 0) {
      shipmentForm.shop_id = shops.value[0].id;
    }
    if ((!selectedCategoryShopId.value || !shops.value.some((shop) => shop.id === selectedCategoryShopId.value)) && shops.value.length > 0) {
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
      refreshInventoryRisks(),
      refreshProductSalesAnalysis(),
      refreshCategoryCatalog(),
      refreshAttributeSuggestions(),
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
  aiProviderForm.provider_type = settings.provider_type || "openai_compatible";
  aiProviderForm.base_url = settings.base_url || aiProviderForm.base_url;
  aiProviderForm.model = settings.model || aiProviderForm.model;
  aiProviderForm.temperature = String(settings.temperature ?? 0.1);
  aiProviderForm.clear_api_key = false;
  aiProviderForm.api_key = "";
}

async function refreshAttributeSuggestions() {
  const result = await command<PublishAttributeSuggestionListResult>("list_publish_attribute_suggestions", {
    status: attributeSuggestionStatusFilter.value,
    limit: 200,
  });
  attributeSuggestions.value = result.items;
  attributeSuggestionTotal.value = result.total;
  pendingAttributeSuggestionCount.value = result.pending_count;
  appliedAttributeSuggestionCount.value = result.applied_count;
  selectedAttributeSuggestions.value = [];
}

async function refreshJobAttributeSuggestions() {
  const taskId = currentJob.value?.id || queriedTaskId.value.trim();
  if (!taskId) {
    jobAttributeSuggestions.value = [];
    return;
  }
  const result = await command<PublishAttributeSuggestionListResult>("list_publish_attribute_suggestions", {
    status: "all",
    jobId: taskId,
    limit: 500,
  });
  jobAttributeSuggestions.value = result.items;
}

async function refreshCategoryCatalog() {
  const result = await command<CategoryCatalogListResult>("list_category_catalog", {
    shopId: selectedCategoryShopId.value,
    keyword: categoryKeyword.value.trim() || null,
    limit: 150,
  });
  categoryCatalogShops.value = result.shops;
  categoryCache.value = result.categories;
  freightTemplates.value = result.freight_templates;
}

async function refreshPurchaseTasks() {
  const result = await command<PurchaseTaskListResult>("list_purchase_tasks", {
    status: purchaseStatusFilter.value,
    limit: 200,
  });
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
  const result = await command<AftersaleEvidenceListResult>("list_aftersale_evidence", {
    targetType: evidenceTargetTypeFilter.value === "all" ? null : evidenceTargetTypeFilter.value,
    targetId: evidenceTargetIdFilter.value.trim() || null,
    status: evidenceStatusFilter.value === "all" ? null : evidenceStatusFilter.value,
    limit: 200,
  });
  aftersaleEvidence.value = result.items;
  aftersaleEvidenceTotal.value = result.total;
}

async function refreshSupplierAftersaleFollowups() {
  const result = await command<SupplierAftersaleFollowupListResult>("list_supplier_aftersale_followups", {
    targetType: supplierFollowupTargetTypeFilter.value === "all" ? null : supplierFollowupTargetTypeFilter.value,
    targetId: supplierFollowupTargetIdFilter.value.trim() || null,
    status: supplierFollowupStatusFilter.value === "all" ? null : supplierFollowupStatusFilter.value,
    limit: 200,
  });
  supplierAftersaleFollowups.value = result.items;
  supplierAftersaleFollowupTotal.value = result.total;
}

async function refreshAftersaleRejectReasons() {
  aftersaleRejectReasons.value = await command<AftersaleRejectReasonView[]>("list_aftersale_reject_reasons");
}

async function refreshGuaranteeOrders() {
  const result = await command<GuaranteeOrderListResult>("list_guarantee_orders", {
    status: guaranteeStatusFilter.value,
    limit: 200,
  });
  guaranteeOrders.value = result.items;
  guaranteeOrderTotal.value = result.total;
}

async function refreshDeliveryCompanies() {
  deliveryCompanies.value = await command<DeliveryCompanyView[]>("list_delivery_companies");
}

async function refreshDeliveryShipments() {
  const result = await command<ShipmentListResult>("list_delivery_shipments", {
    status: deliveryStatusFilter.value,
    limit: 200,
  });
  deliveryShipments.value = result.items;
  deliveryShipmentTotal.value = result.total;
}

async function refreshOrderProfits() {
  const result = await command<OrderProfitListResult>("list_order_profit_summaries", {
    status: orderProfitStatusFilter.value,
    limit: 200,
  });
  orderProfits.value = result.items;
  orderProfitTotal.value = result.total;
  orderProfitTotals.value = result.totals;
}

async function refreshInventoryRisks() {
  const result = await command<InventoryRiskListResult>("list_inventory_risks", {
    status: inventoryRiskStatusFilter.value,
    limit: 200,
  });
  inventoryRisks.value = result.items;
  inventoryRiskTotal.value = result.total;
  inventoryRiskStats.value = {
    low_stock_count: result.low_stock_count,
    out_of_stock_count: result.out_of_stock_count,
    issue_count: result.issue_count,
  };
}

async function refreshProductSalesAnalysis() {
  const result = await command<ProductSalesAnalysisListResult>("list_product_sales_analysis", {
    status: productSalesAnalysisStatusFilter.value,
    limit: 200,
  });
  productSalesAnalysis.value = result.items;
  productSalesAnalysisTotal.value = result.total;
  productSalesAnalysisTotals.value = result.totals;
}

async function runInventoryRiskScan() {
  try {
    const result = await command<InventoryRiskScanResult>("run_inventory_risk_scan_once");
    ElMessage.success(`库存扫描完成：商品 ${result.scanned_products} 个，通知 ${result.notifications_created} 条`);
    await Promise.all([refreshInventoryRisks(), refreshProductSalesAnalysis(), refreshNotifications(), refreshAll()]);
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
    const result = await command<ShopCredentialCheck>("verify_shop_credentials", {
      shopId: shop.id,
      forceRefresh: false,
    });
    if (result.status === "active") {
      ElMessage.success(`凭证验证成功，到期时间：${result.expires_at}`);
    } else {
      ElMessage.error(`凭证验证失败：${result.errmsg || result.errcode || "未知错误"}`);
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
    const result = await command<ShopBasicInfoSyncResult>("sync_shop_basic_info", {
      shopId: shop.id,
    });
    if (result.status === "active") {
      ElMessage.success(`店铺资料已同步：${result.nickname || shop.name}`);
    } else {
      ElMessage.error(`资料同步失败：${result.errmsg || result.errcode || "未知错误"}`);
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
    const result = await command<ApiQuotaCheckResult>("check_shop_api_quota", {
      request: {
        shop_id: shop.id,
        cgi_path: "/channels/ec/basics/info/get",
      },
    });
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
    const result = await command<CategoryCatalogSyncResult>("sync_shop_category_catalog", {
      shopId: shop.id,
    });
    if (result.failed_steps.length > 0) {
      ElMessage.warning(`类目同步部分完成：类目 ${result.synced_categories}，运费模板 ${result.synced_freight_templates}`);
    } else {
      ElMessage.success(`类目同步完成：类目 ${result.synced_categories}，运费模板 ${result.synced_freight_templates}`);
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
    const result = await command<CategoryRuleSyncResult>("sync_category_rules", {
      shopId: shop.id,
      catId: targetCatId,
    });
    if (result.failed_steps.length > 0) {
      ElMessage.warning(`规则同步部分完成：${result.failed_steps.join("；")}`);
    } else {
      ElMessage.success(`规则同步完成：商品属性 ${result.product_attr_count}，销售属性 ${result.sale_attr_count}`);
    }
    await Promise.all([refreshCategoryCatalog(), refreshAll()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function rotateLocalApiKey() {
  try {
    const result = await command<LocalApiKeyRotationResult>("rotate_local_api_key");
    rotatedLocalApiKey.value = result.api_key;
    await refreshAll();
    ElMessage.success("本地 HTTP API Key 已生成，旧 Key 已失效");
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function refreshExternalApiLogs() {
  try {
    externalApiLogs.value = await command<ExternalApiLogView[]>("list_external_api_logs", { limit: 80 });
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
  aiProviderSaving.value = true;
  try {
    const result = await command<AiProviderSettings>("save_ai_provider_settings", {
      request: {
        enabled: aiProviderForm.enabled,
        provider_type: aiProviderForm.provider_type,
        base_url: aiProviderForm.base_url.trim(),
        model: aiProviderForm.model.trim(),
        temperature,
        api_key: aiProviderForm.api_key.trim() || null,
        clear_api_key: aiProviderForm.clear_api_key,
      },
    });
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
    databaseBackups.value = await command<BackupInfo[]>("list_database_backups");
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
    const result = await command<BackupCreateResult>("create_database_backup");
    databaseBackups.value = await command<BackupInfo[]>("list_database_backups");
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
    const result = await command<BackupRestoreResult>("restore_database_backup", {
      request: { file_path: backup.file_path },
    });
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

async function saveAutomationSettings() {
  try {
    automationSettings.value = await command<OperationalAutomationSettings>("set_automation_settings", {
      settings: automationSettings.value,
    });
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
    const result = await command<OperationalAutomationRunResult>("run_operational_automation_once");
    lastAutomationResult.value = result;
    if (result.errors.length > 0) {
      ElMessage.warning(`自动推进完成，但 ${result.errors.length} 个步骤失败，请查看错误。`);
    } else if (result.executed_steps.length === 0) {
      ElMessage.info("没有启用的自动推进步骤");
    } else {
      ElMessage.success(`自动推进完成：执行 ${result.executed_steps.length} 步，跳过 ${result.skipped_steps.length} 步`);
    }
    await Promise.all([refreshAll(), queryJob(), queryPriceUpdateJob()]);
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
    const result = await command<PublishJobCreated>("create_external_publish_job", { request });
    latestTaskId.value = result.task_id;
    queriedTaskId.value = result.task_id;
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
    await refreshJobAttributeSuggestions();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function createPriceUpdateJob() {
  let request: unknown;
  try {
    request = JSON.parse(priceUpdatePayload.value);
  } catch {
    ElMessage.error("改价 JSON 格式不正确");
    return;
  }
  try {
    const result = await command<PriceUpdateJobCreated>("create_price_update_job", { request });
    latestPriceTaskId.value = result.task_id;
    queriedPriceTaskId.value = result.task_id;
    ElMessage.success(`改价任务已创建：${result.task_id}`);
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
    currentPriceJob.value = await command<PriceUpdateJobView>("get_price_update_job", {
      taskId: queriedPriceTaskId.value.trim(),
    });
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishTasksOnce() {
  try {
    const result = await command<PublishTaskBatchResult>("run_publish_tasks_once", { limit: 20 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待执行的铺货任务项");
    } else {
      ElMessage.success(`本批处理 ${result.processed_items} 项，通过 ${result.ready_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishAttributeFillOnce() {
  try {
    const result = await command<PublishAttributeFillBatchResult>("run_publish_attribute_fill_once", { limit: 50 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待补齐属性的铺货失败项");
    } else {
      ElMessage.success(`属性补齐 ${result.processed_items} 项，自动补齐 ${result.auto_filled_items} 项，待确认 ${result.suggestion_only_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishAiAttributeSuggestionsOnce() {
  try {
    const result = await command<PublishAttributeFillBatchResult>("run_publish_ai_attribute_suggestions_once", { limit: 20 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待 AI 生成建议的铺货失败项，或 AI provider 尚未启用");
    } else {
      ElMessage.success(`AI 建议处理 ${result.processed_items} 项，自动补齐 ${result.auto_filled_items} 项，待确认 ${result.suggestion_only_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

function handleAttributeSuggestionSelection(rows: PublishAttributeSuggestionView[]) {
  selectedAttributeSuggestions.value = rows;
}

async function applyAttributeSuggestions(rows: PublishAttributeSuggestionView[]) {
  const ids = rows.filter((row) => !row.applied).map((row) => row.id);
  if (ids.length === 0) {
    ElMessage.info("没有待采纳的属性建议");
    return;
  }
  attributeSuggestionApplying.value = true;
  try {
    const result = await command<PublishAttributeSuggestionApplyResult>("apply_publish_attribute_suggestions", {
      request: { suggestion_ids: ids },
    });
    if (result.failed_suggestions > 0) {
      ElMessage.warning(`${result.message}，失败 ${result.failed_suggestions} 条`);
    } else {
      ElMessage.success(result.message);
    }
    await Promise.all([refreshAttributeSuggestions(), refreshAll()]);
    await queryJob();
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    attributeSuggestionApplying.value = false;
  }
}

async function applySelectedAttributeSuggestions() {
  await applyAttributeSuggestions(selectedAttributeSuggestions.value);
}

async function applySingleAttributeSuggestion(row: PublishAttributeSuggestionView) {
  await applyAttributeSuggestions([row]);
}

function attributeKindLabel(kind: string) {
  return kind === "sale" ? "销售属性" : "商品属性";
}

function attributeSuggestionValue(row: PublishAttributeSuggestionView) {
  if (row.sku_values.length > 0) {
    return row.sku_values
      .map((item) => `SKU${item.sku_index + 1}:${item.value}`)
      .join(" / ");
  }
  return row.suggested_value || "-";
}

function allowedValuesText(values: string[]) {
  if (values.length === 0) {
    return "-";
  }
  return values.slice(0, 8).join(" / ") + (values.length > 8 ? " ..." : "");
}

function attributeSuggestionsForProduct(product: PublishJobProductView) {
  const itemIds = new Set(product.items.map((item) => item.id));
  return jobAttributeSuggestions.value.filter((suggestion) => itemIds.has(suggestion.item_id));
}

function pendingAttributeSuggestionsForProduct(product: PublishJobProductView) {
  return attributeSuggestionsForProduct(product).filter((suggestion) => !suggestion.applied);
}

function attributeSuggestionsForItem(row: PublishJobItemRow) {
  return jobAttributeSuggestions.value.filter((suggestion) => suggestion.item_id === row.id);
}

function pendingAttributeSuggestionCountForItem(row: PublishJobItemRow) {
  return attributeSuggestionsForItem(row).filter((suggestion) => !suggestion.applied).length;
}

async function applyProductAttributeSuggestions(product: PublishJobProductView) {
  await applyAttributeSuggestions(pendingAttributeSuggestionsForProduct(product));
}

async function runPublishCategoryPrechecksOnce() {
  try {
    const result = await command<PublishCategoryPrecheckBatchResult>("run_publish_category_prechecks_once", { limit: 20 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待微信类目预检的铺货任务项");
    } else {
      ElMessage.success(`类目预检 ${result.processed_items} 项，通过 ${result.passed_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPriceUpdatePrecheckOnce() {
  try {
    const result = await command<PriceUpdatePrecheckBatchResult>("run_price_update_precheck_once", { limit: 50 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待校验的改价任务项");
    } else {
      ElMessage.success(`改价校验完成：处理 ${result.processed_items} 项，就绪 ${result.ready_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryPriceUpdateJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPriceUpdateSubmitOnce() {
  try {
    const result = await command<PriceUpdateSubmitBatchResult>("run_price_update_submit_once", { limit: 20 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待提交 updateproduct 的改价任务项");
    } else {
      ElMessage.success(`改价提交完成：处理 ${result.processed_items} 项，已提交 ${result.submitted_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryPriceUpdateJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPriceUpdateConfirmOnce() {
  try {
    const result = await command<PriceUpdateConfirmBatchResult>("run_price_update_confirm_once", { limit: 50 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待确认价格的改价任务项");
    } else {
      ElMessage.success(`改价确认完成：处理 ${result.processed_items} 项，已确认 ${result.confirmed_items} 项，待生效 ${result.pending_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryPriceUpdateJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runOrderSyncOnce() {
  try {
    const result = await command<OrderSyncBatchResult>("run_order_sync_once", {
      lookbackDays: 1,
      pageSize: 100,
    });
    if (result.processed_shops === 0) {
      ElMessage.info("没有可同步订单的 active 店铺");
    } else {
      ElMessage.success(`订单同步完成：店铺 ${result.processed_shops} 个，待发货订单 ${result.synced_orders} 个，失败店铺 ${result.failed_shops} 个`);
    }
    await refreshAll();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runOrderDetailSyncOnce() {
  try {
    const result = await command<OrderDetailSyncBatchResult>("run_order_detail_sync_once", { limit: 50 });
    if (result.processed_orders === 0) {
      ElMessage.info("没有待同步详情的订单");
    } else {
      ElMessage.success(`订单详情同步完成：订单 ${result.synced_orders} 个，订单项 ${result.created_items} 个，失败 ${result.failed_orders} 个`);
    }
    await refreshAll();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runAftersaleSyncOnce() {
  try {
    const result = await command<AftersaleSyncBatchResult>("run_aftersale_sync_once", {
      lookbackHours: 24,
      limit: 200,
    });
    if (result.processed_shops === 0) {
      ElMessage.info("没有可同步售后的 active 店铺");
    } else {
      ElMessage.success(`售后同步完成：店铺 ${result.processed_shops} 个，售后单 ${result.synced_aftersales} 个，失败店铺 ${result.failed_shops} 个，失败售后 ${result.failed_aftersales} 个`);
    }
    await Promise.all([refreshAll(), refreshAftersales(), refreshOrderProfits()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runGuaranteeSyncOnce() {
  try {
    const result = await command<GuaranteeSyncBatchResult>("run_guarantee_sync_once", {
      lookbackHours: 24,
      limit: 200,
    });
    if (result.processed_shops === 0) {
      ElMessage.info("没有可同步纠纷单的 active 店铺");
    } else {
      ElMessage.success(`纠纷单同步完成：店铺 ${result.processed_shops} 个，纠纷单 ${result.synced_guarantees} 个，失败店铺 ${result.failed_shops} 个，失败纠纷 ${result.failed_guarantees} 个`);
    }
    await Promise.all([refreshAll(), refreshGuaranteeOrders(), refreshNotifications()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

function selectAftersaleResponsibility(row: AftersaleView) {
  aftersaleResponsibilityForm.aftersale_id = row.id;
  aftersaleResponsibilityForm.responsibility_party = row.responsibility_party || "supplier";
  aftersaleResponsibilityForm.supplier_compensation_cents = row.supplier_compensation_cents
    ? String(row.supplier_compensation_cents)
    : "";
  aftersaleResponsibilityForm.responsibility_note = row.responsibility_note || row.reason || "";
}

function selectAftersaleAction(row: AftersaleView) {
  aftersaleActionForm.aftersale_id = row.id;
  aftersaleActionForm.shop_id = row.shop_id;
  aftersaleActionForm.address_id = "";
  aftersaleActionForm.accept_type = "";
  aftersaleActionForm.reject_reason_type = aftersaleActionForm.reject_reason_type || "1";
  aftersaleActionForm.reject_reason = row.reason || "";
  aftersaleActionForm.note = row.reason || "";
}

function selectGuaranteeFollowup(row: GuaranteeOrderView) {
  guaranteeFollowupForm.guarantee_order_id = row.id;
  guaranteeFollowupForm.handling_status = row.handling_status || "in_progress";
  guaranteeFollowupForm.responsibility_party = row.responsibility_party || "unknown";
  guaranteeFollowupForm.supplier_compensation_cents = row.supplier_compensation_cents
    ? String(row.supplier_compensation_cents)
    : "";
  guaranteeFollowupForm.handling_note = row.handling_note || row.apply_reason || row.merchant_refuse_reason || "";
}

function selectEvidenceTarget(targetType: "aftersale" | "guarantee", row: AftersaleView | GuaranteeOrderView) {
  evidenceForm.target_type = targetType;
  evidenceForm.target_id = row.id;
  evidenceTargetTypeFilter.value = targetType;
  evidenceTargetIdFilter.value = row.id;
  if (targetType === "guarantee") {
    const guarantee = row as GuaranteeOrderView;
    evidenceForm.title = evidenceForm.title || "纠纷举证资料";
    evidenceForm.content_text = guarantee.apply_reason || guarantee.handling_note || "";
  } else {
    const aftersale = row as AftersaleView;
    evidenceForm.title = evidenceForm.title || "售后处理凭证";
    evidenceForm.content_text = aftersale.reason || aftersale.responsibility_note || "";
  }
  void refreshAftersaleEvidence();
  ElMessage.info("已切换到当前单据的凭证列表");
}

function clearEvidenceTargetFilter() {
  evidenceTargetIdFilter.value = "";
  void refreshAftersaleEvidence();
}

function selectSupplierFollowupTarget(targetType: "aftersale" | "guarantee", row: AftersaleView | GuaranteeOrderView) {
  supplierFollowupForm.target_type = targetType;
  supplierFollowupForm.target_id = row.id;
  supplierFollowupTargetTypeFilter.value = targetType;
  supplierFollowupTargetIdFilter.value = row.id;
  if (targetType === "guarantee") {
    const guarantee = row as GuaranteeOrderView;
    supplierFollowupForm.note = supplierFollowupForm.note || guarantee.handling_note || guarantee.apply_reason || "";
  } else {
    const aftersale = row as AftersaleView;
    supplierFollowupForm.note = supplierFollowupForm.note || aftersale.responsibility_note || aftersale.reason || "";
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
  const reason = aftersaleRejectReasonOptions.value.find((item) => item.reject_reason_type === type)
    || aftersaleRejectReasons.value.find((item) => item.reject_reason_type === type);
  if (!reason) {
    return;
  }
  aftersaleActionForm.reject_reason_type = String(reason.reject_reason_type);
  aftersaleActionForm.reject_reason = reason.reject_reason;
}

async function syncAftersaleRejectReasons() {
  const shopId = aftersaleActionForm.shop_id.trim()
    || aftersales.value[0]?.shop_id
    || shops.value[0]?.id
    || "";
  if (!shopId) {
    ElMessage.warning("先添加店铺或选择一条售后单，再同步拒绝原因");
    return;
  }
  try {
    const result = await command<AftersaleRejectReasonSyncResult>("sync_aftersale_reject_reasons", {
      shopId,
    });
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
  const compensation = guaranteeFollowupForm.supplier_compensation_cents.trim()
    ? Number(guaranteeFollowupForm.supplier_compensation_cents)
    : 0;
  if (!Number.isFinite(compensation) || compensation < 0 || !Number.isInteger(compensation)) {
    ElMessage.warning("供应商赔付金额必须是非负整数，单位分");
    return;
  }
  if (compensation > 0 && guaranteeFollowupForm.responsibility_party !== "supplier") {
    ElMessage.warning("只有供应商责任才能记录纠纷供应商赔付金额");
    return;
  }
  try {
    const result = await command<GuaranteeFollowupResult>("record_guarantee_followup", {
      request: {
        guarantee_order_id: guaranteeFollowupForm.guarantee_order_id.trim(),
        handling_status: guaranteeFollowupForm.handling_status,
        responsibility_party: guaranteeFollowupForm.responsibility_party || null,
        handling_note: guaranteeFollowupForm.handling_note.trim() || null,
        supplier_compensation_cents: compensation,
      },
    });
    ElMessage.success(result.message);
    await Promise.all([refreshGuaranteeOrders(), refreshOrderProfits(), refreshNotifications()]);
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
    !evidenceForm.content_text.trim()
    && !evidenceForm.local_file_path.trim()
    && !evidenceForm.source_url.trim()
  ) {
    ElMessage.warning("凭证说明、本地文件路径和来源链接至少填写一项");
    return;
  }
  try {
    const result = await command<AftersaleEvidenceRecordResult>("record_aftersale_evidence", {
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
    });
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
    const result = await command<SupplierAftersaleFollowupRecordResult>("record_supplier_aftersale_followup", {
      request: {
        target_type: supplierFollowupForm.target_type,
        target_id: supplierFollowupForm.target_id.trim(),
        followup_type: supplierFollowupForm.followup_type,
        status: supplierFollowupForm.status,
        note: supplierFollowupForm.note.trim(),
        purchase_task_id: supplierFollowupForm.purchase_task_id.trim() || null,
        supplier_name: supplierFollowupForm.supplier_name.trim() || null,
      },
    });
    ElMessage.success(result.message);
    supplierFollowupForm.note = "";
    await Promise.all([refreshSupplierAftersaleFollowups(), refreshNotifications()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function updateAftersaleEvidenceStatus(row: AftersaleEvidenceView, status: string) {
  try {
    const result = await command<AftersaleEvidenceStatusUpdateResult>("update_aftersale_evidence_status", {
      request: {
        evidence_id: row.id,
        status,
      },
    });
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
    const result = await command<AftersaleEvidenceExportResult>("export_aftersale_evidence", {
      targetType: targetType ?? (evidenceTargetTypeFilter.value === "all" ? null : evidenceTargetTypeFilter.value),
      targetId: targetId ?? filteredTargetId,
      status: targetId ? null : (evidenceStatusFilter.value === "all" ? null : evidenceStatusFilter.value),
      format: evidenceExportFormat.value,
    });
    evidenceExportPath.value = result.file_path;
    ElMessage.success(`${targetLabel ? `${targetLabel}：` : ""}已导出 ${result.exported_count} 条凭证资料`);
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
  const compensation = aftersaleResponsibilityForm.supplier_compensation_cents.trim()
    ? Number(aftersaleResponsibilityForm.supplier_compensation_cents)
    : 0;
  if (!Number.isFinite(compensation) || compensation < 0 || !Number.isInteger(compensation)) {
    ElMessage.warning("供应商赔付金额必须是非负整数，单位分");
    return;
  }
  if (compensation > 0 && aftersaleResponsibilityForm.responsibility_party !== "supplier") {
    ElMessage.warning("只有供应商责任才能记录供应商赔付金额");
    return;
  }
  try {
    const result = await command<AftersaleResponsibilityResult>("record_aftersale_responsibility", {
      request: {
        aftersale_id: aftersaleResponsibilityForm.aftersale_id.trim(),
        responsibility_party: aftersaleResponsibilityForm.responsibility_party,
        responsibility_note: aftersaleResponsibilityForm.responsibility_note.trim() || null,
        supplier_compensation_cents: compensation,
      },
    });
    ElMessage.success(result.message);
    await Promise.all([refreshAftersales(), refreshOrderProfits(), refreshNotifications()]);
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
  if (acceptType !== null && (![1, 2].includes(acceptType) || !Number.isInteger(acceptType))) {
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
    await Promise.all([refreshAftersales(), refreshNotifications(), refreshDashboardOnly()]);
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
  const selectedReason = aftersaleRejectReasonOptions.value.find((item) => item.reject_reason_type === rejectReasonType);
  try {
    const result = await command<AftersaleActionResult>("reject_aftersale", {
      request: {
        aftersale_id: aftersaleActionForm.aftersale_id.trim(),
        reject_reason_type: rejectReasonType,
        reject_reason: aftersaleActionForm.reject_reason.trim() || selectedReason?.reject_reason || null,
        note: aftersaleActionForm.note.trim() || null,
      },
    });
    if (result.status === "success") {
      ElMessage.success(result.message);
    } else {
      ElMessage.warning(result.message);
    }
    await Promise.all([refreshAftersales(), refreshNotifications(), refreshDashboardOnly()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPurchaseTaskGenerationOnce() {
  try {
    const result = await command<PurchaseTaskBatchResult>("run_purchase_task_generation_once", { limit: 100 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待生成采购任务的订单项");
    } else {
      ElMessage.success(`采购任务生成完成：处理 ${result.processed_items} 项，生成 ${result.created_tasks} 项，跳过 ${result.skipped_items} 项`);
    }
    await refreshAll();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function exportPurchaseTasks() {
  try {
    const result = await command<PurchaseTaskExportResult>("export_purchase_tasks", {
      status: purchaseStatusFilter.value,
    });
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
    const result = await command<SupplierAgentExportResult>("export_supplier_agent_tasks", {
      status: purchaseStatusFilter.value,
      format: supplierAgentExportFormat.value,
    });
    supplierAgentExportPath.value = result.file_path;
    ElMessage.success(`已导出 ${result.exported_count} 条供应商 agent 任务`);
    if (isTauriRuntime) {
      await revealItemInDir(result.file_path);
    }
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function fillSupplierAgentTemplate() {
  try {
    supplierAgentApplyText.value = await command<string>("get_supplier_agent_result_template");
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
    const result = await command<SupplierAgentApplyResult>("apply_supplier_agent_results", {
      request: {
        raw_results: supplierAgentApplyText.value,
        dry_run: supplierAgentDryRun.value,
        continue_on_error: supplierAgentContinueOnError.value,
      },
    });
    supplierAgentApplyResult.value = result;
    if (result.failed > 0) {
      ElMessage.warning(`处理 ${result.processed} 条，失败 ${result.failed} 条`);
    } else if (result.dry_run) {
      ElMessage.success(`干跑通过 ${result.succeeded} 条`);
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
  purchaseMappingForm.estimated_cost = task.estimated_cost !== null && task.estimated_cost !== undefined
    ? String(task.estimated_cost)
    : "";
  purchaseMappingForm.note = task.error_summary || "";
}

async function resolvePurchaseTaskMapping() {
  if (!purchaseMappingForm.purchase_task_id.trim()) {
    ElMessage.warning("先选择或填写采购任务 ID");
    return;
  }
  if (!purchaseMappingForm.external_product_id.trim() || !purchaseMappingForm.external_sku_id.trim()) {
    ElMessage.warning("外部商品 ID 和外部 SKU 必填");
    return;
  }
  const estimatedCost = purchaseMappingForm.estimated_cost.trim()
    ? Number(purchaseMappingForm.estimated_cost)
    : null;
  if (estimatedCost !== null && (!Number.isFinite(estimatedCost) || estimatedCost < 0)) {
    ElMessage.warning("采购成本必须是非负数字");
    return;
  }
  try {
    const result = await command<PurchaseTaskMappingResult>("resolve_purchase_task_mapping", {
      request: {
        purchase_task_id: purchaseMappingForm.purchase_task_id.trim(),
        external_product_id: purchaseMappingForm.external_product_id.trim(),
        external_sku_id: purchaseMappingForm.external_sku_id.trim(),
        source_url: purchaseMappingForm.source_url.trim() || null,
        supplier_name: purchaseMappingForm.supplier_name.trim() || null,
        supplier_product_id: purchaseMappingForm.supplier_product_id.trim() || null,
        estimated_cost: estimatedCost,
        note: purchaseMappingForm.note.trim() || null,
      },
    });
    ElMessage.success(result.message);
    await Promise.all([refreshPurchaseTasks(), refreshNotifications(), refreshDashboardOnly()]);
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
    const result = await command<PurchaseTaskIssueResult>("mark_purchase_task_issue", {
      request: {
        purchase_task_id: purchaseIssueForm.purchase_task_id.trim(),
        issue_type: purchaseIssueForm.issue_type,
        note: purchaseIssueForm.note.trim() || null,
      },
    });
    ElMessage.success(result.message);
    await Promise.all([refreshPurchaseTasks(), refreshNotifications(), refreshDashboardOnly()]);
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
    const result = await command<OrderProfitAdjustmentResult>("record_order_profit_adjustment", {
      request: {
        order_id: orderProfitAdjustmentForm.order_id.trim(),
        kind: orderProfitAdjustmentForm.kind,
        amount_cents: amountCents,
        note: orderProfitAdjustmentForm.note.trim() || null,
      },
    });
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
  if (purchaseShipmentForm.deliver_type === 1 && (!purchaseShipmentForm.delivery_id.trim() || !purchaseShipmentForm.waybill_id.trim())) {
    ElMessage.warning("自寄快递必须填写快递公司和快递单号");
    return;
  }
  const selectedCompany = deliveryCompanyOptions.value.find((item) => item.value === purchaseShipmentForm.delivery_id);
  const estimatedCost = purchaseShipmentForm.estimated_cost.trim()
    ? Number(purchaseShipmentForm.estimated_cost)
    : null;
  if (estimatedCost !== null && (!Number.isFinite(estimatedCost) || estimatedCost < 0)) {
    ElMessage.warning("采购成本必须是非负数字");
    return;
  }
  try {
    const result = await command<PurchaseTaskShipmentResult>("record_purchase_task_shipment", {
      request: {
        purchase_task_id: purchaseShipmentForm.purchase_task_id.trim(),
        delivery_id: purchaseShipmentForm.deliver_type === 1 ? purchaseShipmentForm.delivery_id : null,
        delivery_name: purchaseShipmentForm.deliver_type === 1 ? selectedCompany?.label || null : null,
        waybill_id: purchaseShipmentForm.deliver_type === 1 ? purchaseShipmentForm.waybill_id.trim() : null,
        deliver_type: purchaseShipmentForm.deliver_type,
        estimated_cost: estimatedCost,
      },
    });
    ElMessage.success(result.message);
    await refreshAll();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function setAutoSendDelivery(value: boolean | string | number) {
  try {
    const enabled = Boolean(value);
    deliverySettings.value = await command<DeliverySettings>("set_auto_send_delivery", { enabled });
    ElMessage.success(enabled ? "自动微信发货已开启" : "自动微信发货已关闭");
    await refreshAll();
  } catch (error) {
    deliverySettings.value.auto_send_delivery = !deliverySettings.value.auto_send_delivery;
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
    const result = await command<DeliveryCompanySyncResult>("sync_delivery_companies", {
      shopId,
      ewaybillOnly: false,
    });
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
  const selectedCompany = deliveryCompanyOptions.value.find((item) => item.value === shipmentForm.delivery_id);
  if (!shipmentForm.order_id.trim() && (!shipmentForm.shop_id.trim() || !shipmentForm.wechat_order_id.trim())) {
    ElMessage.warning("填写本地订单 ID，或填写店铺 ID + 微信订单号");
    return;
  }
  if (shipmentForm.deliver_type === 1 && (!shipmentForm.delivery_id.trim() || !shipmentForm.waybill_id.trim())) {
    ElMessage.warning("自寄快递必须填写快递公司和快递单号");
    return;
  }
  try {
    const result = await command<ShipmentRecordResult>("record_order_shipment", {
      request: {
        order_id: shipmentForm.order_id.trim() || null,
        shop_id: shipmentForm.shop_id.trim() || null,
        wechat_order_id: shipmentForm.wechat_order_id.trim() || null,
        delivery_id: shipmentForm.deliver_type === 1 ? shipmentForm.delivery_id : null,
        delivery_name: shipmentForm.deliver_type === 1 ? selectedCompany?.label || null : null,
        waybill_id: shipmentForm.deliver_type === 1 ? shipmentForm.waybill_id.trim() : null,
        deliver_type: shipmentForm.deliver_type,
      },
    });
    ElMessage.success(result.message);
    await refreshAll();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runDeliverySubmissionOnce() {
  try {
    const result = await command<DeliverySubmitBatchResult>("run_delivery_submission_once", { limit: 20 });
    if (result.processed_shipments === 0) {
      ElMessage.info(deliverySettings.value.auto_send_delivery ? "没有待提交微信发货的物流单" : "自动发货开关关闭，未提交微信发货");
    } else {
      ElMessage.success(`微信发货完成：处理 ${result.processed_shipments} 单，成功 ${result.submitted_shipments} 单，失败 ${result.failed_shipments} 单`);
    }
    await refreshAll();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function retryDeliveryShipment(shipment: ShipmentView) {
  try {
    const result = await command<ShipmentRetryResult>("retry_delivery_shipment", {
      shipmentId: shipment.id,
    });
    ElMessage.success(result.message);
    await refreshDeliveryShipments();
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishAssetUploadsOnce() {
  try {
    const result = await command<AssetUploadBatchResult>("run_publish_asset_uploads_once", { limit: 10 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待上传素材的铺货任务项");
    } else {
      ElMessage.success(`素材处理 ${result.processed_items} 项，新上传 ${result.uploaded_assets} 张，复用 ${result.reused_assets} 张，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishSubmitsOnce() {
  try {
    const result = await command<ProductSubmitBatchResult>("run_publish_submits_once", { limit: 10 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待提交 addproduct 的铺货任务项");
    } else {
      ElMessage.success(`发品提交 ${result.processed_items} 项，已提交 ${result.submitted_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishStatusSyncOnce() {
  try {
    const result = await command<ProductStatusSyncBatchResult>("run_publish_status_sync_once", { limit: 20 });
    if (result.processed_items === 0) {
      ElMessage.info("没有待同步审核状态的铺货任务项");
    } else {
      ElMessage.success(`审核同步 ${result.processed_items} 项，通过/上架 ${result.success_items} 项，审核中 ${result.pending_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function runPublishListingOnce() {
  try {
    const result = await command<ProductListingBatchResult>("run_publish_listing_once", { limit: 10 });
    if (result.processed_items === 0) {
      ElMessage.info("没有审核通过待上架的铺货任务项");
    } else {
      ElMessage.success(`上架处理 ${result.processed_items} 项，已提交上架 ${result.listing_submitted_items} 项，失败 ${result.failed_items} 项`);
    }
    await Promise.all([refreshAll(), queryJob()]);
  } catch (error) {
    ElMessage.error(String(error));
  }
}

async function openTask(task: TaskRunView) {
  if (task.task_type.startsWith("aftersales.")) {
    selectedSection.value = "aftersales";
    await Promise.all([refreshAftersales(), refreshGuaranteeOrders()]);
    return;
  }
  if (task.task_type.startsWith("inventory.")) {
    selectedSection.value = "inventory";
    await refreshInventoryRisks();
    return;
  }
  queriedTaskId.value = task.id;
  selectedSection.value = "jobs";
  await queryJob();
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
  return aftersaleResponsibilityOptions.find((item) => item.value === party)?.label || party;
}

function guaranteeHandlingStatusLabel(status: string | null) {
  if (!status) {
    return "未跟进";
  }
  return guaranteeHandlingStatusOptions.find((item) => item.value === status)?.label || status;
}

function evidenceTypeLabel(type: string) {
  return evidenceTypeOptions.find((item) => item.value === type)?.label || type;
}

function evidenceStatusLabel(status: string) {
  return evidenceStatusOptions.find((item) => item.value === status)?.label || status;
}

function supplierFollowupTypeLabel(type: string) {
  return supplierFollowupTypeOptions.find((item) => item.value === type)?.label || type;
}

function supplierFollowupStatusLabel(status: string) {
  return supplierFollowupStatusOptions.find((item) => item.value === status)?.label || status;
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
  return !["STATUS_NO_NEED_PAY", "STATUS_PAY_SUCC", "STATUS_USER_CANCEL", "sync_failed"].includes(status);
}

function aftersaleRejectReasonLabel(reason: AftersaleRejectReasonView) {
  return `${reason.reject_reason_type} · ${reason.reject_reason_type_text}`;
}

async function markNotificationRead(notification: NotificationView) {
  try {
    const result = await command<NotificationMarkResult>("mark_notification_read", {
      notificationId: notification.id,
    });
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
    const result = await command<NotificationMarkResult>("mark_all_notifications_read");
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
    selectedSection.value = "jobs";
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
    selectedSection.value = "inventory";
    inventoryRiskStatusFilter.value = "all";
    await refreshInventoryRisks();
    return;
  }
  if (notification.source_type === "delivery_order" || notification.source_type === "delivery_shipment") {
    selectedSection.value = "delivery";
    deliveryStatusFilter.value = "all";
    await refreshDeliveryShipments();
    return;
  }
  if (
    notification.source_type === "aftersale"
    || notification.source_type === "aftersale_sync"
    || notification.source_type === "aftersale_evidence"
    || notification.source_type === "aftersale_responsibility"
    || notification.source_type === "aftersale_action"
    || notification.source_type === "guarantee_order"
    || notification.source_type === "guarantee_sync"
    || notification.source_type === "guarantee_followup"
  ) {
    selectedSection.value = "aftersales";
    if (notification.source_type === "guarantee_sync") {
      guaranteeStatusFilter.value = "sync_failed";
    } else if (notification.source_type === "guarantee_order" || notification.source_type === "guarantee_followup") {
      guaranteeStatusFilter.value = "active";
    } else {
      aftersaleStatusFilter.value = notification.source_type === "aftersale_sync" ? "sync_failed" : "active";
    }
    await Promise.all([refreshAftersales(), refreshGuaranteeOrders(), refreshAftersaleEvidence()]);
    return;
  }
  selectedSection.value = "tasks";
  await refreshAll();
}

function formatCents(value: number | null) {
  if (value === null || value === undefined) {
    return "-";
  }
  return (value / 100).toFixed(2);
}

function formatUnixTime(value: number | null) {
  if (value === null || value === undefined || value <= 0) {
    return "-";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value * 1000));
}

function formatSignedCents(value: number | null) {
  if (value === null || value === undefined) {
    return "-";
  }
  const sign = value > 0 ? "+" : "";
  return `${sign}${formatCents(value)}`;
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size.toFixed(unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`;
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
  return inventoryRiskStatusOptions.find((item) => item.value === status)?.label || status;
}

function productSalesStatusLabel(status: string) {
  return productSalesStatusOptions.find((item) => item.value === status)?.label || status;
}

onMounted(refreshAll);
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">XD</div>
        <div>
          <strong>微信小店铺货中台</strong>
          <span>{{ runtimeLabel }}</span>
        </div>
      </div>

      <nav class="nav">
        <button :class="{ active: selectedSection === 'overview' }" @click="selectedSection = 'overview'">总览</button>
        <button :class="{ active: selectedSection === 'notifications' }" @click="selectedSection = 'notifications'">通知中心</button>
        <button :class="{ active: selectedSection === 'shops' }" @click="selectedSection = 'shops'">店铺组</button>
        <button :class="{ active: selectedSection === 'publish' }" @click="selectedSection = 'publish'">外部铺货 API</button>
        <button :class="{ active: selectedSection === 'ai' }" @click="selectedSection = 'ai'">AI 设置</button>
        <button :class="{ active: selectedSection === 'catalog' }" @click="selectedSection = 'catalog'">类目规则</button>
        <button :class="{ active: selectedSection === 'price' }" @click="selectedSection = 'price'">批量改价</button>
        <button :class="{ active: selectedSection === 'tasks' }" @click="selectedSection = 'tasks'">任务中心</button>
        <button :class="{ active: selectedSection === 'procurement' }" @click="selectedSection = 'procurement'">采购任务</button>
        <button :class="{ active: selectedSection === 'aftersales' }" @click="selectedSection = 'aftersales'">售后异常</button>
        <button :class="{ active: selectedSection === 'profit' }" @click="selectedSection = 'profit'">利润核算</button>
        <button :class="{ active: selectedSection === 'sales' }" @click="selectedSection = 'sales'">动销分析</button>
        <button :class="{ active: selectedSection === 'inventory' }" @click="selectedSection = 'inventory'">库存风控</button>
        <button :class="{ active: selectedSection === 'delivery' }" @click="selectedSection = 'delivery'">履约发货</button>
        <button :class="{ active: selectedSection === 'jobs' }" @click="selectedSection = 'jobs'">铺货任务</button>
        <button :class="{ active: selectedSection === 'backup' }" @click="selectedSection = 'backup'">数据备份</button>
      </nav>

      <div class="runtime-card">
        <span>本地数据库</span>
        <code>{{ dashboard?.database_path || "初始化中" }}</code>
      </div>
    </aside>

    <main class="workspace" v-loading="loading">
      <header class="topbar">
        <div>
          <p class="eyebrow">Asia/Shanghai</p>
          <h1>待处理订单与铺货任务主控台</h1>
        </div>
        <el-button :icon="Refresh" @click="refreshAll">刷新</el-button>
      </header>

      <section v-if="selectedSection === 'overview'" class="section-grid">
        <div class="metric critical">
          <span>待处理订单</span>
          <strong>{{ dashboard?.pending_order_count ?? 0 }}</strong>
          <em>待采购 / 待发货 / 售后 / 超时</em>
        </div>
        <div class="metric warning">
          <span>异常店铺</span>
          <strong>{{ dashboard?.abnormal_shop_count ?? 0 }}</strong>
          <em>未验证、接口失败或暂停同步</em>
        </div>
        <div class="metric danger">
          <span>铺货失败商品</span>
          <strong>{{ dashboard?.failed_publish_product_count ?? 0 }}</strong>
          <em>按商品聚合查看失败原因</em>
        </div>
        <div class="metric warning">
          <span>未读通知</span>
          <strong>{{ dashboard?.unread_notification_count ?? unreadNotificationCount }}</strong>
          <em>铺货、履约、售后和同步异常</em>
        </div>
        <div class="metric neutral">
          <span>运行中任务</span>
          <strong>{{ dashboard?.running_task_count ?? 0 }}</strong>
          <em>{{ dashboard?.controller_status || "主控机状态未知" }}</em>
        </div>

        <div class="panel wide">
          <div class="panel-title">
            <h2>最近状态</h2>
            <el-tag type="success">{{ dashboard?.now_shanghai || "-" }}</el-tag>
          </div>
          <dl class="status-list">
            <div>
              <dt>最近订单同步</dt>
              <dd>{{ dashboard?.last_order_sync_at || "尚未同步" }}</dd>
            </div>
            <div>
              <dt>最近铺货任务</dt>
              <dd>{{ dashboard?.last_publish_summary || "尚未创建" }}</dd>
            </div>
          </dl>
        </div>
      </section>

      <section v-if="selectedSection === 'notifications'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>通知中心</h2>
              <p>集中展示铺货失败、采购映射缺失、履约发货失败、售后待处理和店铺同步异常。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="notificationStatusFilter"
                class="status-filter"
                @change="refreshNotifications"
              >
                <el-option value="unread" label="未读" />
                <el-option value="all" label="全部状态" />
                <el-option value="read" label="已读" />
              </el-select>
              <el-select
                v-model="notificationSeverityFilter"
                class="status-filter"
                @change="refreshNotifications"
              >
                <el-option value="all" label="全部级别" />
                <el-option value="critical" label="严重" />
                <el-option value="warning" label="提醒" />
                <el-option value="info" label="信息" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshNotifications">刷新</el-button>
              <el-button :icon="CircleCheck" @click="markAllNotificationsRead">全部已读</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>未读通知</dt>
              <dd>{{ unreadNotificationCount }}</dd>
            </div>
            <div>
              <dt>严重未读</dt>
              <dd>{{ criticalNotificationCount }}</dd>
            </div>
            <div>
              <dt>当前筛选</dt>
              <dd>{{ notificationTotal }}</dd>
            </div>
          </dl>

          <el-table :data="notifications" class="dense-table">
            <el-table-column label="级别" width="90">
              <template #default="{ row }">
                <el-tag :type="notificationSeverityType(row.severity)">
                  {{ notificationSeverityLabel(row.severity) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="90">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status === "unread" ? "未读" : "已读" }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="来源" min-width="120">
              <template #default="{ row }">
                {{ notificationSourceLabel(row.source_type) }}
              </template>
            </el-table-column>
            <el-table-column label="店铺" min-width="130">
              <template #default="{ row }">
                {{ row.shop_name || row.shop_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="通知" min-width="380" show-overflow-tooltip>
              <template #default="{ row }">
                <strong class="notification-title">{{ row.title }}</strong>
                <span class="notification-body">{{ row.body }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="updated_at" label="更新时间" min-width="190" />
            <el-table-column label="操作" width="170">
              <template #default="{ row }">
                <el-button size="small" :icon="Bell" @click="openNotification(row)">定位</el-button>
                <el-button
                  v-if="row.status !== 'read'"
                  size="small"
                  :icon="CircleCheck"
                  @click="markNotificationRead(row)"
                >
                  已读
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'backup'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>数据备份</h2>
              <p>备份保存到应用数据目录，创建和恢复都会做 SQLite 完整性校验；恢复前会自动生成回滚备份。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshDatabaseBackups">刷新</el-button>
              <el-button
                type="primary"
                :icon="UploadFilled"
                :loading="backupRunning"
                @click="createDatabaseBackup"
              >
                创建备份
              </el-button>
            </div>
          </div>
          <dl class="status-list compact">
            <div>
              <dt>当前数据库</dt>
              <dd>{{ dashboard?.database_path || "-" }}</dd>
            </div>
            <div>
              <dt>备份数量</dt>
              <dd>{{ databaseBackups.length }}</dd>
            </div>
            <div>
              <dt>最近备份</dt>
              <dd>{{ databaseBackups[0]?.created_at || "尚未备份" }}</dd>
            </div>
          </dl>
          <el-table :data="databaseBackups" class="dense-table">
            <el-table-column prop="file_name" label="备份文件" min-width="270" show-overflow-tooltip />
            <el-table-column label="大小" width="110">
              <template #default="{ row }">
                {{ formatBytes(row.size_bytes) }}
              </template>
            </el-table-column>
            <el-table-column prop="created_at" label="创建时间" min-width="190" />
            <el-table-column label="校验" width="110">
              <template #default="{ row }">
                <el-tag :type="row.integrity_ok ? 'success' : 'danger'">
                  {{ row.integrity_ok ? "通过" : "失败" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="SHA-256" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.sha256 }}
              </template>
            </el-table-column>
            <el-table-column prop="integrity_message" label="校验信息" min-width="150" show-overflow-tooltip />
            <el-table-column label="操作" width="170">
              <template #default="{ row }">
                <el-button size="small" @click="revealBackup(row)">定位</el-button>
                <el-button
                  size="small"
                  type="warning"
                  :disabled="!row.integrity_ok || backupRunning"
                  @click="restoreDatabaseBackup(row)"
                >
                  恢复
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'shops'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <h2>店铺组</h2>
            <el-tag>{{ groups.length }} 组</el-tag>
          </div>
          <div class="inline-form">
            <el-input v-model="groupForm.name" placeholder="例如：默认铺货组" />
            <el-button type="primary" :icon="Plus" @click="createGroup">新建店铺组</el-button>
          </div>
          <el-table :data="groups" class="dense-table">
            <el-table-column prop="name" label="店铺组" />
            <el-table-column prop="shop_count" label="店铺数" width="100" />
            <el-table-column prop="status" label="状态" width="120">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="id" label="ID" min-width="220" />
          </el-table>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>接入店铺</h2>
            <p>保存时 app_secret 只发送到 Tauri 后端加密入库，前端不展示、不回填。</p>
          </div>
          <div class="form-grid">
            <el-input v-model="shopForm.name" placeholder="店铺名称" />
            <el-input v-model="shopForm.appid" placeholder="微信小店 appid" />
            <el-input
              v-model="shopForm.app_secret"
              placeholder="微信小店 app_secret"
              type="password"
              show-password
              autocomplete="new-password"
            />
            <el-select v-model="shopForm.group_id" placeholder="选择店铺组">
              <el-option
                v-for="group in selectedGroupOptions"
                :key="group.value"
                :label="group.label"
                :value="group.value"
              />
            </el-select>
            <el-button type="primary" :icon="Plus" @click="createShop">保存店铺</el-button>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>店铺接入状态</h2>
            <el-tag>{{ shops.length }} 店</el-tag>
          </div>
          <el-table :data="shops" class="dense-table">
            <el-table-column prop="name" label="店铺" min-width="150" />
            <el-table-column prop="appid" label="appid" min-width="170" />
            <el-table-column prop="group_name" label="店铺组" min-width="130" />
            <el-table-column label="微信资料" min-width="170">
              <template #default="{ row }">
                <span>{{ row.wechat_nickname || "-" }}</span>
                <small class="subtext">{{ row.wechat_status || "未同步" }}</small>
              </template>
            </el-table-column>
            <el-table-column prop="status" label="状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="密钥" width="100">
              <template #default="{ row }">
                <el-tag :type="row.has_secret ? 'success' : 'warning'">
                  {{ row.has_secret ? "已保存" : "缺失" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="token_expires_at" label="token 到期" min-width="190">
              <template #default="{ row }">
                {{ row.token_expires_at || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="接口额度" width="110">
              <template #default="{ row }">
                {{ row.last_quota_remain ?? "-" }}
              </template>
            </el-table-column>
            <el-table-column label="操作" width="250">
              <template #default="{ row }">
                <el-button size="small" :disabled="!row.has_secret" @click="verifyShop(row)">
                  验证
                </el-button>
                <el-button size="small" :disabled="!row.has_secret" @click="syncShopBasicInfo(row)">
                  同步资料
                </el-button>
                <el-button size="small" :disabled="!row.has_secret" @click="checkShopQuota(row)">
                  查额度
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'catalog'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>类目规则缓存</h2>
              <p>按店铺同步微信类目树、类目详情、发品规则和运费模板，给批量铺货补齐参数使用。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshCategoryCatalog">刷新</el-button>
              <el-button type="primary" :icon="Refresh" @click="syncSelectedShopCategoryCatalog">同步类目树</el-button>
            </div>
          </div>
          <div class="form-grid compact-form">
            <el-select v-model="selectedCategoryShopId" placeholder="选择店铺" @change="refreshCategoryCatalog">
              <el-option
                v-for="shop in shops"
                :key="shop.id"
                :label="shop.name"
                :value="shop.id"
              />
            </el-select>
            <el-input
              v-model="categoryKeyword"
              placeholder="搜索类目名称或 cat_id"
              clearable
              @change="refreshCategoryCatalog"
            />
            <el-input v-model="categoryRuleCatId" placeholder="同步规则 cat_id" />
            <el-button :icon="Refresh" @click="syncSelectedCategoryRules()">同步选中类目规则</el-button>
          </div>
          <dl class="status-list compact">
            <div v-for="summary in categoryCatalogShops" :key="summary.shop_id">
              <dt>{{ summary.shop_name }}</dt>
              <dd>
                类目 {{ summary.category_count }} / 详情 {{ summary.detail_count }} / 发品规则 {{ summary.product_rule_count }} / 运费模板 {{ summary.freight_template_count }}
              </dd>
            </div>
          </dl>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>类目列表</h2>
            <el-tag>{{ categoryCache.length }} 条</el-tag>
          </div>
          <el-table :data="categoryCache" class="dense-table">
            <el-table-column prop="name" label="类目" min-width="160" />
            <el-table-column prop="cat_id" label="cat_id" min-width="130" />
            <el-table-column prop="parent_cat_id" label="父类目" min-width="120">
              <template #default="{ row }">
                {{ row.parent_cat_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="level" label="层级" width="80" />
            <el-table-column label="详情" width="100">
              <template #default="{ row }">
                <el-tag :type="row.has_detail ? 'success' : 'warning'">
                  {{ row.has_detail ? "已同步" : "未同步" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="属性/资质" min-width="160">
              <template #default="{ row }">
                <span>{{ row.product_attr_count }} 商品 / {{ row.sale_attr_count }} 销售 / {{ row.product_qua_count }} 资质</span>
              </template>
            </el-table-column>
            <el-table-column label="规则" min-width="150">
              <template #default="{ row }">
                <el-tag :type="row.has_product_rule ? 'success' : 'warning'">发品</el-tag>
                <el-tag :type="row.has_delivery_rule ? 'success' : 'warning'">发货</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="130">
              <template #default="{ row }">
                <el-button size="small" @click="syncSelectedCategoryRules(row.cat_id)">同步规则</el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>运费模板</h2>
            <el-tag>{{ freightTemplates.length }} 个</el-tag>
          </div>
          <el-table :data="freightTemplates" class="dense-table">
            <el-table-column prop="shop_name" label="店铺" min-width="150" />
            <el-table-column prop="template_id" label="模板 ID" min-width="180" />
            <el-table-column prop="synced_at" label="同步时间" min-width="190" />
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'publish'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>本地主控 HTTP API</h2>
              <p>外部系统调用本机地址创建铺货任务、改价任务和查询状态。</p>
            </div>
            <el-tag :type="localApiConfig.has_api_key ? 'success' : 'warning'">
              {{ localApiConfig.has_api_key ? "已生成 Key" : "未生成 Key" }}
            </el-tag>
          </div>
          <dl class="status-list compact">
            <div>
              <dt>Base URL</dt>
              <dd>{{ localApiConfig.base_url }}</dd>
            </div>
            <div>
              <dt>认证头</dt>
              <dd>{{ localApiConfig.auth_header }}</dd>
            </div>
            <div>
              <dt>Key 状态</dt>
              <dd>{{ localApiConfig.api_key_hint || "未生成" }}</dd>
            </div>
          </dl>
          <div class="api-endpoints">
            <code>GET /api/shop-groups</code>
            <code>POST /api/publish-jobs</code>
            <code>GET /api/publish-jobs/:task_id</code>
            <code>POST /api/price-update-jobs</code>
            <code>GET /api/price-update-jobs/:task_id</code>
            <code>GET /api/task-runs</code>
            <code>POST /api/runners/price-submit</code>
            <code>POST /api/runners/publish-ai-attributes</code>
            <code>POST /api/runners/operations</code>
          </div>
          <div class="action-row">
            <el-button type="primary" @click="rotateLocalApiKey">
              {{ localApiConfig.has_api_key ? "重置 API Key" : "生成 API Key" }}
            </el-button>
            <el-tag type="info">只监听 127.0.0.1:{{ localApiConfig.port }}</el-tag>
          </div>
          <el-alert
            v-if="rotatedLocalApiKey"
            type="warning"
            :closable="false"
            show-icon
            title="API Key 只展示这一次，外部系统请保存后使用。"
          >
            <template #default>
              <el-input v-model="rotatedLocalApiKey" readonly />
            </template>
          </el-alert>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>外部 API 审计</h2>
              <p>只记录调用方法、路径、状态、耗时和安全摘要，不保存 API Key 或完整请求体。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshExternalApiLogs">刷新</el-button>
              <el-tag>{{ externalApiLogs.length }} 条</el-tag>
            </div>
          </div>
          <el-table :data="externalApiLogs" class="dense-table">
            <el-table-column prop="created_at" label="时间" min-width="190" />
            <el-table-column prop="method" label="方法" width="90" />
            <el-table-column prop="path" label="路径" min-width="230" show-overflow-tooltip />
            <el-table-column label="状态" width="120">
              <template #default="{ row }">
                <el-tag :type="row.status === 'success' ? 'success' : 'danger'">
                  {{ row.status_code }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="耗时" width="100">
              <template #default="{ row }">
                {{ row.duration_ms }} ms
              </template>
            </el-table-column>
            <el-table-column prop="request_summary" label="请求摘要" min-width="260" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.request_summary || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="response_summary" label="响应摘要" min-width="150" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.response_summary || row.error_code || "-" }}
              </template>
            </el-table-column>
          </el-table>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>创建外部铺货任务</h2>
            <p>外部系统可直接按同一 JSON 协议调用本机 HTTP API，桌面端保留手工创建入口。</p>
          </div>
          <el-input
            v-model="publishPayload"
            type="textarea"
            :autosize="{ minRows: 18, maxRows: 28 }"
            spellcheck="false"
            class="json-editor"
          />
          <div class="action-row">
            <el-button type="primary" :icon="UploadFilled" @click="createPublishJob">创建铺货任务</el-button>
            <el-tag v-if="latestTaskId">最新任务：{{ latestTaskId }}</el-tag>
          </div>
        </div>

      </section>

      <section v-if="selectedSection === 'ai'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>AI Provider</h2>
              <p>用于给微信类目必填属性生成候选值；默认关闭，API Key 只加密保存在本机后端。</p>
            </div>
            <el-tag :type="aiProviderSettings.enabled ? 'success' : 'info'">
              {{ aiProviderSettings.enabled ? "已启用" : "已关闭" }}
            </el-tag>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>模型</dt>
              <dd>{{ aiProviderSettings.model || "未配置" }}</dd>
            </div>
            <div>
              <dt>密钥</dt>
              <dd>{{ aiProviderSettings.has_api_key ? (aiProviderSettings.api_key_hint || "已保存") : "未保存" }}</dd>
            </div>
            <div>
              <dt>更新时间</dt>
              <dd>{{ aiProviderSettings.updated_at || "尚未保存" }}</dd>
            </div>
          </dl>

          <div class="ai-settings-form">
            <el-switch
              v-model="aiProviderForm.enabled"
              active-text="启用 AI 属性建议"
              inactive-text="关闭"
            />
            <el-select v-model="aiProviderForm.provider_type" placeholder="Provider">
              <el-option value="openai_compatible" label="OpenAI Compatible" />
            </el-select>
            <el-input v-model="aiProviderForm.base_url" placeholder="Base URL，例如 https://api.openai.com/v1" />
            <el-input v-model="aiProviderForm.model" placeholder="模型，例如 gpt-4.1-mini" />
            <el-input v-model="aiProviderForm.temperature" placeholder="temperature，0 到 1" />
            <el-input
              v-model="aiProviderForm.api_key"
              type="password"
              show-password
              autocomplete="new-password"
              :placeholder="aiProviderSettings.has_api_key ? '留空则继续使用已保存 Key' : 'API Key'"
            />
          </div>

          <div class="action-row ai-actions">
            <el-checkbox
              v-if="aiProviderSettings.has_api_key"
              v-model="aiProviderForm.clear_api_key"
            >
              清除已保存 API Key
            </el-checkbox>
            <span v-else></span>
            <div class="button-group">
              <el-button
                :icon="Refresh"
                :loading="aiProviderTesting"
                :disabled="!aiProviderSettings.enabled || !aiProviderSettings.has_api_key"
                @click="testAiProvider"
              >
                试连
              </el-button>
              <el-button
                type="primary"
                :loading="aiProviderSaving"
                @click="saveAiProviderSettings"
              >
                保存
              </el-button>
            </div>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>属性建议任务</h2>
              <p>从类目预检失败项读取缺失属性和商品资料，请 AI 生成候选值，再交给本地规则校验是否能自动补齐。</p>
            </div>
            <el-button :icon="Refresh" @click="runPublishAiAttributeSuggestionsOnce">生成建议</el-button>
          </div>
          <dl class="status-list">
            <div>
              <dt>输入来源</dt>
              <dd>CATEGORY_ATTRS_NEED_AI_FILL 失败项</dd>
            </div>
            <div>
              <dt>输出位置</dt>
              <dd>publish_attribute_suggestions.prompt_json / suggestion_json</dd>
            </div>
          </dl>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>属性建议确认</h2>
              <p>低置信建议不会自动写回，人工采纳后只回到待类目预检状态，后续仍按正常铺货状态机推进。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="attributeSuggestionStatusFilter"
                class="status-filter"
                @change="refreshAttributeSuggestions"
              >
                <el-option value="pending" label="待确认" />
                <el-option value="all" label="全部建议" />
                <el-option value="applied" label="已采纳" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshAttributeSuggestions">刷新</el-button>
              <el-button
                type="primary"
                :loading="attributeSuggestionApplying"
                :disabled="selectedAttributeSuggestions.filter((row) => !row.applied).length === 0"
                @click="applySelectedAttributeSuggestions"
              >
                批量采纳
              </el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>待确认</dt>
              <dd>{{ pendingAttributeSuggestionCount }}</dd>
            </div>
            <div>
              <dt>已采纳</dt>
              <dd>{{ appliedAttributeSuggestionCount }}</dd>
            </div>
            <div>
              <dt>当前筛选</dt>
              <dd>{{ attributeSuggestionTotal }}</dd>
            </div>
          </dl>

          <el-table
            :data="attributeSuggestions"
            class="dense-table"
            @selection-change="handleAttributeSuggestionSelection"
          >
            <el-table-column type="selection" width="48" />
            <el-table-column label="状态" width="100">
              <template #default="{ row }">
                <el-tag :type="row.applied ? 'success' : 'warning'">
                  {{ row.applied ? "已采纳" : "待确认" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="120" />
            <el-table-column label="商品" min-width="240" show-overflow-tooltip>
              <template #default="{ row }">
                <span>{{ row.title }}</span>
                <small class="subtext">{{ row.external_product_id }}</small>
              </template>
            </el-table-column>
            <el-table-column label="属性" min-width="150">
              <template #default="{ row }">
                <span>{{ row.attr_key }}</span>
                <small class="subtext">{{ attributeKindLabel(row.attr_kind) }}</small>
              </template>
            </el-table-column>
            <el-table-column label="建议值" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                {{ attributeSuggestionValue(row) }}
              </template>
            </el-table-column>
            <el-table-column label="允许值" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                {{ allowedValuesText(row.allowed_values) }}
              </template>
            </el-table-column>
            <el-table-column label="来源/置信" min-width="160">
              <template #default="{ row }">
                <span>{{ row.source }}</span>
                <small class="subtext">confidence {{ row.confidence }}</small>
              </template>
            </el-table-column>
            <el-table-column prop="reason" label="原因" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.reason || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="updated_at" label="更新时间" min-width="190" />
            <el-table-column label="操作" width="100">
              <template #default="{ row }">
                <el-button
                  size="small"
                  :disabled="row.applied || attributeSuggestionApplying"
                  @click="applySingleAttributeSuggestion(row)"
                >
                  采纳
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'price'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>创建批量改价任务</h2>
              <p>先做本地可改价校验，再由队列获取微信商品详情并提交 updateproduct。</p>
            </div>
          </div>
          <el-input
            v-model="priceUpdatePayload"
            type="textarea"
            :autosize="{ minRows: 12, maxRows: 20 }"
            spellcheck="false"
            class="json-editor"
          />
          <div class="action-row">
            <el-button type="primary" :icon="UploadFilled" @click="createPriceUpdateJob">创建改价任务</el-button>
            <el-button :icon="Refresh" @click="runPriceUpdatePrecheckOnce">执行改价校验</el-button>
            <el-button :icon="UploadFilled" @click="runPriceUpdateSubmitOnce">提交微信改价</el-button>
            <el-button :icon="Refresh" @click="runPriceUpdateConfirmOnce">确认改价结果</el-button>
            <el-tag v-if="latestPriceTaskId">最新任务：{{ latestPriceTaskId }}</el-tag>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>查询改价任务</h2>
            <p>改价任务按店铺商品维度展开，能看到缺商品、缺微信商品 ID、店铺未激活等原因。</p>
          </div>
          <div class="inline-form">
            <el-input v-model="queriedPriceTaskId" placeholder="price_xxx" />
            <el-button type="primary" :icon="Search" @click="queryPriceUpdateJob">查询</el-button>
          </div>
        </div>

        <div v-if="currentPriceJob" class="panel">
          <div class="panel-title">
            <h2>{{ currentPriceJob.id }}</h2>
            <el-tag :type="statusType(currentPriceJob.status)">{{ currentPriceJob.status }}</el-tag>
          </div>
          <dl class="status-list compact">
            <div>
              <dt>request_id</dt>
              <dd>{{ currentPriceJob.request_id }}</dd>
            </div>
            <div>
              <dt>商品数</dt>
              <dd>{{ currentPriceJob.accepted_product_count }}</dd>
            </div>
            <div>
              <dt>目标店铺</dt>
              <dd>{{ currentPriceJob.target_shop_count }}</dd>
            </div>
          </dl>
          <el-table :data="currentPriceJob.items" class="dense-table">
            <el-table-column prop="external_product_id" label="外部商品 ID" min-width="180" />
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column label="目标售价" width="110">
              <template #default="{ row }">
                {{ formatCents(row.target_price_cents) }}
              </template>
            </el-table-column>
            <el-table-column prop="status" label="状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="wechat_product_id" label="微信商品 ID" min-width="160">
              <template #default="{ row }">
                {{ row.wechat_product_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="error_summary" label="处理原因" min-width="260" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.error_summary || row.error_code || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="updated_at" label="更新时间" min-width="190" />
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'tasks'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>任务中心</h2>
              <p>前置校验会生成或验证微信发品参数草稿，通过后进入素材上传和真实发品；失败原因保留到商品任务。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="runOrderSyncOnce">同步待发货订单</el-button>
              <el-button :icon="Refresh" @click="runOrderDetailSyncOnce">同步订单详情</el-button>
              <el-button :icon="Refresh" @click="runAftersaleSyncOnce">同步售后</el-button>
              <el-button :icon="Refresh" @click="runGuaranteeSyncOnce">同步纠纷单</el-button>
              <el-button :icon="Refresh" @click="runPurchaseTaskGenerationOnce">生成采购任务</el-button>
              <el-button type="primary" :icon="Refresh" @click="runPublishTasksOnce">执行前置校验</el-button>
              <el-button :icon="Refresh" @click="runPublishAttributeFillOnce">补齐必填属性</el-button>
              <el-button :icon="Refresh" @click="runPublishAiAttributeSuggestionsOnce">AI 生成属性建议</el-button>
              <el-button :icon="Refresh" @click="runPublishCategoryPrechecksOnce">微信类目预检</el-button>
              <el-button :icon="Refresh" @click="runPriceUpdatePrecheckOnce">改价校验</el-button>
              <el-button :icon="UploadFilled" @click="runPriceUpdateSubmitOnce">提交改价</el-button>
              <el-button :icon="Refresh" @click="runPriceUpdateConfirmOnce">确认改价</el-button>
              <el-button :icon="UploadFilled" @click="runPublishAssetUploadsOnce">上传微信素材</el-button>
              <el-button :icon="UploadFilled" @click="runPublishSubmitsOnce">提交微信发品</el-button>
              <el-button :icon="Refresh" @click="runPublishStatusSyncOnce">同步审核状态</el-button>
              <el-button :icon="UploadFilled" @click="runPublishListingOnce">上架通过商品</el-button>
            </div>
          </div>
          <div class="sub-panel automation-panel">
            <div class="automation-head">
              <div>
                <h3>自动推进</h3>
                <p>按开关顺序推进订单同步、采购任务、发货、铺货发布链路和改价确认；单步失败会保留错误并继续后续步骤。</p>
              </div>
              <el-button
                type="primary"
                :icon="Refresh"
                :loading="automationRunning"
                @click="runOperationalAutomationOnce"
              >
                自动推进一轮
              </el-button>
            </div>
            <div class="automation-switches">
              <el-switch
                v-model="automationSettings.order_sync_enabled"
                active-text="同步订单"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.order_detail_sync_enabled"
                active-text="同步详情"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.aftersale_sync_enabled"
                active-text="售后同步"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.purchase_task_enabled"
                active-text="采购任务"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.delivery_submission_enabled"
                active-text="微信发货"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_precheck_enabled"
                active-text="铺货校验"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_attribute_fill_enabled"
                active-text="属性补齐"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_category_precheck_enabled"
                active-text="类目预检"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_asset_upload_enabled"
                active-text="素材上传"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_submit_enabled"
                active-text="提交发品"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_status_sync_enabled"
                active-text="铺货状态"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.publish_listing_enabled"
                active-text="自动上架"
                @change="saveAutomationSettings"
              />
              <el-switch
                v-model="automationSettings.price_confirm_enabled"
                active-text="确认改价"
                @change="saveAutomationSettings"
              />
            </div>
            <div v-if="lastAutomationResult" class="automation-result">
              <el-tag type="success">执行 {{ lastAutomationResult.executed_steps.length }}</el-tag>
              <el-tag type="info">跳过 {{ lastAutomationResult.skipped_steps.length }}</el-tag>
              <el-tag :type="lastAutomationResult.errors.length > 0 ? 'danger' : 'success'">
                失败 {{ lastAutomationResult.errors.length }}
              </el-tag>
              <span v-if="lastAutomationResult.errors.length > 0">
                {{ lastAutomationResult.errors.map((item) => `${item.step}: ${item.error}`).join("；") }}
              </span>
            </div>
          </div>
          <el-table :data="taskRuns" class="dense-table">
            <el-table-column prop="id" label="任务 ID" min-width="260" />
            <el-table-column prop="task_type" label="类型" min-width="210" />
            <el-table-column prop="status" label="状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="进度" width="190">
              <template #default="{ row }">
                <el-progress :percentage="row.progress" :stroke-width="8" />
              </template>
            </el-table-column>
            <el-table-column label="任务项" min-width="180">
              <template #default="{ row }">
                <span>待 {{ row.pending_count }}</span>
                <span class="split-stat">就绪 {{ row.ready_count }}</span>
                <span class="split-stat">失败 {{ row.failed_count }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="created_at" label="创建时间" min-width="190" />
            <el-table-column label="操作" width="110">
              <template #default="{ row }">
                <el-button size="small" @click="openTask(row)">查看</el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'delivery'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>人工物流回填</h2>
              <p>物流单号先人工录入；自动发货开关关闭时，只保存为待确认发货。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="syncDeliveryCompanies">同步快递公司</el-button>
              <el-switch
                v-model="deliverySettings.auto_send_delivery"
                active-text="自动发货"
                inactive-text="仅回填"
                @change="setAutoSendDelivery"
              />
            </div>
          </div>
          <div class="form-grid">
            <el-input v-model="shipmentForm.order_id" placeholder="本地订单 ID，可选" />
            <el-input v-model="shipmentForm.shop_id" placeholder="店铺 ID，未填订单 ID 时必填" />
            <el-input v-model="shipmentForm.wechat_order_id" placeholder="微信订单号，未填订单 ID 时必填" />
            <el-select v-model="shipmentForm.deliver_type" placeholder="发货方式">
              <el-option :value="1" label="自寄快递" />
              <el-option :value="3" label="虚拟无需物流" />
            </el-select>
            <el-select
              v-model="shipmentForm.delivery_id"
              :disabled="shipmentForm.deliver_type !== 1"
              placeholder="快递公司"
            >
              <el-option
                v-for="company in deliveryCompanyOptions"
                :key="company.value"
                :label="company.label"
                :value="company.value"
              />
            </el-select>
            <el-input
              v-model="shipmentForm.waybill_id"
              :disabled="shipmentForm.deliver_type !== 1"
              placeholder="快递单号"
            />
            <el-button type="primary" :icon="UploadFilled" @click="recordOrderShipment">保存物流</el-button>
            <el-button :icon="Refresh" @click="runDeliverySubmissionOnce">提交微信发货</el-button>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>发货队列</h2>
              <p>失败单可以重新放回待提交队列；真正调用微信仍由队列任务执行。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="deliveryStatusFilter"
                class="status-filter"
                @change="refreshDeliveryShipments"
              >
                <el-option value="all" label="全部状态" />
                <el-option value="waiting_confirmation" label="待确认" />
                <el-option value="ready_to_send" label="待提交" />
                <el-option value="send_failed" label="发货失败" />
                <el-option value="wechat_shipped" label="已发货" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshDeliveryShipments">刷新</el-button>
              <el-tag>{{ deliveryShipments.length }} / {{ deliveryShipmentTotal }}</el-tag>
            </div>
          </div>
          <el-table :data="deliveryShipments" class="dense-table">
            <el-table-column prop="id" label="物流单 ID" min-width="230" />
            <el-table-column prop="status" label="状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170" />
            <el-table-column label="物流" min-width="190">
              <template #default="{ row }">
                <span>{{ row.waybill_id || "-" }}</span>
                <small v-if="row.delivery_name || row.delivery_id" class="subtext">
                  {{ row.delivery_name || row.delivery_id }}
                </small>
              </template>
            </el-table-column>
            <el-table-column prop="error_summary" label="失败原因" min-width="260" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.error_summary || row.error_code || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="updated_at" label="更新时间" min-width="190" />
            <el-table-column label="操作" width="110">
              <template #default="{ row }">
                <el-button
                  size="small"
                  :disabled="row.status === 'wechat_shipped'"
                  @click="retryDeliveryShipment(row)"
                >
                  重试
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'procurement'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>采购任务</h2>
              <p>首版采购以人工处理为主；导出表不包含收件人姓名、手机号、地址。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="purchaseStatusFilter"
                class="status-filter"
                @change="refreshPurchaseTasks"
              >
                <el-option value="all" label="全部状态" />
                <el-option value="pending_purchase" label="待采购" />
                <el-option value="needs_mapping" label="待映射" />
                <el-option value="supplier_out_of_stock" label="供应商缺货" />
                <el-option value="supplier_price_changed" label="供应商涨价" />
                <el-option value="supplier_quality_risk" label="质量风险" />
                <el-option value="supplier_shipped" label="供应商已发货" />
                <el-option value="send_failed" label="发货失败" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshPurchaseTasks">刷新</el-button>
              <el-button type="primary" :icon="UploadFilled" @click="exportPurchaseTasks">导出采购表</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>当前筛选</dt>
              <dd>{{ purchaseStatusFilter === "all" ? "全部状态" : purchaseStatusFilter }}</dd>
            </div>
            <div>
              <dt>显示任务</dt>
              <dd>{{ purchaseTasks.length }}</dd>
            </div>
            <div>
              <dt>匹配总数</dt>
              <dd>{{ purchaseTaskTotal }}</dd>
            </div>
          </dl>

          <div class="sub-panel supplier-agent-panel">
            <div class="panel-title tight">
              <div>
                <h2>供应商 Agent 桥</h2>
                <p>导出非敏采购任务，粘贴外部 agent 返回的物流、异常或映射结果；收件信息、密钥、Cookie 和未知字段会被后端拒绝。</p>
              </div>
              <div class="button-group">
                <el-select v-model="supplierAgentExportFormat" class="status-filter">
                  <el-option value="jsonl" label="JSONL" />
                  <el-option value="json" label="JSON" />
                  <el-option value="md" label="Markdown" />
                </el-select>
                <el-button type="primary" :icon="UploadFilled" @click="exportSupplierAgentTasks">导出 Agent 任务</el-button>
                <el-button :icon="Search" @click="fillSupplierAgentTemplate">填入模板</el-button>
              </div>
            </div>
            <div class="supplier-agent-grid">
              <el-input
                v-model="supplierAgentApplyText"
                type="textarea"
                :rows="7"
                class="json-editor"
                placeholder='每行一条 JSON：{"purchase_task_id":"...","action":"shipment","delivery_id":"SF","waybill_id":"..."}'
              />
              <div class="supplier-agent-actions">
                <el-switch v-model="supplierAgentDryRun" active-text="干跑校验" inactive-text="直接写回" />
                <el-switch v-model="supplierAgentContinueOnError" active-text="遇错继续" inactive-text="遇错停止" />
                <el-button type="primary" :icon="CircleCheck" @click="applySupplierAgentResults">
                  {{ supplierAgentDryRun ? "校验结果" : "写回结果" }}
                </el-button>
              </div>
            </div>
            <div v-if="supplierAgentExportPath" class="action-row">
              <el-tag>最近 Agent 导出：{{ supplierAgentExportPath }}</el-tag>
            </div>
            <div v-if="supplierAgentApplyResult" class="supplier-agent-result">
              <el-tag :type="supplierAgentApplyResult.failed > 0 ? 'warning' : 'success'">
                处理 {{ supplierAgentApplyResult.processed }} 条，成功 {{ supplierAgentApplyResult.succeeded }} 条，失败 {{ supplierAgentApplyResult.failed }} 条
              </el-tag>
              <el-table :data="supplierAgentApplyResult.results" class="dense-table compact-table">
                <el-table-column prop="index" label="#" width="70" />
                <el-table-column prop="purchase_task_id" label="采购任务 ID" min-width="180" />
                <el-table-column prop="action" label="动作" width="110" />
                <el-table-column prop="status" label="状态" width="110">
                  <template #default="{ row }">
                    <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="error" label="错误" min-width="260" show-overflow-tooltip>
                  <template #default="{ row }">
                    {{ row.error || "-" }}
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>补齐商品映射</h2>
                <p>从采购任务行点“补映射”带入任务 ID；只补外部商品和供应商字段，不读取或写入收件人信息。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="purchaseMappingForm.purchase_task_id" placeholder="采购任务 ID" />
              <el-input v-model="purchaseMappingForm.external_product_id" placeholder="外部商品 ID" />
              <el-input v-model="purchaseMappingForm.external_sku_id" placeholder="外部 SKU" />
              <el-input v-model="purchaseMappingForm.source_url" placeholder="货源链接，可选" />
              <el-input v-model="purchaseMappingForm.supplier_name" placeholder="供应商，可选" />
              <el-input v-model="purchaseMappingForm.supplier_product_id" placeholder="供应商商品 ID，可选" />
              <el-input v-model="purchaseMappingForm.estimated_cost" placeholder="采购成本，可选" />
              <el-input v-model="purchaseMappingForm.note" placeholder="映射备注，可选" />
              <el-button type="primary" :icon="CircleCheck" @click="resolvePurchaseTaskMapping">补齐映射</el-button>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>供应商物流回填</h2>
                <p>从采购任务行点“回填”带入任务 ID；同一订单全部采购任务回填后才会进入微信发货队列。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="purchaseShipmentForm.purchase_task_id" placeholder="采购任务 ID" />
              <el-select v-model="purchaseShipmentForm.deliver_type" placeholder="发货方式">
                <el-option :value="1" label="自寄快递" />
                <el-option :value="3" label="虚拟无需物流" />
              </el-select>
              <el-select
                v-model="purchaseShipmentForm.delivery_id"
                :disabled="purchaseShipmentForm.deliver_type !== 1"
                placeholder="快递公司"
              >
                <el-option
                  v-for="company in deliveryCompanyOptions"
                  :key="company.value"
                  :label="company.label"
                  :value="company.value"
                />
              </el-select>
              <el-input
                v-model="purchaseShipmentForm.waybill_id"
                :disabled="purchaseShipmentForm.deliver_type !== 1"
                placeholder="供应商物流单号"
              />
              <el-input v-model="purchaseShipmentForm.estimated_cost" placeholder="采购成本，可选" />
              <el-button type="primary" :icon="UploadFilled" @click="recordPurchaseTaskShipment">回填物流</el-button>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>供应商异常</h2>
                <p>缺货、涨价、取消或质量风险只进入人工处理，不自动换供应商或取消订单。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="purchaseIssueForm.purchase_task_id" placeholder="采购任务 ID" />
              <el-select v-model="purchaseIssueForm.issue_type" placeholder="异常类型">
                <el-option
                  v-for="issue in purchaseIssueTypeOptions"
                  :key="issue.value"
                  :label="issue.label"
                  :value="issue.value"
                />
              </el-select>
              <el-input v-model="purchaseIssueForm.note" placeholder="处理备注，可选" />
              <el-button type="warning" :icon="Bell" @click="markPurchaseTaskIssue">标记异常</el-button>
            </div>
          </div>

          <el-table :data="purchaseTasks" class="dense-table">
            <el-table-column prop="id" label="采购任务 ID" min-width="230" />
            <el-table-column prop="status" label="状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170" />
            <el-table-column prop="title" label="商品" min-width="220" show-overflow-tooltip />
            <el-table-column prop="external_product_id" label="外部商品 ID" min-width="170">
              <template #default="{ row }">
                {{ row.external_product_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="source_url" label="货源链接" min-width="130" show-overflow-tooltip>
              <template #default="{ row }">
                <a
                  v-if="row.source_url"
                  class="inline-link"
                  :href="row.source_url"
                  target="_blank"
                  rel="noreferrer"
                >
                  打开货源
                </a>
                <span v-else>-</span>
              </template>
            </el-table-column>
            <el-table-column prop="external_sku_id" label="外部 SKU" min-width="140">
              <template #default="{ row }">
                {{ row.external_sku_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="supplier_name" label="供应商" min-width="150" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.supplier_name || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="quantity" label="数量" width="80" />
            <el-table-column label="成交金额" width="110">
              <template #default="{ row }">
                {{ formatCents(row.estimated_revenue) }}
              </template>
            </el-table-column>
            <el-table-column label="供应商物流" min-width="190">
              <template #default="{ row }">
                <span>{{ row.supplier_waybill_id || "-" }}</span>
                <small v-if="row.supplier_delivery_name || row.supplier_delivery_id" class="subtext">
                  {{ row.supplier_delivery_name || row.supplier_delivery_id }}
                </small>
              </template>
            </el-table-column>
            <el-table-column prop="error_summary" label="处理原因" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.error_summary || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="created_at" label="创建时间" min-width="190" />
            <el-table-column label="操作" width="220" fixed="right">
              <template #default="{ row }">
                <el-button size="small" @click="selectPurchaseTaskMapping(row)">补映射</el-button>
                <el-button size="small" @click="selectPurchaseTaskShipment(row)">回填</el-button>
                <el-button size="small" type="warning" @click="selectPurchaseTaskIssue(row)">异常</el-button>
              </template>
            </el-table-column>
          </el-table>

          <div v-if="purchaseExportPath" class="action-row">
            <el-tag>最近导出：{{ purchaseExportPath }}</el-tag>
          </div>
        </div>
      </section>

      <section v-if="selectedSection === 'aftersales'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>售后异常处理台</h2>
              <p>同步微信售后列表和详情，保留状态、退款金额、关联订单和失败原因；支持人工责任归因、供应商赔付回款和人工触发的同意/拒绝动作。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="aftersaleStatusFilter"
                class="status-filter"
                @change="refreshAftersales"
              >
                <el-option value="active" label="处理中" />
                <el-option value="all" label="全部状态" />
                <el-option value="sync_failed" label="同步失败" />
                <el-option value="MERCHANT_PROCESSING" label="待商家处理" />
                <el-option value="MERCHANT_REFUND_SUCCESS" label="退款成功" />
                <el-option value="MERCHANT_RETURN_SUCCESS" label="退货退款成功" />
                <el-option value="USER_CANCELD" label="用户取消" />
                <el-option value="RETURN_CLOSED" label="退货关闭" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshAftersales">刷新</el-button>
              <el-button type="primary" :icon="Refresh" @click="runAftersaleSyncOnce">同步售后</el-button>
              <el-button :icon="Refresh" @click="runGuaranteeSyncOnce">同步纠纷单</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>当前筛选</dt>
              <dd>{{ aftersaleStatusFilter === "all" ? "全部状态" : aftersaleStatusFilter }}</dd>
            </div>
            <div>
              <dt>显示售后</dt>
              <dd>{{ aftersales.length }}</dd>
            </div>
            <div>
              <dt>匹配总数</dt>
              <dd>{{ aftersaleTotal }}</dd>
            </div>
            <div>
              <dt>纠纷单</dt>
              <dd>{{ guaranteeOrders.length }} / {{ guaranteeOrderTotal }}</dd>
            </div>
            <div>
              <dt>本地凭证</dt>
              <dd>{{ aftersaleEvidence.length }} / {{ aftersaleEvidenceTotal }}</dd>
            </div>
          </dl>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>本地凭证资料包</h2>
                <p>只整理本地资料摘要、文件路径或来源链接；不会读取文件内容、上传微信或提交平台处理。</p>
              </div>
              <div class="button-group">
                <el-select
                  v-model="evidenceTargetTypeFilter"
                  class="status-filter"
                  @change="refreshAftersaleEvidence"
                >
                  <el-option value="all" label="全部目标" />
                  <el-option
                    v-for="item in evidenceTargetTypeOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-input
                  v-model="evidenceTargetIdFilter"
                  class="status-filter evidence-target-filter"
                  placeholder="单据 ID/单号"
                  clearable
                  @change="refreshAftersaleEvidence"
                  @clear="refreshAftersaleEvidence"
                />
                <el-select
                  v-model="evidenceStatusFilter"
                  class="status-filter"
                  @change="refreshAftersaleEvidence"
                >
                  <el-option value="all" label="全部状态" />
                  <el-option
                    v-for="item in evidenceStatusOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-button
                  v-if="evidenceTargetIdFilter"
                  text
                  @click="clearEvidenceTargetFilter"
                >
                  清除单据
                </el-button>
                <el-button :icon="Refresh" @click="refreshAftersaleEvidence">刷新凭证</el-button>
                <el-select v-model="evidenceExportFormat" class="status-filter">
                  <el-option value="md" label="MD" />
                  <el-option value="jsonl" label="JSONL" />
                  <el-option value="json" label="JSON" />
                </el-select>
                <el-button :icon="UploadFilled" @click="exportAftersaleEvidence">导出资料包</el-button>
              </div>
            </div>
            <div class="form-grid evidence-grid">
              <el-select v-model="evidenceForm.target_type" placeholder="目标类型">
                <el-option
                  v-for="item in evidenceTargetTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="evidenceForm.target_id" placeholder="售后/纠纷本地 ID 或微信单号" />
              <el-select v-model="evidenceForm.evidence_type" placeholder="凭证类型">
                <el-option
                  v-for="item in evidenceTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-select v-model="evidenceForm.status" placeholder="整理状态">
                <el-option
                  v-for="item in evidenceStatusOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="evidenceForm.title" placeholder="凭证标题" />
              <el-input v-model="evidenceForm.local_file_path" placeholder="本地文件路径，可选" />
              <el-input v-model="evidenceForm.source_url" placeholder="来源链接，可选" />
              <el-input v-model="evidenceForm.content_text" class="span-2" placeholder="凭证说明，可选" />
              <div class="aftersale-action-buttons">
                <span class="subtext">记录后仅进入本地资料包，后续平台举证仍需人工确认。</span>
                <el-button type="primary" :icon="CircleCheck" @click="recordAftersaleEvidence">记录凭证</el-button>
              </div>
            </div>
            <el-table :data="aftersaleEvidence" class="dense-table">
              <el-table-column prop="status_text" label="状态" width="110">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status_text }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="target_type" label="目标" width="100">
                <template #default="{ row }">
                  {{ row.target_type === "guarantee" ? "纠纷单" : "售后单" }}
                </template>
              </el-table-column>
              <el-table-column prop="external_target_id" label="单号" min-width="180" />
              <el-table-column prop="evidence_type_text" label="类型" width="120" />
              <el-table-column prop="title" label="标题" min-width="180" show-overflow-tooltip />
              <el-table-column prop="content_text" label="说明" min-width="240" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.content_text || "-" }}
                </template>
              </el-table-column>
              <el-table-column label="资料来源" min-width="240" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.local_file_path || row.source_url || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="updated_at" label="更新时间" min-width="190" />
              <el-table-column label="操作" width="210" fixed="right">
                <template #default="{ row }">
                  <div class="table-actions">
                    <el-button
                      v-if="row.status !== 'ready'"
                      size="small"
                      text
                      @click="updateAftersaleEvidenceStatus(row, 'ready')"
                    >
                      已整理
                    </el-button>
                    <el-button
                      v-if="row.status !== 'used'"
                      size="small"
                      text
                      @click="updateAftersaleEvidenceStatus(row, 'used')"
                    >
                      已使用
                    </el-button>
                    <el-button
                      v-if="row.status !== 'archived'"
                      size="small"
                      text
                      @click="updateAftersaleEvidenceStatus(row, 'archived')"
                    >
                      归档
                    </el-button>
                  </div>
                </template>
              </el-table-column>
            </el-table>
            <div v-if="evidenceExportPath" class="action-row">
              <el-tag>最近导出：{{ evidenceExportPath }}</el-tag>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>供应商协同记录</h2>
                <p>只记录售后/纠纷下的供应商沟通摘要，不自动登录供应商平台，不上传微信，不计入利润。</p>
              </div>
              <div class="button-group">
                <el-select
                  v-model="supplierFollowupTargetTypeFilter"
                  class="status-filter"
                  @change="refreshSupplierAftersaleFollowups"
                >
                  <el-option value="all" label="全部目标" />
                  <el-option
                    v-for="item in evidenceTargetTypeOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-input
                  v-model="supplierFollowupTargetIdFilter"
                  class="status-filter evidence-target-filter"
                  placeholder="单据 ID/单号"
                  clearable
                  @change="refreshSupplierAftersaleFollowups"
                  @clear="refreshSupplierAftersaleFollowups"
                />
                <el-select
                  v-model="supplierFollowupStatusFilter"
                  class="status-filter"
                  @change="refreshSupplierAftersaleFollowups"
                >
                  <el-option value="all" label="全部状态" />
                  <el-option
                    v-for="item in supplierFollowupStatusOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-button
                  v-if="supplierFollowupTargetIdFilter"
                  text
                  @click="clearSupplierFollowupTargetFilter"
                >
                  清除单据
                </el-button>
                <el-button :icon="Refresh" @click="refreshSupplierAftersaleFollowups">刷新协同</el-button>
              </div>
            </div>
            <div class="form-grid supplier-followup-grid">
              <el-select v-model="supplierFollowupForm.target_type" placeholder="目标类型">
                <el-option
                  v-for="item in evidenceTargetTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="supplierFollowupForm.target_id" placeholder="售后/纠纷本地 ID 或微信单号" />
              <el-select v-model="supplierFollowupForm.followup_type" placeholder="协同类型">
                <el-option
                  v-for="item in supplierFollowupTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-select v-model="supplierFollowupForm.status" placeholder="协同状态">
                <el-option
                  v-for="item in supplierFollowupStatusOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="supplierFollowupForm.supplier_name" placeholder="供应商名称，可选" />
              <el-input v-model="supplierFollowupForm.purchase_task_id" placeholder="采购任务 ID，可选" />
              <el-input v-model="supplierFollowupForm.note" class="span-2" placeholder="协同备注，脱敏填写" />
              <div class="aftersale-action-buttons">
                <span class="subtext">赔付入账仍走售后归因或纠纷跟进，这里只留协同流水。</span>
                <el-button type="primary" :icon="CircleCheck" @click="recordSupplierAftersaleFollowup">记录协同</el-button>
              </div>
            </div>
            <el-table :data="supplierAftersaleFollowups" class="dense-table">
              <el-table-column prop="status_text" label="状态" width="120">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status_text }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="target_type" label="目标" width="100">
                <template #default="{ row }">
                  {{ row.target_type === "guarantee" ? "纠纷单" : "售后单" }}
                </template>
              </el-table-column>
              <el-table-column prop="external_target_id" label="单号" min-width="180" />
              <el-table-column prop="followup_type_text" label="类型" width="120" />
              <el-table-column prop="supplier_name" label="供应商" min-width="130">
                <template #default="{ row }">
                  {{ row.supplier_name || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="purchase_task_id" label="采购任务" min-width="180" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.purchase_task_id || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="note" label="协同备注" min-width="260" show-overflow-tooltip />
              <el-table-column prop="updated_at" label="更新时间" min-width="190" />
            </el-table>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>责任归因</h2>
                <p>先人工判断责任方；供应商赔付金额会在已关联订单上计入利润回款。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="aftersaleResponsibilityForm.aftersale_id" placeholder="售后单 ID 或微信售后单号" />
              <el-select v-model="aftersaleResponsibilityForm.responsibility_party" placeholder="责任方">
                <el-option
                  v-for="item in aftersaleResponsibilityOptions"
                  :key="item.value"
                  :label="item.label"
                  :value="item.value"
                />
              </el-select>
              <el-input
                v-model="aftersaleResponsibilityForm.supplier_compensation_cents"
                placeholder="供应商赔付金额，单位分"
              />
              <el-input v-model="aftersaleResponsibilityForm.responsibility_note" placeholder="处理备注，可选" />
              <el-button type="primary" :icon="CircleCheck" @click="recordAftersaleResponsibility">记录归因</el-button>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>售后处理动作</h2>
                <p>同意或拒绝必须人工点击触发；系统记录本次微信接口结果，并等待下次同步确认平台状态。</p>
              </div>
            </div>
            <div class="form-grid aftersale-action-grid">
              <el-input v-model="aftersaleActionForm.aftersale_id" placeholder="售后单 ID 或微信售后单号" />
              <el-input v-model="aftersaleActionForm.address_id" placeholder="退货地址 ID，可选" />
              <el-select v-model="aftersaleActionForm.accept_type" placeholder="同意类型">
                <el-option value="" label="平台按状态判断" />
                <el-option value="1" label="同意退货" />
                <el-option value="2" label="同意退款" />
              </el-select>
              <el-select
                v-model="aftersaleActionForm.reject_reason_type"
                filterable
                allow-create
                placeholder="拒绝原因类型，必填"
                @change="applyAftersaleRejectReason"
              >
                <el-option
                  v-for="reason in aftersaleRejectReasonOptions"
                  :key="`${reason.shop_id}-${reason.reject_reason_type}`"
                  :label="aftersaleRejectReasonLabel(reason)"
                  :value="String(reason.reject_reason_type)"
                >
                  <span>{{ aftersaleRejectReasonLabel(reason) }}</span>
                  <small class="option-subtext">{{ reason.reject_scene_text }}</small>
                </el-option>
              </el-select>
              <el-input v-model="aftersaleActionForm.reject_reason" class="span-2" placeholder="拒绝原因，可选" />
              <el-input v-model="aftersaleActionForm.note" class="span-2" placeholder="处理备注，可选" />
              <div class="aftersale-action-buttons">
                <span class="subtext">已缓存拒绝原因 {{ aftersaleRejectReasonOptions.length }} 条</span>
                <el-button :icon="Refresh" @click="syncAftersaleRejectReasons">同步拒绝原因</el-button>
                <el-button type="primary" :icon="CircleCheck" @click="submitAftersaleAccept">提交同意</el-button>
                <el-button type="danger" :icon="CircleClose" @click="submitAftersaleReject">提交拒绝</el-button>
              </div>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>纠纷 / 保障单</h2>
                <p>同步微信保障单状态后，只记录本地跟进、责任归因和供应商赔付，不调用微信纠纷处理接口。</p>
              </div>
              <div class="button-group">
                <el-select
                  v-model="guaranteeStatusFilter"
                  class="status-filter"
                  @change="refreshGuaranteeOrders"
                >
                  <el-option value="active" label="待处理" />
                  <el-option value="all" label="全部状态" />
                  <el-option value="sync_failed" label="同步失败" />
                  <el-option value="STATUS_WAIT_MERCHANT_HANDLE" label="等待商家处理" />
                  <el-option value="STATUS_WAIT_MERCHANT_PROOF" label="等待商家举证" />
                  <el-option value="STATUS_WAIT_BOTH_PROOF" label="等待双方举证" />
                  <el-option value="STATUS_PAY_SUCC" label="赔付成功" />
                  <el-option value="STATUS_USER_CANCEL" label="用户取消" />
                </el-select>
                <el-button :icon="Refresh" @click="refreshGuaranteeOrders">刷新纠纷单</el-button>
                <el-button :icon="Refresh" @click="runGuaranteeSyncOnce">同步纠纷单</el-button>
              </div>
            </div>
            <div class="form-grid aftersale-action-grid">
              <el-input v-model="guaranteeFollowupForm.guarantee_order_id" placeholder="纠纷单 ID 或微信纠纷单号" />
              <el-select v-model="guaranteeFollowupForm.handling_status" placeholder="跟进状态">
                <el-option
                  v-for="item in guaranteeHandlingStatusOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-select v-model="guaranteeFollowupForm.responsibility_party" placeholder="责任方">
                <el-option
                  v-for="item in aftersaleResponsibilityOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input
                v-model="guaranteeFollowupForm.supplier_compensation_cents"
                placeholder="供应商赔付，单位分"
              />
              <el-input v-model="guaranteeFollowupForm.handling_note" class="span-2" placeholder="纠纷跟进备注，可选" />
              <div class="aftersale-action-buttons">
                <span class="subtext">只更新本地记录和利润回款，不上传凭证或处理平台纠纷。</span>
                <el-button type="primary" :icon="CircleCheck" @click="recordGuaranteeFollowup">记录纠纷跟进</el-button>
              </div>
            </div>
            <el-table :data="guaranteeOrders" class="dense-table">
              <el-table-column prop="status" label="状态" min-width="190">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status_text }}</el-tag>
                  <small class="subtext">{{ row.status }}</small>
                </template>
              </el-table-column>
              <el-table-column prop="shop_name" label="店铺" min-width="130" />
              <el-table-column prop="guarantee_order_id" label="纠纷单号" min-width="190" />
              <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170">
                <template #default="{ row }">
                  {{ row.wechat_order_id || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="guarantee_type_text" label="类型" width="130" />
              <el-table-column label="本地跟进" min-width="160">
                <template #default="{ row }">
                  <el-tag type="info">{{ guaranteeHandlingStatusLabel(row.handling_status) }}</el-tag>
                  <small class="subtext">{{ aftersaleResponsibilityLabel(row.responsibility_party) }}</small>
                </template>
              </el-table-column>
              <el-table-column label="凭证" width="90">
                <template #default="{ row }">
                  <el-tag :type="row.evidence_count > 0 ? 'success' : 'info'">
                    {{ row.evidence_count }} 条
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="赔付金额" width="110">
                <template #default="{ row }">
                  {{ formatCents(row.pay_amount_cents) }}
                </template>
              </el-table-column>
              <el-table-column label="供应商赔付" width="120">
                <template #default="{ row }">
                  {{ formatCents(row.supplier_compensation_cents) }}
                </template>
              </el-table-column>
              <el-table-column prop="apply_reason" label="申请原因 / 异常" min-width="260" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.apply_reason || row.merchant_refuse_reason || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="handling_note" label="跟进备注" min-width="220" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.handling_note || "-" }}
                </template>
              </el-table-column>
              <el-table-column label="过期时间" min-width="150">
                <template #default="{ row }">
                  {{ formatUnixTime(row.expire_time) }}
                </template>
              </el-table-column>
              <el-table-column prop="handled_at" label="跟进时间" min-width="190">
                <template #default="{ row }">
                  {{ row.handled_at || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="synced_at" label="同步时间" min-width="190" />
              <el-table-column prop="order_id" label="本地订单" min-width="180" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.order_id || "未关联" }}
                </template>
              </el-table-column>
              <el-table-column label="操作" width="260" fixed="right">
                <template #default="{ row }">
                  <div class="row-actions">
                    <el-button size="small" @click="selectGuaranteeFollowup(row)">跟进</el-button>
                    <el-button size="small" @click="selectEvidenceTarget('guarantee', row)">凭证</el-button>
                    <el-button size="small" @click="selectSupplierFollowupTarget('guarantee', row)">协同</el-button>
                    <el-button
                      size="small"
                      text
                      @click="exportAftersaleEvidence('guarantee', row.id, `纠纷单 ${row.guarantee_order_id}`)"
                    >
                      导出凭证
                    </el-button>
                  </div>
                </template>
              </el-table-column>
            </el-table>
          </div>

          <el-table :data="aftersales" class="dense-table">
            <el-table-column prop="status" label="状态" width="170">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170">
              <template #default="{ row }">
                {{ row.wechat_order_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="wechat_aftersale_id" label="售后单号" min-width="210" />
            <el-table-column prop="aftersale_type" label="类型" width="120">
              <template #default="{ row }">
                {{ row.aftersale_type || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="退款金额" width="110">
              <template #default="{ row }">
                {{ formatCents(row.refund_amount_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="责任归因" width="130">
              <template #default="{ row }">
                <el-tag :type="row.responsibility_party ? statusType(row.responsibility_party) : 'info'">
                  {{ aftersaleResponsibilityLabel(row.responsibility_party) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="供应商赔付" width="120">
              <template #default="{ row }">
                {{ formatCents(row.supplier_compensation_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="最近动作" min-width="190">
              <template #default="{ row }">
                <template v-if="row.last_action">
                  <el-tag :type="statusType(row.last_action_status || '')">
                    {{ aftersaleActionLabel(row.last_action) }} · {{ aftersaleActionStatusLabel(row.last_action_status) }}
                  </el-tag>
                  <small v-if="row.last_action_error" class="subtext">{{ row.last_action_error }}</small>
                  <small v-else-if="row.last_action_note" class="subtext">{{ row.last_action_note }}</small>
                </template>
                <span v-else>-</span>
              </template>
            </el-table-column>
            <el-table-column label="凭证" width="90">
              <template #default="{ row }">
                <el-tag :type="row.evidence_count > 0 ? 'success' : 'info'">
                  {{ row.evidence_count }} 条
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="reason" label="原因 / 异常" min-width="260" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.reason || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="last_action_at" label="动作时间" min-width="190">
              <template #default="{ row }">
                {{ row.last_action_at || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="handled_at" label="处理时间" min-width="190">
              <template #default="{ row }">
                {{ row.handled_at || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="synced_at" label="同步时间" min-width="190" />
            <el-table-column prop="order_id" label="本地订单" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.order_id || "未关联" }}
              </template>
            </el-table-column>
            <el-table-column label="操作" width="330" fixed="right">
              <template #default="{ row }">
                <div class="row-actions">
                  <el-button size="small" :icon="CircleCheck" @click="selectAftersaleAction(row)">处理</el-button>
                  <el-button size="small" @click="selectAftersaleResponsibility(row)">归因</el-button>
                  <el-button size="small" @click="selectEvidenceTarget('aftersale', row)">凭证</el-button>
                  <el-button size="small" @click="selectSupplierFollowupTarget('aftersale', row)">协同</el-button>
                  <el-button
                    size="small"
                    text
                    @click="exportAftersaleEvidence('aftersale', row.id, `售后单 ${row.wechat_aftersale_id}`)"
                  >
                    导出凭证
                  </el-button>
                </div>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'profit'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>订单利润核算</h2>
              <p>按订单聚合成交额、采购成本、运费、退款、售后赔付和其他成本。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="orderProfitStatusFilter"
                class="status-filter"
                @change="refreshOrderProfits"
              >
                <el-option value="all" label="全部利润状态" />
                <el-option value="missing_purchase_task" label="缺采购任务" />
                <el-option value="missing_cost" label="缺成本" />
                <el-option value="loss" label="亏损" />
                <el-option value="profitable" label="有毛利" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshOrderProfits">刷新</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>订单数</dt>
              <dd>{{ orderProfitTotals.order_count }}</dd>
            </div>
            <div>
              <dt>成交额</dt>
              <dd>{{ formatCents(orderProfitTotals.revenue_cents) }}</dd>
            </div>
            <div>
              <dt>采购成本</dt>
              <dd>{{ formatCents(orderProfitTotals.purchase_cost_cents) }}</dd>
            </div>
            <div>
              <dt>预估毛利</dt>
              <dd>{{ formatSignedCents(orderProfitTotals.estimated_profit_cents) }}</dd>
            </div>
            <div>
              <dt>实际毛利</dt>
              <dd>{{ formatSignedCents(orderProfitTotals.actual_profit_cents) }}</dd>
            </div>
            <div>
              <dt>待补成本</dt>
              <dd>{{ orderProfitTotals.unknown_actual_order_count }}</dd>
            </div>
          </dl>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>利润调整项</h2>
                <p>记录订单级采购运费、退款、售后赔付、其他成本或收入。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="orderProfitAdjustmentForm.order_id" placeholder="订单 ID 或微信订单号" />
              <el-select v-model="orderProfitAdjustmentForm.kind" placeholder="调整类型">
                <el-option
                  v-for="kind in profitAdjustmentKindOptions"
                  :key="kind.value"
                  :label="kind.label"
                  :value="kind.value"
                />
              </el-select>
              <el-input v-model="orderProfitAdjustmentForm.amount_cents" placeholder="金额，单位分" />
              <el-input v-model="orderProfitAdjustmentForm.note" placeholder="备注，可选" />
              <el-button type="primary" :icon="UploadFilled" @click="recordOrderProfitAdjustment">记录</el-button>
            </div>
          </div>

          <el-table :data="orderProfits" class="dense-table">
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170" />
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="order_status" label="订单状态" width="140">
              <template #default="{ row }">
                <el-tag :type="statusType(row.order_status)">{{ row.order_status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="profit_status" label="利润状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.profit_status)">{{ profitStatusLabel(row.profit_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="成交额" width="110">
              <template #default="{ row }">{{ formatCents(row.revenue_cents) }}</template>
            </el-table-column>
            <el-table-column label="采购成本" width="110">
              <template #default="{ row }">{{ formatCents(row.purchase_cost_cents) }}</template>
            </el-table-column>
            <el-table-column label="退款/赔付" width="120">
              <template #default="{ row }">
                {{ formatCents(row.refund_cents + row.aftersale_compensation_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="其他成本" width="110">
              <template #default="{ row }">
                {{ formatCents(row.purchase_freight_cents + row.other_cost_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="预估毛利" width="120">
              <template #default="{ row }">{{ formatSignedCents(row.estimated_profit_cents) }}</template>
            </el-table-column>
            <el-table-column label="实际毛利" width="120">
              <template #default="{ row }">{{ formatSignedCents(row.actual_profit_cents) }}</template>
            </el-table-column>
            <el-table-column label="缺成本项" width="100">
              <template #default="{ row }">{{ row.missing_cost_count }}</template>
            </el-table-column>
            <el-table-column prop="updated_at" label="更新时间" min-width="190" />
            <el-table-column label="操作" width="90">
              <template #default="{ row }">
                <el-button size="small" @click="selectOrderProfitAdjustment(row)">调整</el-button>
              </template>
            </el-table-column>
          </el-table>

          <div class="action-row">
            <el-tag>{{ orderProfits.length }} / {{ orderProfitTotal }}</el-tag>
          </div>
        </div>
      </section>

      <section v-if="selectedSection === 'sales'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>商品动销分析</h2>
              <p>按外部商品聚合真实订单、采购成本、售后关联、铺货店铺和库存风险，输出继续铺货、调价、补货或观察建议。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="productSalesAnalysisStatusFilter"
                class="status-filter"
                @change="refreshProductSalesAnalysis"
              >
                <el-option
                  v-for="status in productSalesStatusOptions"
                  :key="status.value"
                  :label="status.label"
                  :value="status.value"
                />
              </el-select>
              <el-button :icon="Refresh" @click="refreshProductSalesAnalysis">刷新</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>商品数</dt>
              <dd>{{ productSalesAnalysisTotals.product_count }}</dd>
            </div>
            <div>
              <dt>已动销</dt>
              <dd>{{ productSalesAnalysisTotals.sold_product_count }}</dd>
            </div>
            <div>
              <dt>销量</dt>
              <dd>{{ productSalesAnalysisTotals.total_units_sold }}</dd>
            </div>
            <div>
              <dt>成交额</dt>
              <dd>{{ formatCents(productSalesAnalysisTotals.revenue_cents) }}</dd>
            </div>
            <div>
              <dt>粗毛利</dt>
              <dd>{{ formatSignedCents(productSalesAnalysisTotals.gross_profit_cents) }}</dd>
            </div>
            <div>
              <dt>可放量</dt>
              <dd>{{ productSalesAnalysisTotals.scale_candidate_count }}</dd>
            </div>
            <div>
              <dt>风险</dt>
              <dd>{{ productSalesAnalysisTotals.risk_product_count }}</dd>
            </div>
            <div>
              <dt>缺成本</dt>
              <dd>{{ productSalesAnalysisTotals.missing_cost_product_count }}</dd>
            </div>
          </dl>

          <el-table :data="productSalesAnalysis" class="dense-table">
            <el-table-column prop="external_product_id" label="外部商品 ID" min-width="170" />
            <el-table-column prop="title" label="商品" min-width="220" show-overflow-tooltip />
            <el-table-column prop="operation_status" label="运营状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.operation_status)">{{ productSalesStatusLabel(row.operation_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="inventory_risk_status" label="库存" width="120">
              <template #default="{ row }">
                <el-tag :type="statusType(row.inventory_risk_status)">{{ inventoryRiskLabel(row.inventory_risk_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="active_shop_count" label="店铺" width="80" />
            <el-table-column prop="order_count" label="订单" width="80" />
            <el-table-column prop="units_sold" label="销量" width="80" />
            <el-table-column label="成交额" width="110">
              <template #default="{ row }">{{ formatCents(row.revenue_cents) }}</template>
            </el-table-column>
            <el-table-column label="采购成本" width="110">
              <template #default="{ row }">{{ formatCents(row.purchase_cost_cents) }}</template>
            </el-table-column>
            <el-table-column label="粗毛利" width="110">
              <template #default="{ row }">
                {{ formatSignedCents(row.revenue_cents - row.purchase_cost_cents - row.related_refund_cents) }}
              </template>
            </el-table-column>
            <el-table-column prop="missing_cost_count" label="缺成本" width="90" />
            <el-table-column prop="related_aftersale_count" label="售后" width="80" />
            <el-table-column prop="available_stock" label="可用库存" width="100" />
            <el-table-column prop="recommendation" label="建议" min-width="300" show-overflow-tooltip />
            <el-table-column prop="last_order_at" label="最近订单" min-width="190">
              <template #default="{ row }">{{ row.last_order_at || "-" }}</template>
            </el-table-column>
          </el-table>

          <div class="action-row">
            <el-tag>{{ productSalesAnalysis.length }} / {{ productSalesAnalysisTotal }}</el-tag>
          </div>
        </div>
      </section>

      <section v-if="selectedSection === 'inventory'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>库存风控</h2>
              <p>基于外部商品 SKU 库存、采购占用、供应商异常和铺货店铺数生成运营提醒。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="inventoryRiskStatusFilter"
                class="status-filter"
                @change="refreshInventoryRisks"
              >
                <el-option
                  v-for="status in inventoryRiskStatusOptions"
                  :key="status.value"
                  :label="status.label"
                  :value="status.value"
                />
              </el-select>
              <el-button :icon="Refresh" @click="refreshInventoryRisks">刷新</el-button>
              <el-button type="primary" :icon="UploadFilled" @click="runInventoryRiskScan">扫描并通知</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>风险商品</dt>
              <dd>{{ inventoryRiskTotal }}</dd>
            </div>
            <div>
              <dt>断货</dt>
              <dd>{{ inventoryRiskStats.out_of_stock_count }}</dd>
            </div>
            <div>
              <dt>低库存/压力</dt>
              <dd>{{ inventoryRiskStats.low_stock_count }}</dd>
            </div>
            <div>
              <dt>供应商异常</dt>
              <dd>{{ inventoryRiskStats.issue_count }}</dd>
            </div>
          </dl>

          <el-table :data="inventoryRisks" class="dense-table">
            <el-table-column prop="external_product_id" label="外部商品 ID" min-width="170" />
            <el-table-column prop="title" label="商品" min-width="220" show-overflow-tooltip />
            <el-table-column prop="supplier_name" label="供应商" min-width="130">
              <template #default="{ row }">{{ row.supplier_name || "-" }}</template>
            </el-table-column>
            <el-table-column prop="risk_status" label="状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.risk_status)">{{ inventoryRiskLabel(row.risk_status) }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="total_stock" label="货源库存" width="100" />
            <el-table-column prop="reserved_quantity" label="采购占用" width="100" />
            <el-table-column prop="available_stock" label="可用库存" width="100" />
            <el-table-column prop="active_shop_count" label="已铺店铺" width="100" />
            <el-table-column prop="recommendation" label="建议" min-width="280" show-overflow-tooltip />
            <el-table-column prop="updated_at" label="更新时间" min-width="190" />
          </el-table>
        </div>
      </section>

      <section v-if="selectedSection === 'jobs'" class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <h2>查询铺货任务</h2>
            <p>任务详情默认按商品聚合，再展开到店铺维度。</p>
          </div>
          <div class="inline-form">
            <el-input v-model="queriedTaskId" placeholder="pub_xxx" />
            <el-button type="primary" :icon="Search" @click="queryJob">查询</el-button>
          </div>
        </div>

        <div v-if="currentJob" class="panel">
          <div class="panel-title">
            <h2>{{ currentJob.id }}</h2>
            <el-tag :type="statusType(currentJob.status)">{{ currentJob.status }}</el-tag>
          </div>
          <dl class="status-list compact">
            <div>
              <dt>request_id</dt>
              <dd>{{ currentJob.request_id }}</dd>
            </div>
            <div>
              <dt>商品数</dt>
              <dd>{{ currentJob.accepted_product_count }}</dd>
            </div>
            <div>
              <dt>目标店铺</dt>
              <dd>{{ currentJob.target_shop_count }}</dd>
            </div>
          </dl>

          <el-collapse>
            <el-collapse-item
              v-for="product in currentJob.products"
              :key="product.external_product_id"
              :title="`${product.title} / 失败 ${product.failed_count} / 待处理 ${product.pending_count}`"
            >
              <el-table :data="product.items" class="dense-table">
                <el-table-column prop="shop_name" label="店铺" min-width="160" />
                <el-table-column prop="status" label="状态" width="120">
                  <template #default="{ row }">
                    <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="error_code" label="错误码" min-width="180" />
                <el-table-column prop="error_summary" label="失败原因" min-width="260" />
                <el-table-column label="属性建议" width="120">
                  <template #default="{ row }">
                    <el-tag
                      v-if="attributeSuggestionsForItem(row).length > 0"
                      :type="pendingAttributeSuggestionCountForItem(row) > 0 ? 'warning' : 'success'"
                    >
                      {{ pendingAttributeSuggestionCountForItem(row) > 0 ? `待采纳 ${pendingAttributeSuggestionCountForItem(row)}` : "已处理" }}
                    </el-tag>
                    <span v-else>-</span>
                  </template>
                </el-table-column>
              </el-table>
              <div v-if="attributeSuggestionsForProduct(product).length > 0" class="sub-panel job-suggestion-panel">
                <div class="automation-head">
                  <div>
                    <h3>属性建议</h3>
                    <p>当前商品的低置信属性建议可在这里直接采纳；采纳后仍需重新执行微信类目预检。</p>
                  </div>
                  <div class="button-group">
                    <el-button :icon="Refresh" @click="refreshJobAttributeSuggestions">刷新建议</el-button>
                    <el-button
                      type="primary"
                      :disabled="pendingAttributeSuggestionsForProduct(product).length === 0 || attributeSuggestionApplying"
                      :loading="attributeSuggestionApplying"
                      @click="applyProductAttributeSuggestions(product)"
                    >
                      采纳当前商品建议
                    </el-button>
                  </div>
                </div>
                <el-table :data="attributeSuggestionsForProduct(product)" class="dense-table compact-table">
                  <el-table-column label="状态" width="95">
                    <template #default="{ row }">
                      <el-tag :type="row.applied ? 'success' : 'warning'">
                        {{ row.applied ? "已采纳" : "待确认" }}
                      </el-tag>
                    </template>
                  </el-table-column>
                  <el-table-column label="店铺/属性" min-width="180">
                    <template #default="{ row }">
                      <span>{{ row.shop_name }} · {{ row.attr_key }}</span>
                      <small class="subtext">{{ attributeKindLabel(row.attr_kind) }}</small>
                    </template>
                  </el-table-column>
                  <el-table-column label="建议值" min-width="210" show-overflow-tooltip>
                    <template #default="{ row }">
                      {{ attributeSuggestionValue(row) }}
                    </template>
                  </el-table-column>
                  <el-table-column label="允许值" min-width="170" show-overflow-tooltip>
                    <template #default="{ row }">
                      {{ allowedValuesText(row.allowed_values) }}
                    </template>
                  </el-table-column>
                  <el-table-column label="来源/置信" min-width="160">
                    <template #default="{ row }">
                      <span>{{ row.source }}</span>
                      <small class="subtext">confidence {{ row.confidence }}</small>
                    </template>
                  </el-table-column>
                  <el-table-column label="操作" width="95">
                    <template #default="{ row }">
                      <el-button
                        size="small"
                        :disabled="row.applied || attributeSuggestionApplying"
                        @click="applySingleAttributeSuggestion(row)"
                      >
                        采纳
                      </el-button>
                    </template>
                  </el-table-column>
                </el-table>
              </div>
            </el-collapse-item>
          </el-collapse>
        </div>
      </section>
    </main>
  </div>
</template>

<style>
:root {
  color: #1d2420;
  background: #f4f1e8;
  font-family: "Aptos", "PingFang SC", "Microsoft YaHei", sans-serif;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-width: 1100px;
  min-height: 720px;
}

button,
input,
textarea {
  font: inherit;
}

.app-shell {
  display: grid;
  grid-template-columns: 292px 1fr;
  min-height: 100vh;
  background:
    linear-gradient(90deg, rgba(30, 80, 72, 0.08) 1px, transparent 1px),
    linear-gradient(180deg, rgba(30, 80, 72, 0.06) 1px, transparent 1px),
    #f4f1e8;
  background-size: 32px 32px;
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 22px;
  padding: 24px;
  color: #f8f1df;
  background: #1d3f37;
  border-right: 1px solid rgba(0, 0, 0, 0.14);
}

.brand {
  display: grid;
  grid-template-columns: 52px 1fr;
  gap: 14px;
  align-items: center;
}

.brand-mark {
  display: grid;
  place-items: center;
  width: 52px;
  height: 52px;
  color: #1d3f37;
  font-weight: 800;
  background: #f3c45b;
  border-radius: 6px;
}

.brand strong,
.brand span {
  display: block;
}

.brand strong {
  font-size: 17px;
}

.brand span {
  margin-top: 4px;
  color: rgba(248, 241, 223, 0.66);
  font-size: 13px;
}

.nav {
  display: grid;
  gap: 8px;
}

.nav button {
  width: 100%;
  padding: 12px 14px;
  color: rgba(248, 241, 223, 0.72);
  text-align: left;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
}

.nav button.active,
.nav button:hover {
  color: #fff9e7;
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.14);
}

.runtime-card {
  margin-top: auto;
  padding: 14px;
  background: rgba(0, 0, 0, 0.14);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 6px;
}

.runtime-card span {
  display: block;
  margin-bottom: 8px;
  color: rgba(248, 241, 223, 0.68);
  font-size: 12px;
}

.runtime-card code {
  display: block;
  overflow: hidden;
  color: #f3c45b;
  font-size: 11px;
  line-height: 1.45;
  text-overflow: ellipsis;
}

.workspace {
  padding: 28px;
  overflow: auto;
}

.topbar {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  align-items: flex-start;
  margin-bottom: 24px;
}

.eyebrow {
  margin: 0 0 6px;
  color: #8f5c2d;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0;
  text-transform: uppercase;
}

h1,
h2,
h3,
p {
  margin: 0;
}

h1 {
  font-size: 28px;
  line-height: 1.2;
}

h2 {
  font-size: 18px;
}

.section-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
}

.metric,
.panel {
  background: rgba(255, 252, 244, 0.92);
  border: 1px solid rgba(57, 53, 44, 0.12);
  border-radius: 6px;
  box-shadow: 0 16px 38px rgba(54, 45, 29, 0.09);
}

.metric {
  min-height: 150px;
  padding: 18px;
  border-top: 4px solid #1d3f37;
}

.metric span,
.metric em {
  display: block;
  color: #6c685e;
  font-size: 13px;
  font-style: normal;
}

.metric strong {
  display: block;
  margin: 16px 0 12px;
  font-size: 38px;
  line-height: 1;
}

.metric.critical {
  border-top-color: #bd4c2f;
}

.metric.warning {
  border-top-color: #c38a21;
}

.metric.danger {
  border-top-color: #8f2e3c;
}

.metric.neutral {
  border-top-color: #2d6158;
}

.panel {
  padding: 18px;
}

.panel.wide {
  grid-column: 1 / -1;
}

.panel-title {
  display: flex;
  justify-content: space-between;
  gap: 18px;
  align-items: flex-start;
  margin-bottom: 16px;
}

.panel-title p {
  max-width: 620px;
  color: #6c685e;
  font-size: 13px;
}

.panel-title.tight {
  margin-bottom: 12px;
}

.content-stack {
  display: grid;
  gap: 16px;
}

.inline-form,
.form-grid,
.action-row,
.button-group {
  display: grid;
  gap: 12px;
  align-items: center;
}

.inline-form {
  grid-template-columns: 1fr auto;
  margin-bottom: 16px;
}

.form-grid {
  grid-template-columns: 1fr 1fr 1fr auto;
}

.aftersale-action-grid {
  grid-template-columns: minmax(220px, 1.3fr) repeat(3, minmax(160px, 1fr));
}

.evidence-grid {
  grid-template-columns: repeat(4, minmax(160px, 1fr));
}

.supplier-followup-grid {
  grid-template-columns: repeat(4, minmax(160px, 1fr));
}

.aftersale-action-grid .span-2,
.evidence-grid .span-2,
.supplier-followup-grid .span-2 {
  grid-column: span 2;
}

.aftersale-action-buttons,
.row-actions,
.table-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.aftersale-action-buttons {
  grid-column: 1 / -1;
  justify-content: flex-end;
}

.option-subtext {
  margin-left: 8px;
  color: #756f63;
  font-size: 12px;
}

.row-actions {
  flex-wrap: wrap;
}

.table-actions {
  flex-wrap: nowrap;
}

.ai-settings-form {
  display: grid;
  grid-template-columns: 220px 220px minmax(260px, 1fr);
  gap: 12px;
  align-items: center;
}

.ai-settings-form .el-input:nth-of-type(2),
.ai-settings-form .el-input:nth-of-type(3) {
  grid-column: span 1;
}

.action-row {
  grid-template-columns: auto 1fr;
  margin-top: 14px;
}

.ai-actions {
  grid-template-columns: minmax(0, 1fr) auto;
}

.button-group {
  grid-auto-flow: column;
  justify-content: end;
}

.status-filter {
  width: 170px;
}

.evidence-target-filter {
  width: 220px;
}

.sub-panel {
  margin-bottom: 16px;
  padding: 14px;
  background: #f6efe0;
  border: 1px solid rgba(57, 53, 44, 0.08);
  border-radius: 6px;
}

.automation-panel {
  display: grid;
  gap: 12px;
}

.automation-head {
  display: flex;
  justify-content: space-between;
  gap: 14px;
  align-items: flex-start;
}

.automation-head h3 {
  margin-bottom: 4px;
  font-size: 15px;
}

.automation-head p,
.automation-result {
  color: #6c685e;
  font-size: 12px;
  line-height: 1.5;
}

.automation-switches {
  display: grid;
  grid-template-columns: repeat(8, minmax(88px, 1fr));
  gap: 8px 12px;
  align-items: center;
}

.automation-result {
  display: flex;
  gap: 8px;
  align-items: center;
  overflow: hidden;
}

.automation-result span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.supplier-agent-panel {
  display: grid;
  gap: 12px;
}

.supplier-agent-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 220px;
  gap: 12px;
  align-items: stretch;
}

.supplier-agent-actions {
  display: grid;
  align-content: start;
  gap: 12px;
}

.supplier-agent-result {
  display: grid;
  gap: 10px;
}

.job-suggestion-panel {
  margin: 14px 0 0;
}

.status-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.status-list.compact {
  grid-template-columns: 1.6fr 0.7fr 0.7fr;
  margin-bottom: 18px;
}

.status-list div {
  padding: 14px;
  background: #f6efe0;
  border: 1px solid rgba(57, 53, 44, 0.08);
  border-radius: 6px;
}

.status-list dt {
  color: #796f60;
  font-size: 12px;
}

.status-list dd {
  margin: 6px 0 0;
  color: #1d2420;
  font-weight: 700;
  word-break: break-all;
}

.api-endpoints {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  margin-top: -4px;
}

.api-endpoints code {
  display: block;
  padding: 10px 12px;
  color: #214139;
  font-size: 12px;
  line-height: 1.35;
  background: #f6efe0;
  border: 1px solid rgba(57, 53, 44, 0.08);
  border-radius: 6px;
  word-break: break-all;
}

.subtext {
  display: block;
  margin-top: 3px;
  color: #756f63;
  font-size: 12px;
}

.inline-link {
  color: #176b5f;
  font-size: 12px;
  font-weight: 600;
  text-decoration: none;
}

.inline-link:hover {
  text-decoration: underline;
}

.notification-title,
.notification-body {
  display: block;
}

.notification-title {
  margin-bottom: 4px;
  color: #20322d;
  font-size: 13px;
}

.notification-body {
  color: #756f63;
  font-size: 12px;
  line-height: 1.45;
}

.split-stat {
  margin-left: 10px;
  color: #756f63;
}

.dense-table {
  width: 100%;
}

.compact-table {
  margin-top: 10px;
}

.json-editor textarea {
  color: #20322d;
  font-family: "SFMono-Regular", Consolas, monospace;
  font-size: 13px;
  line-height: 1.5;
  background: #fffaf0;
}

.el-button {
  border-radius: 6px;
}

.el-input__wrapper,
.el-textarea__inner,
.el-select__wrapper {
  border-radius: 6px;
}
</style>
