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
             SELECT COUNT(*) FROM publish_job_items i
             WHERE i.job_id = t.id
               AND i.status IN ('pending', 'prechecking', 'category_prechecking', 'asset_uploading', 'publishing', 'listing', 'running', 'submitted', 'audit_pending')
           ) + (
             SELECT COUNT(*) FROM price_update_items p
             WHERE p.job_id = t.id
               AND p.status IN ('pending', 'prechecking', 'submitting', 'submitted', 'audit_pending')
           ) AS pending_count,
           (
             SELECT COUNT(*) FROM publish_job_items i
             WHERE i.job_id = t.id AND i.status IN ('ready_to_publish', 'category_prechecked', 'assets_ready')
           ) + (
             SELECT COUNT(*) FROM price_update_items p
             WHERE p.job_id = t.id AND p.status = 'ready_to_update'
           ) AS ready_count,
           (
             SELECT COUNT(*) FROM publish_job_items i
             WHERE i.job_id = t.id AND i.status = 'failed'
           ) + (
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

#[derive(Debug, Clone, Copy)]
struct PublishPipelineStepSwitches {
    precheck: bool,
    attribute_fill: bool,
    category_precheck: bool,
    asset_upload: bool,
    submit: bool,
    status_sync: bool,
    listing: bool,
}

impl PublishPipelineStepSwitches {
    fn all_enabled() -> Self {
        Self {
            precheck: true,
            attribute_fill: true,
            category_precheck: true,
            asset_upload: true,
            submit: true,
            status_sync: true,
            listing: true,
        }
    }

    fn from_automation_settings(settings: &OperationalAutomationSettings) -> Self {
        Self {
            precheck: settings.publish_precheck_enabled,
            attribute_fill: settings.publish_attribute_fill_enabled,
            category_precheck: settings.publish_category_precheck_enabled,
            asset_upload: settings.publish_asset_upload_enabled,
            submit: settings.publish_submit_enabled,
            status_sync: settings.publish_status_sync_enabled,
            listing: settings.publish_listing_enabled,
        }
    }
}

#[tauri::command]
pub async fn run_publish_pipeline_once(app: AppHandle) -> AppResult<PublishPipelineRunResult> {
    Ok(run_publish_pipeline_steps(app, PublishPipelineStepSwitches::all_enabled()).await)
}

async fn run_publish_pipeline_steps(
    app: AppHandle,
    switches: PublishPipelineStepSwitches,
) -> PublishPipelineRunResult {
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

    if switches.precheck {
        match run_publish_tasks_once(app.clone(), Some(50)) {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.precheck_products".to_string());
                result.publish_precheck = Some(step_result);
            }
            Err(error) => {
                push_publish_pipeline_error(&mut result, "publish.precheck_products", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("publish.precheck_products".to_string());
    }

    if switches.attribute_fill {
        match run_publish_ai_attribute_suggestions_once(app.clone(), Some(20)).await {
            Ok(ai_result) => match run_publish_attribute_fill_once(app.clone(), Some(50)) {
                Ok(rule_result) => {
                    let step_result = merge_attribute_fill_results(ai_result, rule_result);
                    result
                        .executed_steps
                        .push("publish.fill_required_attributes".to_string());
                    result.publish_attribute_fill = Some(step_result);
                }
                Err(error) => push_publish_pipeline_error(
                    &mut result,
                    "publish.fill_required_attributes",
                    error,
                ),
            },
            Err(error) => push_publish_pipeline_error(
                &mut result,
                "publish.fill_required_attributes.ai",
                error,
            ),
        }
    } else {
        result
            .skipped_steps
            .push("publish.fill_required_attributes".to_string());
    }

    if switches.category_precheck {
        match run_publish_category_prechecks_once(app.clone(), Some(20)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.category_precheck".to_string());
                result.publish_category_precheck = Some(step_result);
            }
            Err(error) => {
                push_publish_pipeline_error(&mut result, "publish.category_precheck", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("publish.category_precheck".to_string());
    }

    if switches.asset_upload {
        match run_publish_asset_uploads_once(app.clone(), Some(10)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.upload_assets".to_string());
                result.publish_asset_upload = Some(step_result);
            }
            Err(error) => push_publish_pipeline_error(&mut result, "publish.upload_assets", error),
        }
    } else {
        result
            .skipped_steps
            .push("publish.upload_assets".to_string());
    }

    if switches.submit {
        match run_publish_submits_once(app.clone(), Some(10)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.submit_products".to_string());
                result.publish_submit = Some(step_result);
            }
            Err(error) => {
                push_publish_pipeline_error(&mut result, "publish.submit_products", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("publish.submit_products".to_string());
    }

    if switches.status_sync {
        match run_publish_status_sync_once(app.clone(), Some(20)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.sync_status".to_string());
                result.publish_status_sync = Some(step_result);
            }
            Err(error) => push_publish_pipeline_error(&mut result, "publish.sync_status", error),
        }
    } else {
        result.skipped_steps.push("publish.sync_status".to_string());
    }

    if switches.listing {
        match run_publish_listing_once(app.clone(), Some(10)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.listing_products".to_string());
                result.publish_listing = Some(step_result);
            }
            Err(error) => {
                push_publish_pipeline_error(&mut result, "publish.listing_products", error)
            }
        }
    } else {
        result
            .skipped_steps
            .push("publish.listing_products".to_string());
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
    // 1. 采集：拉起 stage=collect 的商品（trigger 内部防重入，不会重复拉起）
    trigger_collection_worker(app.clone());
    // 2. 审查：处理 stage=review 的商品，高置信自动通过并激活各店 target 进入铺货
    eprintln!("[driver] tick: 审查阶段");
    if let Err(error) = run_collection_review_once(
        app.clone(),
        CollectionReviewRunRequest {
            task_ids: Vec::new(),
            target_shop_ids: Vec::new(),
            limit: Some(20),
        },
    )
    .await
    {
        eprintln!("流水线 driver 审查阶段出错：{error}");
    }
    // 3. 退避扫描：把到期的可自动重试 blocked target 置回 pending，让 loader 能重新捞取
    //    (reactivate 内部用 requeue_target 清退避时间并 recompute 受影响商品级状态)
    if let Ok(conn) = open_connection(app) {
        if let Err(error) = reactivate_retriable_blocked_targets(&conn) {
            eprintln!("流水线 driver 退避扫描出错：{error}");
        }
    }
    // 4. 铺货：7 个阶段顺序推进一批（precheck→属性→类目预检→传图→提交→审核同步→上架）
    eprintln!("[driver] tick: 铺货阶段");
    let _ =
        run_publish_pipeline_steps(app.clone(), PublishPipelineStepSwitches::all_enabled()).await;
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

    let publish_result = run_publish_pipeline_steps(
        app.clone(),
        PublishPipelineStepSwitches::from_automation_settings(&settings),
    )
    .await;
    apply_publish_pipeline_result(&mut result, publish_result);

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
