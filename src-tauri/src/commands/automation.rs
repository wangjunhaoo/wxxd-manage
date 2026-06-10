use super::*;

#[tauri::command]
pub fn list_task_runs(app: AppHandle) -> AppResult<Vec<TaskRunView>> {
    let conn = open_connection(&app)?;
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.task_type,
           t.status,
           t.progress,
           t.created_at,
           t.started_at,
           t.finished_at,
           (
             SELECT COUNT(*) FROM price_update_items p
             WHERE p.job_id = t.id
               AND p.status IN ('pending', 'prechecking', 'submitting', 'submitted', 'audit_pending')
           ) AS pending_count,
           (
             SELECT COUNT(*) FROM price_update_items p
             WHERE p.job_id = t.id AND p.status = 'ready_to_update'
           ) AS ready_count,
           (
             SELECT COUNT(*) FROM price_update_items p
             WHERE p.job_id = t.id AND p.status = 'failed'
           ) AS failed_count
         FROM task_runs t
         ORDER BY t.created_at DESC
         LIMIT 80",
    )?;
    let tasks = stmt
        .query_map([], |row| {
            Ok(TaskRunView {
                id: row.get(0)?,
                task_type: row.get(1)?,
                status: row.get(2)?,
                progress: row.get(3)?,
                created_at: row.get(4)?,
                started_at: row.get(5)?,
                finished_at: row.get(6)?,
                pending_count: row.get(7)?,
                ready_count: row.get(8)?,
                failed_count: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

#[tauri::command]
pub fn get_automation_settings(app: AppHandle) -> AppResult<OperationalAutomationSettings> {
    let conn = open_connection(&app)?;
    Ok(load_automation_settings(&conn)?)
}

#[tauri::command]
pub fn set_automation_settings(
    app: AppHandle,
    settings: OperationalAutomationSettings,
) -> AppResult<OperationalAutomationSettings> {
    let conn = open_connection(&app)?;
    save_automation_settings(&conn, &settings)?;
    Ok(settings)
}

/// 铺货 7 个阶段的步骤名（与 run_publish_pipeline_steps 内执行顺序一致），
/// 供「铺货自动化总开关」关闭时整体记入 skipped_steps，保持前端计数语义。
const PUBLISH_PIPELINE_STEPS: [&str; 7] = [
    "publish.precheck_products",
    "publish.fill_required_attributes",
    "publish.category_precheck",
    "publish.upload_assets",
    "publish.submit_products",
    "publish.listing_products",
    "publish.sync_status",
];

#[tauri::command]
pub async fn run_publish_pipeline_once(app: AppHandle) -> AppResult<PublishPipelineRunResult> {
    Ok(run_publish_pipeline_steps(app).await)
}

/// 给单个铺货阶段套独立超时预算后执行。
///
/// 背景：铺货 7 个阶段串行 await，早期阶段（尤其 asset_upload 的图片下载/上传、submit 的微信 API）
/// 一旦因网络慢/死链 hang 住，会把排在它后面的 submit/listing/status_sync 全部饿死，直到外层
/// 180s tick 整体 abort——已就绪可上架的商品因此迟迟卡在 submit。
///
/// 本辅助给每个阶段套独立超时：某阶段卡顿只消耗自己的预算，超时则记一条错误并跳过本轮，
/// 不阻塞后续阶段，下一轮 tick 继续推进。空闲阶段（loader 无数据）瞬间返回、不占预算，
/// 故预算只在真有积压/卡顿时才生效。返回 Some(结果) 表示阶段正常完成，None 表示出错或超时。
async fn run_publish_stage_within<T>(
    result: &mut PublishPipelineRunResult,
    step: &str,
    budget_secs: u64,
    fut: impl std::future::Future<Output = AppResult<T>>,
) -> Option<T> {
    match tokio::time::timeout(std::time::Duration::from_secs(budget_secs), fut).await {
        Ok(Ok(value)) => Some(value),
        Ok(Err(error)) => {
            push_publish_pipeline_error(result, step, error);
            None
        }
        Err(_) => {
            eprintln!("[driver] 铺货阶段「{step}」超时 {budget_secs}s（本轮跳过，下一轮继续，不阻塞后续阶段）");
            result.errors.push(AutomationStepError {
                step: step.to_string(),
                error: format!("阶段超时 {budget_secs}s，已跳过本轮（不阻塞后续阶段，下一轮继续推进）"),
            });
            None
        }
    }
}

async fn run_publish_pipeline_steps(app: AppHandle) -> PublishPipelineRunResult {
    let mut result = PublishPipelineRunResult {
        executed_steps: Vec::new(),
        skipped_steps: Vec::new(),
        errors: Vec::new(),
        publish_precheck: None,
        publish_attribute_fill: None,
        publish_category_precheck: None,
        publish_asset_upload: None,
        publish_submit: None,
        publish_status_sync: None,
        publish_listing: None,
    };

    match run_publish_tasks_once(app.clone(), Some(50)) {
        Ok(step_result) => {
            result
                .executed_steps
                .push("publish.precheck_products".to_string());
            result.publish_precheck = Some(step_result);
        }
        Err(error) => push_publish_pipeline_error(&mut result, "publish.precheck_products", error),
    }

    // AI 补属性走子进程，给 40s 预算；超时/出错则本轮跳过，不阻塞后续阶段。
    if let Some(ai_result) = run_publish_stage_within(
        &mut result,
        "publish.fill_required_attributes.ai",
        40,
        run_publish_ai_attribute_suggestions_once(app.clone(), Some(20)),
    )
    .await
    {
        match run_publish_attribute_fill_once(app.clone(), Some(50)) {
            Ok(rule_result) => {
                let step_result = merge_attribute_fill_results(ai_result, rule_result);
                result
                    .executed_steps
                    .push("publish.fill_required_attributes".to_string());
                result.publish_attribute_fill = Some(step_result);
            }
            Err(error) => {
                push_publish_pipeline_error(&mut result, "publish.fill_required_attributes", error)
            }
        }
    }

    if let Some(step_result) = run_publish_stage_within(
        &mut result,
        "publish.category_precheck",
        25,
        run_publish_category_prechecks_once(app.clone(), Some(20)),
    )
    .await
    {
        result
            .executed_steps
            .push("publish.category_precheck".to_string());
        result.publish_category_precheck = Some(step_result);
    }

    // 图片下载/上传是最易 hang 的阶段（淘宝源图慢/死链 + 微信上传），给 40s 预算硬限；
    // 超时只跳过本轮、卡顿项留在 running 下轮重试，绝不再拖垮后面的 submit/listing。
    if let Some(step_result) = run_publish_stage_within(
        &mut result,
        "publish.upload_assets",
        40,
        run_publish_asset_uploads_once(app.clone(), Some(10)),
    )
    .await
    {
        result
            .executed_steps
            .push("publish.upload_assets".to_string());
        result.publish_asset_upload = Some(step_result);
    }

    if let Some(step_result) = run_publish_stage_within(
        &mut result,
        "publish.submit_products",
        35,
        run_publish_submits_once(app.clone(), Some(10)),
    )
    .await
    {
        result
            .executed_steps
            .push("publish.submit_products".to_string());
        result.publish_submit = Some(step_result);
    }

    // 方案A 正确顺序：submit(add 草稿) → listing(listingproduct 提交上架触发审核)
    // → status_sync(audit 轮询审核/上架结果)。listing 必须排在 status_sync 之前，
    // 否则商品停在草稿态(status=0)、审核轮询永远等不到结果而死锁。
    if let Some(step_result) = run_publish_stage_within(
        &mut result,
        "publish.listing_products",
        20,
        run_publish_listing_once(app.clone(), Some(10)),
    )
    .await
    {
        result
            .executed_steps
            .push("publish.listing_products".to_string());
        result.publish_listing = Some(step_result);
    }

    if let Some(step_result) = run_publish_stage_within(
        &mut result,
        "publish.sync_status",
        20,
        run_publish_status_sync_once(app.clone(), Some(20)),
    )
    .await
    {
        result
            .executed_steps
            .push("publish.sync_status".to_string());
        result.publish_status_sync = Some(step_result);
    }

    result
}

/// 后台流水线 driver：单 worker tick 循环，自动把每个商品从采集一路推进到上架。
/// 采集 worker 内部用 COLLECTOR_RUNNING 防重入；审查/铺货每轮处理一批；
/// 需人工的商品(need_confirm)与冷却中的采集会被各自阶段自动跳过，等待人工或重试。
pub fn start_pipeline_driver(app: AppHandle) {
    // spawn 任务的 panic 默认被 JoinHandle 吞掉、不打印到日志（这是之前 driver
    // "悄无声息卡死"极难定位的根因）。装一个 panic hook 打印 panic 位置与信息。
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("🔥 任务 panic: {info}");
        default_hook(info);
    }));
    tauri::async_runtime::spawn(async move {
        loop {
            let app_tick = app.clone();
            // 单轮 tick 隔离在子任务里：① 子任务 panic 经 JoinError 捕获，不会杀死 driver 主循环
            //（单个商品的坏数据不应让全部商品停摆）；② 外层 timeout 防止某步外部调用
            //（AI/微信/淘宝）无限 hang 卡死整个 driver。
            let handle = tauri::async_runtime::spawn(async move {
                drive_pipeline_once(&app_tick).await;
            });
            match tokio::time::timeout(std::time::Duration::from_secs(180), handle).await {
                Ok(Ok(())) => {}
                Ok(Err(join_err)) => {
                    eprintln!("⚠️ driver tick 异常退出（已隔离，继续下一轮）：{join_err}");
                }
                Err(_) => {
                    eprintln!("⚠️ driver tick 超时 180s（已跳过本轮，继续下一轮）");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        }
    });
}

async fn drive_pipeline_once(app: &AppHandle) {
    // 阶段顺序刻意把「铺货」排在「审查」之前：审查走 AI 子进程（每个商品约 20~30s），
    // 一旦 collecting 积压，单跳审查就会吃满 180s tick 预算，把无需 AI 的铺货链
    // （submit→audit→listing，纯微信 API 调用）彻底饿死，导致已就绪商品迟迟无法上架。
    // 故：先采集 → 退避扫描 → 优先铺货（自动赢家链）→ 最后用剩余预算做审查（limit 收紧）。

    // 1. 采集：拉起 stage=collect 的商品（trigger 内部防重入，不会重复拉起）
    trigger_collection_worker(app.clone());

    // 2. 退避扫描：把到期的可自动重试 blocked target 置回 pending，让 loader 能重新捞取
    //    (reactivate 内部用 requeue_target 清退避时间并 recompute 受影响商品级状态)
    if let Ok(conn) = open_connection(app) {
        if let Err(error) = reactivate_retriable_blocked_targets(&conn) {
            eprintln!("流水线 driver 退避扫描出错：{error}");
        }
    }

    // 3. 铺货（优先）：7 个阶段顺序推进一批（precheck→属性→类目预检→传图→提交→上架→审核同步）。
    //    受「铺货自动化」总开关控制：关闭时 driver 跳过铺货（采集/审查照常），便于风控冷却等场景暂停发品。
    let publish_enabled = open_connection(app)
        .and_then(|conn| load_automation_settings(&conn))
        .map(|settings| settings.publish_enabled)
        .unwrap_or(true);
    if publish_enabled {
        eprintln!("[driver] tick: 铺货阶段");
        let _ = run_publish_pipeline_steps(app.clone()).await;
    } else {
        eprintln!("[driver] tick: 铺货自动化已关闭，跳过");
    }

    // 4. 审查（次之）：处理 stage=review 的商品，高置信自动通过并激活各店 target 进入铺货。
    //    limit 从 20 收紧到 8——单跳审查 ≤8 个 AI 调用可在预算内完成，避免 180s 超时把铺货挤掉；
    //    积压会在后续 tick 持续消化（8s 一跳），不影响最终收敛，只是分摊到多跳。
    eprintln!("[driver] tick: 审查阶段");
    if let Err(error) = run_collection_review_once(
        app.clone(),
        CollectionReviewRunRequest {
            task_ids: Vec::new(),
            target_shop_ids: Vec::new(),
            limit: Some(8),
        },
    )
    .await
    {
        eprintln!("流水线 driver 审查阶段出错：{error}");
    }
    eprintln!("[driver] tick: 本轮完成");
}

fn push_publish_pipeline_error(result: &mut PublishPipelineRunResult, step: &str, error: AppError) {
    result.errors.push(AutomationStepError {
        step: step.to_string(),
        error: error.to_string(),
    });
}

fn apply_publish_pipeline_result(
    result: &mut OperationalAutomationRunResult,
    publish: PublishPipelineRunResult,
) {
    result.executed_steps.extend(publish.executed_steps);
    result.skipped_steps.extend(publish.skipped_steps);
    result.errors.extend(publish.errors);
    result.publish_precheck = publish.publish_precheck;
    result.publish_attribute_fill = publish.publish_attribute_fill;
    result.publish_category_precheck = publish.publish_category_precheck;
    result.publish_asset_upload = publish.publish_asset_upload;
    result.publish_submit = publish.publish_submit;
    result.publish_status_sync = publish.publish_status_sync;
    result.publish_listing = publish.publish_listing;
}

#[tauri::command]
pub async fn run_operational_automation_once(
    app: AppHandle,
) -> AppResult<OperationalAutomationRunResult> {
    let settings = {
        let conn = open_connection(&app)?;
        load_automation_settings(&conn)?
    };
    let mut result = OperationalAutomationRunResult {
        executed_steps: Vec::new(),
        skipped_steps: Vec::new(),
        errors: Vec::new(),
        order_sync: None,
        order_detail_sync: None,
        aftersale_sync: None,
        purchase_task_generation: None,
        delivery_submission: None,
        publish_precheck: None,
        publish_attribute_fill: None,
        publish_category_precheck: None,
        publish_asset_upload: None,
        publish_submit: None,
        publish_status_sync: None,
        publish_listing: None,
        price_confirm: None,
    };

    if settings.order_sync_enabled {
        match run_order_sync_once(app.clone(), Some(1), Some(100), None).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("orders.sync_shop_orders".to_string());
                result.order_sync = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "orders.sync_shop_orders", error),
        }
    } else {
        result
            .skipped_steps
            .push("orders.sync_shop_orders".to_string());
    }

    if settings.order_detail_sync_enabled {
        match run_order_detail_sync_once(app.clone(), Some(50)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("orders.sync_order_details".to_string());
                result.order_detail_sync = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "orders.sync_order_details", error),
        }
    } else {
        result
            .skipped_steps
            .push("orders.sync_order_details".to_string());
    }

    if settings.aftersale_sync_enabled {
        match run_aftersale_sync_once(app.clone(), Some(24), Some(200)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("aftersales.sync_shop_aftersales".to_string());
                result.aftersale_sync = Some(step_result);
            }
            Err(error) => {
                push_automation_error(&mut result, "aftersales.sync_shop_aftersales", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("aftersales.sync_shop_aftersales".to_string());
    }

    if settings.purchase_task_enabled {
        match run_purchase_task_generation_once(app.clone(), Some(100)) {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("procurement.create_purchase_tasks".to_string());
                result.purchase_task_generation = Some(step_result);
            }
            Err(error) => {
                push_automation_error(&mut result, "procurement.create_purchase_tasks", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("procurement.create_purchase_tasks".to_string());
    }

    if settings.delivery_submission_enabled {
        match run_delivery_submission_once(app.clone(), Some(20)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("delivery.submit_wechat_shipment".to_string());
                result.delivery_submission = Some(step_result);
            }
            Err(error) => {
                push_automation_error(&mut result, "delivery.submit_wechat_shipment", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("delivery.submit_wechat_shipment".to_string());
    }

    // 铺货 7 阶段作为一个整体开关：开则全链路推进，关则整体记入 skipped 保持前端计数语义。
    if settings.publish_enabled {
        let publish_result = run_publish_pipeline_steps(app.clone()).await;
        apply_publish_pipeline_result(&mut result, publish_result);
    } else {
        result
            .skipped_steps
            .extend(PUBLISH_PIPELINE_STEPS.iter().map(|s| s.to_string()));
    }

    if settings.price_confirm_enabled {
        match run_price_update_confirm_once(app.clone(), Some(50)).await {
            Ok(step_result) => {
                result.executed_steps.push("price.confirm".to_string());
                result.price_confirm = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "price.confirm", error),
        }
    } else {
        result.skipped_steps.push("price.confirm".to_string());
    }

    Ok(result)
}
