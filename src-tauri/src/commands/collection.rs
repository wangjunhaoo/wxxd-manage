use super::*;

use calamine::{open_workbook, Reader, Xlsx};
use std::sync::atomic::{AtomicBool, Ordering};

static COLLECTOR_RUNNING: AtomicBool = AtomicBool::new(false);

// 1. Excel 导入并创建采集任务
#[tauri::command]
pub fn import_excel_for_collection(
    app: AppHandle,
    file_path: String,
    target_shop_ids: Vec<String>,
) -> AppResult<i64> {
    if target_shop_ids.is_empty() {
        return Err(AppError::Validation("目标店铺不能为空".to_string()));
    }

    let path = PathBuf::from(&file_path);
    if !path.exists() {
        return Err(AppError::Validation(format!(
            "Excel 文件不存在: {}",
            file_path
        )));
    }

    let mut excel: Xlsx<_> = open_workbook(&path)
        .map_err(|e| AppError::Validation(format!("无法打开 Excel 文件: {}", e)))?;

    let conn = open_connection(&app)?;
    let now = now_shanghai();
    let target_shops_json =
        serde_json::to_string(&target_shop_ids).unwrap_or_else(|_| "[]".to_string());

    let mut imported_count = 0i64;

    if let Some(Ok(range)) = excel.worksheet_range_at(0) {
        for row in range.rows() {
            if row.len() < 2 {
                continue;
            }
            let title = row
                .get(0)
                .map(|d| d.to_string().trim().to_string())
                .unwrap_or_default();
            let source_url = row
                .get(1)
                .map(|d| d.to_string().trim().to_string())
                .unwrap_or_default();
            let category_path = row
                .get(2)
                .map(|d| d.to_string().trim().to_string())
                .unwrap_or_default();

            if title.is_empty() || source_url.is_empty() {
                continue;
            }

            // 简单校验链接
            if !source_url.contains("item.taobao.com") && !source_url.contains("detail.tmall.com") {
                return Err(AppError::Validation(format!(
                    "商品 {} 链接格式不正确，仅支持淘宝/天猫商品链接",
                    title
                )));
            }

            let task_id = format!("col_{}", Uuid::new_v4().simple());
            conn.execute(
                "INSERT INTO collection_tasks (id, title, source_url, category_path, target_shop_ids, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)",
                params![
                    task_id,
                    title,
                    source_url,
                    category_path,
                    target_shops_json,
                    now,
                ],
            )?;
            imported_count += 1;
        }
    }

    if imported_count > 0 {
        // 导入成功后触发后台采集 Worker
        trigger_collection_worker(app);
    }

    Ok(imported_count)
}

// 2. 淘宝登录以保存 Profile
#[tauri::command]
pub async fn open_taobao_login(app: AppHandle) -> AppResult<()> {
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    let mut script_path = PathBuf::from("scripts").join("taobao_collector.py");
    if !script_path.exists() {
        if let Ok(res_dir) = app.path().resource_dir() {
            let check_path = res_dir.join("scripts").join("taobao_collector.py");
            if check_path.exists() {
                script_path = check_path;
            }
        }
    }
    if !script_path.exists() {
        script_path = PathBuf::from(
            "/Users/wangjunhao/myself/project/wxxd-manage/scripts/taobao_collector.py",
        );
    }

    // 异步启动登录子进程，直到用户关闭浏览器
    let status = tokio::process::Command::new("python3")
        .arg(&script_path)
        .arg("login")
        .arg("--profile-dir")
        .arg(&profile_dir_str)
        .status()
        .await
        .map_err(|e| AppError::Validation(format!("无法拉起淘宝登录程序: {}", e)))?;

    if !status.success() {
        return Err(AppError::Validation(
            "淘宝登录程序执行失败或被用户强行关闭。".to_string(),
        ));
    }

    Ok(())
}

// 3. 获取采集任务列表
#[tauri::command]
pub fn get_collection_tasks(app: AppHandle) -> AppResult<Vec<CollectionTaskView>> {
    let conn = open_connection(&app)?;
    let mut stmt = conn.prepare(
        "SELECT id, title, source_url, category_path, target_shop_ids, status, error_summary, collected_data, created_at, updated_at
         FROM collection_tasks
         ORDER BY created_at DESC"
    )?;

    let list = stmt
        .query_map([], |row| {
            let target_shop_ids_json: String = row.get(4)?;
            let target_shop_ids: Vec<String> =
                serde_json::from_str(&target_shop_ids_json).unwrap_or_default();
            Ok(CollectionTaskView {
                id: row.get(0)?,
                title: row.get(1)?,
                source_url: row.get(2)?,
                category_path: row.get(3)?,
                target_shop_ids,
                status: row.get(5)?,
                error_summary: row.get(6)?,
                collected_data: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(list)
}

// 4. 重试采集任务
#[tauri::command]
pub fn retry_collection_task(app: AppHandle, task_id: String) -> AppResult<()> {
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE collection_tasks SET status = 'pending', error_summary = NULL, updated_at = ?1 WHERE id = ?2",
        params![now_shanghai(), task_id],
    )?;

    // 重新触发后台采集 Worker
    trigger_collection_worker(app);
    Ok(())
}

// 5. 清理采集任务
#[tauri::command]
pub fn clear_collection_tasks(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    conn.execute("DELETE FROM collection_tasks", [])?;
    Ok(())
}

// ================== 诊断入口 (不写库、不入队) ==================

fn resolve_collector_script(app: &AppHandle) -> PathBuf {
    let mut p = PathBuf::from("scripts").join("taobao_collector.py");
    if !p.exists() {
        if let Ok(res_dir) = app.path().resource_dir() {
            let cand = res_dir.join("scripts").join("taobao_collector.py");
            if cand.exists() {
                p = cand;
            }
        }
    }
    if !p.exists() {
        p = PathBuf::from(
            "/Users/wangjunhao/myself/project/wxxd-manage/scripts/taobao_collector.py",
        );
    }
    p
}

// 6. 检测淘宝登录态
#[tauri::command]
pub async fn check_taobao_login_state(app: AppHandle) -> AppResult<serde_json::Value> {
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    let script_path = resolve_collector_script(&app);

    let output = tokio::process::Command::new("python3")
        .arg(&script_path)
        .arg("check-login")
        .arg("--profile-dir")
        .arg(&profile_dir_str)
        .output()
        .await
        .map_err(|e| AppError::Validation(format!("无法拉起登录态检测程序: {}", e)))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !stdout_str.is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
            return Ok(v);
        }
    }

    Err(AppError::Validation(format!(
        "登录态检测程序返回异常: status={:?}, stderr={}",
        output.status.code(),
        stderr_str
    )))
}

// 7. 测试抓取单个商品 (不写库、不入队、不创建铺货任务)
#[tauri::command]
pub async fn test_taobao_collect(
    app: AppHandle,
    url: String,
    headed: Option<bool>,
) -> AppResult<serde_json::Value> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Validation("淘宝商品链接不能为空".to_string()));
    }
    if !trimmed.contains("item.taobao.com") && !trimmed.contains("detail.tmall.com") {
        return Err(AppError::Validation(
            "仅支持淘宝/天猫商品链接".to_string(),
        ));
    }

    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    let script_path = resolve_collector_script(&app);

    let mut cmd = tokio::process::Command::new("python3");
    cmd.arg(&script_path)
        .arg("collect")
        .arg("--url")
        .arg(&trimmed)
        .arg("--profile-dir")
        .arg(&profile_dir_str);
    if headed.unwrap_or(false) {
        cmd.arg("--headed");
    }
    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::Validation(format!("无法拉起采集程序: {}", e)))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let exit_ok = output.status.success();

    let raw_json_opt: Option<serde_json::Value> = if !stdout_str.is_empty() {
        serde_json::from_str(&stdout_str).ok()
    } else {
        None
    };

    let (success, error_message) = match (&raw_json_opt, exit_ok) {
        (Some(v), true) if v.get("error").is_none() => (true, None),
        (Some(v), _) => {
            let err = v
                .get("error")
                .and_then(|e| e.as_str())
                .map(|s| s.to_string());
            (false, err.or_else(|| Some("采集失败".to_string())))
        }
        (None, _) => (
            false,
            Some(format!("采集程序未输出 JSON 结果: stderr={}", stderr_str)),
        ),
    };

    Ok(serde_json::json!({
        "success": success,
        "error": error_message,
        "raw": raw_json_opt,
        "stderr": stderr_str,
    }))
}

// ================== 后台 Worker 调度机制 ==================

pub fn trigger_collection_worker(app: AppHandle) {
    if COLLECTOR_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    tokio::spawn(async move {
        loop {
            let next_task = match get_next_pending_task(&app) {
                Ok(Some(task)) => task,
                Ok(None) => {
                    COLLECTOR_RUNNING.store(false, Ordering::SeqCst);
                    break;
                }
                Err(e) => {
                    eprintln!("获取待采集任务发生数据库错误: {:?}", e);
                    COLLECTOR_RUNNING.store(false, Ordering::SeqCst);
                    break;
                }
            };

            if let Err(e) = run_single_collection_task(&app, &next_task).await {
                eprintln!("执行采集任务 {} 出错: {:?}", next_task.id, e);
            }
        }
    });
}

fn get_next_pending_task(app: &AppHandle) -> AppResult<Option<CollectionTaskView>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT id, title, source_url, category_path, target_shop_ids, status, error_summary, collected_data, created_at, updated_at
         FROM collection_tasks
         WHERE status = 'pending'
         ORDER BY created_at ASC
         LIMIT 1"
    )?;

    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let target_shop_ids_json: String = row.get(4)?;
        let target_shop_ids: Vec<String> =
            serde_json::from_str(&target_shop_ids_json).unwrap_or_default();
        Ok(Some(CollectionTaskView {
            id: row.get(0)?,
            title: row.get(1)?,
            source_url: row.get(2)?,
            category_path: row.get(3)?,
            target_shop_ids,
            status: row.get(5)?,
            error_summary: row.get(6)?,
            collected_data: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        }))
    } else {
        Ok(None)
    }
}

async fn run_single_collection_task(app: &AppHandle, task: &CollectionTaskView) -> AppResult<()> {
    // 1. 设置状态为 running
    update_task_status(app, &task.id, "running", None, None)?;

    // 2. 准备路径
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    let mut script_path = PathBuf::from("scripts").join("taobao_collector.py");
    if !script_path.exists() {
        if let Ok(res_dir) = app.path().resource_dir() {
            let check_path = res_dir.join("scripts").join("taobao_collector.py");
            if check_path.exists() {
                script_path = check_path;
            }
        }
    }
    if !script_path.exists() {
        script_path = PathBuf::from(
            "/Users/wangjunhao/myself/project/wxxd-manage/scripts/taobao_collector.py",
        );
    }

    // 3. 执行 Python 采集子进程
    let output = match tokio::process::Command::new("python3")
        .arg(&script_path)
        .arg("collect")
        .arg("--url")
        .arg(&task.source_url)
        .arg("--profile-dir")
        .arg(&profile_dir_str)
        .output()
        .await
    {
        Ok(out) => out,
        Err(e) => {
            let err_msg = format!("启动采集脚本失败: {}", e);
            update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
            return Ok(());
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut err_msg = format!("采集程序执行退出非0 (status={})", output.status);
        if !stderr.trim().is_empty() {
            err_msg = format!("{}: {}", err_msg, stderr);
        } else if !stdout.trim().is_empty() {
            err_msg = format!("{}: {}", err_msg, stdout);
        }

        // 尝试从 stdout 中解析错误 JSON
        if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            if let Some(err_val) = err_json.get("error") {
                err_msg = err_val.as_str().unwrap_or("未知采集异常").to_string();
            }
        }

        update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
        return Ok(());
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 校验解析采集结果是否为错误 JSON
    if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
        if let Some(err_val) = err_json.get("error") {
            let err_msg = err_val.as_str().unwrap_or("采集失败").to_string();
            update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
            return Ok(());
        }
    }

    let mut product_input: ExternalProductInput = match serde_json::from_str(&stdout_str) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("解析采集 JSON 结果失败: {}. 原始输出: {}", e, stdout_str);
            update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
            return Ok(());
        }
    };

    // 覆盖主标题，因为 Excel 导入指定的名称具有更高的业务置信度
    product_input.title = task.title.clone();

    // 如果类目提示字段存在，可以注入
    if !task.category_path.is_empty() {
        product_input.category_hint = Some(task.category_path.clone());
    }

    // 4. 自动创建铺货任务 (ExternalPublishJobRequest)
    let request_id = format!("pub-col-{}", Uuid::new_v4());

    let publish_req = ExternalPublishJobRequest {
        request_id,
        target_shop_group_ids: Vec::new(),
        target_shop_ids: task.target_shop_ids.clone(),
        products: vec![product_input],
    };

    // 直接调用已有的 create_external_publish_job
    match create_external_publish_job(app.clone(), publish_req) {
        Ok(_job_created) => {
            update_task_status(app, &task.id, "success", None, Some(stdout_str))?;
        }
        Err(e) => {
            let err_msg = format!("采集成功但自动生成铺货任务失败: {}", e);
            update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
        }
    }

    Ok(())
}

fn update_task_status(
    app: &AppHandle,
    task_id: &str,
    status: &str,
    error_summary: Option<String>,
    collected_data: Option<String>,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();

    if error_summary.is_some() {
        conn.execute(
            "UPDATE collection_tasks SET status = ?1, error_summary = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, error_summary, now, task_id],
        )?;
    } else if collected_data.is_some() {
        conn.execute(
            "UPDATE collection_tasks SET status = ?1, collected_data = ?2, updated_at = ?3 WHERE id = ?4",
            params![status, collected_data, now, task_id],
        )?;
    } else {
        conn.execute(
            "UPDATE collection_tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, task_id],
        )?;
    }

    Ok(())
}
