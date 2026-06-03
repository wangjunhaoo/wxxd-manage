use crate::commands;
use crate::models::{
    AftersaleAcceptRequest, AftersaleEvidenceRecordRequest, AftersaleEvidenceStatusUpdateRequest,
    AftersaleRejectRequest, AftersaleResponsibilityRequest, ExternalPublishJobRequest,
    GuaranteeFollowupRequest, LocalApiConfig, LocalApiKeyRotationResult,
    OrderProfitAdjustmentRequest, PriceUpdateJobRequest, PurchaseTaskIssueRequest,
    PurchaseTaskMappingRequest, PurchaseTaskShipmentRequest, ShipmentRecordRequest,
    SupplierAftersaleFollowupRecordRequest,
};
use crate::storage::{now_shanghai, open_connection, AppError, AppResult};
use axum::{
    extract::{Path, Query, State},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderMap, HeaderName, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::time::Instant;
use tauri::AppHandle;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

const LOCAL_API_HOST: &str = "127.0.0.1";
const LOCAL_API_PORT: u16 = 17890;
const LOCAL_API_KEY_HASH_SETTING: &str = "local_api.key_sha256";
const LOCAL_API_KEY_HINT_SETTING: &str = "local_api.key_hint";
const LOCAL_API_HEADER: &str = "x-wx-xd-api-key";

#[derive(Clone)]
struct LocalApiState {
    app: AppHandle,
}

#[derive(Debug, Deserialize)]
struct PurchaseTaskQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InventoryRiskQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProductSalesAnalysisQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleEvidenceQuery {
    target_type: Option<String>,
    target_id: Option<String>,
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleRejectReasonQuery {
    shop_id: Option<String>,
    reject_scene: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GuaranteeOrderQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeliveryCompanyQuery {
    shop_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeliveryShipmentQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OrderProfitQuery {
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeliveryCompanySyncBody {
    shop_id: String,
    ewaybill_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AutoSendDeliveryBody {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct ShipmentRecordBody {
    order_id: Option<String>,
    shop_id: Option<String>,
    wechat_order_id: Option<String>,
    delivery_id: Option<String>,
    delivery_name: Option<String>,
    waybill_id: Option<String>,
    deliver_type: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PurchaseTaskShipmentBody {
    delivery_id: Option<String>,
    delivery_name: Option<String>,
    waybill_id: Option<String>,
    deliver_type: Option<i64>,
    estimated_cost: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PurchaseTaskMappingBody {
    external_product_id: String,
    external_sku_id: String,
    source_url: Option<String>,
    supplier_name: Option<String>,
    supplier_product_id: Option<String>,
    estimated_cost: Option<f64>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PurchaseTaskIssueBody {
    issue_type: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleAcceptBody {
    address_id: Option<String>,
    accept_type: Option<i64>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleRejectBody {
    reject_reason_type: i64,
    reject_reason: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleResponsibilityBody {
    responsibility_party: String,
    responsibility_note: Option<String>,
    supplier_compensation_cents: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AftersaleRejectReasonSyncBody {
    shop_id: String,
}

#[derive(Debug, Deserialize)]
struct AftersaleEvidenceRecordBody {
    target_type: String,
    target_id: String,
    evidence_type: String,
    title: String,
    content_text: Option<String>,
    local_file_path: Option<String>,
    source_url: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AftersaleEvidenceStatusBody {
    status: String,
}

#[derive(Debug, Deserialize)]
struct AftersaleEvidenceExportBody {
    target_type: Option<String>,
    target_id: Option<String>,
    status: Option<String>,
    format: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SupplierAftersaleFollowupQuery {
    target_type: Option<String>,
    target_id: Option<String>,
    status: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SupplierAftersaleFollowupBody {
    target_type: String,
    target_id: String,
    followup_type: String,
    status: String,
    note: String,
    purchase_task_id: Option<String>,
    supplier_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GuaranteeFollowupBody {
    handling_status: String,
    responsibility_party: Option<String>,
    handling_note: Option<String>,
    supplier_compensation_cents: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OrderProfitAdjustmentBody {
    kind: String,
    amount_cents: i64,
    note: Option<String>,
}

#[tauri::command]
pub fn get_local_api_config(app: AppHandle) -> AppResult<LocalApiConfig> {
    local_api_config(&app)
}

#[tauri::command]
pub fn rotate_local_api_key(app: AppHandle) -> AppResult<LocalApiKeyRotationResult> {
    let api_key = generate_api_key();
    let key_hash = hash_api_key(&api_key);
    let key_hint = format!(
        "末尾 {}",
        api_key
            .chars()
            .rev()
            .take(6)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
    );
    set_string_setting(&app, LOCAL_API_KEY_HASH_SETTING, &key_hash)?;
    set_string_setting(&app, LOCAL_API_KEY_HINT_SETTING, &key_hint)?;
    Ok(LocalApiKeyRotationResult {
        api_key,
        key_hint,
        base_url: local_api_base_url(),
        warning: "API Key 只展示这一次；重置后旧 Key 立即失效。".to_string(),
    })
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = serve(app).await {
            eprintln!("本地 HTTP API 启动失败：{error}");
        }
    });
}

async fn serve(app: AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = LocalApiState { app };
    let api_key_header = HeaderName::from_static(LOCAL_API_HEADER);
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, api_key_header]);
    let router = Router::new()
        .route("/health", get(health_handler))
        .route("/api/shop-groups", get(list_shop_groups_handler))
        .route("/api/shops", get(list_shops_handler))
        .route("/api/publish-jobs", post(create_publish_job_handler))
        .route("/api/publish-jobs/{task_id}", get(get_publish_job_handler))
        .route(
            "/api/price-update-jobs",
            post(create_price_update_job_handler),
        )
        .route(
            "/api/price-update-jobs/{task_id}",
            get(get_price_update_job_handler),
        )
        .route("/api/task-runs", get(list_task_runs_handler))
        .route("/api/notifications", get(list_notifications_handler))
        .route(
            "/api/notifications/{notification_id}/read",
            post(mark_notification_read_handler),
        )
        .route("/api/purchase-tasks", get(list_purchase_tasks_handler))
        .route(
            "/api/purchase-tasks/{purchase_task_id}/mapping",
            post(resolve_purchase_task_mapping_handler),
        )
        .route(
            "/api/purchase-tasks/{purchase_task_id}/shipment",
            post(record_purchase_task_shipment_handler),
        )
        .route(
            "/api/purchase-tasks/{purchase_task_id}/issue",
            post(mark_purchase_task_issue_handler),
        )
        .route("/api/inventory-risks", get(list_inventory_risks_handler))
        .route(
            "/api/product-sales-analysis",
            get(list_product_sales_analysis_handler),
        )
        .route("/api/aftersales", get(list_aftersales_handler))
        .route(
            "/api/aftersale-evidence",
            get(list_aftersale_evidence_handler).post(record_aftersale_evidence_handler),
        )
        .route(
            "/api/aftersale-evidence/export",
            post(export_aftersale_evidence_handler),
        )
        .route(
            "/api/aftersale-evidence/{evidence_id}/status",
            post(update_aftersale_evidence_status_handler),
        )
        .route(
            "/api/supplier-aftersale-followups",
            get(list_supplier_aftersale_followups_handler)
                .post(record_supplier_aftersale_followup_handler),
        )
        .route(
            "/api/aftersale-reject-reasons",
            get(list_aftersale_reject_reasons_handler),
        )
        .route(
            "/api/aftersale-reject-reasons/sync",
            post(sync_aftersale_reject_reasons_handler),
        )
        .route(
            "/api/aftersales/{aftersale_id}/accept",
            post(accept_aftersale_handler),
        )
        .route(
            "/api/aftersales/{aftersale_id}/reject",
            post(reject_aftersale_handler),
        )
        .route(
            "/api/aftersales/{aftersale_id}/responsibility",
            post(record_aftersale_responsibility_handler),
        )
        .route("/api/guarantee-orders", get(list_guarantee_orders_handler))
        .route(
            "/api/guarantee-orders/{guarantee_order_id}/followup",
            post(record_guarantee_followup_handler),
        )
        .route("/api/order-profits", get(list_order_profits_handler))
        .route(
            "/api/order-profits/{order_id}/adjustments",
            post(record_order_profit_adjustment_handler),
        )
        .route("/api/delivery-settings", get(get_delivery_settings_handler))
        .route(
            "/api/delivery-settings/auto-send",
            post(set_auto_send_delivery_handler),
        )
        .route(
            "/api/delivery-companies",
            get(list_delivery_companies_handler),
        )
        .route(
            "/api/delivery-companies/sync",
            post(sync_delivery_companies_handler),
        )
        .route(
            "/api/delivery-shipments",
            get(list_delivery_shipments_handler).post(record_delivery_shipment_handler),
        )
        .route(
            "/api/delivery-shipments/{shipment_id}/retry",
            post(retry_delivery_shipment_handler),
        )
        .route("/api/runners/order-sync", post(run_order_sync_handler))
        .route(
            "/api/runners/order-detail-sync",
            post(run_order_detail_sync_handler),
        )
        .route(
            "/api/runners/aftersale-sync",
            post(run_aftersale_sync_handler),
        )
        .route(
            "/api/runners/purchase-task-generation",
            post(run_purchase_task_generation_handler),
        )
        .route(
            "/api/runners/delivery-submit",
            post(run_delivery_submit_handler),
        )
        .route(
            "/api/runners/publish-pipeline",
            post(run_publish_pipeline_handler),
        )
        .route(
            "/api/runners/price-precheck",
            post(run_price_precheck_handler),
        )
        .route("/api/runners/price-submit", post(run_price_submit_handler))
        .route(
            "/api/runners/price-confirm",
            post(run_price_confirm_handler),
        )
        .route(
            "/api/runners/inventory-risk-scan",
            post(run_inventory_risk_scan_handler),
        )
        .route(
            "/api/runners/guarantee-sync",
            post(run_guarantee_sync_handler),
        )
        .route("/api/runners/operations", post(run_operations_handler))
        // ── Agent 工具端点 ──────────────────────────────────
        .route(
            "/api/agent/products/{task_id}",
            get(get_agent_product_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/categories/search",
            get(search_agent_categories_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/categories/{cat_id}",
            get(get_agent_category_detail_handler),
        )
        .route("/api/agent/shops/{shop_id}", get(get_agent_shop_handler))
        .route("/api/agent/docs/search", get(search_agent_docs_handler))
        .route(
            "/api/agent/shops/{shop_id}/categories/active",
            get(get_agent_active_categories_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/categories/tree",
            get(get_agent_category_tree_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/freight-templates",
            get(get_agent_freight_templates_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/after-sale-addresses",
            get(get_agent_after_sale_addresses_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/delivery-companies",
            get(get_agent_delivery_companies_handler),
        )
        // P2：订单 / 售后 / 采购 / 商品 / 分析
        .route("/api/agent/orders/{order_id}", get(get_agent_order_handler))
        .route("/api/agent/orders", get(list_agent_orders_handler))
        .route(
            "/api/agent/aftersales/{aftersale_id}",
            get(get_agent_aftersale_handler),
        )
        .route("/api/agent/aftersales", get(list_agent_aftersales_handler))
        .route(
            "/api/agent/shops/{shop_id}/reject-reasons",
            get(get_agent_reject_reasons_handler),
        )
        .route(
            "/api/agent/purchase-tasks/{task_id}",
            get(get_agent_purchase_task_handler),
        )
        .route(
            "/api/agent/shops/{shop_id}/products/{external_product_id}",
            get(get_agent_shop_product_handler),
        )
        .route(
            "/api/agent/collection-tasks",
            get(list_agent_collections_handler),
        )
        .route(
            "/api/agent/product-sales/{product_id}",
            get(get_agent_product_sales_handler),
        )
        .route(
            "/api/agent/inventory-risks",
            get(get_agent_inventory_risk_handler),
        )
        .route(
            "/api/agent/profit-summary",
            get(get_agent_profit_summary_handler),
        )
        .with_state(state)
        .layer(cors);

    let addr: SocketAddr = format!("{LOCAL_API_HOST}:{LOCAL_API_PORT}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

async fn health_handler(State(state): State<LocalApiState>) -> Response {
    json_success(json!({
        "status": "ok",
        "service": "wx-xd-local-api",
        "base_url": local_api_base_url(),
        "has_api_key": local_api_config(&state.app)
            .map(|config| config.has_api_key)
            .unwrap_or(false),
    }))
}

async fn list_shop_groups_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(&state, &headers, "GET", "/api/shop-groups", None, |app| {
        commands::list_shop_groups(app)
    })
}

async fn list_shops_handler(State(state): State<LocalApiState>, headers: HeaderMap) -> Response {
    authenticated(&state, &headers, "GET", "/api/shops", None, |app| {
        commands::list_shops(app)
    })
}

async fn create_publish_job_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(request): Json<ExternalPublishJobRequest>,
) -> Response {
    let request_summary = publish_job_request_summary(&request);
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/publish-jobs",
        Some(request_summary),
        |app| commands::create_external_publish_job(app, request),
    )
}

async fn get_publish_job_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> Response {
    let request_summary = format!("task_id={task_id}");
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/publish-jobs/{task_id}",
        Some(request_summary),
        |app| commands::get_publish_job(app, task_id),
    )
}

async fn create_price_update_job_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(request): Json<PriceUpdateJobRequest>,
) -> Response {
    let request_summary = price_update_request_summary(&request);
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/price-update-jobs",
        Some(request_summary),
        |app| commands::create_price_update_job(app, request),
    )
}

async fn get_price_update_job_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> Response {
    let request_summary = format!("task_id={task_id}");
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/price-update-jobs/{task_id}",
        Some(request_summary),
        |app| commands::get_price_update_job(app, task_id),
    )
}

async fn list_task_runs_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(&state, &headers, "GET", "/api/task-runs", None, |app| {
        commands::list_task_runs(app)
    })
}

async fn list_notifications_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/notifications",
        Some("status=unread limit=120".to_string()),
        |app| commands::list_notifications(app, Some("unread".to_string()), None, Some(120)),
    )
}

async fn mark_notification_read_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(notification_id): Path<String>,
) -> Response {
    let request_summary = format!("notification_id={notification_id}");
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/notifications/{notification_id}/read",
        Some(request_summary),
        |app| commands::mark_notification_read(app, notification_id),
    )
}

async fn list_purchase_tasks_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<PurchaseTaskQuery>,
) -> Response {
    let request_summary = purchase_tasks_query_summary(&query);
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/purchase-tasks",
        Some(request_summary),
        |app| {
            commands::list_purchase_tasks(app, status, parse_purchase_task_limit(limit.as_deref())?)
        },
    )
}

async fn resolve_purchase_task_mapping_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(purchase_task_id): Path<String>,
    Json(body): Json<PurchaseTaskMappingBody>,
) -> Response {
    let request_summary = purchase_mapping_request_summary(&purchase_task_id, &body);
    let request = PurchaseTaskMappingRequest {
        purchase_task_id,
        external_product_id: body.external_product_id,
        external_sku_id: body.external_sku_id,
        source_url: body.source_url,
        supplier_name: body.supplier_name,
        supplier_product_id: body.supplier_product_id,
        estimated_cost: body.estimated_cost,
        note: body.note,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/purchase-tasks/{purchase_task_id}/mapping",
        Some(request_summary),
        |app| commands::resolve_purchase_task_mapping(app, request),
    )
}

async fn record_purchase_task_shipment_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(purchase_task_id): Path<String>,
    Json(body): Json<PurchaseTaskShipmentBody>,
) -> Response {
    let request_summary = purchase_shipment_request_summary(&purchase_task_id, &body);
    let request = PurchaseTaskShipmentRequest {
        purchase_task_id,
        delivery_id: body.delivery_id,
        delivery_name: body.delivery_name,
        waybill_id: body.waybill_id,
        deliver_type: body.deliver_type,
        estimated_cost: body.estimated_cost,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/purchase-tasks/{purchase_task_id}/shipment",
        Some(request_summary),
        |app| commands::record_purchase_task_shipment(app, request),
    )
}

async fn mark_purchase_task_issue_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(purchase_task_id): Path<String>,
    Json(body): Json<PurchaseTaskIssueBody>,
) -> Response {
    let request_summary = purchase_issue_request_summary(&purchase_task_id, &body);
    let request = PurchaseTaskIssueRequest {
        purchase_task_id,
        issue_type: body.issue_type,
        note: body.note,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/purchase-tasks/{purchase_task_id}/issue",
        Some(request_summary),
        |app| commands::mark_purchase_task_issue(app, request),
    )
}

async fn list_inventory_risks_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<InventoryRiskQuery>,
) -> Response {
    let request_summary = inventory_risks_query_summary(&query);
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/inventory-risks",
        Some(request_summary),
        |app| commands::list_inventory_risks(app, status, parse_limit(limit.as_deref())?),
    )
}

async fn list_product_sales_analysis_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<ProductSalesAnalysisQuery>,
) -> Response {
    let request_summary = product_sales_analysis_query_summary(&query);
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/product-sales-analysis",
        Some(request_summary),
        |app| commands::list_product_sales_analysis(app, status, parse_limit(limit.as_deref())?),
    )
}

async fn list_aftersales_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<AftersaleQuery>,
) -> Response {
    let request_summary = format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    );
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/aftersales",
        Some(request_summary),
        |app| commands::list_aftersales(app, status, parse_limit(limit.as_deref())?),
    )
}

async fn list_aftersale_evidence_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<AftersaleEvidenceQuery>,
) -> Response {
    let request_summary = format!(
        "target_type={} target_id_present={} status={} limit={}",
        query
            .target_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .target_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some(),
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    );
    let target_type = query.target_type;
    let target_id = query.target_id;
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/aftersale-evidence",
        Some(request_summary),
        |app| {
            commands::list_aftersale_evidence(
                app,
                target_type,
                target_id,
                status,
                parse_limit(limit.as_deref())?,
            )
        },
    )
}

async fn record_aftersale_evidence_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<AftersaleEvidenceRecordBody>,
) -> Response {
    let request_summary = format!(
        "target_type={} target_id_present={} evidence_type={} status={} has_content={} has_file={} has_source={}",
        body.target_type.trim(),
        !body.target_id.trim().is_empty(),
        body.evidence_type.trim(),
        body.status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("draft"),
        body.content_text
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.local_file_path
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.source_url
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    );
    let request = AftersaleEvidenceRecordRequest {
        target_type: body.target_type,
        target_id: body.target_id,
        evidence_type: body.evidence_type,
        title: body.title,
        content_text: body.content_text,
        local_file_path: body.local_file_path,
        source_url: body.source_url,
        status: body.status,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/aftersale-evidence",
        Some(request_summary),
        |app| commands::record_aftersale_evidence(app, request),
    )
}

async fn update_aftersale_evidence_status_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(evidence_id): Path<String>,
    Json(body): Json<AftersaleEvidenceStatusBody>,
) -> Response {
    let request_summary = format!(
        "evidence_id_present={} status={}",
        !evidence_id.trim().is_empty(),
        body.status.trim()
    );
    let request = AftersaleEvidenceStatusUpdateRequest {
        evidence_id,
        status: body.status,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/aftersale-evidence/{evidence_id}/status",
        Some(request_summary),
        |app| commands::update_aftersale_evidence_status(app, request),
    )
}

async fn export_aftersale_evidence_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<AftersaleEvidenceExportBody>,
) -> Response {
    let request_summary = format!(
        "target_type={} target_id_present={} status={} format={}",
        body.target_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        body.target_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some(),
        body.status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        body.format
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("md")
    );
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/aftersale-evidence/export",
        Some(request_summary),
        |app| {
            commands::export_aftersale_evidence(
                app,
                body.target_type,
                body.target_id,
                body.status,
                body.format,
            )
        },
    )
}

async fn list_supplier_aftersale_followups_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<SupplierAftersaleFollowupQuery>,
) -> Response {
    let request_summary = format!(
        "target_type={} target_id_present={} status={} limit={}",
        query
            .target_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .target_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some(),
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    );
    let target_type = query.target_type;
    let target_id = query.target_id;
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/supplier-aftersale-followups",
        Some(request_summary),
        |app| {
            commands::list_supplier_aftersale_followups(
                app,
                target_type,
                target_id,
                status,
                parse_limit(limit.as_deref())?,
            )
        },
    )
}

async fn record_supplier_aftersale_followup_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<SupplierAftersaleFollowupBody>,
) -> Response {
    let request_summary = format!(
        "target_type={} target_id_present={} followup_type={} status={} purchase_task_present={} supplier_present={} has_note={}",
        body.target_type.trim(),
        !body.target_id.trim().is_empty(),
        body.followup_type.trim(),
        body.status.trim(),
        body.purchase_task_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.supplier_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        !body.note.trim().is_empty()
    );
    let request = SupplierAftersaleFollowupRecordRequest {
        target_type: body.target_type,
        target_id: body.target_id,
        followup_type: body.followup_type,
        status: body.status,
        note: body.note,
        purchase_task_id: body.purchase_task_id,
        supplier_name: body.supplier_name,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/supplier-aftersale-followups",
        Some(request_summary),
        |app| commands::record_supplier_aftersale_followup(app, request),
    )
}

async fn list_aftersale_reject_reasons_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<AftersaleRejectReasonQuery>,
) -> Response {
    let request_summary = format!(
        "shop_id={} reject_scene={}",
        query
            .shop_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .reject_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all")
    );
    let shop_id = query.shop_id;
    let reject_scene = parse_limit(query.reject_scene.as_deref());
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/aftersale-reject-reasons",
        Some(request_summary),
        |app| commands::list_aftersale_reject_reasons(app, shop_id, reject_scene?),
    )
}

async fn sync_aftersale_reject_reasons_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<AftersaleRejectReasonSyncBody>,
) -> Response {
    let request_summary = format!("shop_id={}", body.shop_id.trim());
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/aftersale-reject-reasons/sync",
        Some(request_summary),
        |app| commands::sync_aftersale_reject_reasons(app, body.shop_id),
    )
    .await
}

async fn accept_aftersale_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(aftersale_id): Path<String>,
    Json(body): Json<AftersaleAcceptBody>,
) -> Response {
    let request_summary = format!(
        "aftersale_id={} has_address={} accept_type={} has_note={}",
        aftersale_id,
        body.address_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some(),
        body.accept_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".to_string()),
        body.note
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
    );
    let request = AftersaleAcceptRequest {
        aftersale_id,
        address_id: body.address_id,
        accept_type: body.accept_type,
        note: body.note,
    };
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/aftersales/{aftersale_id}/accept",
        Some(request_summary),
        |app| commands::accept_aftersale(app, request),
    )
    .await
}

async fn reject_aftersale_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(aftersale_id): Path<String>,
    Json(body): Json<AftersaleRejectBody>,
) -> Response {
    let request_summary = format!(
        "aftersale_id={} reject_reason_type={} has_reason={} has_note={}",
        aftersale_id,
        body.reject_reason_type,
        body.reject_reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some(),
        body.note
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
    );
    let request = AftersaleRejectRequest {
        aftersale_id,
        reject_reason_type: body.reject_reason_type,
        reject_reason: body.reject_reason,
        note: body.note,
    };
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/aftersales/{aftersale_id}/reject",
        Some(request_summary),
        |app| commands::reject_aftersale(app, request),
    )
    .await
}

async fn record_aftersale_responsibility_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(aftersale_id): Path<String>,
    Json(body): Json<AftersaleResponsibilityBody>,
) -> Response {
    let request_summary = format!(
        "aftersale_id={} responsibility_party={} supplier_compensation_present={} has_note={}",
        aftersale_id,
        body.responsibility_party.trim(),
        body.supplier_compensation_cents.is_some(),
        body.responsibility_note
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    );
    let request = AftersaleResponsibilityRequest {
        aftersale_id,
        responsibility_party: body.responsibility_party,
        responsibility_note: body.responsibility_note,
        supplier_compensation_cents: body.supplier_compensation_cents,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/aftersales/{aftersale_id}/responsibility",
        Some(request_summary),
        |app| commands::record_aftersale_responsibility(app, request),
    )
}

async fn list_guarantee_orders_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<GuaranteeOrderQuery>,
) -> Response {
    let request_summary = format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    );
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/guarantee-orders",
        Some(request_summary),
        |app| commands::list_guarantee_orders(app, status, parse_limit(limit.as_deref())?),
    )
}

async fn record_guarantee_followup_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(guarantee_order_id): Path<String>,
    Json(body): Json<GuaranteeFollowupBody>,
) -> Response {
    let request_summary = format!(
        "guarantee_order_id={} handling_status={} responsibility_party={} supplier_compensation_present={} has_note={}",
        guarantee_order_id,
        body.handling_status.trim(),
        body.responsibility_party
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("none"),
        body.supplier_compensation_cents.is_some(),
        body.handling_note
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    );
    let request = GuaranteeFollowupRequest {
        guarantee_order_id,
        handling_status: body.handling_status,
        responsibility_party: body.responsibility_party,
        handling_note: body.handling_note,
        supplier_compensation_cents: body.supplier_compensation_cents,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/guarantee-orders/{guarantee_order_id}/followup",
        Some(request_summary),
        |app| commands::record_guarantee_followup(app, request),
    )
}

async fn list_order_profits_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<OrderProfitQuery>,
) -> Response {
    let request_summary = format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    );
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/order-profits",
        Some(request_summary),
        |app| commands::list_order_profit_summaries(app, status, parse_limit(limit.as_deref())?),
    )
}

async fn record_order_profit_adjustment_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
    Json(body): Json<OrderProfitAdjustmentBody>,
) -> Response {
    let request_summary = format!(
        "order_id={} kind={} amount_cents={} has_note={}",
        order_id,
        body.kind.trim(),
        body.amount_cents,
        body.note
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    );
    let request = OrderProfitAdjustmentRequest {
        order_id,
        kind: body.kind,
        amount_cents: body.amount_cents,
        note: body.note,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/order-profits/{order_id}/adjustments",
        Some(request_summary),
        |app| commands::record_order_profit_adjustment(app, request),
    )
}

async fn get_delivery_settings_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/delivery-settings",
        None,
        commands::get_delivery_settings,
    )
}

async fn set_auto_send_delivery_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<AutoSendDeliveryBody>,
) -> Response {
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/delivery-settings/auto-send",
        Some(format!("enabled={}", body.enabled)),
        |app| commands::set_auto_send_delivery(app, body.enabled),
    )
}

async fn list_delivery_companies_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<DeliveryCompanyQuery>,
) -> Response {
    let request_summary = format!(
        "shop_id={}",
        query
            .shop_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all")
    );
    let shop_id = query.shop_id;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/delivery-companies",
        Some(request_summary),
        |app| commands::list_delivery_companies(app, shop_id),
    )
}

async fn sync_delivery_companies_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<DeliveryCompanySyncBody>,
) -> Response {
    let request_summary = format!(
        "shop_id={} ewaybill_only={}",
        body.shop_id.trim(),
        body.ewaybill_only.unwrap_or(false)
    );
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/delivery-companies/sync",
        Some(request_summary),
        |app| commands::sync_delivery_companies(app, body.shop_id, body.ewaybill_only),
    )
    .await
}

async fn list_delivery_shipments_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Query(query): Query<DeliveryShipmentQuery>,
) -> Response {
    let request_summary = format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("100")
    );
    let status = query.status;
    let limit = query.limit;
    authenticated(
        &state,
        &headers,
        "GET",
        "/api/delivery-shipments",
        Some(request_summary),
        |app| commands::list_delivery_shipments(app, status, parse_limit(limit.as_deref())?),
    )
}

async fn record_delivery_shipment_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Json(body): Json<ShipmentRecordBody>,
) -> Response {
    let request_summary = delivery_shipment_record_summary(&body);
    let request = ShipmentRecordRequest {
        order_id: body.order_id,
        shop_id: body.shop_id,
        wechat_order_id: body.wechat_order_id,
        delivery_id: body.delivery_id,
        delivery_name: body.delivery_name,
        waybill_id: body.waybill_id,
        deliver_type: body.deliver_type,
    };
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/delivery-shipments",
        Some(request_summary),
        |app| commands::record_order_shipment(app, request),
    )
}

async fn retry_delivery_shipment_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    Path(shipment_id): Path<String>,
) -> Response {
    let request_summary = format!("shipment_id={shipment_id}");
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/delivery-shipments/{shipment_id}/retry",
        Some(request_summary),
        |app| commands::retry_delivery_shipment(app, shipment_id),
    )
}

async fn run_order_sync_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/runners/order-sync",
        Some("lookback_days=1 page_size=100".to_string()),
        |app| commands::run_order_sync_once(app, Some(1), Some(100), None),
    )
    .await
}

async fn run_order_detail_sync_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/runners/order-detail-sync",
        Some("limit=50".to_string()),
        |app| commands::run_order_detail_sync_once(app, Some(50)),
    )
    .await
}

async fn run_aftersale_sync_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/runners/aftersale-sync",
        Some("lookback_hours=24 limit=200".to_string()),
        |app| commands::run_aftersale_sync_once(app, Some(24), Some(200)),
    )
    .await
}

async fn run_purchase_task_generation_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/runners/purchase-task-generation",
        Some("limit=100".to_string()),
        |app| commands::run_purchase_task_generation_once(app, Some(100)),
    )
}

async fn run_delivery_submit_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/runners/delivery-submit",
        Some("limit=20".to_string()),
        |app| commands::run_delivery_submission_once(app, Some(20)),
    )
    .await
}

async fn run_publish_pipeline_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    let started = Instant::now();
    let response = if let Err(response) = authorize(&state.app, &headers) {
        response
    } else {
        match commands::run_publish_pipeline_once(state.app.clone()).await {
            Ok(payload) => json_success(payload),
            Err(error) => app_error_response(error),
        }
    };
    audit_response(
        &state.app,
        "POST",
        "/api/runners/publish-pipeline",
        Some("publish_pipeline=true".to_string()),
        response,
        started,
    )
}

async fn run_price_precheck_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/runners/price-precheck",
        Some("limit=50".to_string()),
        |app| commands::run_price_update_precheck_once(app, Some(50)),
    )
}

async fn run_price_submit_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    let started = Instant::now();
    let response = if let Err(response) = authorize(&state.app, &headers) {
        response
    } else {
        match commands::run_price_update_submit_once(state.app.clone(), Some(20)).await {
            Ok(payload) => json_success(payload),
            Err(error) => app_error_response(error),
        }
    };
    audit_response(
        &state.app,
        "POST",
        "/api/runners/price-submit",
        Some("limit=20".to_string()),
        response,
        started,
    )
}

async fn run_price_confirm_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    let started = Instant::now();
    let response = if let Err(response) = authorize(&state.app, &headers) {
        response
    } else {
        match commands::run_price_update_confirm_once(state.app.clone(), Some(50)).await {
            Ok(payload) => json_success(payload),
            Err(error) => app_error_response(error),
        }
    };
    audit_response(
        &state.app,
        "POST",
        "/api/runners/price-confirm",
        Some("limit=50".to_string()),
        response,
        started,
    )
}

async fn run_inventory_risk_scan_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated(
        &state,
        &headers,
        "POST",
        "/api/runners/inventory-risk-scan",
        Some("inventory.scan_risks=true".to_string()),
        |app| commands::run_inventory_risk_scan_once(app),
    )
}

async fn run_guarantee_sync_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    authenticated_async(
        &state,
        &headers,
        "POST",
        "/api/runners/guarantee-sync",
        Some("lookback_hours=24 limit=200".to_string()),
        |app| commands::run_guarantee_sync_once(app, Some(24), Some(200)),
    )
    .await
}

async fn run_operations_handler(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Response {
    let started = Instant::now();
    let response = if let Err(response) = authorize(&state.app, &headers) {
        response
    } else {
        match commands::run_operational_automation_once(state.app.clone()).await {
            Ok(payload) => json_success(payload),
            Err(error) => app_error_response(error),
        }
    };
    audit_response(
        &state.app,
        "POST",
        "/api/runners/operations",
        Some("operational_automation=true".to_string()),
        response,
        started,
    )
}

// ── Agent 工具处理器（无需认证，仅 localhost 可访问）─────────

#[derive(Debug, Deserialize)]
struct AgentCategorySearchQuery {
    q: Option<String>,
    limit: Option<String>,
    query: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentDocsQuery {
    q: Option<String>,
    query: Option<String>,
}

async fn get_agent_product_handler(
    State(state): State<LocalApiState>,
    Path(task_id): Path<String>,
) -> Response {
    match commands::get_agent_product_detail(&state.app, &task_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_shop_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
) -> Response {
    match commands::get_agent_shop_info(&state.app, &shop_id) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            "SHOP_NOT_FOUND",
            &format!("店铺不存在：{shop_id}"),
        ),
        Err(err) => app_error_response(err),
    }
}

async fn search_agent_categories_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
    Query(query): Query<AgentCategorySearchQuery>,
) -> Response {
    let q = query
        .q
        .or(query.query)
        .unwrap_or_default()
        .trim()
        .to_string();
    let limit = query
        .limit
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    match commands::search_agent_categories(&state.app, &shop_id, &q, limit) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_category_detail_handler(
    State(state): State<LocalApiState>,
    Path((shop_id, cat_id)): Path<(String, i64)>,
) -> Response {
    match commands::get_agent_category_detail(&state.app, &shop_id, cat_id) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            "CATEGORY_DETAIL_NOT_FOUND",
            &format!("类目详情不存在：shop_id={shop_id}, cat_id={cat_id}"),
        ),
        Err(err) => app_error_response(err),
    }
}

async fn search_agent_docs_handler(
    State(state): State<LocalApiState>,
    Query(query): Query<AgentDocsQuery>,
) -> Response {
    let q = query
        .q
        .or(query.query)
        .unwrap_or_default()
        .trim()
        .to_string();
    match commands::search_agent_docs(&state.app, &q) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

#[derive(Debug, Deserialize)]
struct AgentCategoryTreeQuery {
    parent_cat_id: Option<i64>,
}

async fn get_agent_active_categories_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
) -> Response {
    match commands::get_agent_active_categories(&state.app, &shop_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_category_tree_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
    Query(query): Query<AgentCategoryTreeQuery>,
) -> Response {
    match commands::get_agent_category_tree(&state.app, &shop_id, query.parent_cat_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_freight_templates_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
) -> Response {
    match commands::get_agent_freight_templates(&state.app, &shop_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_after_sale_addresses_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
) -> Response {
    match commands::get_agent_after_sale_addresses(&state.app, &shop_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_delivery_companies_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
) -> Response {
    match commands::get_agent_delivery_companies(&state.app, &shop_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

// ── P2 Agent 处理器 ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AgentListQuery {
    shop_id: Option<String>,
    status: Option<String>,
    limit: Option<String>,
    external_product_id: Option<String>,
    order_id: Option<String>,
}

async fn get_agent_order_handler(
    State(state): State<LocalApiState>,
    Path(order_id): Path<String>,
) -> Response {
    match commands::get_agent_order(&state.app, &order_id) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "ORDER_NOT_FOUND", "订单不存在"),
        Err(err) => app_error_response(err),
    }
}

async fn list_agent_orders_handler(
    State(state): State<LocalApiState>,
    Query(query): Query<AgentListQuery>,
) -> Response {
    let limit = query
        .limit
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    match commands::list_agent_orders(
        &state.app,
        query.shop_id.as_deref(),
        query.status.as_deref(),
        limit,
    ) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_aftersale_handler(
    State(state): State<LocalApiState>,
    Path(aftersale_id): Path<String>,
) -> Response {
    match commands::get_agent_aftersale(&state.app, &aftersale_id) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "AFTERSALE_NOT_FOUND", "售后单不存在"),
        Err(err) => app_error_response(err),
    }
}

async fn list_agent_aftersales_handler(
    State(state): State<LocalApiState>,
    Query(query): Query<AgentListQuery>,
) -> Response {
    let limit = query
        .limit
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    match commands::list_agent_aftersales(
        &state.app,
        query.shop_id.as_deref(),
        query.status.as_deref(),
        limit,
    ) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_reject_reasons_handler(
    State(state): State<LocalApiState>,
    Path(shop_id): Path<String>,
) -> Response {
    match commands::get_agent_reject_reasons(&state.app, &shop_id) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_purchase_task_handler(
    State(state): State<LocalApiState>,
    Path(task_id): Path<String>,
) -> Response {
    match commands::get_agent_purchase_task(&state.app, &task_id) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            "PURCHASE_TASK_NOT_FOUND",
            "采购任务不存在",
        ),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_shop_product_handler(
    State(state): State<LocalApiState>,
    Path((shop_id, external_product_id)): Path<(String, String)>,
) -> Response {
    match commands::get_agent_shop_product(&state.app, &shop_id, &external_product_id) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            "SHOP_PRODUCT_NOT_FOUND",
            "铺货记录不存在",
        ),
        Err(err) => app_error_response(err),
    }
}

async fn list_agent_collections_handler(
    State(state): State<LocalApiState>,
    Query(query): Query<AgentListQuery>,
) -> Response {
    let limit = query
        .limit
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 50);
    match commands::list_agent_collections(&state.app, query.status.as_deref(), limit) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_product_sales_handler(
    State(state): State<LocalApiState>,
    Path(product_id): Path<String>,
    Query(query): Query<AgentListQuery>,
) -> Response {
    match commands::get_agent_product_sales(&state.app, &product_id, query.shop_id.as_deref()) {
        Ok(Some(data)) => json_success(data),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "PRODUCT_NOT_FOUND", "商品无销售数据"),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_inventory_risk_handler(
    State(state): State<LocalApiState>,
    Query(query): Query<AgentListQuery>,
) -> Response {
    match commands::get_agent_inventory_risk(&state.app, query.external_product_id.as_deref()) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

async fn get_agent_profit_summary_handler(
    State(state): State<LocalApiState>,
    Query(query): Query<AgentListQuery>,
) -> Response {
    let limit = query
        .limit
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 50);
    match commands::get_agent_profit_summary(&state.app, query.order_id.as_deref(), limit) {
        Ok(data) => json_success(data),
        Err(err) => app_error_response(err),
    }
}

// ── 认证处理器 ──────────────────────────────────────────────

fn authenticated<T, F>(
    state: &LocalApiState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    request_summary: Option<String>,
    handler: F,
) -> Response
where
    T: Serialize,
    F: FnOnce(AppHandle) -> AppResult<T>,
{
    let started = Instant::now();
    let response = if let Err(response) = authorize(&state.app, headers) {
        response
    } else {
        match handler(state.app.clone()) {
            Ok(payload) => json_success(payload),
            Err(error) => app_error_response(error),
        }
    };
    audit_response(&state.app, method, path, request_summary, response, started)
}

async fn authenticated_async<T, F, Fut>(
    state: &LocalApiState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    request_summary: Option<String>,
    handler: F,
) -> Response
where
    T: Serialize,
    F: FnOnce(AppHandle) -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    let started = Instant::now();
    let response = if let Err(response) = authorize(&state.app, headers) {
        response
    } else {
        match handler(state.app.clone()).await {
            Ok(payload) => json_success(payload),
            Err(error) => app_error_response(error),
        }
    };
    audit_response(&state.app, method, path, request_summary, response, started)
}

fn audit_response(
    app: &AppHandle,
    method: &str,
    path: &str,
    request_summary: Option<String>,
    response: Response,
    started: Instant,
) -> Response {
    let status = response.status();
    let status_code = i64::from(status.as_u16());
    let status_label = if status.is_success() {
        "success"
    } else {
        "failed"
    };
    let error_code = if status.is_success() {
        None
    } else {
        Some(
            status
                .canonical_reason()
                .unwrap_or("HTTP_ERROR")
                .to_string(),
        )
    };
    let response_summary = if status.is_success() {
        Some("ok".to_string())
    } else {
        Some(format!("HTTP {}", status.as_u16()))
    };
    if let Err(error) = insert_external_api_log(
        app,
        ExternalApiLogRecord {
            method,
            path,
            status: status_label,
            status_code,
            error_code: error_code.as_deref(),
            request_summary: request_summary.as_deref(),
            response_summary: response_summary.as_deref(),
            duration_ms: started.elapsed().as_millis().min(i64::MAX as u128) as i64,
        },
    ) {
        eprintln!("外部 HTTP API 审计日志写入失败：{error}");
    }
    response
}

struct ExternalApiLogRecord<'a> {
    method: &'a str,
    path: &'a str,
    status: &'a str,
    status_code: i64,
    error_code: Option<&'a str>,
    request_summary: Option<&'a str>,
    response_summary: Option<&'a str>,
    duration_ms: i64,
}

fn insert_external_api_log(app: &AppHandle, record: ExternalApiLogRecord<'_>) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "INSERT INTO external_api_logs
         (id, method, path, status, status_code, error_code, request_summary,
          response_summary, duration_ms, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            format!("external-api-log-{}", Uuid::new_v4()),
            record.method,
            record.path,
            record.status,
            record.status_code,
            record.error_code,
            record.request_summary,
            record.response_summary,
            record.duration_ms,
            now_shanghai(),
        ],
    )?;
    Ok(())
}

fn publish_job_request_summary(request: &ExternalPublishJobRequest) -> String {
    format!(
        "request_id={} products={} target_groups={} target_shops={}",
        request.request_id,
        request.products.len(),
        request.target_shop_group_ids.len(),
        request.target_shop_ids.len()
    )
}

fn price_update_request_summary(request: &PriceUpdateJobRequest) -> String {
    format!(
        "request_id={} products={} target_groups={} target_shops={}",
        request.request_id,
        request.products.len(),
        request.target_shop_group_ids.len(),
        request.target_shop_ids.len()
    )
}

fn purchase_tasks_query_summary(query: &PurchaseTaskQuery) -> String {
    format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("100")
    )
}

fn parse_purchase_task_limit(limit: Option<&str>) -> AppResult<Option<i64>> {
    parse_limit(limit)
}

fn purchase_mapping_request_summary(
    purchase_task_id: &str,
    body: &PurchaseTaskMappingBody,
) -> String {
    format!(
        "purchase_task_id={} has_external_product_id={} has_external_sku_id={} has_source_url={} has_supplier_name={} has_supplier_product_id={} estimated_cost_present={} has_note={}",
        purchase_task_id,
        !body.external_product_id.trim().is_empty(),
        !body.external_sku_id.trim().is_empty(),
        body.source_url
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.supplier_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.supplier_product_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.estimated_cost.is_some(),
        body.note
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    )
}

fn inventory_risks_query_summary(query: &InventoryRiskQuery) -> String {
    format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    )
}

fn product_sales_analysis_query_summary(query: &ProductSalesAnalysisQuery) -> String {
    format!(
        "status={} limit={}",
        query
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all"),
        query
            .limit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("200")
    )
}

fn parse_limit(limit: Option<&str>) -> AppResult<Option<i64>> {
    limit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| AppError::Validation("limit 必须是整数".to_string()))
        })
        .transpose()
}

fn delivery_shipment_record_summary(body: &ShipmentRecordBody) -> String {
    format!(
        "has_order_id={} has_shop_id={} has_wechat_order_id={} deliver_type={} has_delivery_id={} has_delivery_name={} has_waybill={}",
        body.order_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.shop_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.wechat_order_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.deliver_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string()),
        body.delivery_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.delivery_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.waybill_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    )
}

fn purchase_shipment_request_summary(
    purchase_task_id: &str,
    body: &PurchaseTaskShipmentBody,
) -> String {
    format!(
        "purchase_task_id={} deliver_type={} has_delivery_id={} has_delivery_name={} has_waybill={} estimated_cost_present={}",
        purchase_task_id,
        body.deliver_type
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string()),
        body.delivery_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.delivery_name
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.waybill_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        body.estimated_cost.is_some()
    )
}

fn purchase_issue_request_summary(purchase_task_id: &str, body: &PurchaseTaskIssueBody) -> String {
    format!(
        "purchase_task_id={} issue_type={} has_note={}",
        purchase_task_id,
        body.issue_type.trim(),
        body.note
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    )
}

fn authorize(app: &AppHandle, headers: &HeaderMap) -> Result<(), Response> {
    let configured_hash = get_string_setting(app, LOCAL_API_KEY_HASH_SETTING)
        .map_err(app_error_response)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "LOCAL_API_KEY_NOT_CONFIGURED",
                "本地 HTTP API 尚未生成 API Key，请先在桌面端生成。",
            )
        })?;
    let provided_key = extract_api_key(headers).ok_or_else(|| {
        json_error(
            StatusCode::UNAUTHORIZED,
            "LOCAL_API_KEY_MISSING",
            "缺少 x-wx-xd-api-key 或 Authorization: Bearer。",
        )
    })?;
    if hash_api_key(&provided_key) != configured_hash {
        return Err(json_error(
            StatusCode::UNAUTHORIZED,
            "LOCAL_API_KEY_INVALID",
            "本地 HTTP API Key 不正确。",
        ));
    }
    Ok(())
}

fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(LOCAL_API_HEADER)
        .and_then(|value| value.to_str().ok())
    {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?.trim();
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
}

fn local_api_config(app: &AppHandle) -> AppResult<LocalApiConfig> {
    let api_key_hint = get_string_setting(app, LOCAL_API_KEY_HINT_SETTING)?;
    Ok(LocalApiConfig {
        enabled: true,
        host: LOCAL_API_HOST.to_string(),
        port: LOCAL_API_PORT,
        base_url: local_api_base_url(),
        has_api_key: get_string_setting(app, LOCAL_API_KEY_HASH_SETTING)?
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false),
        api_key_hint,
        auth_header: LOCAL_API_HEADER.to_string(),
    })
}

fn local_api_base_url() -> String {
    format!("http://{LOCAL_API_HOST}:{LOCAL_API_PORT}")
}

fn generate_api_key() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_api_key(api_key: &str) -> String {
    let digest = Sha256::digest(api_key.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn get_string_setting(app: &AppHandle, key: &str) -> AppResult<Option<String>> {
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .optional()?
    .map(|raw| {
        serde_json::from_str::<String>(&raw)
            .map_err(|error| AppError::Validation(format!("应用设置 {key} 格式错误：{error}")))
    })
    .transpose()
}

fn set_string_setting(app: &AppHandle, key: &str, value: &str) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = excluded.updated_at",
        params![key, serde_json::json!(value).to_string(), now_shanghai()],
    )?;
    Ok(())
}

fn app_error_response(error: AppError) -> Response {
    match error {
        AppError::Validation(message) | AppError::Security(message) => {
            json_error(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", &message)
        }
        AppError::WechatApi { errcode, errmsg } => json_error(
            StatusCode::BAD_GATEWAY,
            "WECHAT_API_ERROR",
            &format!("微信接口错误 {errcode}: {errmsg}"),
        ),
        other => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            &other.to_string(),
        ),
    }
}

fn json_success<T: Serialize>(payload: T) -> Response {
    Json(payload).into_response()
}

fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": message,
            }
        })),
    )
        .into_response()
}
