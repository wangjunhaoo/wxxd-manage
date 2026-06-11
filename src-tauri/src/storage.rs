use chrono::SecondsFormat;
use chrono::{DateTime, Duration, FixedOffset, Utc};
use chrono_tz::Asia::Shanghai;
use rusqlite::{Connection, OptionalExtension};
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

/// 全库时间文本的唯一格式来源（固定宽度 RFC3339 +08:00）。
/// 注意：多处 SQL 直接对该格式做字典序比较（详情回刷分档、解密重试退避、
/// submitting 回收、明文 GC 等），格式一旦变更这些 WHERE 会静默失配——不得改动。
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
    // 后台 driver 与前端命令会并发访问同一 SQLite，设置忙等超时避免 SQLITE_BUSY 直接失败
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    // 外键约束暂时关闭：重构遗留多张仍在用的表(task_logs/publish_attribute_suggestions 等)外键
    // 仍指向已废弃旧表(task_runs/publish_jobs/publish_products/publish_job_items)，开启会触发
    // FOREIGN KEY constraint failed → 静默回滚、卡死流水线(precheck/attr_fill 都中过招)。新表
    // (pipeline_*)完整性由创建顺序的代码逻辑 + loader 的 JOIN 过滤天然保证；待删旧表后可重开。
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn initialize(app: &AppHandle) -> AppResult<()> {
    let conn = open_connection(app)?;
    seed_defaults(&conn)?;
    Ok(())
}

pub(crate) fn migrate(conn: &Connection) -> AppResult<()> {
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

        -- 微信小店真实商品缓存（sync_shop_products 写入，列表页读取）。
        CREATE TABLE IF NOT EXISTS wechat_shop_products (
          id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          shop_name TEXT NOT NULL,
          wechat_product_id TEXT NOT NULL,
          out_product_id TEXT,
          title TEXT NOT NULL,
          head_img TEXT,
          status INTEGER NOT NULL,
          edit_status INTEGER,
          min_price_cents INTEGER,
          cat_id INTEGER,
          total_stock INTEGER NOT NULL DEFAULT 0,
          sku_count INTEGER NOT NULL DEFAULT 0,
          audit_summary TEXT,
          raw_payload TEXT NOT NULL,
          synced_at TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(shop_id, wechat_product_id)
        );

        CREATE TABLE IF NOT EXISTS wechat_shop_product_skus (
          id TEXT PRIMARY KEY,
          product_row_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          wechat_product_id TEXT NOT NULL,
          sku_id TEXT NOT NULL,
          out_sku_id TEXT,
          sku_code TEXT,
          sale_price_cents INTEGER,
          stock_num INTEGER,
          sku_attrs TEXT,
          thumb_img TEXT,
          synced_at TEXT NOT NULL,
          UNIQUE(shop_id, wechat_product_id, sku_id)
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
          created_at TEXT NOT NULL
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

        CREATE TABLE IF NOT EXISTS wechat_category_relations (
          shop_id TEXT NOT NULL,
          cat_id INTEGER NOT NULL,
          status INTEGER NOT NULL,
          uneffective_reason TEXT,
          effective_time INTEGER,
          uneffective_time INTEGER,
          qua_id INTEGER,
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

        -- 统一流水线主表：一行 = 一个商品（一行 Excel 导入），贯穿采集→审查→铺货全生命周期。
        -- status 是对外 6 个人话态，stage 是内部技术阶段（driver 用，前端不读）。
        CREATE TABLE IF NOT EXISTS pipeline_products (
          id TEXT PRIMARY KEY,
          external_product_id TEXT,
          title TEXT NOT NULL,
          source_url TEXT NOT NULL,
          category_path TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL,
          stage TEXT NOT NULL,
          attention TEXT NOT NULL DEFAULT 'none',
          error_code TEXT,
          error_reason TEXT,
          progress_text TEXT,
          collected_data TEXT,
          reviewed_data TEXT,
          review_result_json TEXT,
          pricing_strategy_json TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        -- 推进执行单位：商品 × 目标店（多对多），driver 真正推进的行；每店各自的阶段/状态/发品结果。
        CREATE TABLE IF NOT EXISTS pipeline_shop_targets (
          id TEXT PRIMARY KEY,
          product_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          shop_name TEXT NOT NULL,
          stage TEXT NOT NULL,
          status TEXT NOT NULL,
          error_code TEXT,
          error_summary TEXT,
          raw_payload TEXT,
          wechat_product_id TEXT,
          wechat_sku_id TEXT,
          wechat_status INTEGER,
          wechat_edit_status INTEGER,
          last_status_sync_at TEXT,
          audit_summary TEXT,
          retry_count INTEGER NOT NULL DEFAULT 0,
          next_retry_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(product_id, shop_id),
          FOREIGN KEY(product_id) REFERENCES pipeline_products(id)
        );

        -- 素材表：店级图片上传缓存，按 (target, 源链接) 唯一，天然幂等。
        CREATE TABLE IF NOT EXISTS pipeline_assets (
          id TEXT PRIMARY KEY,
          target_id TEXT NOT NULL,
          product_id TEXT NOT NULL,
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
          UNIQUE(target_id, source_url),
          FOREIGN KEY(target_id) REFERENCES pipeline_shop_targets(id),
          FOREIGN KEY(product_id) REFERENCES pipeline_products(id)
        );

        -- 导入批次：一行 = 一次导入操作（Excel 导入 / 采集任务转铺货）。
        -- 商品通过 pipeline_products.import_batch_id 归属批次（无外键，批次行后插）；
        -- 工作台按批次筛选查看与批量操作，名称自动生成、可重命名。
        CREATE TABLE IF NOT EXISTS import_batches (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          source TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        "#,
    )?;

    // 历史遗留迁移：task_logs.task_id 的旧外键指向已废弃的 task_runs；新流水线模型用
    // product_id 作 task_id 写日志会触发 FOREIGN KEY constraint failed —— 导致铺货 precheck
    // 事务回滚、整条流水线悄无声息卡死。检测到旧外键则重建 task_logs 去掉该外键。
    let task_logs_has_legacy_fk = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_foreign_key_list('task_logs') WHERE \"table\" = 'task_runs'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false);
    if task_logs_has_legacy_fk {
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        conn.execute_batch(
            r#"
            CREATE TABLE task_logs_rebuilt (
              id TEXT PRIMARY KEY,
              task_id TEXT NOT NULL,
              item_id TEXT,
              level TEXT NOT NULL,
              message TEXT NOT NULL,
              detail_json TEXT,
              created_at TEXT NOT NULL
            );
            INSERT INTO task_logs_rebuilt
              SELECT id, task_id, item_id, level, message, detail_json, created_at FROM task_logs;
            DROP TABLE task_logs;
            ALTER TABLE task_logs_rebuilt RENAME TO task_logs;
            "#,
        )?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
    }

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
    // 已上架商品归档时间：非空=已从铺货工作台默认视图隐藏（仅 status=listed 可归档）
    ensure_column(conn, "pipeline_products", "archived_at", "TEXT")?;
    // 导入批次归属：每次导入操作打同一批次标，工作台按批次筛选与批量操作
    ensure_column(conn, "pipeline_products", "import_batch_id", "TEXT")?;
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
    // ===== 订单履约重设计一期（docs/order-fulfillment-redesign.md §9）=====
    // 详情回刷脏标 + 售后活跃标志（双轴状态机的横向标志列，不再吞履约主状态）
    ensure_column(conn, "orders", "detail_dirty", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(
        conn,
        "orders",
        "has_active_aftersale",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    // 金额镜像：改价/退款后由详情回刷保持新鲜（merchant_receive 对应官方 merchant_receieve_price 原拼写）
    ensure_column(conn, "orders", "order_price_cents", "INTEGER")?;
    ensure_column(conn, "orders", "merchant_receive_cents", "INTEGER")?;
    ensure_column(conn, "orders", "freight_cents", "INTEGER")?;
    ensure_column(conn, "orders", "is_change_price", "INTEGER")?;
    // 协商镜像：买家改址申请（12h 超时=自动同意）/ 发货前换SKU（超时=自动拒绝）/ 发货时效
    ensure_column(
        conn,
        "orders",
        "address_under_review",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(conn, "orders", "address_apply_time", "INTEGER")?;
    ensure_column(conn, "orders", "change_sku_state", "INTEGER")?;
    ensure_column(conn, "orders", "change_sku_ddl", "INTEGER")?;
    ensure_column(conn, "orders", "delivery_deadline", "INTEGER")?;
    ensure_column(conn, "orders", "predict_delivery_time", "INTEGER")?;
    ensure_column(conn, "orders", "delivery_time_type", "INTEGER")?;
    ensure_column(conn, "orders", "merchant_notes", "TEXT")?;
    ensure_column(conn, "orders", "customer_notes", "TEXT")?;
    // 虚拟号镜像：联系买家与到期巡检用
    ensure_column(conn, "orders", "use_tel_number", "INTEGER")?;
    ensure_column(conn, "orders", "virtual_tel_expire_time", "INTEGER")?;
    // 订单项金额与身份锚点（product_unique_id 官方注明下单后不变，换SKU 对照锚点）
    ensure_column(conn, "order_items", "estimate_price", "INTEGER")?;
    ensure_column(conn, "order_items", "change_price", "INTEGER")?;
    ensure_column(conn, "order_items", "product_unique_id", "TEXT")?;
    // ===== 订单履约重设计二期 =====
    // 采购双节点：purchased_at 把 pending_purchase 拆成「待采购」与「待回运单」两条队列
    ensure_column(conn, "purchase_tasks", "purchased_at", "TEXT")?;
    // 换SKU 同意后旧任务的审计链指针
    ensure_column(conn, "purchase_tasks", "superseded_by", "TEXT")?;
    // 发货前置守卫拦截原因（守卫把官方错误码前移，避免盲调 API）
    ensure_column(conn, "shipments", "blocked_reason", "TEXT")?;
    // ===== 订单履约重设计三期 =====
    // 改运单（官方上限 3 次）与补发（官方上限 10 个包裹）的本地计数，超限前置禁用
    ensure_column(
        conn,
        "orders",
        "delivery_change_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "orders",
        "compensation_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    // 虚拟号已延期次数镜像（virtualnumber/delay 必须回传该值）
    ensure_column(conn, "orders", "virtual_tel_delay_times", "INTEGER")?;
    conn.execute_batch(
        r#"
        -- 解密收货信息专表（与脱敏 raw_payload 隔离）：
        -- 姓名/电话/详址为 AES-256-GCM 密文 JSON（{"ciphertext","nonce"}），省市区留明文供展示分组；
        -- 备份/导出默认排除本表，订单终态 30 天后 GC 置空密文字段。
        CREATE TABLE IF NOT EXISTS order_decoded_addresses (
          order_id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          wechat_order_id TEXT NOT NULL,
          user_name_enc TEXT,
          tel_number_enc TEXT,
          detail_info_enc TEXT,
          province TEXT,
          city TEXT,
          county TEXT,
          virtual_number TEXT,
          virtual_extension TEXT,
          virtual_expiration INTEGER,
          hash_code TEXT,
          decode_error TEXT,
          decode_skip_reason TEXT,
          decoded_at TEXT,
          purged_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        -- 包裹一等实体（订单履约三期）：官方 delivery_product_info 天然是数组（一单多包裹），
        -- source: local_send=本系统提交 / wechat_mirror=详情回刷镜像 / compensation=补发
        CREATE TABLE IF NOT EXISTS order_packages (
          id TEXT PRIMARY KEY,
          order_id TEXT NOT NULL,
          shop_id TEXT NOT NULL,
          wechat_order_id TEXT NOT NULL,
          delivery_id TEXT,
          delivery_name TEXT,
          waybill_id TEXT,
          deliver_type INTEGER NOT NULL DEFAULT 1,
          source TEXT NOT NULL,
          status TEXT NOT NULL,
          delivery_time INTEGER,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(order_id, delivery_id, waybill_id)
        );

        CREATE TABLE IF NOT EXISTS order_package_items (
          id TEXT PRIMARY KEY,
          package_id TEXT NOT NULL,
          order_item_id TEXT,
          wechat_product_id TEXT,
          wechat_sku_id TEXT,
          product_cnt INTEGER NOT NULL DEFAULT 1,
          created_at TEXT NOT NULL,
          FOREIGN KEY(package_id) REFERENCES order_packages(id)
        );

        CREATE INDEX IF NOT EXISTS idx_order_packages_order
          ON order_packages(order_id);

        CREATE INDEX IF NOT EXISTS idx_order_package_items_package
          ON order_package_items(package_id);

        -- 申请收件箱：改址/换SKU/发货协商等待办一等实体（镜像列做发货守卫，收件箱做待办与审计）
        CREATE TABLE IF NOT EXISTS order_requests (
          id TEXT PRIMARY KEY,
          shop_id TEXT NOT NULL,
          order_id TEXT,
          wechat_order_id TEXT NOT NULL,
          kind TEXT NOT NULL,
          state TEXT NOT NULL,
          deadline_at INTEGER,
          payload_json TEXT,
          resolution TEXT,
          resolved_at TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(shop_id, wechat_order_id, kind)
        );

        CREATE INDEX IF NOT EXISTS idx_order_requests_state
          ON order_requests(state, deadline_at);
        "#,
    )?;
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

        CREATE INDEX IF NOT EXISTS idx_wechat_category_relations_status
          ON wechat_category_relations(shop_id, status);

        CREATE INDEX IF NOT EXISTS idx_wechat_category_prechecks_shop
          ON wechat_category_prechecks(shop_id, checked_at);

        CREATE INDEX IF NOT EXISTS idx_wechat_freight_templates_shop
          ON wechat_freight_templates(shop_id);

        CREATE INDEX IF NOT EXISTS idx_delivery_companies_shop
          ON delivery_companies(shop_id);

        CREATE INDEX IF NOT EXISTS idx_collection_tasks_status
          ON collection_tasks(status);

        CREATE INDEX IF NOT EXISTS idx_pipeline_products_status
          ON pipeline_products(status);

        CREATE INDEX IF NOT EXISTS idx_pipeline_products_stage
          ON pipeline_products(stage);

        CREATE INDEX IF NOT EXISTS idx_pipeline_shop_targets_product
          ON pipeline_shop_targets(product_id);

        CREATE INDEX IF NOT EXISTS idx_pipeline_shop_targets_stage_status
          ON pipeline_shop_targets(stage, status);

        CREATE INDEX IF NOT EXISTS idx_pipeline_shop_targets_retry
          ON pipeline_shop_targets(next_retry_at);

        CREATE INDEX IF NOT EXISTS idx_pipeline_assets_target
          ON pipeline_assets(target_id, status);

        CREATE INDEX IF NOT EXISTS idx_wechat_shop_products_shop_status
          ON wechat_shop_products(shop_id, status);

        CREATE INDEX IF NOT EXISTS idx_wechat_shop_product_skus_product_row
          ON wechat_shop_product_skus(product_row_id);
        "#,
    )?;
    // 本机 HTTP API 已整体移除，连带清理其调用日志表（一次性，幂等）
    conn.execute_batch("DROP TABLE IF EXISTS external_api_logs;")?;
    // 订单履约重设计一期：详情回刷与队列索引
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_orders_detail_dirty
          ON orders(detail_dirty, status);

        CREATE INDEX IF NOT EXISTS idx_orders_fulfillment
          ON orders(shop_id, status);
        "#,
    )?;
    migrate_publish_automation_switch(conn)?;
    migrate_legacy_import_batch(conn)?;
    migrate_order_fulfillment_status(conn)?;
    cleanup_stale_wechat_category_cache(conn)?;
    Ok(())
}

/// 一次性幂等迁移（订单履约重设计一期）：旧实现把售后激活写成 status='aftersale_active'，
/// 吞掉了订单的履约位置。迁移为：标志位 has_active_aftersale=1 + status 按微信状态重算 +
/// detail_dirty=1 强制详情回刷校准（老单的 wechat_status 可能是列表同步写的过滤值，不准）。
/// 迁移后不再有 aftersale_active 行，后续每次空跑 0 行，开销可忽略。
fn migrate_order_fulfillment_status(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE orders
         SET has_active_aftersale = 1,
             detail_dirty = 1,
             status = CASE
               WHEN wechat_status IN (10, 12, 13) THEN 'unpaid'
               WHEN wechat_status = 20 THEN 'pending_shipment'
               WHEN wechat_status = 21 THEN 'partially_shipped'
               WHEN wechat_status = 30 THEN 'wechat_shipped'
               WHEN wechat_status = 100 THEN 'completed'
               WHEN wechat_status IN (200, 250) THEN 'cancelled'
               ELSE 'synced'
             END
         WHERE status = 'aftersale_active'",
        [],
    )?;
    Ok(())
}

/// 批次维度上线前导入的存量商品统一归入固定的「历史数据」批次（幂等：
/// 仅当存在未归批商品时补建批次行并回填，新导入的商品入口处必带批次不会再触发）。
fn migrate_legacy_import_batch(conn: &Connection) -> AppResult<()> {
    let has_unbatched: bool = conn
        .query_row(
            "SELECT 1 FROM pipeline_products WHERE import_batch_id IS NULL LIMIT 1",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !has_unbatched {
        return Ok(());
    }
    conn.execute(
        "INSERT OR IGNORE INTO import_batches (id, name, source, created_at)
         VALUES ('batch_legacy', '历史数据', 'legacy', ?1)",
        [now_shanghai()],
    )?;
    conn.execute(
        "UPDATE pipeline_products SET import_batch_id = 'batch_legacy'
         WHERE import_batch_id IS NULL",
        [],
    )?;
    Ok(())
}

/// 一次性迁移：铺货 7 个细粒度自动化开关收敛为单一总开关 automation.publish_enabled。
/// 新值 = 7 个旧开关的 AND（与前端原聚合语义一致），写入后删除旧键，不保留旧键读取路径。
fn migrate_publish_automation_switch(conn: &Connection) -> AppResult<()> {
    const LEGACY_KEYS: [&str; 7] = [
        "automation.publish_precheck_enabled",
        "automation.publish_attribute_fill_enabled",
        "automation.publish_category_precheck_enabled",
        "automation.publish_asset_upload_enabled",
        "automation.publish_submit_enabled",
        "automation.publish_status_sync_enabled",
        "automation.publish_listing_enabled",
    ];
    let new_key_exists: bool = conn
        .query_row(
            "SELECT 1 FROM app_settings WHERE key = 'automation.publish_enabled'",
            [],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !new_key_exists {
        let mut all_enabled = true;
        let mut has_legacy = false;
        for key in LEGACY_KEYS {
            let value: Option<String> = conn
                .query_row(
                    "SELECT value_json FROM app_settings WHERE key = ?1",
                    [key],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(raw) = value {
                has_legacy = true;
                if serde_json::from_str::<bool>(&raw).ok() == Some(false) {
                    all_enabled = false;
                }
            }
        }
        if has_legacy {
            conn.execute(
                "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at)
                 VALUES ('automation.publish_enabled', ?1, ?2)",
                (if all_enabled { "true" } else { "false" }, now_shanghai()),
            )?;
        }
    }
    for key in LEGACY_KEYS {
        conn.execute("DELETE FROM app_settings WHERE key = ?1", [key])?;
    }
    Ok(())
}

fn cleanup_stale_wechat_category_cache(conn: &Connection) -> AppResult<()> {
    // 只清理已经同步过店铺类目权限的店铺，并保留生效类目的完整父级路径。
    conn.execute(
        r#"
        WITH RECURSIVE
          shops_with_relations(shop_id) AS (
            SELECT DISTINCT shop_id
            FROM wechat_category_relations
          ),
          keep(shop_id, cat_id, parent_cat_id) AS (
            SELECT category.shop_id, category.cat_id, category.parent_cat_id
            FROM wechat_categories category
            JOIN wechat_category_relations relation
              ON relation.shop_id = category.shop_id
             AND relation.cat_id = category.cat_id
             AND relation.status = 1
            UNION
            SELECT parent.shop_id, parent.cat_id, parent.parent_cat_id
            FROM wechat_categories parent
            JOIN keep child
              ON child.shop_id = parent.shop_id
             AND child.parent_cat_id = parent.cat_id
          )
        DELETE FROM wechat_categories
        WHERE shop_id IN (SELECT shop_id FROM shops_with_relations)
          AND NOT EXISTS (
            SELECT 1
            FROM keep
            WHERE keep.shop_id = wechat_categories.shop_id
              AND keep.cat_id = wechat_categories.cat_id
          )
        "#,
        [],
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
