mod aftersales;
mod ai;
mod analytics;
mod automation;
mod collection;
mod delivery;
mod errors;
mod fulfillment;
mod jobs;
mod order_price_adjustment;
mod orders;
mod pipeline;
mod price_update;
mod publish;
mod purchase;
mod shop_products;
mod shops;
mod support;
mod system;

pub use aftersales::*;
pub use ai::*;
pub use analytics::*;
pub use automation::*;
pub use collection::*;
pub use delivery::*;
pub use errors::*;
pub use fulfillment::*;
pub use jobs::*;
pub use order_price_adjustment::*;
pub use orders::*;
pub use pipeline::*;
pub use price_update::*;
pub use publish::*;
pub use purchase::*;
pub use shop_products::*;
pub use shops::*;
use support::*;
pub use system::*;

use crate::crypto::{
    decrypt_access_token, decrypt_secret, encrypt_access_token, encrypt_secret, secret_fingerprint,
};
use crate::models::{
    AftersaleAcceptRequest, AftersaleActionResult, AftersaleEvidenceExportResult,
    AftersaleEvidenceListResult, AftersaleEvidenceRecordRequest, AftersaleEvidenceRecordResult,
    AftersaleEvidenceStatusUpdateRequest, AftersaleEvidenceStatusUpdateResult,
    AftersaleEvidenceView, AftersaleListResult, AftersaleRejectReasonSyncResult,
    AftersaleRejectReasonView, AftersaleRejectRequest, AftersaleResponsibilityRequest,
    AftersaleResponsibilityResult, AftersaleSyncBatchResult, AftersaleView, AgentRunEventView,
    AgentRunView, AgentSkillSettingsRequest, AgentSkillTestResult, AgentSkillView,
    AiProviderSettings, AiProviderSettingsRequest, AiProviderTestResult, ApiQuotaCheckRequest,
    ApiQuotaCheckResult, AssetUploadBatchResult, AutomationStepError, BackupCreateResult,
    BackupInfo, BackupRestoreRequest, BackupRestoreResult, BatchShopProductActionResult,
    CategoryCacheView,
    CategoryCatalogListResult, CategoryCatalogShopSummary, CategoryCatalogSyncResult,
    CategoryDetailPrewarmResult, CategoryRelationView, CategoryRuleSyncResult,
    AddressDecodeBatchResult, DecodedOrderAddressView, NegotiationScanResult,
    OrderRequestDecisionResult, OrderRequestListResult, OrderRequestView,
    PurchaseTaskPurchasedResult,
    CompensateDeliveryRequest, CompensateDeliveryResult, DeliveryChangeRequest,
    DeliveryChangeResult, DecodedAddressGcResult, VirtualTelDelayScanResult,
    CleanupOrphanDraftsResult, CollectionImageRemoveRequest, CollectionImageUploadRequest,
    CollectionPublishRequest, CollectionPublishWorkspaceResetRequest,
    CollectionPublishWorkspaceResetResult, CollectionReviewBatchResult,
    CollectionReviewCategoryCandidate, CollectionReviewConfirmRequest, CollectionReviewRunRequest,
    CollectionTaskView, CreateShopRequest, DashboardSummary, DeliveryCompanySyncResult,
    DeliveryCompanyView, DeliverySettings, DeliverySubmitBatchResult, ExcelImportResult,
    ExternalProductInput, ExternalPublishJobRequest, FreightTemplateView, GuaranteeFollowupRequest,
    GuaranteeFollowupResult, GuaranteeOrderListResult, GuaranteeOrderView,
    GuaranteeSyncBatchResult, ImportBatchView, InventoryRiskListResult, InventoryRiskScanResult,
    InventoryRiskView, NotificationListResult, NotificationMarkResult, NotificationView,
    OperationalAutomationRunResult, OperationalAutomationSettings, OrderDetailSyncBatchResult,
    OrderManagementItemView, OrderManagementListResult, OrderManagementView,
    OrderPriceAdjustmentBatchResult, OrderPriceAdjustmentItemView, OrderPriceAdjustmentJobCreated,
    OrderPriceAdjustmentJobRequest, OrderPriceAdjustmentJobView, OrderPriceAdjustmentLineInput,
    OrderPriceAdjustmentOrderInput, OrderProfitAdjustmentRequest, OrderProfitAdjustmentResult,
    OrderProfitListResult, OrderProfitTotals, OrderProfitView, OrderSyncBatchResult,
    PipelineProductDetailSku, PipelineProductDetailView, PipelineProductView,
    PipelineShopTargetView, PipelineStageBucket, PipelineStageStats, PipelineStats,
    PipelineWorkbenchView, PriceUpdateConfirmBatchResult, PriceUpdateItemView,
    PriceUpdateJobCreated, PriceUpdateJobRequest, PriceUpdateJobView,
    PriceUpdatePrecheckBatchResult, PriceUpdateSubmitBatchResult, ProductListingBatchResult,
    ProductManagementListResult, ProductManagementShopView, ProductManagementView,
    ProductSalesAnalysisListResult, ProductSalesAnalysisTotals, ProductSalesAnalysisView,
    ProductStatusSyncBatchResult, ProductSubmitBatchResult, PublishAttributeFillBatchResult,
    PublishCategoryPrecheckBatchResult, PublishJobCreated, PublishJobItemView, PublishJobView,
    PublishPipelineRunResult, PublishPricingStrategy, PublishProductView, PublishTaskBatchResult,
    PurchaseTaskBatchResult, PurchaseTaskExportResult, PurchaseTaskIssueRequest,
    PurchaseTaskIssueResult, PurchaseTaskListResult, PurchaseTaskMappingRequest,
    PurchaseTaskMappingResult, PurchaseTaskShipmentRequest, PurchaseTaskShipmentResult,
    PurchaseTaskView, ShipmentListResult, ShipmentRecordRequest, ShipmentRecordResult,
    ShipmentRetryResult, ShipmentView, Shop, ShopBasicInfoSyncResult, ShopCredentialCheck,
    ShopGroup, ShopListItem, ShopProductFailedItem, ShopProductListResult, ShopProductStatusCount,
    ShopProductSummaryView, ShopProductTaskProgress, SupplierAftersaleFollowupListResult,
    SupplierAftersaleFollowupRecordRequest, SupplierAftersaleFollowupRecordResult,
    SupplierAftersaleFollowupView, SupplierAgentApplyItemResult, SupplierAgentApplyRequest,
    SupplierAgentApplyResult, SupplierAgentExportResult, SyncShopProductsResult, TaskRunView,
    WechatShopProductDetailView, WechatShopProductSkuView, WechatShopProductView,
    WorkspaceResetTableCount,
};
use crate::storage::{
    database_path, expires_at_shanghai, format_shanghai, is_future_rfc3339, now_shanghai,
    open_connection, AppError, AppResult,
};
use crate::wechat::{
    MerchantAddressDetailSummary, OrderListTimeField, OrderPriceUpdateInfo, ProductGetInfo,
    WechatApiError, WechatCallMeta, WechatCallResult, WechatProductSnapshot, WechatRawCall,
    WechatShopClient,
};
use chrono::{DateTime, Duration, Utc};
use image::{codecs::jpeg::JpegEncoder, DynamicImage, GenericImageView, ImageFormat};
use reqwest::{
    header::{ACCEPT, ACCEPT_ENCODING, CONTENT_TYPE, USER_AGENT},
    redirect::Policy,
    Url,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration as StdDuration;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const AUTO_SEND_DELIVERY_SETTING: &str = "delivery.auto_send_delivery";
// 订单履约三期：多运单拆包发货 feature 开关（默认关，关闭时多物流订单仍走人工确认整单发货）
const MULTI_PACKAGE_SETTING: &str = "delivery.multi_package_enabled";
// 订单履约二期：订单 driver L1 总开关 + 申请扫描/地址解密步骤开关 + 解密额度熔断日期
const AUTOMATION_ORDER_DRIVER_SETTING: &str = "automation.order_automation_enabled";
const AUTOMATION_NEGOTIATION_SCAN_SETTING: &str = "automation.negotiation_scan_enabled";
const AUTOMATION_ADDRESS_DECODE_SETTING: &str = "automation.address_decode_enabled";
const ORDER_DECODE_CIRCUIT_SETTING: &str = "orders.decode_circuit_open_date";
const AUTOMATION_ORDER_SYNC_SETTING: &str = "automation.order_sync_enabled";
const AUTOMATION_ORDER_DETAIL_SYNC_SETTING: &str = "automation.order_detail_sync_enabled";
const AUTOMATION_AFTERSALE_SYNC_SETTING: &str = "automation.aftersale_sync_enabled";
const AUTOMATION_PURCHASE_TASK_SETTING: &str = "automation.purchase_task_enabled";
const AUTOMATION_DELIVERY_SUBMISSION_SETTING: &str = "automation.delivery_submission_enabled";
/// 铺货自动化总开关：driver 与运营自动化的铺货 7 阶段整体开/关（细粒度阶段开关已收敛移除）。
const AUTOMATION_PUBLISH_ENABLED_SETTING: &str = "automation.publish_enabled";
const AUTOMATION_PRICE_CONFIRM_SETTING: &str = "automation.price_confirm_enabled";
const PUBLISH_PRICING_STRATEGY_SETTING: &str = "publish.pricing_strategy";
// 店铺级默认运费模板：JSON map {shop_id: template_id}。铺货时优先用，未设置则自动选第一个。
const PUBLISH_DEFAULT_FREIGHT_TEMPLATES_SETTING: &str = "publish.default_freight_templates";
const AUTO_BACKUP_LAST_DATE_SETTING: &str = "backup.last_auto_created_date";
const AI_PROVIDER_CUSTOM: &str = "custom";
const AI_PROVIDER_LEGACY_PI: &str = "pi_coding_agent";
const AI_PROVIDER_DEFAULT_CUSTOM_ID: &str = "wx-xd-custom";
const AI_PROVIDER_DEFAULT_API: &str = "openai-completions";
const AI_PROVIDER_DEFAULT_CONTEXT_WINDOW: i64 = 128_000;
const AI_PROVIDER_DEFAULT_MAX_TOKENS: i64 = 4_096;
const AI_PROVIDER_ENABLED_SETTING: &str = "ai_provider.enabled";
const AI_PROVIDER_TYPE_SETTING: &str = "ai_provider.provider_type";
const AI_PROVIDER_CUSTOM_ID_SETTING: &str = "ai_provider.custom_provider_id";
const AI_PROVIDER_API_SETTING: &str = "ai_provider.api";
const AI_PROVIDER_BASE_URL_SETTING: &str = "ai_provider.base_url";
const AI_PROVIDER_MODEL_SETTING: &str = "ai_provider.model";
const AI_PROVIDER_TEMPERATURE_SETTING: &str = "ai_provider.temperature";
const AI_PROVIDER_CONTEXT_WINDOW_SETTING: &str = "ai_provider.context_window";
const AI_PROVIDER_MAX_TOKENS_SETTING: &str = "ai_provider.max_tokens";
const IMAGE_DOWNLOAD_TIMEOUT_SECONDS: u64 = 15;
const IMAGE_DOWNLOAD_MAX_BYTES: u64 = 20 * 1024 * 1024;
// 微信 shop/ec/basics/img/upload 单图体积上限：实测约 2MB。此前误设为 10MB，导致 2~10MB 的
// 淘宝原图被当作合规直传、被微信回 45002 content size out of limit。超过 MAX 的图会走
// encode_jpeg_under_limit 压缩转 JPEG；TARGET 留 buffer，确保压缩结果稳定低于硬限制。
const WECHAT_IMAGE_MAX_BYTES: usize = 2 * 1024 * 1024;
const WECHAT_IMAGE_TARGET_BYTES: usize = 1_800_000;
const IMAGE_USER_AGENT: &str = "wx-xd-image-preflight/0.1";
const IMAGE_ACCEPT_HEADER: &str = "image/avif,image/webp,image/apng,image/*,*/*;q=0.8";
// 主图视频搬运（淘宝 → 微信小店 head_videos）相关常量。微信 scene_type=162 商品视频限制 500MB/180秒。
// 淘宝 cloud.video.taobao.com 有防盗链：裸请求返回 490「非法访问」，必须带浏览器 UA + Referer 且
// 跟随 302 才能下载到 video/mp4（实测 Chrome UA + item.taobao.com Referer 可过）。
const VIDEO_DOWNLOAD_TIMEOUT_SECONDS: u64 = 60;
const VIDEO_DOWNLOAD_MAX_BYTES: u64 = 500 * 1024 * 1024;
const VIDEO_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const VIDEO_DOWNLOAD_REFERER: &str = "https://item.taobao.com/";

#[derive(Debug)]
struct TargetShop {
    id: String,
    name: String,
}

#[derive(Debug)]
struct PendingPublishItem {
    item_id: String,
    job_id: String,
    product_row_id: String,
    shop_id: String,
    shop_status: String,
    shop_has_secret: bool,
    external_product_id: String,
    raw_payload: String,
}

#[derive(Debug)]
struct PendingPriceUpdateItem {
    item_id: String,
    job_id: String,
    shop_id: String,
    shop_status: String,
    shop_has_secret: bool,
    external_product_id: String,
    wechat_product_id: Option<String>,
    target_price_cents: i64,
}

#[derive(Debug)]
struct ProductAsset {
    kind: &'static str,
    sort_order: i64,
    source_url: String,
}

#[derive(Debug)]
struct PreparedImageUpload {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    mime_type: &'static str,
    file_name: String,
    local_path: PathBuf,
    summary: String,
}

#[derive(Debug)]
struct AddProductPayloadDraft {
    payload: Value,
    source: &'static str,
    warnings: Vec<String>,
}

#[derive(Debug)]
struct AddProductPayloadPrepare {
    source: &'static str,
    warnings: Vec<String>,
}

#[derive(Debug)]
struct CachedWechatCategory {
    cat_id: i64,
    parent_cat_id: Option<i64>,
    level: Option<i64>,
    name: String,
    raw_payload: Value,
}

#[derive(Debug)]
struct CachedWechatCategoryRelation {
    cat_id: i64,
    status: i64,
    uneffective_reason: Option<String>,
    effective_time: Option<i64>,
    uneffective_time: Option<i64>,
    qua_id: Option<i64>,
    raw_payload: Value,
}

#[derive(Debug, Default)]
struct CategoryDetailCounts {
    product_attr_count: i64,
    sale_attr_count: i64,
    product_qua_count: i64,
}

#[derive(Debug, Default)]
struct CachedCategoryRequirementCheck {
    missing_product_attrs: Vec<String>,
    missing_sale_attrs: Vec<String>,
}

impl CachedCategoryRequirementCheck {
    fn has_missing_attrs(&self) -> bool {
        !self.missing_product_attrs.is_empty() || !self.missing_sale_attrs.is_empty()
    }

    fn failure_summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.missing_product_attrs.is_empty() {
            parts.push(format!(
                "缺少商品必填属性：{}",
                self.missing_product_attrs.join("、")
            ));
        }
        if !self.missing_sale_attrs.is_empty() {
            parts.push(format!(
                "缺少销售必填属性：{}",
                self.missing_sale_attrs.join("、")
            ));
        }
        format!(
            "{}。请用 AI/外部系统补齐 metadata.wechat_attrs 和 skus[].specs，或直接传完整 metadata.wechat_add_product_payload",
            parts.join("；")
        )
    }
}

#[derive(Debug, Clone)]
struct CategoryRequiredAttr {
    key: String,
    options: Vec<String>,
    attr_type: Option<String>,
    append_allowed: bool,
    related_options: Vec<String>,
}

#[derive(Debug, Clone)]
struct SkuAttrFill {
    sku_index: usize,
    value: String,
}

#[derive(Debug, Clone)]
struct AttributeFillSuggestion {
    attr_kind: &'static str,
    attr_key: String,
    suggested_value: Option<String>,
    sku_values: Vec<SkuAttrFill>,
    confidence: i64,
    source: String,
    applied: bool,
    prompt_json: Value,
}

#[derive(Debug, Default)]
struct AttributeFillPlan {
    suggestions: Vec<AttributeFillSuggestion>,
}

impl AttributeFillPlan {
    fn can_auto_apply(&self) -> bool {
        !self.suggestions.is_empty()
            && self.suggestions.iter().all(|suggestion| {
                suggestion.applied
                    && suggestion.confidence >= 65
                    && (suggestion.suggested_value.is_some() || !suggestion.sku_values.is_empty())
            })
    }

    fn suggestion_summary(&self) -> String {
        let unresolved = self
            .suggestions
            .iter()
            .filter(|suggestion| !suggestion.applied)
            .map(|suggestion| {
                format!(
                    "{}属性 {}",
                    if suggestion.attr_kind == "product" {
                        "商品"
                    } else {
                        "销售"
                    },
                    suggestion.attr_key
                )
            })
            .collect::<Vec<_>>();
        if unresolved.is_empty() {
            format!(
                "已生成 {} 条必填属性补齐建议，但置信度不足，需人工确认后重试",
                self.suggestions.len()
            )
        } else {
            format!(
                "仍需 AI/人工确认：{}；已记录补齐建议，确认后可重新执行属性补齐",
                unresolved.join("、")
            )
        }
    }
}

#[derive(Debug)]
enum AssetUploadOutcome {
    Uploaded,
    Reused,
    Failed,
}

#[derive(Debug)]
struct PreparedAsset {
    kind: String,
    sort_order: i64,
    wechat_url: String,
    /// 原始淘宝图片 URL，作为 SKU 颜色图 → 微信 thumb_img 的映射键。
    source_url: String,
}

#[derive(Debug)]
struct StatusSyncItem {
    item_id: String,
    job_id: String,
    shop_id: String,
    external_product_id: String,
    wechat_product_id: String,
}

#[derive(Debug)]
struct ProductAuditResolution {
    status: &'static str,
    error_code: Option<String>,
    summary: String,
    wechat_status: Option<i64>,
    wechat_edit_status: Option<i64>,
}

#[derive(Debug)]
struct PriceUpdateConfirmationResolution {
    status: &'static str,
    error_code: Option<String>,
    summary: String,
    wechat_status: Option<i64>,
    wechat_edit_status: Option<i64>,
}

#[derive(Debug, Clone)]
struct AiProviderConfig {
    provider_type: String,
    custom_provider_id: String,
    api: String,
    base_url: String,
    model: String,
    temperature: f64,
    context_window: i64,
    max_tokens: i64,
    api_key: String,
}

#[derive(Debug)]
struct AiApiKeyRecord {
    encrypted_api_key: String,
    api_key_nonce: String,
    api_key_fingerprint: String,
}

#[derive(Debug)]
struct OrderSyncShop {
    shop_id: String,
    shop_name: String,
}

#[derive(Debug)]
struct OrderDetailSyncItem {
    order_id: String,
    shop_id: String,
    wechat_order_id: String,
}

#[derive(Debug)]
struct PurchaseCandidate {
    order_id: String,
    order_item_id: String,
    shop_id: String,
    out_product_id: Option<String>,
    out_sku_id: Option<String>,
    source_url: Option<String>,
    quantity: i64,
}

#[derive(Debug, Clone)]
struct AftersaleActionTarget {
    id: String,
    shop_id: String,
    wechat_aftersale_id: String,
    status: String,
}

#[derive(Debug, Default)]
struct InventorySourceProduct {
    title: String,
    supplier_name: Option<String>,
    supplier_product_id: Option<String>,
    total_stock: i64,
    updated_at: String,
}

#[derive(Debug, Default, Clone)]
struct InventoryPurchaseAggregate {
    reserved_quantity: i64,
    pending_purchase_quantity: i64,
    supplier_issue_count: i64,
}

#[derive(Debug, Default, Clone)]
struct ProductSalesAggregate {
    title: String,
    order_count: i64,
    units_sold: i64,
    revenue_cents: i64,
    last_order_at: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct ProductPurchaseCostAggregate {
    purchase_task_count: i64,
    missing_cost_count: i64,
    purchase_cost_cents: i64,
}

#[derive(Debug, Default, Clone)]
struct ProductAftersaleAggregate {
    related_aftersale_count: i64,
    related_refund_cents: i64,
}

#[derive(Debug)]
struct PurchaseTaskShipmentRef {
    purchase_task_id: String,
    order: ShipmentOrderRef,
}

#[derive(Debug)]
struct ShipmentOrderRef {
    order_id: String,
    shop_id: String,
    wechat_order_id: String,
}

#[derive(Debug)]
struct ShipmentFields {
    delivery_id: Option<String>,
    delivery_name: Option<String>,
    waybill_id: Option<String>,
    deliver_type: i64,
}

#[derive(Debug)]
struct ShipmentUpsertResult {
    shipment_id: String,
    status: String,
    auto_send_enabled: bool,
    message: String,
}

/// 多运单拆包结果（订单履约三期）：一单多供应商物流时按运单分组生成的多个 shipment 汇总。
#[derive(Debug)]
struct MultiShipmentUpsertResult {
    status: String,
    auto_send_enabled: bool,
    message: String,
}

#[derive(Debug)]
struct ShipmentCandidate {
    shipment_id: String,
    order_id: String,
    shop_id: String,
    wechat_order_id: String,
    delivery_id: Option<String>,
    waybill_id: Option<String>,
    deliver_type: i64,
    /// 候选捞取时的状态（ready_to_send / blocked）：守卫拦截通知只在首次拦截时发
    status: String,
}

#[derive(Debug)]
struct ShipmentProductInfo {
    product_id: String,
    sku_id: String,
    product_cnt: i64,
}
