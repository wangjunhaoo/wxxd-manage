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

#[tauri::command]
pub fn list_external_api_logs(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<Vec<ExternalApiLogView>> {
    let limit = limit.unwrap_or(80).clamp(1, 300);
    let conn = open_connection(&app)?;
    let mut stmt = conn.prepare(
        "SELECT id, method, path, status, status_code, error_code,
                request_summary, response_summary, duration_ms, created_at
           FROM external_api_logs
          ORDER BY created_at DESC
          LIMIT ?1",
    )?;
    let logs = stmt
        .query_map([limit], |row| {
            Ok(ExternalApiLogView {
                id: row.get(0)?,
                method: row.get(1)?,
                path: row.get(2)?,
                status: row.get(3)?,
                status_code: row.get(4)?,
                error_code: row.get(5)?,
                request_summary: row.get(6)?,
                response_summary: row.get(7)?,
                duration_ms: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(logs)
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
