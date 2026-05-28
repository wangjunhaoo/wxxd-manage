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

    if settings.publish_precheck_enabled {
        match run_publish_tasks_once(app.clone(), Some(50)) {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.precheck_products".to_string());
                result.publish_precheck = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "publish.precheck_products", error),
        }
    } else {
        result
            .skipped_steps
            .push("publish.precheck_products".to_string());
    }

    if settings.publish_attribute_fill_enabled {
        match run_publish_ai_attribute_suggestions_once(app.clone(), Some(20)).await {
            Ok(ai_result) => match run_publish_attribute_fill_once(app.clone(), Some(50)) {
                Ok(rule_result) => {
                    let step_result = merge_attribute_fill_results(ai_result, rule_result);
                    result
                        .executed_steps
                        .push("publish.fill_required_attributes".to_string());
                    result.publish_attribute_fill = Some(step_result);
                }
                Err(error) => {
                    push_automation_error(&mut result, "publish.fill_required_attributes", error)
                }
            },
            Err(error) => {
                result.errors.push(AutomationStepError {
                    step: "publish.fill_required_attributes.ai".to_string(),
                    error: error.to_string(),
                });
            }
        }
    } else {
        result
            .skipped_steps
            .push("publish.fill_required_attributes".to_string());
    }

    if settings.publish_category_precheck_enabled {
        match run_publish_category_prechecks_once(app.clone(), Some(20)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.category_precheck".to_string());
                result.publish_category_precheck = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "publish.category_precheck", error),
        }
    } else {
        result
            .skipped_steps
            .push("publish.category_precheck".to_string());
    }

    if settings.publish_asset_upload_enabled {
        match run_publish_asset_uploads_once(app.clone(), Some(10)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.upload_assets".to_string());
                result.publish_asset_upload = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "publish.upload_assets", error),
        }
    } else {
        result
            .skipped_steps
            .push("publish.upload_assets".to_string());
    }

    if settings.publish_submit_enabled {
        match run_publish_submits_once(app.clone(), Some(10)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.submit_products".to_string());
                result.publish_submit = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "publish.submit_products", error),
        }
    } else {
        result
            .skipped_steps
            .push("publish.submit_products".to_string());
    }

    if settings.publish_status_sync_enabled {
        match run_publish_status_sync_once(app.clone(), Some(20)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.sync_status".to_string());
                result.publish_status_sync = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "publish.sync_status", error),
        }
    } else {
        result.skipped_steps.push("publish.sync_status".to_string());
    }

    if settings.publish_listing_enabled {
        match run_publish_listing_once(app.clone(), Some(10)).await {
            Ok(step_result) => {
                result
                    .executed_steps
                    .push("publish.listing_products".to_string());
                result.publish_listing = Some(step_result);
            }
            Err(error) => push_automation_error(&mut result, "publish.listing_products", error),
        }
    } else {
        result
            .skipped_steps
            .push("publish.listing_products".to_string());
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
