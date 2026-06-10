use super::*;

use calamine::{open_workbook, Reader, Xlsx};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

#[tauri::command]
pub fn get_publish_default_freight_templates(
    app: AppHandle,
) -> AppResult<std::collections::HashMap<String, String>> {
    let conn = open_connection(&app)?;
    load_publish_default_freight_templates(&conn)
}

#[tauri::command]
pub fn set_publish_default_freight_template(
    app: AppHandle,
    shop_id: String,
    template_id: Option<String>,
) -> AppResult<()> {
    let conn = open_connection(&app)?;
    // 指定模板时校验它在该店铺已同步列表内（清除时跳过）；防止存入无效模板 ID。
    if let Some(tid) = template_id.as_deref() {
        if !cached_freight_template_exists(&conn, &shop_id, tid)? {
            return Err(AppError::Validation(format!(
                "运费模板 {tid} 不在店铺 {shop_id} 的已同步列表中，请先在该店铺同步运费模板"
            )));
        }
    }
    save_shop_default_freight_template_to_db(&conn, &shop_id, template_id.as_deref())?;
    Ok(())
}

// 1. Excel 导入并创建采集任务
#[tauri::command]
pub fn import_excel_for_collection(
    app: AppHandle,
    file_path: String,
    target_shop_ids: Vec<String>,
) -> AppResult<i64> {
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

    // 目标店可选：不选店则只采集不铺货（采集审查后停在「待铺货」，稍后可补选店）。
    // 选了店则导入时即为「每个商品 × 每个目标店」建好流水线推进单位。
    let target_shops = if target_shop_ids.is_empty() {
        Vec::new()
    } else {
        let shops = resolve_target_shops_for_scope(&conn, &[], &target_shop_ids)?;
        if shops.len() != target_shop_ids.len() {
            return Err(AppError::Validation(
                "目标微信小店不存在或已被删除，请刷新店铺列表后重试".to_string(),
            ));
        }
        shops
    };

    let mut imported_count = 0i64;
    // 导入去重：同一淘宝链接只建一个商品。批内用 seen 去重，跨批查库去重(避免与历史/已上架商品
    // 重复采集铺货后在 precheck 撞 DUPLICATE_EXTERNAL_PRODUCT_IN_SHOP)。跳过数用通知告知用户，
    // 返回值仍是去重后的真实导入数，杜绝「静默丢数据」误解。
    let mut skipped_count = 0i64;
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

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

            // 去重：同一淘宝链接(批内重复 / 库内已存在)只建一个商品，避免重复铺货撞 DUPLICATE。
            if !seen.insert(source_url.clone()) {
                skipped_count += 1;
                continue;
            }
            let already_exists = conn
                .query_row(
                    "SELECT 1 FROM pipeline_products WHERE source_url = ?1 LIMIT 1",
                    [&source_url],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if already_exists {
                skipped_count += 1;
                continue;
            }

            let product_id = format!("prod_{}", Uuid::new_v4().simple());
            conn.execute(
                "INSERT INTO pipeline_products
                 (id, title, source_url, category_path, status, stage, attention, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'pending_collect', 'collect', 'none', ?5, ?5)",
                params![product_id, title, source_url, category_path, now],
            )?;
            for shop in &target_shops {
                let target_id = format!("tgt_{}", Uuid::new_v4().simple());
                conn.execute(
                    "INSERT INTO pipeline_shop_targets
                     (id, product_id, shop_id, shop_name, stage, status, retry_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 'await_review', 'pending', 0, ?5, ?5)",
                    params![target_id, product_id, shop.id, shop.name, now],
                )?;
            }
            imported_count += 1;
        }
    }

    if skipped_count > 0 {
        // 通知告知用户有重复链接被跳过(返回值是去重后的真实导入数，避免「导入了全部行」的误解)。
        let _ = upsert_notification(
            &conn,
            "info",
            "import_dedup",
            "import_excel",
            None,
            "已跳过重复淘宝链接",
            &format!(
                "本次导入跳过 {skipped_count} 条重复淘宝链接(表内重复或库内已存在)，实际导入 {imported_count} 个商品。"
            ),
            None,
        );
    }

    if imported_count > 0 {
        // 导入成功后触发后台采集 Worker
        trigger_collection_worker(app);
    }

    Ok(imported_count)
}

/// 给已存在的商品补选目标店并铺货（「只采集」后再选店的入口）。
///
/// 按商品当前所处阶段决定新建 target 的初始 stage：
/// - 已采集审查完成（stage=publish/done，即 collected 或已在铺货）→ target 从 precheck 起，直接铺货；
/// - 仍在采集/审查中（stage=collect/review）→ target 置 await_review，待审查通过时一并激活。
///
/// 已存在的店（UNIQUE(product_id, shop_id)）由 INSERT OR IGNORE 跳过，重复补店无副作用。
#[tauri::command]
pub fn add_publish_targets(
    app: AppHandle,
    product_ids: Vec<String>,
    target_shop_ids: Vec<String>,
) -> AppResult<()> {
    if target_shop_ids.is_empty() {
        return Err(AppError::Validation(
            "请先选择要铺货的目标微信小店".to_string(),
        ));
    }
    if product_ids.is_empty() {
        return Err(AppError::Validation("没有指定要铺货的商品".to_string()));
    }

    let conn = open_connection(&app)?;
    let now = now_shanghai();

    let target_shops = resolve_target_shops_for_scope(&conn, &[], &target_shop_ids)?;
    if target_shops.len() != target_shop_ids.len() {
        return Err(AppError::Validation(
            "目标微信小店不存在或已被删除，请刷新店铺列表后重试".to_string(),
        ));
    }

    for product_id in &product_ids {
        // 读商品阶段、状态与已审查数据：阶段决定新 target 初始 stage，状态用于跳过已上架商品，
        // reviewed_data 用于补店前的类目准入校验
        let row: Option<(String, String, Option<String>)> = conn
            .query_row(
                "SELECT stage, status, reviewed_data FROM pipeline_products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((stage, status, reviewed_data)) = row else {
            continue; // 商品不存在则跳过
        };
        // 已全部上架的商品不再补货：防止陈旧的批量勾选把 listed 商品误回退到「铺货中」
        if status == product_status::LISTED {
            continue;
        }
        // 审查已通过（已进入铺货阶段）→ 直接 precheck；否则等审查激活（await_review）
        let reviewed = stage == product_stage::PUBLISH || stage == product_stage::DONE;
        let target_stage = if reviewed {
            target_stage::PRECHECK
        } else {
            target_stage::AWAIT_REVIEW
        };

        // 已审查商品补店即将直接铺货：先按「真实目标店」校验微信类目准入，把原来推迟到铺货
        // precheck 阶段才暴雷的类目不准入提前到补店点立即反馈（审查期用任意店校验无法覆盖真实目标店）。
        if reviewed {
            let leaf_cat_id = reviewed_data
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .and_then(|data| {
                    data.get("metadata")
                        .and_then(|m| m.get("wechat_category_ids"))
                        .and_then(Value::as_array)
                        .and_then(|ids| ids.last())
                        .and_then(Value::as_i64)
                });
            if let Some(leaf_cat_id) = leaf_cat_id {
                for shop in &target_shops {
                    ensure_category_available_for_shop(&conn, &shop.id, leaf_cat_id)?;
                }
            }
        }

        for shop in &target_shops {
            let target_id = format!("tgt_{}", Uuid::new_v4().simple());
            conn.execute(
                "INSERT OR IGNORE INTO pipeline_shop_targets
                 (id, product_id, shop_id, shop_name, stage, status, retry_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 0, ?6, ?6)",
                params![target_id, product_id, shop.id, shop.name, target_stage, now],
            )?;
        }

        // 重算商品级状态：collected（stage=publish）补 precheck target 后聚合成 publishing；
        // 采集/审查中的商品 recompute 直接返回，保持当前状态，待审查通过时激活。
        recompute_pipeline_product(&conn, product_id)?;
    }

    Ok(())
}

// 2. 淘宝登录以保存 Profile
#[tauri::command]
pub async fn open_taobao_login(app: AppHandle) -> AppResult<()> {
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    let script_path = resolve_collector_script(&app);

    // 异步启动登录子进程，直到用户关闭浏览器；本地冷却期会由脚本预检拦截。
    let mut command = python_command(&app);
    command.env("WX_XD_TAOBAO_IGNORE_CAPTCHA_FAILURE_COOLDOWN", "1");
    let output = command
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
    let task = conn
        .query_row(
            "SELECT id, title, source_url, category_path FROM pipeline_products WHERE id = ?1",
            [task_id.as_str()],
            |row| {
                Ok(CollectTask {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    source_url: row.get(2)?,
                    category_path: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation(format!("流水线商品 {} 不存在", task_id)))?;

    // 重置为待采集，并立即采集单个商品；清掉上一轮采集/审查的残留结果，避免确认时用到旧数据
    conn.execute(
        "UPDATE pipeline_products
         SET status = 'pending_collect', stage = 'collect', attention = 'none',
             error_code = NULL, error_reason = NULL, progress_text = NULL,
             reviewed_data = NULL, review_result_json = NULL, updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), task_id],
    )?;

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_single_collection_task(&app_clone, &task).await {
            eprintln!("重试采集任务 {} 失败: {:?}", task.id, e);
        }
    });

    Ok(())
}

// 5. 清理采集任务
#[tauri::command]
pub fn clear_collection_tasks(app: AppHandle) -> AppResult<()> {
    let conn = open_connection(&app)?;
    conn.execute("DELETE FROM pipeline_assets", [])?;
    conn.execute("DELETE FROM pipeline_shop_targets", [])?;
    conn.execute("DELETE FROM pipeline_products", [])?;
    Ok(())
}

#[tauri::command]
pub fn delete_taobao_profile(app: AppHandle) -> AppResult<String> {
    if COLLECTOR_RUNNING.load(Ordering::SeqCst) {
        return Err(AppError::Validation(
            "后台采集正在运行，请等待采集结束后再删除 profile。".to_string(),
        ));
    }
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_str = profile_dir.to_string_lossy().to_string();
    if profile_dir.exists() {
        std::fs::remove_dir_all(&profile_dir)
            .map_err(|e| AppError::Validation(format!("删除 profile 失败: {}", e)))?;
    }
    Ok(profile_str)
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
pub fn retry_all_failed_collection_tasks(app: AppHandle) -> AppResult<i64> {
    let conn = open_connection(&app)?;
    let now = now_shanghai();
    let count = conn.execute(
        "UPDATE pipeline_products
         SET status = 'pending_collect', stage = 'collect', attention = 'none',
             error_code = NULL, error_reason = NULL, progress_text = NULL, updated_at = ?1
         WHERE stage = 'collect' AND status = 'error'",
        params![now],
    )?;
    if count > 0 {
        trigger_collection_worker(app);
    }
    Ok(count as i64)
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
    ensure_collection_review_target_shop_ready(&app, &tasks, &target_shop_ids)?;
    let ai_config = load_optional_ai_provider_config(&app)?;

    // 并发审查：每个采集任务独立启动 agent session
    // pi-coding-agent 以 session 为颗粒度，天然支持并行
    let max_concurrency = collection_review_max_concurrency();
    let stagger_ms = collection_review_stagger_ms(max_concurrency);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let ai_rate_limited = Arc::new(AtomicBool::new(false));

    let handles: Vec<_> = tasks
        .into_iter()
        .enumerate()
        .map(|(index, task)| {
            let app = app.clone();
            let config = ai_config.clone();
            let shop_ids = target_shop_ids.clone();
            let sem = semaphore.clone();
            let ai_rate_limited = ai_rate_limited.clone();
            tokio::spawn(async move {
                // 小错峰只发生在并发启动前，避免串行审查时每个商品额外空等。
                let delay_ms = collection_review_start_delay_ms(index, max_concurrency, stagger_ms);
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                let _permit = sem.acquire().await;
                // AI 审查调用加超时：provider(小米 mimo)偶发 hang，无超时会卡死整轮 driver tick
                // 并可能累积 agent 子进程；120s 内未返回按失败处理，走退避重试而非永久挂起。
                let review_result = match tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    review_collection_task(
                        &app,
                        &task,
                        config.as_ref(),
                        &shop_ids,
                        &ai_rate_limited,
                    ),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err(AppError::Validation(
                        "审查 AI 调用超时(120s)，已按失败处理".to_string(),
                    )),
                };
                (task.id, review_result)
            })
        })
        .collect();

    let mut result = CollectionReviewBatchResult {
        processed_items: handles.len() as i64,
        passed_items: 0,
        needs_review_items: 0,
        blocked_items: 0,
        failed_items: 0,
    };

    for handle in handles {
        let (task_id, review_result) = match handle.await {
            Ok(output) => output,
            Err(error) => {
                result.failed_items += 1;
                eprintln!("审查任务 panic：{error}");
                continue;
            }
        };
        match review_result {
            Ok(review) => {
                match review.status.as_str() {
                    "passed" => result.passed_items += 1,
                    "needs_review" => result.needs_review_items += 1,
                    "blocked" => result.blocked_items += 1,
                    _ => result.failed_items += 1,
                }
                if let Err(error) = persist_collection_review_result(&app, &task_id, &review) {
                    eprintln!("持久化审查结果失败 task_id={task_id}：{error}");
                }
            }
            Err(error) => {
                let summary = error.to_string();
                if is_ai_rate_limit_error(&summary) || summary.contains("超时") {
                    // 限流/AI 超时属瞬时故障：不落 status='error' 终态(否则审查 loader 不再捞、卡死)。
                    // 商品 status 在审查全程一直是 'collecting'，此处不持久化失败即天然保持可重审；
                    // 配合 ai_provider 的限流冷却，driver 下个 tick 不会立刻再打爆 provider。
                    eprintln!("审查遇限流/超时，保持待重审 task_id={task_id}：{summary}");
                } else if let Err(persist_error) =
                    persist_collection_review_failure(&app, &task_id, &summary)
                {
                    eprintln!("持久化审查失败 task_id={task_id}：{persist_error}");
                }
                result.failed_items += 1;
            }
        }
    }

    Ok(result)
}

fn collection_review_max_concurrency() -> usize {
    std::env::var("WX_XD_REVIEW_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2)
        .clamp(1, 4)
}

fn collection_review_stagger_ms(max_concurrency: usize) -> u64 {
    if max_concurrency <= 1 {
        return 0;
    }
    std::env::var("WX_XD_REVIEW_STAGGER_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1_200)
        .min(30_000)
}

fn collection_review_start_delay_ms(index: usize, max_concurrency: usize, stagger_ms: u64) -> u64 {
    if max_concurrency <= 1 || stagger_ms == 0 {
        0
    } else {
        (index % max_concurrency) as u64 * stagger_ms
    }
}

#[tauri::command]
pub fn confirm_collection_review(
    app: AppHandle,
    request: CollectionReviewConfirmRequest,
) -> AppResult<()> {
    let conn = open_connection(&app)?;
    let product_id = request.task_id.trim().to_string();
    let (reviewed_data, collected_data, review_result_json): (
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT reviewed_data, collected_data, review_result_json
             FROM pipeline_products WHERE id = ?1",
            [product_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation(format!("流水线商品 {product_id} 不存在")))?;
    let raw = reviewed_data
        .as_deref()
        .or(collected_data.as_deref())
        .unwrap_or_default()
        .trim()
        .to_string();
    if raw.is_empty() {
        return Err(AppError::Validation("商品缺少可确认的数据".to_string()));
    }
    let mut product = serde_json::from_str::<ExternalProductInput>(&raw)
        .map_err(|error| AppError::Validation(format!("采集审查结果无法解析：{error}")))?;
    if let Some(title) = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        product.title = title.to_string();
    }
    // 目标店从该商品的 targets 聚合（新模型目标店在 pipeline_shop_targets）
    let target_shop_ids = load_review_target_shop_ids(&conn, &product_id)?;
    if let Some(category_ids) = request.category_ids.as_ref().filter(|ids| ids.len() >= 3) {
        let leaf_cat_id = *category_ids
            .last()
            .ok_or_else(|| AppError::Validation("微信类目 ID 不能为空".to_string()))?;
        let effective_shop_ids = if request.target_shop_ids.is_empty() {
            target_shop_ids.clone()
        } else {
            request.target_shop_ids.clone()
        };
        let normalized_target_shop_ids = effective_shop_ids
            .iter()
            .map(|shop_id| shop_id.trim())
            .filter(|shop_id| !shop_id.is_empty())
            .collect::<Vec<_>>();
        if normalized_target_shop_ids.is_empty() {
            let Some(category_shop_id) =
                resolve_collection_review_category_shop(&conn, &target_shop_ids)?
            else {
                return Err(AppError::Validation(
                    "未同步目标店铺的生效类目权限，不能确认通过审查".to_string(),
                ));
            };
            ensure_category_available_for_shop(&conn, &category_shop_id, leaf_cat_id)?;
        } else {
            for shop_id in normalized_target_shop_ids {
                ensure_category_available_for_shop(&conn, shop_id, leaf_cat_id)?;
            }
        }
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
        review_result_json.as_deref(),
        request.category_ids,
        request.category_path,
    )?;
    // 确认抽屉补选了目标店（没店的「只采集」商品在确认时一并选店）：先按选定店建 target，
    // 让下面按 target_count>0 分支激活铺货，实现「补选店→选类目→确认即铺货」一步到位。
    // 铺货阶段卡 need_confirm 的商品（已有店）前端传空 target_shop_ids，不进此分支。
    if !request.target_shop_ids.is_empty() {
        let existing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pipeline_shop_targets WHERE product_id = ?1",
            [product_id.as_str()],
            |row| row.get(0),
        )?;
        if existing == 0 {
            let target_shops =
                resolve_target_shops_for_scope(&conn, &[], &request.target_shop_ids)?;
            if target_shops.len() != request.target_shop_ids.len() {
                return Err(AppError::Validation(
                    "目标微信小店不存在或已被删除，请刷新店铺列表后重试".to_string(),
                ));
            }
            for shop in &target_shops {
                let target_id = format!("tgt_{}", Uuid::new_v4().simple());
                conn.execute(
                    "INSERT OR IGNORE INTO pipeline_shop_targets
                     (id, product_id, shop_id, shop_name, stage, status, retry_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 'precheck', 'pending', 0, ?5, ?5)",
                    params![target_id, product_id, shop.id, shop.name, now],
                )?;
            }
        }
    }
    let target_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pipeline_shop_targets WHERE product_id = ?1",
        [product_id.as_str()],
        |row| row.get(0),
    )?;
    if target_count > 0 {
        // 人工确认通过且已选店：商品进入铺货阶段，激活各目标店 target 从 precheck 开始推进
        conn.execute(
            "UPDATE pipeline_products
             SET reviewed_data = ?1, review_result_json = ?2, stage = 'publish', status = 'publishing',
                 attention = 'none', error_code = NULL, error_reason = NULL,
                 progress_text = '准备铺货', updated_at = ?3
             WHERE id = ?4",
            params![reviewed_data, result_json, now, product_id],
        )?;
        conn.execute(
            "UPDATE pipeline_shop_targets
             SET stage = 'precheck', status = 'pending', error_code = NULL,
                 error_summary = NULL, updated_at = ?1
             WHERE product_id = ?2 AND wechat_product_id IS NULL",
            params![now, product_id],
        )?;
    } else {
        // 人工确认通过但还没选店（「只采集」模式）：稳定在「待铺货」，stage 保持 publish
        // 让 recompute_pipeline_product 能正确聚合补店后的状态，等用户补选店后由
        // add_publish_targets 推进铺货。绝不能写成 publishing，否则视图层会因无 target 误判异常。
        conn.execute(
            "UPDATE pipeline_products
             SET reviewed_data = ?1, review_result_json = ?2, stage = 'publish', status = 'collected',
                 attention = 'none', error_code = NULL, error_reason = NULL,
                 progress_text = '待选店铺货', updated_at = ?3
             WHERE id = ?4",
            params![reviewed_data, result_json, now, product_id],
        )?;
    }
    Ok(())
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
pub fn import_collection_task_image(
    app: AppHandle,
    request: CollectionImageUploadRequest,
) -> AppResult<CollectionTaskView> {
    let kind = normalize_collection_image_kind(&request.kind)?;
    let imported_path = import_collection_image_file(&app, &request.file_path)?;
    let mut product = load_collection_product_for_edit(&app, &request.task_id)?;
    let target = collection_product_images_mut(&mut product, kind);
    let imported = imported_path.to_string_lossy().to_string();
    if !target.iter().any(|image| image == &imported) {
        target.push(imported);
    }
    persist_collection_product_after_image_edit(&app, &request.task_id, product)
}

#[tauri::command]
pub fn remove_collection_task_image(
    app: AppHandle,
    request: CollectionImageRemoveRequest,
) -> AppResult<CollectionTaskView> {
    let kind = normalize_collection_image_kind(&request.kind)?;
    let image_url = request.image_url.trim();
    if image_url.is_empty() {
        return Err(AppError::Validation("图片地址不能为空".to_string()));
    }
    let mut product = load_collection_product_for_edit(&app, &request.task_id)?;
    let target = collection_product_images_mut(&mut product, kind);
    let before = target.len();
    target.retain(|image| image.trim() != image_url);
    if target.len() == before {
        return Err(AppError::Validation(
            "采集结果中没有找到这张图片".to_string(),
        ));
    }
    persist_collection_product_after_image_edit(&app, &request.task_id, product)
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

fn normalize_collection_image_kind(kind: &str) -> AppResult<&'static str> {
    match kind.trim() {
        "main" | "image" | "images" => Ok("main"),
        "detail" | "detail_image" | "detail_images" => Ok("detail"),
        _ => Err(AppError::Validation(
            "图片类型只能是主图或详情图".to_string(),
        )),
    }
}

fn collection_product_images_mut<'a>(
    product: &'a mut ExternalProductInput,
    kind: &str,
) -> &'a mut Vec<String> {
    if kind == "detail" {
        &mut product.detail_images
    } else {
        &mut product.images
    }
}

fn import_collection_image_file(app: &AppHandle, file_path: &str) -> AppResult<PathBuf> {
    let source = PathBuf::from(file_path.trim());
    if !source.is_file() {
        return Err(AppError::Validation("请选择有效的本地图片文件".to_string()));
    }
    let bytes = fs::read(&source)?;
    if bytes.is_empty() {
        return Err(AppError::Validation("图片内容为空".to_string()));
    }
    if bytes.len() > IMAGE_DOWNLOAD_MAX_BYTES as usize {
        return Err(AppError::Validation(format!(
            "图片过大：{}，超过本地导入上限 {}",
            format_bytes_short(bytes.len()),
            format_bytes_short(IMAGE_DOWNLOAD_MAX_BYTES as usize)
        )));
    }
    let format = image::guess_format(&bytes)
        .map_err(|error| AppError::Validation(format!("图片格式无法识别：{error}")))?;
    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|error| AppError::Validation(format!("图片解码失败：{error}")))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(AppError::Validation("图片宽高无效".to_string()));
    }
    let extension = original_upload_extension(format).unwrap_or("img");
    let hash = hex_sha256(&bytes);
    let dir = collection_uploaded_image_dir(app)?;
    let target = dir.join(format!("{hash}.{extension}"));
    if !target.exists() {
        fs::write(&target, &bytes)?;
    }
    Ok(target)
}

fn load_collection_product_for_edit(
    app: &AppHandle,
    task_id: &str,
) -> AppResult<ExternalProductInput> {
    let conn = open_connection(app)?;
    let task = load_collection_task_by_id(&conn, task_id.trim())?;
    if task.status != "success" {
        return Err(AppError::Validation(
            "只能编辑采集成功的商品图片".to_string(),
        ));
    }
    let raw = task.collected_data.as_deref().unwrap_or_default().trim();
    if raw.is_empty() {
        return Err(AppError::Validation(
            "采集结果为空，不能编辑图片".to_string(),
        ));
    }
    serde_json::from_str::<ExternalProductInput>(raw)
        .map_err(|error| AppError::Validation(format!("采集结果无法解析：{error}")))
}

fn persist_collection_product_after_image_edit(
    app: &AppHandle,
    task_id: &str,
    mut product: ExternalProductInput,
) -> AppResult<CollectionTaskView> {
    let now = now_shanghai();
    if !product.metadata.is_object() {
        product.metadata = Value::Object(serde_json::Map::new());
    }
    if let Some(metadata) = product.metadata.as_object_mut() {
        metadata.insert(
            "collection_image_edit".to_string(),
            serde_json::json!({
                "updated_at": now,
                "review_reset": true
            }),
        );
    }
    let collected_data = serde_json::to_string(&product)
        .map_err(|error| AppError::Validation(format!("采集结果无法序列化：{error}")))?;
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE collection_tasks
         SET collected_data = ?1,
             review_status = 'pending',
             review_summary = NULL,
             reviewed_data = NULL,
             review_result_json = NULL,
             reviewed_at = NULL,
             updated_at = ?2
         WHERE id = ?3",
        params![collected_data, now, task_id.trim()],
    )?;
    load_collection_task_by_id(&conn, task_id.trim())
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
                   AND i.status NOT IN ('failed', 'cancelled')
                 LIMIT 1",
                params![shop_id, external_product_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if existing_publish_item.is_some() {
            return Err(AppError::Validation(format!(
                "外部商品 {} 已经给店铺 {} 创建过铺货任务（非失败状态），不能重复创建",
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

/// 审查 handler 内部传递的轻量商品（来自 pipeline_products，目标店从 targets 聚合）。
struct ReviewTask {
    id: String,
    status: String,
    collected_data: Option<String>,
    target_shop_ids: Vec<String>,
}

fn load_review_target_shop_ids(conn: &Connection, product_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT shop_id FROM pipeline_shop_targets WHERE product_id = ?1 ORDER BY shop_name",
    )?;
    let ids = stmt
        .query_map([product_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

fn load_collection_review_tasks(
    app: &AppHandle,
    task_ids: &[String],
    limit: i64,
) -> AppResult<Vec<ReviewTask>> {
    let conn = open_connection(app)?;
    let mut products: Vec<(String, Option<String>)> = Vec::new();
    if task_ids.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT id, collected_data FROM pipeline_products
             WHERE stage = 'review'
               AND status = 'collecting'
               AND collected_data IS NOT NULL
               AND TRIM(collected_data) <> ''
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;
        products = stmt
            .query_map([limit], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
    } else {
        for task_id in task_ids.iter().take(limit as usize) {
            if let Some(row) = conn
                .query_row(
                    "SELECT id, collected_data FROM pipeline_products WHERE id = ?1",
                    [task_id.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
            {
                products.push(row);
            }
        }
    }

    let mut tasks = Vec::with_capacity(products.len());
    for (id, collected_data) in products {
        let target_shop_ids = load_review_target_shop_ids(&conn, &id)?;
        tasks.push(ReviewTask {
            id,
            status: "success".to_string(),
            collected_data,
            target_shop_ids,
        });
    }
    Ok(tasks)
}

fn ensure_collection_review_target_shop_ready(
    app: &AppHandle,
    tasks: &[ReviewTask],
    target_shop_ids: &[String],
) -> AppResult<()> {
    if !target_shop_ids.is_empty()
        || tasks.is_empty()
        || tasks.iter().all(|task| !task.target_shop_ids.is_empty())
    {
        return Ok(());
    }
    let conn = open_connection(app)?;
    resolve_collection_review_category_shop(&conn, &[]).map(|_| ())
}

async fn review_collection_task(
    app: &AppHandle,
    task: &ReviewTask,
    ai_config: Option<&AiProviderConfig>,
    target_shop_ids: &[String],
    ai_rate_limited: &AtomicBool,
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
    let effective_target_shop_ids = if target_shop_ids.is_empty() {
        task.target_shop_ids.clone()
    } else {
        target_shop_ids.to_vec()
    };

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
        resolve_collection_review_category_shop(&conn, &effective_target_shop_ids)?
    };
    let mut category_candidates = Vec::<CollectionReviewCategoryCandidate>::new();
    if let Some(shop_id) = category_shop_id.as_deref() {
        let conn = open_connection(app)?;
        category_candidates =
            suggest_wechat_category_candidates_from_cache(&conn, shop_id, &product, 5)?;
        if category_candidates.is_empty() {
            category_candidates =
                suggest_wechat_category_broad_candidates_from_cache(&conn, shop_id, &product, 80)?;
        }
    }
    let mut category_applied = false;
    let mut attr_suggestions = serde_json::Map::new();
    let mut attr_fill_suggestions = None::<Value>;

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
            "info",
            "采集审查 Agent 技能已停用，已改用本地审查规则",
        ));
    } else if let Some(config) = ai_config {
        if ai_rate_limited.load(Ordering::SeqCst) {
            agent_error_code_for_run = Some("AI_RATE_LIMIT_COOLDOWN".to_string());
            agent_error_summary_for_run =
                Some("本批审查已触发 AI provider 限流，图文 AI 审查已跳过".to_string());
            ai_warning = agent_error_summary_for_run.clone();
            issues.push(review_issue(
                "image",
                "info",
                "本批审查已触发 AI 限流，后续商品改用本地图片规则过滤",
            ));
        } else {
            ai_used = true;
            let ai_images = review_ai_image_urls(&product);
            let ai_input = serde_json::json!({
                "task_id": &task.id,
                "target_shop_ids": &effective_target_shop_ids,
                "original_title": &original_title,
                "category_candidates": &category_candidates
            });
            match request_ai_collection_review_json(
            app,
            config,
            &format!(
                "审查采集任务 {}，使用 get_product_detail 获取商品数据，使用 search_categories/get_category_detail 匹配类目并补齐属性，使用 search_wechat_docs 查询不确定的属性定义。按 wx-xd-product-review 技能输出 JSON。",
                &task.id
            ),
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
                // 提取 AI 产出的属性建议（agent 工具流产物）
                if let Some(ai_attrs) = ai_result
                    .get("ai_attr_suggestions")
                    .and_then(Value::as_object)
                {
                    for (key, value) in ai_attrs {
                        if let Some(v) = value.as_str() {
                            if !v.trim().is_empty() {
                                attr_suggestions
                                    .entry(key.clone())
                                    .or_insert(Value::String(v.trim().to_string()));
                            }
                        }
                    }
                }
            }
            Err(error) => {
                let error_summary = error.to_string();
                if is_ai_rate_limit_error(&error_summary) {
                    ai_rate_limited.store(true, Ordering::SeqCst);
                }
                agent_error_code_for_run = Some(agent_error_code(&error).to_string());
                agent_error_summary_for_run = Some(error_summary.clone());
                ai_warning = Some(error_summary);
                issues.push(review_issue(
                    "image",
                    "info",
                    "AI 图文审查未完成，已改用本地图片规则过滤",
                ));
            }
        }
        }
    } else {
        agent_error_code_for_run = Some("PROVIDER_NOT_CONFIGURED".to_string());
        agent_error_summary_for_run = Some("未启用 AI provider，图片内容仅做规则过滤".to_string());
        issues.push(review_issue(
            "image",
            "info",
            "未启用 AI provider，图片内容仅做本地规则过滤",
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
        if let Some(best) =
            select_collection_review_category_candidate(&product, &category_candidates)
        {
            apply_review_category(&mut product, &best.category_ids, &best.category_path);
            category_applied = true;
        }
    }
    if !category_applied {
        // 没选店（「只采集」）时类目无法落定不算异常：微信类目须按真实目标店确定，此时强行卡
        // 「待确认」会让用户既搜不了类目（没店）也选不了店而死锁。故没店时降为 info 放行到
        // 「待铺货」，把类目确认延到选店后的铺货阶段（attr_fill 用真实店补类目，补不出再以
        // need_confirm 暴露，那时已有店可搜类目）；有店则保持 confirm 拦人工确认。
        let no_shop = effective_target_shop_ids.is_empty();
        let severity = if no_shop { "info" } else { "confirm" };
        let message = match (category_candidates.is_empty(), no_shop) {
            (true, true) => "未匹配到微信类目，待选店铺货时再按目标店确定",
            (true, false) => "未匹配到可用微信类目，需要人工补充类目",
            (false, true) => "类目候选置信度不足，待选店铺货时再确认",
            (false, false) => "类目匹配置信度不足，需要从候选类目中确认",
        };
        issues.push(review_issue("category", severity, message));
    }

    // ---- 微信类目属性预检 & 补齐 ----
    // 先写入 Agent 已产出的 ai_attr_suggestions，让后续本地推断能读到
    if !attr_suggestions.is_empty() {
        if !product.metadata.is_object() {
            product.metadata = Value::Object(serde_json::Map::new());
        }
        if let Some(metadata) = product.metadata.as_object_mut() {
            metadata.insert(
                "ai_attr_suggestions".to_string(),
                Value::Object(attr_suggestions.clone()),
            );
        }
    }
    // 对仍未覆盖的属性做补充补齐
    if category_applied {
        // 有类目：按类目要求补齐
        let cat_id = product
            .metadata
            .as_object()
            .and_then(|m| m.get("wechat_category_ids"))
            .and_then(Value::as_array)
            .and_then(|ids| ids.last())
            .and_then(|id| id.as_i64());
        if let (Some(shop_id), Some(cat_id)) = (category_shop_id.as_deref(), cat_id) {
            let conn = open_connection(app)?;
            if let Ok(Some(raw_detail)) =
                load_cached_category_detail_payload(&conn, shop_id, cat_id)
            {
                let mut payload = collection_review_requirement_payload(&product);
                let req_check = check_category_requirements_from_detail(&raw_detail, &payload);
                if req_check.has_missing_attrs() {
                    let item = collection_review_pending_publish_item(task, shop_id, &product)?;
                    let mut plan = build_attribute_fill_plan(
                        &item,
                        &product,
                        &payload,
                        &raw_detail,
                        &req_check,
                    );
                    let requires_ai = plan
                        .suggestions
                        .iter()
                        .any(|suggestion| !suggestion.applied && suggestion.source == "needs_ai");
                    if requires_ai {
                        match ai_config {
                            Some(config) => {
                                if let Err(error) = run_collection_attribute_ai(
                                    app, config, &task.id, shop_id, &mut plan,
                                )
                                .await
                                {
                                    let summary = error.to_string();
                                    if is_ai_rate_limit_error(&summary) {
                                        // 限流不传播 Err(否则整轮审查落 status='error'，同样不会被
                                        // driver 重审=同根死锁)；记 warning 让 persist 走「保持
                                        // collecting 自动重审」，缺的必填属性留待铺货阶段补齐。
                                        if ai_warning.is_none() {
                                            ai_warning = Some(summary);
                                        }
                                        issues.push(review_issue(
                                            "attr",
                                            "info",
                                            "AI 限流，必填属性将在铺货阶段补齐",
                                        ));
                                    } else {
                                        return Err(error);
                                    }
                                }
                            }
                            None => {
                                issues.push(review_issue(
                                    "attr",
                                    "confirm",
                                    "未启用 AI provider，无法自动补齐微信必填属性",
                                ));
                            }
                        }
                    }
                    promote_collection_ai_attribute_suggestions(&mut plan);
                    apply_collection_attribute_plan_to_product(
                        &mut product,
                        &mut payload,
                        &plan,
                        &mut attr_suggestions,
                    )?;
                    attr_fill_suggestions = Some(attribute_suggestions_json(&plan.suggestions));
                    for suggestion in &plan.suggestions {
                        if !suggestion.applied {
                            issues.push(review_issue(
                                "attr",
                                "info",
                                &collection_unresolved_attr_message(suggestion),
                            ));
                        }
                    }
                }
            }
        }
    } else if ai_config.is_some() {
        // 无类目但有 AI：用 AI 推断基础属性（不依赖类目详情）
        // 这些属性在大多数童装类目中都是必填的
        let basic_attrs = [
            "安全等级",
            "适用年龄",
            "面料材质",
            "面料材质成分含量（%）",
            "颜色",
            "风格",
        ];
        let existing_attrs = collection_review_existing_attr_keys(&product);
        let missing: Vec<&str> = basic_attrs
            .iter()
            .filter(|attr| !existing_attrs.contains(&attr.to_string()))
            .copied()
            .collect();
        if !missing.is_empty() {
            if let Some(shop_id) = category_shop_id.as_deref() {
                let item = collection_review_pending_publish_item(task, shop_id, &product)?;
                let mut plan = build_basic_attribute_fill_plan(&item, &product, &missing);
                if let Some(config) = ai_config {
                    if let Err(error) =
                        run_collection_attribute_ai(app, config, &task.id, shop_id, &mut plan).await
                    {
                        let summary = error.to_string();
                        if is_ai_rate_limit_error(&summary) {
                            // 同上：限流不传播，保持可重审。
                            if ai_warning.is_none() {
                                ai_warning = Some(summary);
                            }
                        } else {
                            return Err(error);
                        }
                    }
                }
                promote_collection_ai_attribute_suggestions(&mut plan);
                apply_basic_attribute_suggestions(&product, &plan, &mut attr_suggestions);
                attr_fill_suggestions = Some(attribute_suggestions_json(&plan.suggestions));
            }
        }
    }

    if !product.metadata.is_object() {
        product.metadata = Value::Object(serde_json::Map::new());
    }
    if let Some(metadata) = product.metadata.as_object_mut() {
        if !attr_suggestions.is_empty() {
            metadata.insert(
                "ai_attr_suggestions".to_string(),
                Value::Object(attr_suggestions),
            );
        }
        if let Some(suggestions) = attr_fill_suggestions.clone() {
            metadata.insert("wechat_attr_fill_suggestions".to_string(), suggestions);
        }
        metadata.insert(
            "collection_review".to_string(),
            serde_json::json!({
                "original_title": original_title.clone(),
                "reviewed_title": product.title.clone(),
                "removed_title_terms": title_removed_terms.clone(),
                "removed_images": removed_images.clone(),
                "category_applied": category_applied,
                "ai_used": ai_used,
                "attribute_suggestions": attr_fill_suggestions,
                "skill": {
                    "name": PRODUCT_REVIEW_SKILL.name,
                    "version": PRODUCT_REVIEW_SKILL.version
                },
                "ai_warning": ai_warning.clone(),
                "reviewed_at": now_shanghai()
            }),
        );
    }

    let status = collection_review_status_from_issues(&issues).to_string();
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

fn is_ai_rate_limit_error(summary: &str) -> bool {
    summary.contains("429")
        || summary.contains("Too many requests")
        || summary.contains("too many requests")
        || summary.contains("limitation")
        || summary.contains("限流")
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

    if review.status == "passed" {
        let target_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pipeline_shop_targets WHERE product_id = ?1",
            [task_id],
            |row| row.get(0),
        )?;
        if target_count > 0 {
            // 审查通过且已选店：商品进入铺货阶段，激活各目标店 target 从类目预检开始推进
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'publishing', stage = 'publish', attention = 'none',
                     error_code = NULL, error_reason = NULL, progress_text = '准备铺货',
                     reviewed_data = ?1, review_result_json = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![reviewed_data, review_result_json, now, task_id],
            )?;
            conn.execute(
                "UPDATE pipeline_shop_targets
                 SET stage = 'precheck', status = 'pending', error_code = NULL,
                     error_summary = NULL, updated_at = ?1
                 WHERE product_id = ?2 AND wechat_product_id IS NULL",
                params![now, task_id],
            )?;
        } else {
            // 审查通过但还没选店（「只采集」模式）：稳定在「待铺货」，reviewed_data 已是
            // 平台级完整数据，等用户补选店后由 add_publish_targets 推进铺货。
            // stage 保持 publish 让 recompute_pipeline_product 能正确聚合补店后的状态。
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'collected', stage = 'publish', attention = 'none',
                     error_code = NULL, error_reason = NULL, progress_text = '待选店铺货',
                     reviewed_data = ?1, review_result_json = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![reviewed_data, review_result_json, now, task_id],
            )?;
        }
        // 兜底：审查通过后统一重算一次商品级状态。若在上面 COUNT 读取与 UPDATE 之间，
        // 用户并发调用 add_publish_targets 插入了 await_review target，这里的 recompute 会
        // 把它提升为 precheck 并聚合成 publishing，彻底堵死「补店与审查并发」的孤儿窗口。
        recompute_pipeline_product(&conn, task_id)?;
    } else {
        // needs_review / blocked：默认停在待确认，等人工处理。
        // 但若是 AI provider 限流(429/超时)导致的降级——AI 没调通、退回本地匹配——
        // 这并非「真的需要人工」，而是瞬时故障。此时只要本地仍有类目候选，就保持 collecting
        // 让 driver 下个 tick 自动重审，避免限流被误当成 need_confirm 终态卡死(审查 loader 只捞
        // status='collecting')。判定锚点严格绑定：needs_review + ai.warning 命中限流 + 有类目候选；
        // blocked(图片违禁等真问题)、无候选(本地确实定不了)一律照常落终态等人工。
        let ai_warning = review
            .result_json
            .pointer("/ai/warning")
            .and_then(Value::as_str)
            .unwrap_or("");
        let has_category_candidates = review
            .result_json
            .pointer("/category/candidates")
            .and_then(Value::as_array)
            .is_some_and(|candidates| !candidates.is_empty());
        let ai_unavailable = review.status == "needs_review"
            && is_ai_rate_limit_error(ai_warning)
            && has_category_candidates;
        if ai_unavailable {
            // 保持 collecting 让 driver 自动重审；不写 reviewed_data，保留原 collected_data 供重审复用。
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'collecting', stage = 'review', attention = 'none',
                     error_code = NULL, error_reason = NULL,
                     progress_text = 'AI 限流，等待自动重审',
                     review_result_json = ?1, updated_at = ?2
                 WHERE id = ?3",
                params![review_result_json, now, task_id],
            )?;
        } else {
            let error_code = if review.status == "blocked" {
                "REVIEW_BLOCKED"
            } else {
                "REVIEW_NEEDS_CONFIRM"
            };
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'need_confirm', stage = 'review', attention = 'need_confirm',
                     error_code = ?1, error_reason = ?2, progress_text = NULL,
                     reviewed_data = ?3, review_result_json = ?4, updated_at = ?5
                 WHERE id = ?6",
                params![
                    error_code,
                    review.summary,
                    reviewed_data,
                    review_result_json,
                    now,
                    task_id
                ],
            )?;
        }
    }
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
        "UPDATE pipeline_products
         SET status = 'error', stage = 'review', attention = 'error',
             error_code = 'UNKNOWN_AGENT_ERROR', error_reason = ?1,
             review_result_json = ?2, progress_text = NULL, updated_at = ?3
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
    if lower.contains("-tps-") {
        return Some("疑似平台标识、店招、头像或营销图");
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
        "shopmanager",
        "-0-shopmanager",
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

fn select_collection_review_category_candidate<'a>(
    product: &ExternalProductInput,
    candidates: &'a [CollectionReviewCategoryCandidate],
) -> Option<&'a CollectionReviewCategoryCandidate> {
    let mut scored = candidates
        .iter()
        .map(|candidate| {
            (
                score_collection_review_category_candidate(product, candidate),
                candidate,
            )
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.score.cmp(&left.1.score))
            .then_with(|| left.1.category_path.cmp(&right.1.category_path))
    });
    let (best_score, best) = scored.first().copied()?;
    let second_score = scored.get(1).map(|item| item.0).unwrap_or(i64::MIN);
    if best_score >= 130 && best_score - second_score >= 18 {
        Some(best)
    } else {
        None
    }
}

fn score_collection_review_category_candidate(
    product: &ExternalProductInput,
    candidate: &CollectionReviewCategoryCandidate,
) -> i64 {
    let mut score = candidate.score;
    let text = collection_category_evidence_text(product);
    let leaf = collection_category_leaf(&candidate.category_path);
    let leaf_norm = normalize_collection_category_text(&leaf);
    if is_generic_collection_category_leaf(&leaf_norm) && !text.contains("亲子") {
        score -= 80;
    }
    if let Some(hint_leaf) = product
        .category_hint
        .as_deref()
        .and_then(collection_category_hint_leaf)
    {
        let hint_norm = normalize_collection_category_text(&hint_leaf);
        if !hint_norm.is_empty()
            && (leaf_norm == hint_norm
                || leaf_norm.contains(&hint_norm)
                || hint_norm.contains(&leaf_norm))
        {
            score += 120;
        }
    }
    for (keywords, leaf_keywords, weight) in collection_category_keyword_rules() {
        if keywords.iter().any(|keyword| text.contains(keyword))
            && leaf_keywords
                .iter()
                .any(|keyword| leaf_norm.contains(&normalize_collection_category_text(keyword)))
        {
            score += weight;
        }
    }
    score
}

fn collection_category_keyword_rules(
) -> &'static [(&'static [&'static str], &'static [&'static str], i64)] {
    &[
        (&["t恤", "T恤", "短袖", "半袖"], &["t恤"], 90),
        (&["连衣裙", "公主裙"], &["连衣裙"], 100),
        (&["裙子", "半身裙"], &["裙"], 70),
        (&["打底裤", "防蚊裤", "长裤", "裤子", "束脚"], &["裤"], 90),
        (
            &["汉服", "唐装", "旗袍", "民族服", "国风", "古装"],
            &["旗袍", "唐装", "民族"],
            100,
        ),
        (&["篮球", "球衣", "队服"], &["套装", "T恤", "t恤"], 55),
    ]
}

fn collection_category_evidence_text(product: &ExternalProductInput) -> String {
    let mut text = product.title.clone();
    if let Some(value) = product.category_hint.as_deref() {
        text.push_str(value);
    }
    text
}

fn collection_category_leaf(category_path: &str) -> String {
    category_path
        .split('>')
        .next_back()
        .unwrap_or(category_path)
        .trim()
        .to_string()
}

fn collection_category_hint_leaf(category_hint: &str) -> Option<String> {
    category_hint
        .split(|ch| matches!(ch, '>' | '＞' | '/' | '／'))
        .next_back()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_collection_category_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || *ch == '恤')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_generic_collection_category_leaf(leaf_norm: &str) -> bool {
    matches!(leaf_norm, "亲子装" | "其他")
}

fn apply_ai_collection_review(
    product: &mut ExternalProductInput,
    ai_result: &Value,
    removed_images: &mut Vec<Value>,
    _issues: &mut Vec<Value>,
) {
    let confidence = json_value_to_i64(ai_result.get("confidence")).unwrap_or(0);
    if let Some(title) = json_value_to_string(ai_result.get("title")) {
        let title = normalize_review_title(&title);
        if title.chars().count() >= 4 {
            product.title = title;
        }
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

    let _needs_human_review = ai_result
        .get("needs_human_review")
        .and_then(Value::as_bool)
        .unwrap_or(false);
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

fn collection_review_requirement_payload(product: &ExternalProductInput) -> Value {
    serde_json::json!({
        "attrs": [],
        "skus": product.skus.iter().map(|sku| {
            serde_json::json!({
                "sku_attrs": collection_review_sku_attrs_from_specs(&sku.specs),
                "external_sku_id": &sku.external_sku_id,
                "specs": &sku.specs,
            })
        }).collect::<Vec<_>>()
    })
}

fn collection_review_sku_attrs_from_specs(specs: &Value) -> Vec<Value> {
    match specs {
        Value::Object(object) => object
            .iter()
            .filter_map(|(key, value)| {
                let value = json_value_to_string(Some(value))?;
                let key = key.trim();
                let value = value.trim();
                if key.is_empty() || value.is_empty() {
                    return None;
                }
                Some(serde_json::json!({
                    "attr_key": key,
                    "attr_value": value
                }))
            })
            .collect(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                let object = item.as_object()?;
                let key = ["attr_key", "name", "attr_name", "key"]
                    .iter()
                    .find_map(|key| json_value_to_string(object.get(*key)))?;
                let value = ["attr_value", "value", "value_name", "name"]
                    .iter()
                    .find_map(|key| json_value_to_string(object.get(*key)))?;
                let key = key.trim();
                let value = value.trim();
                if key.is_empty() || value.is_empty() {
                    return None;
                }
                Some(serde_json::json!({
                    "attr_key": key,
                    "attr_value": value
                }))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn collection_review_pending_publish_item(
    task: &ReviewTask,
    shop_id: &str,
    product: &ExternalProductInput,
) -> AppResult<PendingPublishItem> {
    let raw_payload = serde_json::to_string(product)
        .map_err(|error| AppError::Validation(format!("采集商品数据无法序列化：{error}")))?;
    Ok(PendingPublishItem {
        item_id: format!("collection-review-{}", task.id),
        job_id: "collection_review".to_string(),
        product_row_id: task.id.clone(),
        shop_id: shop_id.to_string(),
        shop_status: "active".to_string(),
        shop_has_secret: true,
        external_product_id: product.external_product_id.clone(),
        raw_payload,
    })
}

/// 获取商品已有的属性键（从 metadata.ai_attr_suggestions 和 payload.attrs 中）
fn collection_review_existing_attr_keys(
    product: &ExternalProductInput,
) -> std::collections::HashSet<String> {
    let mut keys = std::collections::HashSet::new();
    if let Some(metadata) = product.metadata.as_object() {
        // 从 ai_attr_suggestions 中获取
        if let Some(suggestions) = metadata
            .get("ai_attr_suggestions")
            .and_then(Value::as_object)
        {
            keys.extend(suggestions.keys().cloned());
        }
        // 从 wechat_add_product_payload.attrs 中获取
        if let Some(payload) = metadata
            .get("wechat_add_product_payload")
            .and_then(Value::as_object)
        {
            if let Some(attrs) = payload.get("attrs").and_then(Value::as_array) {
                for attr in attrs {
                    if let Some(key) = attr.as_object().and_then(|o| {
                        ["attr_key", "name", "attr_name", "key"]
                            .iter()
                            .find_map(|k| json_value_to_string(o.get(*k)))
                    }) {
                        keys.insert(key);
                    }
                }
            }
        }
    }
    keys
}

/// 无类目详情时，构建基础属性补齐计划（仅包含常见必填属性）
fn build_basic_attribute_fill_plan(
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    missing_attrs: &[&str],
) -> AttributeFillPlan {
    let mut plan = AttributeFillPlan::default();
    for attr_key in missing_attrs {
        let spec = CategoryRequiredAttr {
            key: attr_key.to_string(),
            options: Vec::new(),
            attr_type: Some("string".to_string()),
            append_allowed: true,
            related_options: Vec::new(),
        };
        plan.suggestions
            .push(build_product_attr_suggestion(item, product, &spec));
    }
    plan
}

/// 将基础属性建议写入 attr_suggestions map
fn apply_basic_attribute_suggestions(
    product: &ExternalProductInput,
    plan: &AttributeFillPlan,
    attr_suggestions: &mut serde_json::Map<String, Value>,
) {
    for suggestion in &plan.suggestions {
        if suggestion.applied {
            if let Some(value) = &suggestion.suggested_value {
                attr_suggestions.insert(suggestion.attr_key.clone(), Value::String(value.clone()));
            }
        }
    }
    // 保留已有的 ai_attr_suggestions
    if let Some(metadata) = product.metadata.as_object() {
        if let Some(existing) = metadata
            .get("ai_attr_suggestions")
            .and_then(Value::as_object)
        {
            for (key, value) in existing {
                if !attr_suggestions.contains_key(key) {
                    attr_suggestions.insert(key.clone(), value.clone());
                }
            }
        }
    }
}

async fn run_collection_attribute_ai(
    app: &AppHandle,
    config: &AiProviderConfig,
    task_id: &str,
    shop_id: &str,
    plan: &mut AttributeFillPlan,
) -> AppResult<i64> {
    let started = std::time::Instant::now();
    let input_snapshot = serde_json::json!({
        "task_id": task_id,
        "shop_id": shop_id,
        "suggestions": attribute_suggestions_json(&plan.suggestions)
    });
    let input_summary = format!(
        "采集属性补齐 task_id={} 缺失属性={}",
        task_id,
        plan.suggestions.len()
    );
    let run_id = {
        let conn = open_connection(app)?;
        create_agent_run_for_skill(
            &conn,
            &ATTRIBUTE_SUGGESTION_SKILL,
            "collection_attribute",
            "collection_task",
            task_id,
            Some(shop_id),
            Some(config),
            &input_summary,
            Some(&input_snapshot),
        )?
    };
    let generated = fill_attribute_plan_with_ai(app, config, plan).await?;
    let output = attribute_suggestions_json(&plan.suggestions);
    let conn = open_connection(app)?;
    finish_agent_run(
        &conn,
        &run_id,
        AgentRunFinish {
            status: if plan.can_auto_apply() {
                "succeeded"
            } else {
                "needs_review"
            },
            output: None,
            validated_output: Some(&output),
            tool_calls: None,
            decision: Some(if plan.can_auto_apply() {
                "attributes_filled"
            } else {
                "needs_review"
            }),
            error_code: None,
            error_summary: None,
            duration_ms: Some(started.elapsed().as_millis().min(i64::MAX as u128) as i64),
        },
    )?;
    Ok(generated)
}

fn apply_collection_attribute_plan_to_product(
    product: &mut ExternalProductInput,
    payload: &mut Value,
    plan: &AttributeFillPlan,
    attr_suggestions: &mut serde_json::Map<String, Value>,
) -> AppResult<()> {
    apply_attribute_fill_plan_to_payload(payload, plan)?;
    for suggestion in &plan.suggestions {
        if !suggestion.applied {
            continue;
        }
        if suggestion.attr_kind == "product" {
            if let Some(value) = suggestion.suggested_value.as_deref() {
                attr_suggestions.insert(
                    suggestion.attr_key.clone(),
                    Value::String(value.to_string()),
                );
            }
        } else if !suggestion.sku_values.is_empty() {
            for sku_value in &suggestion.sku_values {
                if let Some(sku) = product.skus.get_mut(sku_value.sku_index) {
                    ensure_collection_product_sku_spec(sku, &suggestion.attr_key, &sku_value.value);
                }
            }
        } else if let Some(value) = suggestion.suggested_value.as_deref() {
            for sku in &mut product.skus {
                ensure_collection_product_sku_spec(sku, &suggestion.attr_key, value);
            }
        }
    }
    Ok(())
}

fn promote_collection_ai_attribute_suggestions(plan: &mut AttributeFillPlan) {
    for suggestion in &mut plan.suggestions {
        if suggestion.applied || !suggestion.source.starts_with("ai_provider:") {
            continue;
        }
        if suggestion.confidence < 75 {
            continue;
        }
        if suggestion.suggested_value.is_none() && suggestion.sku_values.is_empty() {
            continue;
        }
        let allowed_values = collection_ai_suggestion_allowed_values(suggestion);
        if suggestion_values_allowed(suggestion, &allowed_values) {
            suggestion.applied = true;
        }
    }
}

fn collection_ai_suggestion_allowed_values(suggestion: &AttributeFillSuggestion) -> Vec<String> {
    let direct = response_allowed_values(&suggestion.prompt_json);
    if !direct.is_empty() {
        return direct;
    }
    suggestion
        .prompt_json
        .get("request")
        .map(response_allowed_values)
        .unwrap_or_default()
}

fn ensure_collection_product_sku_spec(
    sku: &mut crate::models::ExternalSkuInput,
    attr_key: &str,
    attr_value: &str,
) {
    match &mut sku.specs {
        Value::Object(object) => {
            object
                .entry(attr_key.to_string())
                .or_insert_with(|| Value::String(attr_value.to_string()));
        }
        Value::Array(items) => {
            let exists = items.iter().any(|item| {
                item.as_object()
                    .and_then(|object| {
                        ["attr_key", "name", "attr_name", "key"]
                            .iter()
                            .find_map(|key| json_value_to_string(object.get(*key)))
                    })
                    .as_deref()
                    == Some(attr_key)
            });
            if !exists {
                items.push(serde_json::json!({
                    "attr_key": attr_key,
                    "attr_value": attr_value
                }));
            }
        }
        _ => {
            sku.specs = serde_json::json!({
                attr_key: attr_value
            });
        }
    }
}

fn collection_unresolved_attr_message(suggestion: &AttributeFillSuggestion) -> String {
    let attr_kind = if suggestion.attr_kind == "product" {
        "商品"
    } else {
        "销售"
    };
    format!(
        "AI 暂未高置信补齐微信类目必填{attr_kind}属性「{}」，已保留到铺货属性补齐流程",
        suggestion.attr_key
    )
}

fn ensure_category_available_for_shop(
    conn: &rusqlite::Connection,
    shop_id: &str,
    leaf_cat_id: i64,
) -> AppResult<()> {
    let available_count = conn.query_row(
        "SELECT COUNT(*)
         FROM wechat_category_relations
         WHERE shop_id = ?1 AND cat_id = ?2 AND status = 1",
        params![shop_id, leaf_cat_id],
        |row| row.get::<_, i64>(0),
    )?;
    if available_count == 0 {
        return Err(AppError::Validation(format!(
            "微信类目 {leaf_cat_id} 不在目标店铺 {shop_id} 的生效类目权限内，请先同步店铺类目权限或选择已准入类目"
        )));
    }
    Ok(())
}

fn resolve_collection_review_category_shop(
    conn: &rusqlite::Connection,
    target_shop_ids: &[String],
) -> AppResult<Option<String>> {
    let normalized_target_shop_ids = target_shop_ids
        .iter()
        .map(|shop_id| shop_id.trim())
        .filter(|shop_id| !shop_id.is_empty())
        .collect::<Vec<_>>();
    for shop_id in &normalized_target_shop_ids {
        let count = conn.query_row(
            "SELECT COUNT(*) FROM wechat_category_relations WHERE shop_id = ?1 AND status = 1",
            [*shop_id],
            |row| row.get::<_, i64>(0),
        )?;
        if count > 0 {
            return Ok(Some((*shop_id).to_string()));
        }
    }
    if !normalized_target_shop_ids.is_empty() {
        return Ok(None);
    }
    // 未指定目标店（如「只采集」模式下的自动审查）：微信类目树是平台级的，
    // 任意一个已同步类目权限的店都能作为类目匹配数据源，确定性取第一个即可，
    // 不再因「多店有缓存」而报错拦截审查。店级类目准入由铺货阶段按目标店兜底校验。
    let category_shop_id = conn
        .query_row(
            "SELECT shop_id
             FROM wechat_category_relations
             WHERE status = 1
             GROUP BY shop_id
             ORDER BY shop_id ASC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(category_shop_id)
}

fn review_issue(kind: &str, severity: &str, message: &str) -> Value {
    serde_json::json!({
        "kind": kind,
        "severity": severity,
        "message": message
    })
}

fn collection_review_status_from_issues(issues: &[Value]) -> &'static str {
    if issues
        .iter()
        .any(|issue| issue.get("severity").and_then(Value::as_str) == Some("block"))
    {
        return "blocked";
    }
    if issues
        .iter()
        .any(|issue| issue.get("severity").and_then(Value::as_str) == Some("confirm"))
    {
        return "needs_review";
    }
    "passed"
}

fn collection_review_summary(
    status: &str,
    issues: &[Value],
    removed_images: &[Value],
    category_applied: bool,
) -> String {
    let first_issue = first_actionable_collection_issue_message(issues);
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
            let attr_hint_count = issues
                .iter()
                .filter(|issue| {
                    issue.get("kind").and_then(Value::as_str) == Some("attr")
                        && issue.get("severity").and_then(Value::as_str) == Some("info")
                })
                .count();
            if attr_hint_count > 0 {
                format!(
                    "审查通过，{image_text}，{category_text}，{attr_hint_count} 个属性待铺货补齐"
                )
            } else {
                format!("审查通过，{image_text}，{category_text}")
            }
        }
        "needs_review" => format!("需要人工确认：{first_issue}"),
        "blocked" => format!("已拦截：{first_issue}"),
        _ => "审查失败".to_string(),
    }
}

fn first_actionable_collection_issue_message(issues: &[Value]) -> &str {
    for severity in ["block", "confirm", "info"] {
        if let Some(message) = issues.iter().find_map(|issue| {
            (issue.get("severity").and_then(Value::as_str) == Some(severity))
                .then(|| issue.get("message").and_then(Value::as_str))
                .flatten()
        }) {
            return message;
        }
    }
    ""
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
    // 前两个候选(工作目录相对路径 / 资源目录)未命中时返回相对路径，
    // 由调用方拉起子进程时报"脚本不存在"，不再指向已不存在的他项目绝对路径。
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
        "/usr/bin/python3",
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

fn resolve_python_vendor_paths(app: &AppHandle) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("python-vendor");
        if candidate.exists() {
            paths.push(candidate);
        }
    }

    let local_candidate = PathBuf::from("runtime").join("python-vendor");
    if local_candidate.exists() {
        paths.push(local_candidate);
    }

    paths
}

fn python_command(app: &AppHandle) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(resolve_python_binary());
    let mut python_paths: Vec<String> = resolve_python_vendor_paths(app)
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    if let Ok(existing) = std::env::var("PYTHONPATH") {
        if !existing.trim().is_empty() {
            python_paths.push(existing);
        }
    }
    if !python_paths.is_empty() {
        let separator = if cfg!(windows) { ";" } else { ":" };
        command.env("PYTHONPATH", python_paths.join(separator));
    }
    command
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

    let mut cmd = python_command(app);
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
    if COLLECTOR_RUNNING.load(Ordering::SeqCst) {
        return Ok(serde_json::json!({
            "success": false,
            "error": "后台批量采集正在运行，淘宝采集浏览器 profile 已被占用。请等待采集结束后再测试抓取，或先停止/关闭当前采集窗口。",
            "raw": null,
            "stderr": ""
        }));
    }

    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    let script_path = resolve_collector_script(&app);

    let mut cmd = python_command(&app);
    cmd.env("WX_XD_TAOBAO_PROFILE_LOCK_TIMEOUT_SECONDS", "3");
    if headed.unwrap_or(false) {
        cmd.env("WX_XD_TAOBAO_IGNORE_CAPTCHA_FAILURE_COOLDOWN", "1");
    }
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
        let pending_tasks: Vec<CollectTask> = match get_all_pending_tasks(&app) {
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

/// 采集 worker 内部传递的轻量商品（来自 pipeline_products 主表）。
struct CollectTask {
    id: String,
    title: String,
    source_url: String,
    category_path: String,
}

fn get_all_pending_tasks(app: &AppHandle) -> AppResult<Vec<CollectTask>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT id, title, source_url, category_path
         FROM pipeline_products
         WHERE stage = 'collect' AND status IN ('pending_collect', 'collecting')
         ORDER BY created_at ASC",
    )?;
    let list = stmt
        .query_map([], |row| {
            Ok(CollectTask {
                id: row.get(0)?,
                title: row.get(1)?,
                source_url: row.get(2)?,
                category_path: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(list)
}

async fn run_batch_collection_tasks(app: &AppHandle, tasks: &[CollectTask]) -> AppResult<()> {
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
    let mut command = python_command(app);
    command
        .arg(&script_path)
        .arg("batch-collect")
        .arg("--urls")
        .arg(&urls_json)
        .arg("--profile-dir")
        .arg(&profile_dir_str);
    // 实测：淘宝对无头浏览器即便带有效登录态仍会触发短信验证，可见窗口则直接放行。
    // 故批量采集默认走 headed（复用 headed 登录的低风控通道）；
    // 仅排查时可设 WX_XD_TAOBAO_BATCH_HEADLESS=1 强制无头（大概率被风控拦截）。
    if std::env::var("WX_XD_TAOBAO_BATCH_HEADLESS").as_deref() != Ok("1") {
        command.arg("--headed");
    }
    let mut child = match command
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
    let task_map: std::collections::HashMap<&str, &CollectTask> =
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

        // 致命错误：仅标记尚未出结果的任务，避免覆盖已成功项。
        if result.get("fatal_error").is_some() {
            let err = result["fatal_error"].as_str().unwrap_or("未知致命错误");
            for task in tasks {
                if !handled_task_ids.contains(&task.id) {
                    let _ = update_task_status(
                        app,
                        &task.id,
                        "failed",
                        Some(format!("批量采集致命错误: {}", err)),
                        None,
                    );
                }
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

async fn run_single_collection_task(app: &AppHandle, task: &CollectTask) -> AppResult<bool> {
    // 1. 设置状态为 running
    update_task_status(app, &task.id, "running", None, None)?;

    // 2. 准备路径
    let app_data_dir = app.path().app_data_dir()?;
    let profile_dir = app_data_dir.join("taobao_profile");
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    let script_path = resolve_collector_script(app);

    // 3. 执行 Python 采集子进程
    let mut command = python_command(app);
    let output = match command
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
            main_video: None,
            skus: vec![crate::models::ExternalSkuInput {
                external_sku_id: "sku-1".to_string(),
                specs: serde_json::json!({ "颜色": "白色" }),
                cost_price: 10.0,
                stock: 10,
                sku_image: None,
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
    fn ai_review_low_overall_confidence_does_not_force_title_confirmation() {
        let mut product = demo_product();
        product.title = "2026 新款儿童短袖".to_string();
        let mut removed_images = Vec::new();
        let mut issues = Vec::new();
        let ai_result = serde_json::json!({
            "title": "2026 新款儿童短袖",
            "remove_image_urls": [],
            "category": null,
            "needs_human_review": true,
            "notes": ["类目候选为空"],
            "confidence": 30
        });

        apply_ai_collection_review(&mut product, &ai_result, &mut removed_images, &mut issues);

        assert_eq!(product.title, "2026 新款儿童短袖");
        assert!(removed_images.is_empty());
        assert!(issues.is_empty());
    }

    #[test]
    fn attr_info_does_not_force_collection_review_confirmation() {
        let issues = vec![review_issue(
            "attr",
            "info",
            "微信类目要求填写「安全等级」，可选值：A类、B类、C类、其他",
        )];

        assert_eq!(collection_review_status_from_issues(&issues), "passed");
        assert!(
            collection_review_summary("passed", &issues, &[], true).contains("1 个属性待铺货补齐")
        );
    }

    #[test]
    fn ai_image_info_does_not_force_collection_review_confirmation() {
        let issues = vec![review_issue(
            "image",
            "info",
            "AI 图文审查未完成，已改用本地图片规则过滤",
        )];

        assert_eq!(collection_review_status_from_issues(&issues), "passed");
    }

    #[test]
    fn collection_review_summary_prefers_confirm_issue() {
        let issues = vec![
            review_issue("image", "info", "AI 图文审查未完成，已改用本地图片规则过滤"),
            review_issue(
                "category",
                "confirm",
                "类目匹配置信度不足，需要从候选类目中确认",
            ),
        ];

        assert_eq!(
            collection_review_summary("needs_review", &issues, &[], false),
            "需要人工确认：类目匹配置信度不足，需要从候选类目中确认"
        );
    }

    #[test]
    fn collection_review_category_selects_exact_leaf_over_generic_tie() {
        let mut product = demo_product();
        product.title = "卡通印花儿童潮牌t恤短袖夏装中小童纯棉半袖".to_string();
        product.category_hint = Some("童装/婴儿装/亲子装>T恤".to_string());
        let candidates = vec![
            CollectionReviewCategoryCandidate {
                category_ids: vec![10000116, 10000123, 6216],
                category_path: "母婴 > 童装 > 亲子装".to_string(),
                score: 168,
                source: "local_category_cache_match".to_string(),
            },
            CollectionReviewCategoryCandidate {
                category_ids: vec![10000116, 10000123, 6215],
                category_path: "母婴 > 童装 > T恤".to_string(),
                score: 168,
                source: "local_category_cache_match".to_string(),
            },
        ];

        let selected = select_collection_review_category_candidate(&product, &candidates)
            .expect("应选择明确叶子类目");

        assert_eq!(selected.category_path, "母婴 > 童装 > T恤");
    }

    #[test]
    fn collection_review_category_selects_pants_from_title_broad_candidates() {
        let mut product = demo_product();
        product.title = "女小童薄款打底裤宝宝弹力紧身小脚裤长裤百搭潮".to_string();
        product.category_hint = Some("女装/女士精品>卫裤".to_string());
        let candidates = vec![
            CollectionReviewCategoryCandidate {
                category_ids: vec![10000116, 10000123, 6216],
                category_path: "母婴 > 童装 > 亲子装".to_string(),
                score: 48,
                source: "local_category_cache_broad".to_string(),
            },
            CollectionReviewCategoryCandidate {
                category_ids: vec![10000116, 10000123, 494041],
                category_path: "母婴 > 童装 > 休闲裤".to_string(),
                score: 48,
                source: "local_category_cache_broad".to_string(),
            },
        ];

        let selected = select_collection_review_category_candidate(&product, &candidates)
            .expect("应根据标题选择裤装类目");

        assert_eq!(selected.category_path, "母婴 > 童装 > 休闲裤");
    }

    #[test]
    fn real_confirm_issue_still_requires_manual_collection_review() {
        let issues = vec![review_issue(
            "category",
            "confirm",
            "类目匹配置信度不足，需要从候选类目中确认",
        )];

        assert_eq!(
            collection_review_status_from_issues(&issues),
            "needs_review"
        );
    }

    #[test]
    fn collection_review_payload_uses_existing_sku_specs() {
        let product = demo_product();
        let payload = collection_review_requirement_payload(&product);
        let sku_attrs = payload
            .get("skus")
            .and_then(Value::as_array)
            .and_then(|skus| skus.first())
            .and_then(|sku| sku.get("sku_attrs"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        assert!(sku_attrs.iter().any(|attr| {
            attr.get("attr_key").and_then(Value::as_str) == Some("颜色")
                && attr.get("attr_value").and_then(Value::as_str) == Some("白色")
        }));
    }

    #[test]
    fn applied_sale_attr_suggestion_updates_collection_sku_specs() {
        let mut product = demo_product();
        let mut payload = collection_review_requirement_payload(&product);
        let mut attr_suggestions = serde_json::Map::new();
        let plan = AttributeFillPlan {
            suggestions: vec![AttributeFillSuggestion {
                attr_kind: "sale",
                attr_key: "尺码".to_string(),
                suggested_value: Some("按 SKU 映射：110cm".to_string()),
                sku_values: vec![SkuAttrFill {
                    sku_index: 0,
                    value: "110cm".to_string(),
                }],
                confidence: 90,
                source: "ai_provider:test".to_string(),
                applied: true,
                prompt_json: Value::Null,
            }],
        };

        apply_collection_attribute_plan_to_product(
            &mut product,
            &mut payload,
            &plan,
            &mut attr_suggestions,
        )
        .expect("应能写回 SKU 规格");

        assert_eq!(
            product.skus[0].specs.get("尺码").and_then(Value::as_str),
            Some("110cm")
        );
    }

    #[test]
    fn collection_review_trusts_ai_suggestion_after_schema_check() {
        let mut plan = AttributeFillPlan {
            suggestions: vec![AttributeFillSuggestion {
                attr_kind: "product",
                attr_key: "风格".to_string(),
                suggested_value: Some("运动风".to_string()),
                sku_values: Vec::new(),
                confidence: 80,
                source: "ai_provider:test".to_string(),
                applied: false,
                prompt_json: serde_json::json!({
                    "request": {
                        "allowed_values": ["运动风", "可爱风"]
                    },
                    "response": {
                        "value": "运动风",
                        "confidence": 80,
                        "reason": "AI 根据标题和类目判断"
                    }
                }),
            }],
        };

        promote_collection_ai_attribute_suggestions(&mut plan);

        assert!(plan.suggestions[0].applied);
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
        assert!(rule_image_reject_reason(
            "https://gw.alicdn.com/imgextra/i4/O1CN012YkS1S20pKuSLCT05_!!6000000006898-0-tps-720-280.jpg"
        )
        .is_some());
        assert!(rule_image_reject_reason(
            "https://img.alicdn.com/imgextra/i2/O1CN01a69z6z_!!6000000004257-2-tps-174-106.png"
        )
        .is_some());
        assert!(rule_image_reject_reason(
            "https://img.alicdn.com/imgextra/i1/6000000008015/O1CN01A9_!!6000000008015-0-shopmanager.jpg"
        )
        .is_some());
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

    match status {
        "running" => {
            // 采集进行中；error_summary 此处承载进度文案
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'collecting', stage = 'collect', attention = 'none',
                     error_code = NULL, error_reason = NULL, progress_text = ?1, updated_at = ?2
                 WHERE id = ?3",
                params![error_summary, now, task_id],
            )?;
        }
        "success" => {
            // 采集成功 → 进入审查阶段，等审查 handler 处理。
            // 同时把采集结果中的 external_product_id（淘宝商品唯一链接）回填到列：
            // 该列此前恒为 NULL，铺货 loader 用 COALESCE(...,'') 会把所有商品归一成空串，
            // 导致第一个商品铺成功后，同店后续商品在 precheck 全部撞空串去重键 → 误判 DUPLICATE。
            // 回填真实唯一值后去重才正确；解析不到则存 NULL（空值不参与去重，见 precheck 防御）。
            let external_id = collected_data
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                .and_then(|value| {
                    value
                        .get("external_product_id")
                        .and_then(|field| field.as_str())
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                });
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'collecting', stage = 'review', attention = 'none',
                     collected_data = ?1, external_product_id = ?2,
                     reviewed_data = NULL, review_result_json = NULL,
                     error_code = NULL, error_reason = NULL, progress_text = '等待审查',
                     updated_at = ?3
                 WHERE id = ?4",
                params![collected_data, external_id, now, task_id],
            )?;
        }
        _ => {
            // 采集失败：区分淘宝风控冷却与一般失败
            let message = error_summary.unwrap_or_else(|| "采集失败".to_string());
            let code = if is_taobao_collection_protection_error(&message) {
                "COLLECT_ACCESS_LIMITED"
            } else {
                "COLLECT_FAILED"
            };
            conn.execute(
                "UPDATE pipeline_products
                 SET status = 'error', stage = 'collect', attention = 'error',
                     error_code = ?1, error_reason = ?2, progress_text = NULL, updated_at = ?3
                 WHERE id = ?4",
                params![code, message, now, task_id],
            )?;
        }
    }

    Ok(())
}
