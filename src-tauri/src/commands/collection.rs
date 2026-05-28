use super::*;

use calamine::{open_workbook, Reader, Xlsx};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

static COLLECTOR_RUNNING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn get_publish_pricing_strategy(app: AppHandle) -> AppResult<PublishPricingStrategy> {
    let conn = open_connection(&app)?;
    load_publish_pricing_strategy(&conn)
}

#[tauri::command]
pub fn save_publish_pricing_strategy(
    app: AppHandle,
    strategy: PublishPricingStrategy,
) -> AppResult<PublishPricingStrategy> {
    let conn = open_connection(&app)?;
    save_publish_pricing_strategy_to_db(&conn, &strategy)?;
    Ok(strategy)
}

// 1. Excel 导入并创建采集任务
#[tauri::command]
pub fn import_excel_for_collection(app: AppHandle, file_path: String) -> AppResult<i64> {
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
    let target_shops_json = "[]";

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
        script_path =
            PathBuf::from("/Users/wangjunhao/Code/project/wx-xd/scripts/taobao_collector.py");
    }

    // 异步启动登录子进程，直到用户关闭浏览器；本地冷却期会由脚本预检拦截。
    let output = tokio::process::Command::new(resolve_python_binary())
        .arg(&script_path)
        .arg("login")
        .arg("--profile-dir")
        .arg(&profile_dir_str)
        .output()
        .await
        .map_err(|e| AppError::Validation(format!("无法拉起淘宝登录程序: {}", e)))?;

    if !output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
            if let Some(err_msg) = v.get("error").and_then(|value| value.as_str()) {
                return Err(AppError::Validation(err_msg.to_string()));
            }
        }

        let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr_str.is_empty() {
            "淘宝登录程序执行失败或被用户强行关闭。".to_string()
        } else {
            format!("淘宝登录程序执行失败: {}", stderr_str)
        };
        return Err(AppError::Validation(detail));
    }

    Ok(())
}

fn collection_task_select_sql(where_clause: &str) -> String {
    format!(
        "SELECT id, title, source_url, category_path, target_shop_ids, status, error_summary, collected_data,
                COALESCE(review_status, 'pending'), review_summary, reviewed_data, review_result_json, reviewed_at,
                COALESCE(published_shop_ids, '[]'), COALESCE(publish_job_ids, '[]'), published_at,
                created_at, updated_at
         FROM collection_tasks {where_clause}"
    )
}

fn parse_json_string_list(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn map_collection_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionTaskView> {
    let target_shop_ids_json: String = row.get(4)?;
    let published_shop_ids_json: String = row.get(13)?;
    let publish_job_ids_json: String = row.get(14)?;
    Ok(CollectionTaskView {
        id: row.get(0)?,
        title: row.get(1)?,
        source_url: row.get(2)?,
        category_path: row.get(3)?,
        target_shop_ids: parse_json_string_list(&target_shop_ids_json),
        status: row.get(5)?,
        error_summary: row.get(6)?,
        collected_data: row.get(7)?,
        review_status: row.get(8)?,
        review_summary: row.get(9)?,
        reviewed_data: row.get(10)?,
        review_result_json: row.get(11)?,
        reviewed_at: row.get(12)?,
        published_shop_ids: parse_json_string_list(&published_shop_ids_json),
        publish_job_ids: parse_json_string_list(&publish_job_ids_json),
        published_at: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

// 3. 获取采集任务列表
#[tauri::command]
pub fn get_collection_tasks(app: AppHandle) -> AppResult<Vec<CollectionTaskView>> {
    let conn = open_connection(&app)?;
    let sql = collection_task_select_sql("ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;

    let list = stmt
        .query_map([], map_collection_task)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(list)
}

// 4. 重试采集任务（直接执行单个任务，立即返回结果，不走批量队列）
#[tauri::command]
pub async fn retry_collection_task(app: AppHandle, task_id: String) -> AppResult<()> {
    let conn = open_connection(&app)?;
    let task = {
        let sql = collection_task_select_sql("WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![task_id])?;
        match rows.next()? {
            Some(row) => map_collection_task(row)?,
            None => return Err(AppError::Validation(format!("采集任务 {} 不存在", task_id))),
        }
    };

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        match run_single_collection_task(&app_clone, &task).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("重试采集任务 {} 失败: {:?}", task.id, e);
            }
        }
    });

    Ok(())
}

// 5. 清理采集任务
#[tauri::command]
pub fn clear_collection_tasks(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    conn.execute("DELETE FROM collection_tasks", [])?;
    Ok(())
}

#[tauri::command]
pub fn resume_collection_tasks(app: AppHandle) -> AppResult<i64> {
    let active_count = get_all_pending_tasks(&app)?.len() as i64;
    if active_count > 0 {
        trigger_collection_worker(app);
    }
    Ok(active_count)
}

#[tauri::command]
pub async fn run_collection_review_once(
    app: AppHandle,
    request: CollectionReviewRunRequest,
) -> AppResult<CollectionReviewBatchResult> {
    let task_ids = unique_non_empty_strings(request.task_ids);
    let target_shop_ids = unique_non_empty_strings(request.target_shop_ids);
    let limit = request.limit.unwrap_or(50).clamp(1, 200);
    let tasks = load_collection_review_tasks(&app, &task_ids, limit)?;
    let ai_config = load_optional_ai_provider_config(&app)?;

    let mut result = CollectionReviewBatchResult {
        processed_items: 0,
        passed_items: 0,
        needs_review_items: 0,
        blocked_items: 0,
        failed_items: 0,
    };

    for task in tasks {
        result.processed_items += 1;
        let review =
            match review_collection_task(&app, &task, ai_config.as_ref(), &target_shop_ids).await {
                Ok(review) => review,
                Err(error) => {
                    persist_collection_review_failure(&app, &task.id, &error.to_string())?;
                    result.failed_items += 1;
                    continue;
                }
            };
        match review.status.as_str() {
            "passed" => result.passed_items += 1,
            "needs_review" => result.needs_review_items += 1,
            "blocked" => result.blocked_items += 1,
            _ => result.failed_items += 1,
        }
        persist_collection_review_result(&app, &task.id, &review)?;
    }

    Ok(result)
}

#[tauri::command]
pub fn confirm_collection_review(
    app: AppHandle,
    request: CollectionReviewConfirmRequest,
) -> AppResult<CollectionTaskView> {
    let conn = open_connection(&app)?;
    let task = load_collection_task_by_id(&conn, request.task_id.trim())?;
    let raw = task
        .reviewed_data
        .as_deref()
        .or(task.collected_data.as_deref())
        .unwrap_or_default()
        .trim();
    if raw.is_empty() {
        return Err(AppError::Validation(
            "采集任务缺少可确认的商品数据".to_string(),
        ));
    }
    let mut product = serde_json::from_str::<ExternalProductInput>(raw)
        .map_err(|error| AppError::Validation(format!("采集审查结果无法解析：{error}")))?;
    if let Some(title) = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        product.title = title.to_string();
    }
    if let Some(category_ids) = request.category_ids.as_ref().filter(|ids| ids.len() >= 3) {
        if !product.metadata.is_object() {
            product.metadata = Value::Object(serde_json::Map::new());
        }
        if let Some(metadata) = product.metadata.as_object_mut() {
            metadata.insert(
                "wechat_category_ids".to_string(),
                Value::Array(
                    category_ids
                        .iter()
                        .map(|cat_id| Value::Number(serde_json::Number::from(*cat_id)))
                        .collect(),
                ),
            );
            metadata.insert(
                "wechat_category_infer_source".to_string(),
                Value::String("manual_review_confirm".to_string()),
            );
            metadata.insert(
                "wechat_category_infer_at".to_string(),
                Value::String(now_shanghai()),
            );
        }
    }
    if let Some(path) = request
        .category_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        product.category_hint = Some(path.to_string());
    }
    validate_reviewed_product_for_publish(&product)?;

    let reviewed_data = serde_json::to_string(&product)
        .map_err(|error| AppError::Validation(format!("审查确认结果无法序列化：{error}")))?;
    let now = now_shanghai();
    let result_json = merge_review_confirmation_json(
        task.review_result_json.as_deref(),
        request.category_ids,
        request.category_path,
    )?;
    conn.execute(
        "UPDATE collection_tasks
         SET review_status = 'passed',
             review_summary = ?1,
             reviewed_data = ?2,
             review_result_json = ?3,
             reviewed_at = ?4,
             updated_at = ?4
         WHERE id = ?5",
        params![
            "人工确认采集审查通过",
            reviewed_data,
            result_json,
            now,
            task.id
        ],
    )?;
    load_collection_task_by_id(&conn, &task.id)
}

#[tauri::command]
pub fn reset_collection_review(app: AppHandle, task_id: String) -> AppResult<CollectionTaskView> {
    let conn = open_connection(&app)?;
    let task = load_collection_task_by_id(&conn, task_id.trim())?;
    let now = now_shanghai();
    conn.execute(
        "UPDATE collection_tasks
         SET review_status = 'pending',
             review_summary = NULL,
             reviewed_data = NULL,
             review_result_json = NULL,
             reviewed_at = NULL,
             updated_at = ?1
         WHERE id = ?2",
        params![now, task.id],
    )?;
    load_collection_task_by_id(&conn, &task.id)
}

#[tauri::command]
pub fn create_publish_job_from_collection_tasks(
    app: AppHandle,
    request: CollectionPublishRequest,
) -> AppResult<PublishJobCreated> {
    let collection_task_ids = unique_non_empty_strings(request.collection_task_ids);
    let target_shop_ids = unique_non_empty_strings(request.target_shop_ids);
    if collection_task_ids.is_empty() {
        return Err(AppError::Validation("请选择至少一个采集结果".to_string()));
    }
    if target_shop_ids.is_empty() {
        return Err(AppError::Validation(
            "请选择至少一个目标微信小店".to_string(),
        ));
    }

    let conn = open_connection(&app)?;
    validate_collection_target_shops(&conn, &target_shop_ids)?;
    let pricing_strategy = match request.pricing_strategy {
        Some(strategy) => {
            validate_publish_pricing_strategy(&strategy)?;
            strategy
        }
        None => load_publish_pricing_strategy(&conn)?,
    };

    let mut products = Vec::new();
    let mut external_product_ids = BTreeSet::new();
    for task_id in &collection_task_ids {
        let task = load_collection_task_by_id(&conn, task_id)?;
        if task.status != "success" {
            return Err(AppError::Validation(format!(
                "采集任务 {} 还未成功，不能创建铺货任务",
                task.title
            )));
        }
        if task.review_status != "passed" {
            return Err(AppError::Validation(format!(
                "采集任务 {} 尚未通过审查，不能创建铺货任务",
                task.title
            )));
        }
        let raw = task
            .reviewed_data
            .as_deref()
            .or(task.collected_data.as_deref())
            .unwrap_or_default()
            .trim();
        if raw.is_empty() {
            return Err(AppError::Validation(format!(
                "采集任务 {} 缺少采集结果，不能创建铺货任务",
                task.title
            )));
        }
        let mut product = serde_json::from_str::<ExternalProductInput>(raw).map_err(|error| {
            AppError::Validation(format!(
                "采集任务 {} 的商品数据无法解析：{error}",
                task.title
            ))
        })?;
        if !external_product_ids.insert(product.external_product_id.clone()) {
            return Err(AppError::Validation(format!(
                "本次选择里存在重复 external_product_id：{}",
                product.external_product_id
            )));
        }
        apply_publish_pricing_strategy(&mut product, &pricing_strategy);
        validate_collection_publish_duplicates(
            &conn,
            &product.external_product_id,
            &target_shop_ids,
        )?;
        products.push(product);
    }

    let publish_request = ExternalPublishJobRequest {
        request_id: format!("pub-col-{}", Uuid::new_v4()),
        target_shop_group_ids: Vec::new(),
        target_shop_ids: target_shop_ids.clone(),
        products,
    };
    let created = create_external_publish_job(app.clone(), publish_request)?;
    mark_collection_tasks_published(
        &app,
        &collection_task_ids,
        &target_shop_ids,
        &created.task_id,
    )?;
    Ok(created)
}

fn unique_non_empty_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if !value.is_empty() && seen.insert(value.clone()) {
            result.push(value);
        }
    }
    result
}

fn load_collection_task_by_id(
    conn: &rusqlite::Connection,
    task_id: &str,
) -> AppResult<CollectionTaskView> {
    let sql = collection_task_select_sql("WHERE id = ?1");
    conn.query_row(&sql, [task_id], map_collection_task)
        .optional()?
        .ok_or_else(|| AppError::Validation(format!("采集任务 {} 不存在", task_id)))
}

fn validate_collection_target_shops(
    conn: &rusqlite::Connection,
    target_shop_ids: &[String],
) -> AppResult<()> {
    let targets = resolve_target_shops_for_scope(conn, &[], target_shop_ids)?;
    if targets.len() != target_shop_ids.len() {
        return Err(AppError::Validation(
            "目标微信小店不存在或已被删除，请刷新店铺列表后重试".to_string(),
        ));
    }
    Ok(())
}

fn validate_collection_publish_duplicates(
    conn: &rusqlite::Connection,
    external_product_id: &str,
    target_shop_ids: &[String],
) -> AppResult<()> {
    for shop_id in target_shop_ids {
        let existing_shop_product = conn
            .query_row(
                "SELECT id FROM shop_products WHERE shop_id = ?1 AND external_product_id = ?2",
                params![shop_id, external_product_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if existing_shop_product.is_some() {
            return Err(AppError::Validation(format!(
                "外部商品 {} 已经在店铺 {} 形成店铺商品，不能重复创建",
                external_product_id, shop_id
            )));
        }

        let existing_publish_item = conn
            .query_row(
                "SELECT i.id
                 FROM publish_job_items i
                 JOIN publish_products p ON p.id = i.product_row_id
                 WHERE i.shop_id = ?1 AND p.external_product_id = ?2
                 LIMIT 1",
                params![shop_id, external_product_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if existing_publish_item.is_some() {
            return Err(AppError::Validation(format!(
                "外部商品 {} 已经给店铺 {} 创建过铺货任务，不能重复创建",
                external_product_id, shop_id
            )));
        }
    }
    Ok(())
}

fn mark_collection_tasks_published(
    app: &AppHandle,
    collection_task_ids: &[String],
    target_shop_ids: &[String],
    publish_job_id: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    for task_id in collection_task_ids {
        let task = load_collection_task_by_id(&conn, task_id)?;
        let mut published_shop_ids = task.published_shop_ids;
        for shop_id in target_shop_ids {
            if !published_shop_ids
                .iter()
                .any(|existing| existing == shop_id)
            {
                published_shop_ids.push(shop_id.clone());
            }
        }
        let mut publish_job_ids = task.publish_job_ids;
        if !publish_job_ids
            .iter()
            .any(|existing| existing == publish_job_id)
        {
            publish_job_ids.push(publish_job_id.to_string());
        }
        conn.execute(
            "UPDATE collection_tasks
             SET published_shop_ids = ?1, publish_job_ids = ?2, published_at = ?3, updated_at = ?3
             WHERE id = ?4",
            params![
                serde_json::to_string(&published_shop_ids).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&publish_job_ids).unwrap_or_else(|_| "[]".to_string()),
                now,
                task_id,
            ],
        )?;
    }
    Ok(())
}

struct CollectionReviewOutcome {
    status: String,
    summary: String,
    reviewed_product: ExternalProductInput,
    result_json: Value,
}

fn load_collection_review_tasks(
    app: &AppHandle,
    task_ids: &[String],
    limit: i64,
) -> AppResult<Vec<CollectionTaskView>> {
    let conn = open_connection(app)?;
    if task_ids.is_empty() {
        let sql = collection_task_select_sql(
            "WHERE status = 'success'
               AND collected_data IS NOT NULL
               AND TRIM(collected_data) <> ''
               AND COALESCE(review_status, 'pending') <> 'passed'
             ORDER BY updated_at DESC
             LIMIT ?1",
        );
        let mut stmt = conn.prepare(&sql)?;
        return Ok(stmt
            .query_map([limit], map_collection_task)?
            .collect::<Result<Vec<_>, _>>()?);
    }

    let mut tasks = Vec::new();
    for task_id in task_ids.iter().take(limit as usize) {
        tasks.push(load_collection_task_by_id(&conn, task_id)?);
    }
    Ok(tasks)
}

async fn review_collection_task(
    app: &AppHandle,
    task: &CollectionTaskView,
    ai_config: Option<&AiProviderConfig>,
    target_shop_ids: &[String],
) -> AppResult<CollectionReviewOutcome> {
    if task.status != "success" {
        return Err(AppError::Validation("只允许审查采集成功的商品".to_string()));
    }
    let raw = task.collected_data.as_deref().unwrap_or_default().trim();
    if raw.is_empty() {
        return Err(AppError::Validation("采集结果为空".to_string()));
    }

    let mut product = serde_json::from_str::<ExternalProductInput>(raw)
        .map_err(|error| AppError::Validation(format!("采集结果无法解析：{error}")))?;
    let original_title = product.title.clone();
    let original_images = product.images.clone();
    let original_detail_images = product.detail_images.clone();
    let mut issues = Vec::<Value>::new();
    let mut removed_images = Vec::<Value>::new();

    let title_review = clean_collection_title(&product);
    product.title = title_review.cleaned_title.clone();
    let title_removed_terms = title_review.removed_terms;
    if product.title.chars().count() < 4 {
        issues.push(review_issue(
            "title",
            "block",
            "清洗品牌/店铺词后标题过短，需要人工重写标题",
        ));
    }

    product.images = filter_collection_images(&original_images, "main", &mut removed_images);
    product.detail_images =
        filter_collection_images(&original_detail_images, "detail", &mut removed_images);

    let category_shop_id = {
        let conn = open_connection(app)?;
        resolve_collection_review_category_shop(&conn, target_shop_ids)?
    };
    let mut category_candidates = Vec::<CollectionReviewCategoryCandidate>::new();
    if let Some(shop_id) = category_shop_id.as_deref() {
        let conn = open_connection(app)?;
        category_candidates =
            suggest_wechat_category_candidates_from_cache(&conn, shop_id, &product, 5)?;
    }
    let mut category_applied = false;

    let mut ai_used = false;
    let mut ai_warning = None::<String>;
    let agent_started = std::time::Instant::now();
    let agent_input_snapshot = serde_json::json!({
        "task_id": &task.id,
        "title": &product.title,
        "original_title": &original_title,
        "source_url": &product.source_url,
        "main_image_count": product.images.len(),
        "detail_image_count": product.detail_images.len(),
        "sku_count": product.skus.len(),
        "category_candidates": &category_candidates
    });
    let agent_input_summary = format!(
        "采集审查 task_id={} 主图={} 详情图={} 类目候选={}",
        task.id,
        product.images.len(),
        product.detail_images.len(),
        category_candidates.len()
    );
    let agent_run_id = {
        let conn = open_connection(app)?;
        create_agent_run_for_skill(
            &conn,
            &PRODUCT_REVIEW_SKILL,
            "collection_review",
            "collection_task",
            &task.id,
            category_shop_id.as_deref(),
            ai_config,
            &agent_input_summary,
            Some(&agent_input_snapshot),
        )?
    };
    let mut ai_result_for_run = None::<Value>;
    let mut agent_error_code_for_run = None::<String>;
    let mut agent_error_summary_for_run = None::<String>;
    let review_skill_enabled = {
        let conn = open_connection(app)?;
        load_agent_skill_enabled(&conn, &PRODUCT_REVIEW_SKILL)?
    };
    if !review_skill_enabled {
        agent_error_code_for_run = Some("SKILL_DISABLED".to_string());
        agent_error_summary_for_run = Some("采集审查 Agent 技能已停用，仅执行本地规则".to_string());
        issues.push(review_issue(
            "ai",
            "confirm",
            "采集审查 Agent 技能已停用，仅执行本地规则，需要人工确认",
        ));
    } else if let Some(config) = ai_config {
        ai_used = true;
        let ai_images = review_ai_image_urls(&product);
        let ai_input = serde_json::json!({
            "product": {
                "title": &product.title,
                "original_title": &original_title,
                "supplier_name": &product.supplier_name,
                "brand_hint": &product.brand_hint,
                "category_hint": &product.category_hint,
                "source_url": &product.source_url,
                "main_images": product.images.iter().take(8).collect::<Vec<_>>(),
                "detail_images": product.detail_images.iter().take(6).collect::<Vec<_>>(),
                "sku_count": product.skus.len()
            },
            "category_candidates": &category_candidates,
            "review_rules": [
                "标题不能包含品牌名、供应商店名、淘宝/天猫等来源平台词",
                "主图/详情图不能包含淘宝标识、店铺招牌、二维码、联系方式、明显水印或促销贴片",
                "微信类目只能从 category_candidates 中选择，不能编造类目 ID",
                "只在高置信时建议移除图片；不确定时返回低 confidence"
            ]
        });
        match request_ai_collection_review_json(
            app,
            config,
            "审查无货源商品采集结果，按 wx-xd-product-review 技能输出 JSON。",
            &ai_input,
            &ai_images,
        )
        .await
        {
            Ok(ai_result) => {
                ai_result_for_run = Some(ai_result.clone());
                apply_ai_collection_review(
                    &mut product,
                    &ai_result,
                    &mut removed_images,
                    &mut issues,
                );
                category_applied = apply_ai_review_category(
                    &mut product,
                    &ai_result,
                    &category_candidates,
                    &mut issues,
                );
            }
            Err(error) => {
                let error_summary = error.to_string();
                agent_error_code_for_run = Some(agent_error_code(&error).to_string());
                agent_error_summary_for_run = Some(error_summary.clone());
                ai_warning = Some(error_summary);
                issues.push(review_issue(
                    "image",
                    "confirm",
                    "AI 图文审查未完成，需要人工确认图片是否可用于微信铺货",
                ));
            }
        }
    } else {
        agent_error_code_for_run = Some("PROVIDER_NOT_CONFIGURED".to_string());
        agent_error_summary_for_run = Some("未启用 AI provider，图片内容仅做规则过滤".to_string());
        issues.push(review_issue(
            "image",
            "confirm",
            "未启用 AI provider，图片内容仅做规则过滤，需要人工确认",
        ));
    }

    if product.images.len() < 3 {
        issues.push(review_issue(
            "image",
            "block",
            "剔除不可用图片后主图少于 3 张，需要换图或重新采集",
        ));
    }
    if product.detail_images.is_empty() {
        issues.push(review_issue(
            "image",
            "block",
            "剔除不可用图片后详情图为空，需要换图或重新采集",
        ));
    }

    if !category_applied {
        if let Some(best) = category_candidates.first() {
            let ambiguous = category_candidates
                .get(1)
                .map(|second| best.score - second.score < 18 && best.score < 180)
                .unwrap_or(false);
            if best.score >= 90 && !ambiguous {
                apply_review_category(&mut product, &best.category_ids, &best.category_path);
                category_applied = true;
            }
        }
    }
    if !category_applied && category_candidates.is_empty() {
        issues.push(review_issue(
            "category",
            "confirm",
            "未匹配到可用微信类目，需要人工补充类目",
        ));
    } else if !category_applied {
        issues.push(review_issue(
            "category",
            "confirm",
            "类目匹配置信度不足，需要从候选类目中确认",
        ));
    }

    if !product.metadata.is_object() {
        product.metadata = Value::Object(serde_json::Map::new());
    }
    if let Some(metadata) = product.metadata.as_object_mut() {
        metadata.insert(
            "collection_review".to_string(),
            serde_json::json!({
                "original_title": original_title.clone(),
                "reviewed_title": product.title.clone(),
                "removed_title_terms": title_removed_terms.clone(),
                "removed_images": removed_images.clone(),
                "category_applied": category_applied,
                "ai_used": ai_used,
                "skill": {
                    "name": PRODUCT_REVIEW_SKILL.name,
                    "version": PRODUCT_REVIEW_SKILL.version
                },
                "ai_warning": ai_warning.clone(),
                "reviewed_at": now_shanghai()
            }),
        );
    }

    let has_block = issues
        .iter()
        .any(|issue| issue.get("severity").and_then(Value::as_str) == Some("block"));
    let has_confirm = issues
        .iter()
        .any(|issue| issue.get("severity").and_then(Value::as_str) == Some("confirm"));
    let status = if has_block {
        "blocked"
    } else if has_confirm {
        "needs_review"
    } else {
        "passed"
    }
    .to_string();
    let summary = collection_review_summary(&status, &issues, &removed_images, category_applied);
    let result_json = serde_json::json!({
        "status": &status,
        "summary": &summary,
        "issues": issues,
        "title": {
            "original": original_title,
            "reviewed": product.title.clone(),
            "removed_terms": title_removed_terms
        },
        "images": {
            "original_main_count": original_images.len(),
            "reviewed_main_count": product.images.len(),
            "original_detail_count": original_detail_images.len(),
            "reviewed_detail_count": product.detail_images.len(),
            "removed": removed_images
        },
        "category": {
            "applied": category_applied,
            "candidates": category_candidates
        },
        "ai": {
            "used": ai_used,
            "skill": {
                "name": PRODUCT_REVIEW_SKILL.name,
                "version": PRODUCT_REVIEW_SKILL.version
            },
            "warning": ai_warning.clone()
        },
        "reviewed_at": now_shanghai()
    });

    {
        let conn = open_connection(app)?;
        let run_status = if agent_error_code_for_run.is_some() {
            if ai_used {
                "failed"
            } else {
                "blocked"
            }
        } else {
            agent_status_for_business_decision(&status)
        };
        finish_agent_run(
            &conn,
            &agent_run_id,
            AgentRunFinish {
                status: run_status,
                output: ai_result_for_run.as_ref(),
                validated_output: Some(&result_json),
                tool_calls: None,
                decision: Some(&status),
                error_code: agent_error_code_for_run.as_deref(),
                error_summary: agent_error_summary_for_run.as_deref(),
                duration_ms: Some(agent_started.elapsed().as_millis().min(i64::MAX as u128) as i64),
            },
        )?;
    }

    Ok(CollectionReviewOutcome {
        status,
        summary,
        reviewed_product: product,
        result_json,
    })
}

fn persist_collection_review_result(
    app: &AppHandle,
    task_id: &str,
    review: &CollectionReviewOutcome,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    let reviewed_data = serde_json::to_string(&review.reviewed_product)
        .map_err(|error| AppError::Validation(format!("审查结果无法序列化：{error}")))?;
    let review_result_json = serde_json::to_string(&review.result_json)
        .map_err(|error| AppError::Validation(format!("审查详情无法序列化：{error}")))?;
    conn.execute(
        "UPDATE collection_tasks
         SET review_status = ?1,
             review_summary = ?2,
             reviewed_data = ?3,
             review_result_json = ?4,
             reviewed_at = ?5,
             updated_at = ?5
         WHERE id = ?6",
        params![
            review.status,
            review.summary,
            reviewed_data,
            review_result_json,
            now,
            task_id
        ],
    )?;
    Ok(())
}

fn persist_collection_review_failure(app: &AppHandle, task_id: &str, error: &str) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    let summary = truncate_for_summary(error, 240);
    let result_json = serde_json::json!({
        "status": "failed",
        "summary": summary,
        "issues": [review_issue("system", "block", "采集审查执行失败")],
        "error": error,
        "reviewed_at": now
    });
    conn.execute(
        "UPDATE collection_tasks
         SET review_status = 'failed',
             review_summary = ?1,
             review_result_json = ?2,
             reviewed_at = ?3,
             updated_at = ?3
         WHERE id = ?4",
        params![summary, result_json.to_string(), now, task_id],
    )?;
    Ok(())
}

struct TitleReview {
    cleaned_title: String,
    removed_terms: Vec<String>,
}

fn clean_collection_title(product: &ExternalProductInput) -> TitleReview {
    let mut title = product.title.trim().to_string();
    let mut terms = vec![
        "淘宝".to_string(),
        "天猫".to_string(),
        "tmall".to_string(),
        "taobao".to_string(),
        "官方旗舰店".to_string(),
        "旗舰店".to_string(),
        "专卖店".to_string(),
        "专营店".to_string(),
        "店铺".to_string(),
    ];
    if let Some(value) = product.supplier_name.as_deref() {
        terms.push(value.to_string());
    }
    if let Some(value) = product.brand_hint.as_deref() {
        terms.push(value.to_string());
    }
    collect_metadata_brand_terms(&product.metadata, &mut terms);

    let mut removed = Vec::new();
    for term in terms {
        let term = term.trim();
        if term.chars().count() < 2 {
            continue;
        }
        if title.contains(term) {
            title = title.replace(term, "");
            removed.push(term.to_string());
        }
    }
    title = normalize_review_title(&title);
    TitleReview {
        cleaned_title: title,
        removed_terms: removed,
    }
}

fn collect_metadata_brand_terms(metadata: &Value, terms: &mut Vec<String>) {
    let Some(object) = metadata.as_object() else {
        return;
    };
    for key in [
        "brand",
        "brand_name",
        "品牌",
        "店铺",
        "shop_name",
        "seller_name",
    ] {
        if let Some(value) = object
            .get(key)
            .and_then(|value| json_value_to_string(Some(value)))
        {
            terms.push(value);
        }
    }
    if let Some(params) = object.get("taobao_item_params").and_then(Value::as_object) {
        for (key, value) in params {
            if key.contains("品牌") || key.to_ascii_lowercase().contains("brand") {
                if let Some(value) = json_value_to_string(Some(value)) {
                    terms.push(value);
                }
            }
        }
    }
}

fn normalize_review_title(title: &str) -> String {
    let normalized = title
        .chars()
        .map(|ch| {
            if matches!(ch, '【' | '】' | '[' | ']' | '（' | '）' | '(' | ')') {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>();
    let mut result = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in ["  ", "--", "__"] {
        while result.contains(needle) {
            result = result.replace(needle, " ");
        }
    }
    result
        .trim_matches(|ch: char| ch.is_ascii_punctuation() || matches!(ch, '，' | '。' | '、'))
        .trim()
        .to_string()
}

fn filter_collection_images(
    images: &[String],
    kind: &str,
    removed: &mut Vec<Value>,
) -> Vec<String> {
    let mut kept = Vec::new();
    let mut seen = BTreeSet::new();
    for image in images {
        let url = image.trim();
        if url.is_empty() || !seen.insert(url.to_string()) {
            continue;
        }
        if let Some(reason) = rule_image_reject_reason(url) {
            removed.push(serde_json::json!({
                "kind": kind,
                "url": url,
                "reason": reason,
                "source": "rule"
            }));
            continue;
        }
        kept.push(url.to_string());
    }
    kept
}

fn rule_image_reject_reason(url: &str) -> Option<&'static str> {
    let lower = url.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Some("图片链接不是 HTTP/HTTPS 地址");
    }
    for needle in [
        "logo",
        "qrcode",
        "qr_code",
        "wangwang",
        "shop-sign",
        "store-sign",
        "banner",
        "taobao-logo",
        "tmall-logo",
        "avatar",
        "userheaderimgshow",
        "headimg",
        "/shophead/",
        "seller_logo",
        "sellerlogo",
        "shop_logo",
        "shoplogo",
        "wwc.alicdn.com",
    ] {
        if lower.contains(needle) {
            return Some("疑似平台标识、店招、头像或营销图");
        }
    }
    None
}

fn review_ai_image_urls(product: &ExternalProductInput) -> Vec<String> {
    product
        .images
        .iter()
        .take(8)
        .chain(product.detail_images.iter().take(4))
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        .cloned()
        .collect()
}

fn apply_ai_collection_review(
    product: &mut ExternalProductInput,
    ai_result: &Value,
    removed_images: &mut Vec<Value>,
    issues: &mut Vec<Value>,
) {
    let confidence = json_value_to_i64(ai_result.get("confidence")).unwrap_or(0);
    if confidence >= 75 {
        if let Some(title) = json_value_to_string(ai_result.get("title")) {
            let title = normalize_review_title(&title);
            if title.chars().count() >= 4 {
                product.title = title;
            }
        }
    } else {
        issues.push(review_issue(
            "title",
            "confirm",
            "AI 对标题清洗置信度不足，需要人工确认",
        ));
    }

    let remove_items = ai_result
        .get("remove_image_urls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut remove_urls = BTreeSet::new();
    for item in remove_items {
        let url = item
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if url.is_empty() {
            continue;
        }
        let item_confidence = json_value_to_i64(item.get("confidence")).unwrap_or(confidence);
        if item_confidence < 80 {
            continue;
        }
        let reason = item
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("AI 判定图片不适合微信铺货");
        remove_urls.insert(url.to_string());
        removed_images.push(serde_json::json!({
            "kind": "image",
            "url": url,
            "reason": reason,
            "confidence": item_confidence,
            "source": "ai"
        }));
    }
    if !remove_urls.is_empty() {
        product.images.retain(|url| !remove_urls.contains(url));
        product
            .detail_images
            .retain(|url| !remove_urls.contains(url));
    }

    if ai_result
        .get("needs_human_review")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        issues.push(review_issue(
            "ai",
            "confirm",
            "AI skill 标记该采集结果需要人工复核",
        ));
    }
}

fn apply_ai_review_category(
    product: &mut ExternalProductInput,
    ai_result: &Value,
    candidates: &[CollectionReviewCategoryCandidate],
    issues: &mut Vec<Value>,
) -> bool {
    let Some(category) = ai_result.get("category").filter(|value| value.is_object()) else {
        return false;
    };
    let confidence = json_value_to_i64(category.get("confidence")).unwrap_or(0);
    if confidence < 75 {
        return false;
    }
    let category_ids = category
        .get("category_ids")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_i64).collect::<Vec<_>>())
        .unwrap_or_default();
    if category_ids.len() < 3 {
        return false;
    }
    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.category_ids == category_ids)
    {
        apply_review_category(product, &candidate.category_ids, &candidate.category_path);
        return true;
    }
    issues.push(review_issue(
        "category",
        "confirm",
        "AI skill 返回的微信类目不在本地候选中，需要人工确认",
    ));
    false
}

fn apply_review_category(
    product: &mut ExternalProductInput,
    category_ids: &[i64],
    category_path: &str,
) {
    if !product.metadata.is_object() {
        product.metadata = Value::Object(serde_json::Map::new());
    }
    if let Some(metadata) = product.metadata.as_object_mut() {
        metadata.insert(
            "wechat_category_ids".to_string(),
            Value::Array(
                category_ids
                    .iter()
                    .map(|cat_id| Value::Number(serde_json::Number::from(*cat_id)))
                    .collect(),
            ),
        );
        metadata.insert(
            "wechat_category_infer_source".to_string(),
            Value::String("collection_review_agent".to_string()),
        );
        metadata.insert(
            "wechat_category_infer_at".to_string(),
            Value::String(now_shanghai()),
        );
    }
    product.category_hint = Some(category_path.to_string());
}

fn resolve_collection_review_category_shop(
    conn: &rusqlite::Connection,
    target_shop_ids: &[String],
) -> AppResult<Option<String>> {
    for shop_id in target_shop_ids {
        let count = conn.query_row(
            "SELECT COUNT(*) FROM wechat_categories WHERE shop_id = ?1",
            [shop_id],
            |row| row.get::<_, i64>(0),
        )?;
        if count > 0 {
            return Ok(Some(shop_id.clone()));
        }
    }
    let shop_id = conn
        .query_row(
            "SELECT shop_id
         FROM wechat_categories
         GROUP BY shop_id
         ORDER BY COUNT(*) DESC, shop_id ASC
         LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(shop_id)
}

fn review_issue(kind: &str, severity: &str, message: &str) -> Value {
    serde_json::json!({
        "kind": kind,
        "severity": severity,
        "message": message
    })
}

fn collection_review_summary(
    status: &str,
    issues: &[Value],
    removed_images: &[Value],
    category_applied: bool,
) -> String {
    let first_issue = issues
        .iter()
        .filter_map(|issue| issue.get("message").and_then(Value::as_str))
        .next()
        .unwrap_or_default();
    match status {
        "passed" => {
            let image_text = if removed_images.is_empty() {
                "未剔除图片".to_string()
            } else {
                format!("剔除 {} 张图片", removed_images.len())
            };
            let category_text = if category_applied {
                "已匹配微信类目"
            } else {
                "类目沿用原数据"
            };
            format!("审查通过，{image_text}，{category_text}")
        }
        "needs_review" => format!("需要人工确认：{first_issue}"),
        "blocked" => format!("已拦截：{first_issue}"),
        _ => "审查失败".to_string(),
    }
}

fn validate_reviewed_product_for_publish(product: &ExternalProductInput) -> AppResult<()> {
    if product.title.trim().chars().count() < 4 {
        return Err(AppError::Validation(
            "标题过短，不能确认通过审查".to_string(),
        ));
    }
    if product.images.len() < 3 {
        return Err(AppError::Validation(
            "主图少于 3 张，不能确认通过审查".to_string(),
        ));
    }
    if product.detail_images.is_empty() {
        return Err(AppError::Validation(
            "详情图为空，不能确认通过审查".to_string(),
        ));
    }
    if !product_has_wechat_category_metadata(product)? {
        return Err(AppError::Validation(
            "缺少微信类目，不能确认通过审查".to_string(),
        ));
    }
    Ok(())
}

fn merge_review_confirmation_json(
    raw_result: Option<&str>,
    category_ids: Option<Vec<i64>>,
    category_path: Option<String>,
) -> AppResult<String> {
    let mut value = raw_result
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let object = value
        .as_object_mut()
        .ok_or_else(|| AppError::Validation("审查详情不是 JSON 对象".to_string()))?;
    object.insert("status".to_string(), Value::String("passed".to_string()));
    object.insert(
        "summary".to_string(),
        Value::String("人工确认采集审查通过".to_string()),
    );
    object.insert("confirmed_at".to_string(), Value::String(now_shanghai()));
    if let Some(category_ids) = category_ids {
        object.insert(
            "confirmed_category_ids".to_string(),
            Value::Array(
                category_ids
                    .iter()
                    .map(|cat_id| Value::Number(serde_json::Number::from(*cat_id)))
                    .collect(),
            ),
        );
    }
    if let Some(category_path) = category_path {
        object.insert(
            "confirmed_category_path".to_string(),
            Value::String(category_path),
        );
    }
    serde_json::to_string(&value)
        .map_err(|error| AppError::Validation(format!("确认结果无法序列化：{error}")))
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
        p = PathBuf::from("/Users/wangjunhao/Code/project/wx-xd/scripts/taobao_collector.py");
    }
    p
}

fn resolve_python_binary() -> PathBuf {
    if let Ok(path) = std::env::var("WX_XD_PYTHON") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return candidate;
        }
    }
    for candidate in [
        PathBuf::from(".venv/bin/python"),
        PathBuf::from("/Users/wangjunhao/Code/project/wx-xd/.venv/bin/python"),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }
    let candidates = [
        "/opt/homebrew/opt/python@3.12/libexec/bin/python3",
        "/opt/homebrew/bin/python3",
        "/usr/local/bin/python3",
    ];
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return path;
        }
    }
    PathBuf::from("python3")
}

async fn run_collector_profile_json_command(
    app: &AppHandle,
    label: &str,
    subcommand: &str,
    extra_args: Vec<String>,
) -> AppResult<serde_json::Value> {
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    let script_path = resolve_collector_script(app);

    let mut cmd = tokio::process::Command::new(resolve_python_binary());
    cmd.arg(&script_path)
        .arg(subcommand)
        .arg("--profile-dir")
        .arg(&profile_dir_str);
    for arg in extra_args {
        cmd.arg(arg);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::Validation(format!("无法拉起{label}程序: {}", e)))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !stdout_str.is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
            if output.status.success() && v.get("error").is_none() {
                return Ok(v);
            }
            let err_msg = v
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("程序返回错误")
                .to_string();
            return Err(AppError::Validation(err_msg));
        }
    }

    Err(AppError::Validation(format!(
        "{label}程序返回异常: status={:?}, stderr={}",
        output.status.code(),
        stderr_str
    )))
}

// 6. 检测淘宝登录态
#[tauri::command]
pub async fn check_taobao_login_state(app: AppHandle) -> AppResult<serde_json::Value> {
    run_collector_profile_json_command(&app, "登录态检测", "check-login", Vec::new()).await
}

// 7. 查询本地淘宝访问限制冷却状态，不访问淘宝。
#[tauri::command]
pub async fn get_taobao_access_limit_state(app: AppHandle) -> AppResult<serde_json::Value> {
    run_collector_profile_json_command(
        &app,
        "淘宝访问限制状态查询",
        "access-limit-state",
        Vec::new(),
    )
    .await
}

// 8. 清除本地淘宝访问限制冷却状态，不访问淘宝。
#[tauri::command]
pub async fn clear_taobao_access_limit_state(app: AppHandle) -> AppResult<serde_json::Value> {
    run_collector_profile_json_command(
        &app,
        "淘宝访问限制状态清除",
        "clear-access-limit",
        Vec::new(),
    )
    .await
}

// 8. 手动写入本地淘宝访问限制冷却状态，不访问淘宝。
#[tauri::command]
pub async fn mark_taobao_access_limited(
    app: AppHandle,
    resume_at: Option<String>,
    reason: Option<String>,
) -> AppResult<serde_json::Value> {
    let mut args = Vec::new();
    if let Some(value) = resume_at {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            args.push("--resume-at".to_string());
            args.push(trimmed.to_string());
        }
    }
    if let Some(value) = reason {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            args.push("--reason".to_string());
            args.push(trimmed.to_string());
        }
    }
    run_collector_profile_json_command(&app, "淘宝访问限制标记", "mark-access-limited", args).await
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
        return Err(AppError::Validation("仅支持淘宝/天猫商品链接".to_string()));
    }

    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    let script_path = resolve_collector_script(&app);

    let mut cmd = tokio::process::Command::new(resolve_python_binary());
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

    tauri::async_runtime::spawn(async move {
        // 一次性取出所有待采集任务，批量处理
        let pending_tasks: Vec<CollectionTaskView> = match get_all_pending_tasks(&app) {
            Ok(tasks) => tasks,
            Err(e) => {
                eprintln!("获取待采集任务发生数据库错误: {:?}", e);
                COLLECTOR_RUNNING.store(false, Ordering::SeqCst);
                return;
            }
        };

        if pending_tasks.is_empty() {
            COLLECTOR_RUNNING.store(false, Ordering::SeqCst);
            return;
        }

        eprintln!("批量采集启动: 共 {} 个待采集任务", pending_tasks.len());
        match run_batch_collection_tasks(&app, &pending_tasks).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("批量采集出错: {:?}", e);
            }
        }

        COLLECTOR_RUNNING.store(false, Ordering::SeqCst);
    });
}

fn is_taobao_collection_protection_error(message: &str) -> bool {
    [
        "本次未打开浏览器访问淘宝",
        "淘宝账号仍处于访问限制冷却期",
        "淘宝验证码自动处理失败后处于本地保护冷却期",
        "淘宝滑块验证码自动处理失败",
        "淘宝触发人脸验证",
        "淘宝触发短信验证",
        "淘宝触发二维码/扫码验证",
        "请打开淘宝登录窗口人工完成验证后重试",
        "账号近期访问行为异常",
    ]
    .iter()
    .any(|keyword| message.contains(keyword))
}

fn get_all_pending_tasks(app: &AppHandle) -> AppResult<Vec<CollectionTaskView>> {
    let conn = open_connection(app)?;
    let sql = collection_task_select_sql(
        "WHERE status IN ('pending', 'running') ORDER BY created_at ASC",
    );
    let mut stmt = conn.prepare(&sql)?;

    let list = stmt
        .query_map([], map_collection_task)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(list)
}

async fn run_batch_collection_tasks(
    app: &AppHandle,
    tasks: &[CollectionTaskView],
) -> AppResult<()> {
    if tasks.is_empty() {
        return Ok(());
    }

    // 1. 准备 URL 列表（含 id 用于结果关联）
    let url_items: Vec<serde_json::Value> = tasks
        .iter()
        .map(|t| {
            serde_json::json!({
                "url": t.source_url,
                "id": t.id,
            })
        })
        .collect();
    let urls_json = serde_json::to_string(&url_items).unwrap_or_else(|_| "[]".to_string());

    // 3. 准备路径
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    let script_path = resolve_collector_script(app);

    // 4. 执行批量采集，并实时读取 NDJSON 输出，让页面能看到逐条进度。
    let mut child = match tokio::process::Command::new(resolve_python_binary())
        .arg(&script_path)
        .arg("batch-collect")
        .arg("--urls")
        .arg(&urls_json)
        .arg("--profile-dir")
        .arg(&profile_dir_str)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            for task in tasks {
                let _ = update_task_status(
                    app,
                    &task.id,
                    "failed",
                    Some(format!("无法拉起批量采集程序: {}", error)),
                    None,
                );
            }
            return Err(AppError::Validation(format!(
                "无法拉起批量采集程序: {}",
                error
            )));
        }
    };

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Validation("无法读取批量采集程序输出".to_string()))?;
    let stderr_task = child.stderr.take().map(|stderr| {
        tauri::async_runtime::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut text = String::new();
            let _ = reader.read_to_string(&mut text).await;
            text
        })
    });

    // 5. 解析 NDJSON 结果，逐行处理
    let task_map: std::collections::HashMap<&str, &CollectionTaskView> =
        tasks.iter().map(|t| (t.id.as_str(), t)).collect();

    let mut protection_hit = false;
    let mut handled_task_ids = BTreeSet::new();
    let mut lines = BufReader::new(stdout).lines();

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| AppError::Validation(format!("读取批量采集输出失败: {}", error)))?
    {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let result: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("批量采集结果行解析失败: {} — {}", e, line);
                continue;
            }
        };

        // 致命错误
        if result.get("fatal_error").is_some() {
            let err = result["fatal_error"].as_str().unwrap_or("未知致命错误");
            for task in tasks {
                let _ = update_task_status(
                    app,
                    &task.id,
                    "failed",
                    Some(format!("批量采集致命错误: {}", err)),
                    None,
                );
            }
            return Err(AppError::Validation(format!("批量采集失败: {}", err)));
        }

        let task_id = result.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if result.get("event").and_then(|v| v.as_str()) == Some("start") {
            let index = result
                .get("index")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            let total = result
                .get("total")
                .and_then(|value| value.as_i64())
                .unwrap_or_default();
            if !task_id.is_empty() {
                let _ = update_task_status(
                    app,
                    task_id,
                    "running",
                    Some(format!("正在采集第 {}/{} 个", index, total)),
                    None,
                );
            }
            continue;
        }

        if !task_id.is_empty() {
            handled_task_ids.insert(task_id.to_string());
        }
        let task = task_map.get(task_id);

        if result
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            // 采集成功
            if let (Some(task), Some(data)) = (task, result.get("data")) {
                let mut product_input: ExternalProductInput =
                    match serde_json::from_value(data.clone()) {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = update_task_status(
                                app,
                                task_id,
                                "failed",
                                Some(format!("解析采集结果失败: {}", e)),
                                None,
                            );
                            continue;
                        }
                    };

                product_input.title = task.title.clone();
                if !task.category_path.is_empty() {
                    product_input.category_hint = Some(task.category_path.clone());
                }

                let collected_data =
                    serde_json::to_string(&product_input).unwrap_or_else(|_| data.to_string());
                let _ = update_task_status(app, task_id, "success", None, Some(collected_data));
            } else if task.is_some() {
                let _ = update_task_status(
                    app,
                    task_id,
                    "failed",
                    Some("采集成功但结果缺少商品数据".to_string()),
                    None,
                );
            }
        } else {
            // 采集失败
            let error_msg = result
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("采集失败")
                .to_string();

            if is_taobao_collection_protection_error(&error_msg) {
                protection_hit = true;
            }

            let _ = update_task_status(app, task_id, "failed", Some(error_msg), None);

            if protection_hit {
                break;
            }
        }
    }

    let exit_status = child
        .wait()
        .await
        .map_err(|error| AppError::Validation(format!("等待批量采集程序退出失败: {}", error)))?;
    let stderr = match stderr_task {
        Some(handle) => handle.await.unwrap_or_default(),
        None => String::new(),
    };
    if !stderr.trim().is_empty() {
        eprintln!("批量采集 stderr: {}", stderr.trim());
    }

    if !exit_status.success() && !protection_hit {
        let message = if stderr.trim().is_empty() {
            format!("批量采集程序异常退出: {}", exit_status)
        } else {
            format!("批量采集程序异常退出: {}", stderr.trim())
        };
        for task in tasks {
            if !handled_task_ids.contains(&task.id) {
                let _ = update_task_status(app, &task.id, "failed", Some(message.clone()), None);
            }
        }
        return Err(AppError::Validation(message));
    }

    // 将未在输出中出现的任务标记为失败
    let unhandled_message = if protection_hit {
        "批量采集中断：触发淘宝保护，未继续采集".to_string()
    } else {
        "未收到采集结果，可能被跳过".to_string()
    };
    for task in tasks {
        if !handled_task_ids.contains(&task.id) {
            let _ = update_task_status(
                app,
                &task.id,
                "failed",
                Some(unhandled_message.clone()),
                None,
            );
        }
    }

    if protection_hit {
        eprintln!("批量采集中断：触发淘宝保护冷却");
    }

    Ok(())
}

async fn run_single_collection_task(app: &AppHandle, task: &CollectionTaskView) -> AppResult<bool> {
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
        script_path =
            PathBuf::from("/Users/wangjunhao/Code/project/wx-xd/scripts/taobao_collector.py");
    }

    // 3. 执行 Python 采集子进程
    let output = match tokio::process::Command::new(resolve_python_binary())
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
            return Ok(true);
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

        let should_continue = !is_taobao_collection_protection_error(&err_msg);
        update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
        return Ok(should_continue);
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 校验解析采集结果是否为错误 JSON
    if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&stdout_str) {
        if let Some(err_val) = err_json.get("error") {
            let err_msg = err_val.as_str().unwrap_or("采集失败").to_string();
            let should_continue = !is_taobao_collection_protection_error(&err_msg);
            update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
            return Ok(should_continue);
        }
    }

    let mut product_input: ExternalProductInput = match serde_json::from_str(&stdout_str) {
        Ok(p) => p,
        Err(e) => {
            let err_msg = format!("解析采集 JSON 结果失败: {}. 原始输出: {}", e, stdout_str);
            update_task_status(app, &task.id, "failed", Some(err_msg), None)?;
            return Ok(true);
        }
    };

    // 覆盖主标题，因为 Excel 导入指定的名称具有更高的业务置信度
    product_input.title = task.title.clone();

    // 如果类目提示字段存在，可以注入
    if !task.category_path.is_empty() {
        product_input.category_hint = Some(task.category_path.clone());
    }

    let collected_data = serde_json::to_string(&product_input).unwrap_or(stdout_str);
    update_task_status(app, &task.id, "success", None, Some(collected_data))?;

    Ok(true)
}

#[cfg(test)]
mod collection_worker_tests {
    use super::*;

    #[test]
    fn detects_taobao_protection_cooldown_errors() {
        assert!(is_taobao_collection_protection_error(
            "淘宝账号仍处于访问限制冷却期，平台提示恢复时间：2026-05-30 11:00:00+08:00。本次未打开浏览器访问淘宝。"
        ));
        assert!(is_taobao_collection_protection_error(
            "淘宝验证码自动处理失败后处于本地保护冷却期，建议恢复时间：2026-05-27 12:39:06+08:00。本次未打开浏览器访问淘宝。"
        ));
        assert!(is_taobao_collection_protection_error(
            "mock 人工验证检测到淘宝触发人脸验证，当前脚本只能自动处理滑块验证码，请打开淘宝登录窗口人工完成验证后重试。"
        ));
        assert!(!is_taobao_collection_protection_error(
            "解析采集 JSON 结果失败: expected value"
        ));
    }

    fn demo_product() -> ExternalProductInput {
        ExternalProductInput {
            external_product_id: "demo".to_string(),
            title: "江陵童话 淘宝 2026 新款儿童短袖".to_string(),
            source_url: "https://item.taobao.com/item.htm?id=1".to_string(),
            images: vec![],
            detail_images: vec![],
            skus: vec![crate::models::ExternalSkuInput {
                external_sku_id: "sku-1".to_string(),
                specs: serde_json::json!({ "颜色": "白色" }),
                cost_price: 10.0,
                stock: 10,
            }],
            supplier_name: Some("江陵童话".to_string()),
            supplier_product_id: Some("1".to_string()),
            category_hint: Some("童装/短袖".to_string()),
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn cleans_source_and_supplier_terms_from_collection_title() {
        let review = clean_collection_title(&demo_product());

        assert_eq!(review.cleaned_title, "2026 新款儿童短袖");
        assert!(review.removed_terms.contains(&"江陵童话".to_string()));
        assert!(review.removed_terms.contains(&"淘宝".to_string()));
    }

    #[test]
    fn rejects_obvious_platform_or_store_images_by_rule() {
        assert!(rule_image_reject_reason("https://example.com/taobao-logo.png").is_some());
        assert!(rule_image_reject_reason("file:///tmp/head.jpg").is_some());
        // 店铺头像 / 用户头像
        assert!(rule_image_reject_reason(
            "https://img.alicdn.com/imgextra/userheaderimgshow/xxx.jpg"
        )
        .is_some());
        assert!(rule_image_reject_reason("https://wwc.alicdn.com/avatar/xxx.png").is_some());
        assert!(rule_image_reject_reason("https://img.alicdn.com/bao/shophead/xxx.jpg").is_some());
        // 正常商品图不应被误杀
        assert!(rule_image_reject_reason("https://example.com/product-head.jpg").is_none());
        assert!(rule_image_reject_reason("https://img.alicdn.com/imgextra/abc123.jpg").is_none());
    }
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
            "UPDATE collection_tasks
             SET status = ?1,
                 error_summary = NULL,
                 collected_data = ?2,
                 review_status = 'pending',
                 review_summary = NULL,
                 reviewed_data = NULL,
                 review_result_json = NULL,
                 reviewed_at = NULL,
                 updated_at = ?3
             WHERE id = ?4",
            params![status, collected_data, now, task_id],
        )?;
    } else {
        conn.execute(
            "UPDATE collection_tasks SET status = ?1, error_summary = NULL, updated_at = ?2 WHERE id = ?3",
            params![status, now, task_id],
        )?;
    }

    Ok(())
}
