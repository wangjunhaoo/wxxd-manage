use chrono::SecondsFormat;
use chrono::{DateTime, Duration, FixedOffset, Utc};
use chrono_tz::Asia::Shanghai;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("微信接口错误 {errcode}: {errmsg}")]
    WechatApi { errcode: i64, errmsg: String },
    #[error("{0}")]
    Security(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub fn now_shanghai() -> String {
    format_shanghai(Utc::now())
}

pub fn format_shanghai(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Shanghai)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn expires_at_shanghai(expires_in_seconds: i64) -> String {
    let safety_window = 300;
    let ttl = expires_in_seconds.saturating_sub(safety_window).max(60);
    format_shanghai(Utc::now() + Duration::seconds(ttl))
}

pub fn is_future_rfc3339(value: &str) -> bool {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed: DateTime<FixedOffset>| parsed.with_timezone(&Utc) > Utc::now())
        .unwrap_or(false)
}

pub fn database_path(app: &AppHandle) -> AppResult<PathBuf> {
    let data_dir = app.path().app_data_dir()?;
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir.join("wx-xd.sqlite"))
}

pub fn open_connection(app: &AppHandle) -> AppResult<Connection> {
    let path = database_path(app)?;
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn initialize(app: &AppHandle) -> AppResult<()> {
    let conn = open_connection(app)?;
    seed_defaults(&conn)?;
    Ok(())
}

fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS shop_groups (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL UNIQUE,
          status TEXT NOT NULL DEFAULT 'active',
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS shops (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          appid TEXT NOT NULL,
          status TEXT NOT NULL DEFAULT 'not_verified',
          group_id TEXT NOT NULL,
          last_sync_at TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(group_id) REFERENCES shop_groups(id)
        );

        CREATE TABLE IF NOT EXISTS shop_credentials (
          shop_id TEXT PRIMARY KEY,
          encrypted_secret TEXT NOT NULL,
          secret_nonce TEXT NOT NULL,
          key_version TEXT NOT NULL,
          secret_fingerprint TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS ai_provider_credentials (
          id TEXT PRIMARY KEY,
          encrypted_api_key TEXT NOT NULL,
          api_key_nonce TEXT NOT NULL,
          key_version TEXT NOT NULL,
          api_key_fingerprint TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS access_tokens (
          shop_id TEXT PRIMARY KEY,
          encrypted_access_token TEXT NOT NULL,
          token_nonce TEXT NOT NULL,
          key_version TEXT NOT NULL,
          expires_at TEXT NOT NULL,
          refreshed_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS publish_jobs (
          id TEXT PRIMARY KEY,
          request_id TEXT NOT NULL UNIQUE,
          status TEXT NOT NULL,
          accepted_product_count INTEGER NOT NULL,
          target_shop_count INTEGER NOT NULL,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS publish_products (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          external_product_id TEXT NOT NULL,
          title TEXT NOT NULL,
          source_url TEXT NOT NULL,
          raw_payload TEXT NOT NULL,
          status TEXT NOT NULL,
          error_summary TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES publish_jobs(id)
        );

        CREATE TABLE IF NOT EXISTS publish_job_items (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          product_row_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          shop_name TEXT NOT NULL,
          status TEXT NOT NULL,
          error_code TEXT,
          error_summary TEXT,
          wechat_product_id TEXT,
          wechat_sku_id TEXT,
          retry_count INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES publish_jobs(id),
          FOREIGN KEY(product_row_id) REFERENCES publish_products(id)
        );

        CREATE TABLE IF NOT EXISTS publish_assets (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          item_id TEXT NOT NULL,
          product_row_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          source_url TEXT NOT NULL,
          asset_kind TEXT NOT NULL,
          sort_order INTEGER NOT NULL,
          wechat_url TEXT,
          status TEXT NOT NULL,
          error_code TEXT,
          error_summary TEXT,
          uploaded_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(item_id, source_url),
          FOREIGN KEY(job_id) REFERENCES publish_jobs(id),
          FOREIGN KEY(item_id) REFERENCES publish_job_items(id),
          FOREIGN KEY(product_row_id) REFERENCES publish_products(id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS shop_products (
          id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          external_product_id TEXT NOT NULL,
          source_url TEXT,
          wechat_product_id TEXT,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL,
          UNIQUE(shop_id, external_product_id)
        );

        CREATE TABLE IF NOT EXISTS price_update_jobs (
          id TEXT PRIMARY KEY,
          request_id TEXT NOT NULL UNIQUE,
          status TEXT NOT NULL,
          accepted_product_count INTEGER NOT NULL,
          target_shop_count INTEGER NOT NULL,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS price_update_items (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          shop_name TEXT NOT NULL,
          external_product_id TEXT NOT NULL,
          target_price_cents INTEGER NOT NULL,
          status TEXT NOT NULL,
          error_code TEXT,
          error_summary TEXT,
          wechat_product_id TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(job_id) REFERENCES price_update_jobs(id)
        );

        CREATE TABLE IF NOT EXISTS orders (
          id TEXT PRIMARY KEY,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS order_items (
          id TEXT PRIMARY KEY,
          order_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          wechat_order_id TEXT NOT NULL,
          wechat_product_id TEXT,
          wechat_sku_id TEXT,
          out_product_id TEXT,
          out_sku_id TEXT,
          title TEXT,
          sku_count INTEGER NOT NULL DEFAULT 0,
          sale_price INTEGER,
          real_price INTEGER,
          raw_payload TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(order_id, wechat_product_id, wechat_sku_id, out_product_id, out_sku_id),
          FOREIGN KEY(order_id) REFERENCES orders(id)
        );

        CREATE TABLE IF NOT EXISTS order_price_adjustment_jobs (
          id TEXT PRIMARY KEY,
          request_id TEXT NOT NULL UNIQUE,
          status TEXT NOT NULL,
          accepted_order_count INTEGER NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS order_price_adjustment_items (
          id TEXT PRIMARY KEY,
          job_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          shop_name TEXT NOT NULL,
          order_id TEXT,
          wechat_order_id TEXT NOT NULL,
          wechat_status INTEGER,
          change_order_infos_json TEXT NOT NULL,
          change_express INTEGER NOT NULL DEFAULT 0,
          express_fee_cents INTEGER,
          note TEXT,
          status TEXT NOT NULL,
          error_code TEXT,
          error_summary TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          submitted_at TEXT,
          FOREIGN KEY(job_id) REFERENCES order_price_adjustment_jobs(id),
          FOREIGN KEY(order_id) REFERENCES orders(id)
        );

        CREATE TABLE IF NOT EXISTS purchase_tasks (
          id TEXT PRIMARY KEY,
          order_id TEXT NOT NULL,
          order_item_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          status TEXT NOT NULL,
          external_product_id TEXT,
          external_sku_id TEXT,
          source_url TEXT,
          supplier_name TEXT,
          supplier_product_id TEXT,
          quantity INTEGER NOT NULL DEFAULT 0,
          estimated_cost REAL,
          error_summary TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(order_item_id),
          FOREIGN KEY(order_id) REFERENCES orders(id),
          FOREIGN KEY(order_item_id) REFERENCES order_items(id)
        );

        CREATE TABLE IF NOT EXISTS shipments (
          id TEXT PRIMARY KEY,
          order_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          wechat_order_id TEXT NOT NULL,
          delivery_id TEXT,
          delivery_name TEXT,
          waybill_id TEXT,
          deliver_type INTEGER NOT NULL DEFAULT 1,
          status TEXT NOT NULL,
          send_payload TEXT,
          error_code TEXT,
          error_summary TEXT,
          submitted_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(order_id, delivery_id, waybill_id),
          FOREIGN KEY(order_id) REFERENCES orders(id)
        );

        CREATE TABLE IF NOT EXISTS order_profit_adjustments (
          id TEXT PRIMARY KEY,
          order_id TEXT NOT NULL,
          kind TEXT NOT NULL,
          amount_cents INTEGER NOT NULL,
          note TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(order_id) REFERENCES orders(id)
        );

        CREATE TABLE IF NOT EXISTS aftersales (
          id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          wechat_aftersale_id TEXT NOT NULL,
          order_id TEXT,
          wechat_order_id TEXT,
          status TEXT NOT NULL,
          aftersale_type TEXT,
          reason TEXT,
          refund_amount_cents INTEGER,
          raw_payload TEXT NOT NULL,
          responsibility_party TEXT,
          responsibility_note TEXT,
          supplier_compensation_cents INTEGER NOT NULL DEFAULT 0,
          supplier_compensation_adjustment_id TEXT,
          handled_at TEXT,
          synced_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(shop_id, wechat_aftersale_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS aftersale_reject_reasons (
          shop_id TEXT NOT NULL,
          reject_reason_type INTEGER NOT NULL,
          reject_reason_type_text TEXT NOT NULL,
          reject_reason TEXT NOT NULL,
          reject_scene INTEGER,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, reject_reason_type),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS guarantee_orders (
          id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          guarantee_order_id TEXT NOT NULL,
          order_id TEXT,
          wechat_order_id TEXT,
          guarantee_type INTEGER,
          status TEXT NOT NULL,
          apply_reason TEXT,
          pay_amount_cents INTEGER,
          merchant_refuse_reason TEXT,
          handling_status TEXT,
          handling_note TEXT,
          responsibility_party TEXT,
          supplier_compensation_cents INTEGER NOT NULL DEFAULT 0,
          supplier_compensation_adjustment_id TEXT,
          handled_at TEXT,
          raw_payload TEXT NOT NULL,
          created_time INTEGER,
          updated_time INTEGER,
          expire_time INTEGER,
          complete_time INTEGER,
          synced_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(shop_id, guarantee_order_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS aftersale_evidence_records (
          id TEXT PRIMARY KEY,
          target_type TEXT NOT NULL,
          target_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          external_target_id TEXT NOT NULL,
          evidence_type TEXT NOT NULL,
          title TEXT NOT NULL,
          content_text TEXT,
          local_file_path TEXT,
          source_url TEXT,
          status TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS supplier_aftersale_followups (
          id TEXT PRIMARY KEY,
          target_type TEXT NOT NULL,
          target_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          external_target_id TEXT NOT NULL,
          purchase_task_id TEXT,
          supplier_name TEXT,
          followup_type TEXT NOT NULL,
          status TEXT NOT NULL,
          note TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS task_runs (
          id TEXT PRIMARY KEY,
          task_type TEXT NOT NULL,
          status TEXT NOT NULL,
          progress INTEGER NOT NULL DEFAULT 0,
          started_at TEXT,
          finished_at TEXT,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS task_logs (
          id TEXT PRIMARY KEY,
          task_id TEXT NOT NULL,
          item_id TEXT,
          level TEXT NOT NULL,
          message TEXT NOT NULL,
          detail_json TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(task_id) REFERENCES task_runs(id)
        );

        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value_json TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS api_call_logs (
          id TEXT PRIMARY KEY,
          shop_id TEXT,
          provider TEXT NOT NULL,
          endpoint TEXT NOT NULL,
          method TEXT NOT NULL,
          status TEXT NOT NULL,
          errcode INTEGER,
          errmsg TEXT,
          response_summary TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS external_api_logs (
          id TEXT PRIMARY KEY,
          method TEXT NOT NULL,
          path TEXT NOT NULL,
          status TEXT NOT NULL,
          status_code INTEGER NOT NULL,
          error_code TEXT,
          request_summary TEXT,
          response_summary TEXT,
          duration_ms INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS notifications (
          id TEXT PRIMARY KEY,
          severity TEXT NOT NULL,
          source_type TEXT NOT NULL,
          source_id TEXT NOT NULL,
          shop_id TEXT,
          title TEXT NOT NULL,
          body TEXT NOT NULL,
          status TEXT NOT NULL DEFAULT 'unread',
          dedupe_key TEXT NOT NULL UNIQUE,
          data_json TEXT,
          read_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS publish_attribute_suggestions (
          id TEXT PRIMARY KEY,
          item_id TEXT NOT NULL,
          job_id TEXT NOT NULL,
          product_row_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          external_product_id TEXT NOT NULL,
          attr_kind TEXT NOT NULL,
          attr_key TEXT NOT NULL,
          suggested_value TEXT,
          sku_values_json TEXT,
          confidence INTEGER NOT NULL DEFAULT 0,
          source TEXT NOT NULL,
          applied INTEGER NOT NULL DEFAULT 0,
          prompt_json TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(item_id, attr_kind, attr_key),
          FOREIGN KEY(item_id) REFERENCES publish_job_items(id),
          FOREIGN KEY(product_row_id) REFERENCES publish_products(id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS wechat_categories (
          shop_id TEXT NOT NULL,
          cat_id INTEGER NOT NULL,
          parent_cat_id INTEGER,
          level INTEGER,
          name TEXT NOT NULL,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, cat_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS wechat_category_details (
          shop_id TEXT NOT NULL,
          cat_id INTEGER NOT NULL,
          product_attr_count INTEGER NOT NULL DEFAULT 0,
          sale_attr_count INTEGER NOT NULL DEFAULT 0,
          product_qua_count INTEGER NOT NULL DEFAULT 0,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, cat_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS wechat_category_rules (
          shop_id TEXT NOT NULL,
          cat_id INTEGER NOT NULL,
          rule_type TEXT NOT NULL,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, cat_id, rule_type),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS wechat_category_prechecks (
          shop_id TEXT NOT NULL,
          cat_id INTEGER NOT NULL,
          all_pass INTEGER NOT NULL,
          fail_reasons TEXT NOT NULL,
          raw_payload TEXT NOT NULL,
          checked_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, cat_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS wechat_freight_templates (
          shop_id TEXT NOT NULL,
          template_id TEXT NOT NULL,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, template_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS delivery_companies (
          shop_id TEXT NOT NULL,
          delivery_id TEXT NOT NULL,
          delivery_name TEXT NOT NULL,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          PRIMARY KEY(shop_id, delivery_id),
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE TABLE IF NOT EXISTS agent_skill_settings (
          name TEXT PRIMARY KEY,
          enabled INTEGER NOT NULL,
          model TEXT,
          temperature REAL,
          last_test_status TEXT,
          last_test_summary TEXT,
          last_test_at TEXT,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_runs (
          id TEXT PRIMARY KEY,
          skill_name TEXT NOT NULL,
          skill_version TEXT NOT NULL,
          scene TEXT NOT NULL,
          source_type TEXT NOT NULL,
          source_id TEXT NOT NULL,
          shop_id TEXT,
          status TEXT NOT NULL,
          provider_type TEXT,
          model TEXT,
          temperature REAL,
          input_summary TEXT NOT NULL,
          input_snapshot_json TEXT,
          output_json TEXT,
          validated_output_json TEXT,
          tool_calls_json TEXT,
          decision TEXT,
          error_code TEXT,
          error_summary TEXT,
          started_at TEXT,
          finished_at TEXT,
          duration_ms INTEGER,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_run_events (
          id TEXT PRIMARY KEY,
          run_id TEXT NOT NULL,
          event_type TEXT NOT NULL,
          level TEXT NOT NULL,
          message TEXT NOT NULL,
          data_json TEXT,
          created_at TEXT NOT NULL,
          FOREIGN KEY(run_id) REFERENCES agent_runs(id)
        );

        CREATE TABLE IF NOT EXISTS collection_tasks (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          source_url TEXT NOT NULL,
          category_path TEXT NOT NULL,
          target_shop_ids TEXT NOT NULL,
          status TEXT NOT NULL,
          error_summary TEXT,
          collected_data TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        "#,
    )?;
    ensure_column(conn, "shops", "wechat_nickname", "TEXT")?;
    ensure_column(conn, "shops", "wechat_headimg_url", "TEXT")?;
    ensure_column(conn, "shops", "wechat_subject_type", "TEXT")?;
    ensure_column(conn, "shops", "wechat_status", "TEXT")?;
    ensure_column(conn, "shops", "wechat_username", "TEXT")?;
    ensure_column(conn, "shops", "is_local_life", "INTEGER")?;
    ensure_column(conn, "shops", "open_timestamp", "INTEGER")?;
    ensure_column(conn, "shops", "last_health_check_at", "TEXT")?;
    ensure_column(conn, "collection_tasks", "published_shop_ids", "TEXT")?;
    ensure_column(conn, "collection_tasks", "publish_job_ids", "TEXT")?;
    ensure_column(conn, "collection_tasks", "published_at", "TEXT")?;
    ensure_column(conn, "collection_tasks", "review_status", "TEXT")?;
    ensure_column(conn, "collection_tasks", "review_summary", "TEXT")?;
    ensure_column(conn, "collection_tasks", "reviewed_data", "TEXT")?;
    ensure_column(conn, "collection_tasks", "review_result_json", "TEXT")?;
    ensure_column(conn, "collection_tasks", "reviewed_at", "TEXT")?;
    ensure_column(conn, "publish_job_items", "wechat_status", "INTEGER")?;
    ensure_column(conn, "publish_job_items", "wechat_edit_status", "INTEGER")?;
    ensure_column(conn, "publish_job_items", "last_status_sync_at", "TEXT")?;
    ensure_column(conn, "publish_job_items", "audit_summary", "TEXT")?;
    ensure_column(
        conn,
        "publish_attribute_suggestions",
        "sku_values_json",
        "TEXT",
    )?;
    ensure_column(conn, "shop_products", "wechat_status", "INTEGER")?;
    ensure_column(conn, "shop_products", "wechat_edit_status", "INTEGER")?;
    ensure_column(conn, "shop_products", "last_status_sync_at", "TEXT")?;
    ensure_column(conn, "shop_products", "audit_summary", "TEXT")?;
    ensure_column(conn, "shop_products", "current_price_cents", "INTEGER")?;
    ensure_column(conn, "shop_products", "last_price_update_at", "TEXT")?;
    ensure_column(conn, "shop_products", "source_url", "TEXT")?;
    ensure_column(conn, "orders", "shop_id", "TEXT")?;
    ensure_column(conn, "orders", "wechat_order_id", "TEXT")?;
    ensure_column(conn, "orders", "wechat_status", "INTEGER")?;
    ensure_column(conn, "orders", "raw_payload", "TEXT")?;
    ensure_column(conn, "orders", "synced_at", "TEXT")?;
    ensure_column(conn, "orders", "updated_at", "TEXT")?;
    ensure_column(conn, "orders", "order_created_at", "INTEGER")?;
    ensure_column(conn, "orders", "order_updated_at", "INTEGER")?;
    ensure_column(conn, "orders", "detail_synced_at", "TEXT")?;
    ensure_column(conn, "orders", "detail_error", "TEXT")?;
    ensure_column(conn, "purchase_tasks", "supplier_delivery_id", "TEXT")?;
    ensure_column(conn, "purchase_tasks", "supplier_delivery_name", "TEXT")?;
    ensure_column(conn, "purchase_tasks", "supplier_waybill_id", "TEXT")?;
    ensure_column(conn, "purchase_tasks", "supplier_deliver_type", "INTEGER")?;
    ensure_column(conn, "purchase_tasks", "supplier_shipped_at", "TEXT")?;
    ensure_column(conn, "purchase_tasks", "source_url", "TEXT")?;
    ensure_column(conn, "aftersales", "responsibility_party", "TEXT")?;
    ensure_column(conn, "aftersales", "responsibility_note", "TEXT")?;
    ensure_column(
        conn,
        "aftersales",
        "supplier_compensation_cents",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "aftersales",
        "supplier_compensation_adjustment_id",
        "TEXT",
    )?;
    ensure_column(conn, "aftersales", "handled_at", "TEXT")?;
    ensure_column(conn, "aftersales", "last_action", "TEXT")?;
    ensure_column(conn, "aftersales", "last_action_status", "TEXT")?;
    ensure_column(conn, "aftersales", "last_action_error", "TEXT")?;
    ensure_column(conn, "aftersales", "last_action_note", "TEXT")?;
    ensure_column(conn, "aftersales", "last_action_at", "TEXT")?;
    ensure_column(conn, "guarantee_orders", "handling_status", "TEXT")?;
    ensure_column(conn, "guarantee_orders", "handling_note", "TEXT")?;
    ensure_column(conn, "guarantee_orders", "responsibility_party", "TEXT")?;
    ensure_column(
        conn,
        "guarantee_orders",
        "supplier_compensation_cents",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "guarantee_orders",
        "supplier_compensation_adjustment_id",
        "TEXT",
    )?;
    ensure_column(conn, "guarantee_orders", "handled_at", "TEXT")?;
    conn.execute_batch(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_orders_shop_wechat_order
          ON orders(shop_id, wechat_order_id);

        CREATE INDEX IF NOT EXISTS idx_order_items_order_id
          ON order_items(order_id);

        CREATE INDEX IF NOT EXISTS idx_order_price_adjustment_items_job_status
          ON order_price_adjustment_items(job_id, status);

        CREATE INDEX IF NOT EXISTS idx_order_price_adjustment_items_order
          ON order_price_adjustment_items(shop_id, wechat_order_id);

        CREATE INDEX IF NOT EXISTS idx_purchase_tasks_status
          ON purchase_tasks(status);

        CREATE INDEX IF NOT EXISTS idx_shipments_status
          ON shipments(status);

        CREATE INDEX IF NOT EXISTS idx_shipments_order_id
          ON shipments(order_id);

        CREATE INDEX IF NOT EXISTS idx_order_profit_adjustments_order_kind
          ON order_profit_adjustments(order_id, kind);

        CREATE INDEX IF NOT EXISTS idx_aftersales_status
          ON aftersales(status);

        CREATE INDEX IF NOT EXISTS idx_aftersales_shop_wechat_order
          ON aftersales(shop_id, wechat_order_id);

        CREATE INDEX IF NOT EXISTS idx_aftersale_reject_reasons_shop_scene
          ON aftersale_reject_reasons(shop_id, reject_scene);

        CREATE INDEX IF NOT EXISTS idx_guarantee_orders_status
          ON guarantee_orders(status);

        CREATE INDEX IF NOT EXISTS idx_guarantee_orders_shop_wechat_order
          ON guarantee_orders(shop_id, wechat_order_id);

        CREATE INDEX IF NOT EXISTS idx_aftersale_evidence_target
          ON aftersale_evidence_records(target_type, target_id);

        CREATE INDEX IF NOT EXISTS idx_aftersale_evidence_status
          ON aftersale_evidence_records(status);

        CREATE INDEX IF NOT EXISTS idx_supplier_aftersale_followups_target
          ON supplier_aftersale_followups(target_type, target_id);

        CREATE INDEX IF NOT EXISTS idx_supplier_aftersale_followups_status
          ON supplier_aftersale_followups(status);

        CREATE INDEX IF NOT EXISTS idx_price_update_items_job_status
          ON price_update_items(job_id, status);

        CREATE TABLE IF NOT EXISTS api_quota_snapshots (
          id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          cgi_path TEXT NOT NULL,
          daily_limit INTEGER,
          used INTEGER,
          remain INTEGER,
          rate_call_count INTEGER,
          rate_refresh_second INTEGER,
          raw_payload TEXT NOT NULL,
          checked_at TEXT NOT NULL,
          FOREIGN KEY(shop_id) REFERENCES shops(id)
        );

        CREATE INDEX IF NOT EXISTS idx_external_api_logs_created_at
          ON external_api_logs(created_at);

        CREATE INDEX IF NOT EXISTS idx_notifications_status_updated
          ON notifications(status, updated_at);

        CREATE INDEX IF NOT EXISTS idx_notifications_severity_updated
          ON notifications(severity, updated_at);

        CREATE INDEX IF NOT EXISTS idx_notifications_source
          ON notifications(source_type, source_id);

        CREATE INDEX IF NOT EXISTS idx_publish_attribute_suggestions_item
          ON publish_attribute_suggestions(item_id, applied);

        CREATE INDEX IF NOT EXISTS idx_agent_runs_created_at
          ON agent_runs(created_at);

        CREATE INDEX IF NOT EXISTS idx_agent_runs_scene_status
          ON agent_runs(scene, status, created_at);

        CREATE INDEX IF NOT EXISTS idx_agent_runs_source
          ON agent_runs(source_type, source_id);

        CREATE INDEX IF NOT EXISTS idx_agent_run_events_run
          ON agent_run_events(run_id, created_at);

        CREATE INDEX IF NOT EXISTS idx_wechat_categories_shop_name
          ON wechat_categories(shop_id, name);

        CREATE INDEX IF NOT EXISTS idx_wechat_categories_parent
          ON wechat_categories(shop_id, parent_cat_id);

        CREATE INDEX IF NOT EXISTS idx_wechat_category_prechecks_shop
          ON wechat_category_prechecks(shop_id, checked_at);

        CREATE INDEX IF NOT EXISTS idx_wechat_freight_templates_shop
          ON wechat_freight_templates(shop_id);

        CREATE INDEX IF NOT EXISTS idx_delivery_companies_shop
          ON delivery_companies(shop_id);

        CREATE INDEX IF NOT EXISTS idx_collection_tasks_status
          ON collection_tasks(status);
        "#,
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> AppResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !columns.iter().any(|existing| existing == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn seed_defaults(conn: &Connection) -> AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM shop_groups", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO shop_groups (id, name, status, created_at) VALUES (?1, ?2, 'active', ?3)",
            ("group-default", "默认店铺组", now_shanghai()),
        )?;
    }
    Ok(())
}
