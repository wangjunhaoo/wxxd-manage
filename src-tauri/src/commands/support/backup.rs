use super::*;

pub(in crate::commands) fn database_backup_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let database = database_path(app)?;
    let parent = database.parent().ok_or_else(|| {
        AppError::Validation("无法定位数据库所在目录，不能创建备份目录".to_string())
    })?;
    let dir = parent.join("backups");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(in crate::commands) fn create_database_backup_file(
    app: &AppHandle,
    prefix: &str,
) -> AppResult<BackupInfo> {
    checkpoint_database(app)?;
    let database = database_path(app)?;
    if !database.exists() {
        return Err(AppError::Validation(
            "本地数据库文件不存在，无法备份".to_string(),
        ));
    }
    let backup_dir = database_backup_dir(app)?;
    let timestamp = safe_backup_timestamp();
    let mut backup_path = backup_dir.join(format!("wx-xd-{prefix}-{timestamp}.sqlite"));
    if backup_path.exists() {
        backup_path = backup_dir.join(format!(
            "wx-xd-{prefix}-{timestamp}-{}.sqlite",
            Uuid::new_v4()
        ));
    }
    fs::copy(&database, &backup_path)?;
    let backup = backup_info_from_path(&backup_path)?;
    if !backup.integrity_ok {
        let _ = fs::remove_file(&backup_path);
        return Err(AppError::Validation(format!(
            "备份完成后完整性校验失败：{}",
            backup.integrity_message
        )));
    }
    Ok(backup)
}

pub(in crate::commands) fn checkpoint_database(app: &AppHandle) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

pub(in crate::commands) fn backup_info_from_path(path: &Path) -> AppResult<BackupInfo> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()
        .map(DateTime::<Utc>::from)
        .map(format_shanghai)
        .unwrap_or_else(|_| now_shanghai());
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown.sqlite")
        .to_string();
    let id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&file_name)
        .to_string();
    let (integrity_ok, integrity_message) = check_sqlite_integrity(path);
    Ok(BackupInfo {
        id,
        file_name,
        file_path: path.display().to_string(),
        size_bytes: metadata.len(),
        sha256: sha256_file(path)?,
        created_at: modified,
        integrity_ok,
        integrity_message,
    })
}

pub(in crate::commands) fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(in crate::commands) fn check_sqlite_integrity(path: &Path) -> (bool, String) {
    match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|conn| sqlite_integrity_message(&conn))
    {
        Ok(message) => (message.eq_ignore_ascii_case("ok"), message),
        Err(error) => (false, error.to_string()),
    }
}

pub(in crate::commands) fn sqlite_integrity_message(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
}

pub(in crate::commands) fn safe_backup_timestamp() -> String {
    now_shanghai()
        .replace(':', "-")
        .replace('+', "plus")
        .replace('/', "-")
}

pub(in crate::commands) fn shanghai_date_key() -> String {
    now_shanghai().chars().take(10).collect()
}

pub(in crate::commands) fn remove_sqlite_sidecars(database: &Path) -> AppResult<()> {
    let file_name = database
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::Validation("数据库路径缺少文件名".to_string()))?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = database.with_file_name(format!("{file_name}{suffix}"));
        if sidecar.exists() {
            fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}

pub(in crate::commands) fn set_bool_setting(
    conn: &Connection,
    key: &str,
    value: bool,
) -> AppResult<()> {
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
