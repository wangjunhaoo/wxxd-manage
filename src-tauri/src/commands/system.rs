use super::*;

#[tauri::command]
pub fn get_dashboard(app: AppHandle) -> AppResult<DashboardSummary> {
    let conn = open_connection(&app)?;
    let pending_order_count = count_by_sql(
        &conn,
        "SELECT COUNT(*) FROM orders WHERE status NOT IN ('completed', 'cancelled')",
    )?;
    let abnormal_shop_count =
        count_by_sql(&conn, "SELECT COUNT(*) FROM shops WHERE status != 'active'")?;
    let failed_publish_product_count = count_by_sql(
        &conn,
        "SELECT COUNT(DISTINCT product_row_id) FROM publish_job_items WHERE status = 'failed'",
    )?;
    let unread_notification_count = count_by_sql(
        &conn,
        "SELECT COUNT(*) FROM notifications WHERE status = 'unread'",
    )?;
    let running_task_count = count_by_sql(
        &conn,
        "SELECT COUNT(*) FROM task_runs WHERE status IN ('pending', 'queued', 'running', 'paused')",
    )?;
    let last_publish_summary = conn
        .query_row(
            "SELECT id || ' / ' || status || ' / 商品 ' || accepted_product_count || ' / 店铺 ' || target_shop_count
             FROM publish_jobs ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let last_order_sync_at = conn
        .query_row("SELECT MAX(synced_at) FROM orders", [], |row| {
            row.get::<_, Option<String>>(0)
        })?
        .filter(|value| !value.trim().is_empty());
    let order_driver_heartbeat_at =
        get_string_setting(&conn, ORDER_DRIVER_HEARTBEAT_SETTING)?.filter(|v| !v.trim().is_empty());

    Ok(DashboardSummary {
        pending_order_count,
        abnormal_shop_count,
        failed_publish_product_count,
        unread_notification_count,
        running_task_count,
        controller_status: "主控机运行中".to_string(),
        database_path: database_path(&app)?.display().to_string(),
        now_shanghai: now_shanghai(),
        last_order_sync_at,
        last_publish_summary,
        order_driver_heartbeat_at,
    })
}

#[tauri::command]
pub fn list_database_backups(app: AppHandle) -> AppResult<Vec<BackupInfo>> {
    let dir = database_backup_dir(&app)?;
    let mut backups = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sqlite") {
            continue;
        }
        backups.push(backup_info_from_path(&path)?);
    }
    backups.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(backups)
}

#[tauri::command]
pub fn create_database_backup(app: AppHandle) -> AppResult<BackupCreateResult> {
    Ok(BackupCreateResult {
        backup: create_database_backup_file(&app, "manual")?,
    })
}

pub fn run_startup_database_backup(app: AppHandle) -> AppResult<Option<BackupInfo>> {
    let today = shanghai_date_key();
    let conn = open_connection(&app)?;
    let last_backup_date = get_string_setting(&conn, AUTO_BACKUP_LAST_DATE_SETTING)?;
    if last_backup_date.as_deref() == Some(today.as_str()) {
        return Ok(None);
    }
    drop(conn);

    let backup = create_database_backup_file(&app, "auto")?;
    let conn = open_connection(&app)?;
    set_string_setting(&conn, AUTO_BACKUP_LAST_DATE_SETTING, &today)?;
    Ok(Some(backup))
}

#[tauri::command]
pub fn restore_database_backup(
    app: AppHandle,
    request: BackupRestoreRequest,
) -> AppResult<BackupRestoreResult> {
    let backup_dir = database_backup_dir(&app)?;
    let canonical_backup_dir = backup_dir.canonicalize()?;
    let source_path = PathBuf::from(request.file_path);
    let canonical_source = source_path
        .canonicalize()
        .map_err(|error| AppError::Validation(format!("备份文件不存在或无法读取：{error}")))?;
    if !canonical_source.starts_with(&canonical_backup_dir) {
        return Err(AppError::Security(
            "只能从应用备份目录恢复数据库，禁止使用任意外部路径覆盖本地数据".to_string(),
        ));
    }
    let source_info = backup_info_from_path(&canonical_source)?;
    if !source_info.integrity_ok {
        return Err(AppError::Validation(format!(
            "备份文件校验失败：{}",
            source_info.integrity_message
        )));
    }

    let rollback_backup = create_database_backup_file(&app, "pre-restore")?;
    checkpoint_database(&app)?;
    let database = database_path(&app)?;
    fs::copy(&canonical_source, &database)?;
    remove_sqlite_sidecars(&database)?;

    let conn = open_connection(&app)?;
    let integrity_message = sqlite_integrity_message(&conn)?;
    let integrity_ok = integrity_message.eq_ignore_ascii_case("ok");
    if !integrity_ok {
        return Err(AppError::Validation(format!(
            "恢复后数据库完整性校验失败：{integrity_message}；已保留回滚备份 {}",
            rollback_backup.file_path
        )));
    }
    Ok(BackupRestoreResult {
        restored_from: source_info,
        rollback_backup,
        integrity_ok,
        message: "数据库已恢复并通过完整性校验，建议重启应用以确保所有页面读取最新连接。"
            .to_string(),
    })
}

const COLLECTION_PUBLISH_RESET_CONFIRM_TEXT: &str = "确认清理采集和铺货数据";

#[tauri::command]
pub fn reset_collection_publish_workspace(
    app: AppHandle,
    request: CollectionPublishWorkspaceResetRequest,
) -> AppResult<CollectionPublishWorkspaceResetResult> {
    if request.confirm_text.trim() != COLLECTION_PUBLISH_RESET_CONFIRM_TEXT {
        return Err(AppError::Validation(format!(
            "确认文本不匹配，请输入：{COLLECTION_PUBLISH_RESET_CONFIRM_TEXT}"
        )));
    }

    let conn = open_connection(&app)?;
    let before_integrity = sqlite_integrity_message(&conn)?;
    if !before_integrity.eq_ignore_ascii_case("ok") {
        return Err(AppError::Validation(format!(
            "清理前数据库完整性校验失败：{before_integrity}"
        )));
    }
    let before_counts = collection_publish_workspace_counts(&conn)?;
    drop(conn);

    let backup = create_database_backup_file(&app, "manual-before-reset-collection-publish")?;

    let mut conn = open_connection(&app)?;
    {
        let tx = conn.transaction()?;
        tx.execute_batch(
            r#"
            DELETE FROM task_logs
             WHERE task_id IN (SELECT id FROM publish_jobs)
                OR item_id IN (SELECT id FROM publish_job_items)
                OR task_id IN (SELECT id FROM collection_tasks)
                OR item_id IN (SELECT id FROM collection_tasks)
                OR task_id IN (
                     SELECT id FROM task_runs
                      WHERE task_type LIKE 'publish.%'
                         OR task_type LIKE 'collection.%'
                   );

            DELETE FROM task_runs
             WHERE task_type LIKE 'publish.%'
                OR task_type LIKE 'collection.%';

            DELETE FROM notifications
             WHERE source_type IN ('publish_item', 'publish_job', 'collection_task');

            DELETE FROM agent_run_events
             WHERE run_id IN (
               SELECT id FROM agent_runs
                WHERE scene IN ('collection_review', 'publish_attribute')
                   OR source_type IN ('collection_task', 'publish_item')
             );

            DELETE FROM agent_runs
             WHERE scene IN ('collection_review', 'publish_attribute')
                OR source_type IN ('collection_task', 'publish_item');

            DELETE FROM publish_attribute_suggestions;
            DELETE FROM publish_assets;
            DELETE FROM publish_job_items;
            DELETE FROM publish_products;
            DELETE FROM publish_jobs;
            DELETE FROM collection_tasks;
            DELETE FROM pipeline_assets;
            DELETE FROM pipeline_shop_targets;
            DELETE FROM pipeline_products;
            DELETE FROM import_batches;
            "#,
        )?;
        tx.commit()?;
    }

    let after_counts = collection_publish_workspace_counts(&conn)?;
    let integrity_message = sqlite_integrity_message(&conn)?;
    let integrity_ok = integrity_message.eq_ignore_ascii_case("ok");
    if !integrity_ok {
        return Err(AppError::Validation(format!(
            "清理后数据库完整性校验失败：{integrity_message}；清理前备份为 {}",
            backup.file_path
        )));
    }

    let counts = before_counts
        .into_iter()
        .zip(after_counts)
        .map(|(before, after)| WorkspaceResetTableCount {
            name: before.name,
            before: before.count,
            after: after.count,
        })
        .collect::<Vec<_>>();

    Ok(CollectionPublishWorkspaceResetResult {
        backup,
        integrity_ok,
        integrity_message,
        counts,
        message: "采集/铺货工作区已清理；店铺、密钥、微信类目缓存、运费模板、订单、售后、采购和发货数据已保留。".to_string(),
    })
}

struct WorkspaceResetCountSnapshot {
    name: String,
    count: i64,
}

fn collection_publish_workspace_counts(
    conn: &Connection,
) -> AppResult<Vec<WorkspaceResetCountSnapshot>> {
    let queries = [
        (
            "pipeline_products",
            "SELECT COUNT(*) FROM pipeline_products",
        ),
        (
            "pipeline_shop_targets",
            "SELECT COUNT(*) FROM pipeline_shop_targets",
        ),
        ("pipeline_assets", "SELECT COUNT(*) FROM pipeline_assets"),
        ("import_batches", "SELECT COUNT(*) FROM import_batches"),
        ("collection_tasks", "SELECT COUNT(*) FROM collection_tasks"),
        ("publish_jobs", "SELECT COUNT(*) FROM publish_jobs"),
        ("publish_products", "SELECT COUNT(*) FROM publish_products"),
        ("publish_job_items", "SELECT COUNT(*) FROM publish_job_items"),
        ("publish_assets", "SELECT COUNT(*) FROM publish_assets"),
        (
            "publish_attribute_suggestions",
            "SELECT COUNT(*) FROM publish_attribute_suggestions",
        ),
        (
            "publish_collection_notifications",
            "SELECT COUNT(*) FROM notifications WHERE source_type IN ('publish_item', 'publish_job', 'collection_task')",
        ),
        (
            "publish_collection_task_logs",
            "SELECT COUNT(*) FROM task_logs
              WHERE task_id IN (SELECT id FROM publish_jobs)
                 OR item_id IN (SELECT id FROM publish_job_items)
                 OR task_id IN (SELECT id FROM collection_tasks)
                 OR item_id IN (SELECT id FROM collection_tasks)
                 OR task_id IN (
                      SELECT id FROM task_runs
                       WHERE task_type LIKE 'publish.%'
                          OR task_type LIKE 'collection.%'
                    )",
        ),
        (
            "publish_collection_task_runs",
            "SELECT COUNT(*) FROM task_runs WHERE task_type LIKE 'publish.%' OR task_type LIKE 'collection.%'",
        ),
        (
            "publish_collection_agent_runs",
            "SELECT COUNT(*) FROM agent_runs
              WHERE scene IN ('collection_review', 'publish_attribute')
                 OR source_type IN ('collection_task', 'publish_item')",
        ),
        (
            "publish_collection_agent_run_events",
            "SELECT COUNT(*) FROM agent_run_events
              WHERE run_id IN (
                SELECT id FROM agent_runs
                 WHERE scene IN ('collection_review', 'publish_attribute')
                    OR source_type IN ('collection_task', 'publish_item')
              )",
        ),
    ];

    queries
        .iter()
        .map(|(name, sql)| {
            let count = count_by_sql(conn, sql)?;
            Ok(WorkspaceResetCountSnapshot {
                name: (*name).to_string(),
                count,
            })
        })
        .collect()
}

#[tauri::command]
pub fn list_notifications(
    app: AppHandle,
    status: Option<String>,
    severity: Option<String>,
    limit: Option<i64>,
) -> AppResult<NotificationListResult> {
    let conn = open_connection(&app)?;
    let status = normalize_optional_filter(status);
    let severity = normalize_optional_filter(severity);
    let limit = limit.unwrap_or(120).clamp(1, 500);
    let total = count_notifications(&conn, status.as_deref(), severity.as_deref())?;
    let unread_count = count_notifications(&conn, Some("unread"), None)?;
    let critical_count = conn.query_row(
        "SELECT COUNT(*)
         FROM notifications
         WHERE status = 'unread' AND severity = 'critical'",
        [],
        |row| row.get(0),
    )?;
    let items = load_notification_views(&conn, status.as_deref(), severity.as_deref(), limit)?;
    Ok(NotificationListResult {
        items,
        total,
        unread_count,
        critical_count,
    })
}

#[tauri::command]
pub fn mark_notification_read(
    app: AppHandle,
    notification_id: String,
) -> AppResult<NotificationMarkResult> {
    let notification_id = notification_id.trim();
    if notification_id.is_empty() {
        return Err(AppError::Validation("通知 ID 不能为空".to_string()));
    }
    let conn = open_connection(&app)?;
    let updated_count = conn.execute(
        "UPDATE notifications
         SET status = 'read', read_at = COALESCE(read_at, ?1), updated_at = ?1
         WHERE id = ?2 AND status != 'read'",
        params![now_shanghai(), notification_id],
    )? as i64;
    Ok(NotificationMarkResult {
        updated_count,
        message: if updated_count > 0 {
            "通知已标记已读".to_string()
        } else {
            "通知已经是已读状态".to_string()
        },
    })
}

#[tauri::command]
pub fn mark_all_notifications_read(app: AppHandle) -> AppResult<NotificationMarkResult> {
    let conn = open_connection(&app)?;
    let updated_count = conn.execute(
        "UPDATE notifications
         SET status = 'read', read_at = COALESCE(read_at, ?1), updated_at = ?1
         WHERE status = 'unread'",
        [now_shanghai()],
    )? as i64;
    Ok(NotificationMarkResult {
        updated_count,
        message: format!("已标记 {updated_count} 条通知为已读"),
    })
}
