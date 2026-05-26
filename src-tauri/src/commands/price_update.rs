use super::*;

#[tauri::command]
pub fn run_price_update_precheck_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PriceUpdatePrecheckBatchResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let items = load_pending_price_update_items(&conn, limit)?;
    if items.is_empty() {
        return Ok(PriceUpdatePrecheckBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            ready_items: 0,
            failed_items: 0,
        });
    }

    let job_ids = items
        .iter()
        .map(|item| item.job_id.clone())
        .collect::<BTreeSet<_>>();
    for job_id in &job_ids {
        conn.execute(
            "UPDATE task_runs SET status = 'running', started_at = COALESCE(started_at, ?1) WHERE id = ?2",
            params![now_shanghai(), job_id],
        )?;
        conn.execute(
            "UPDATE price_update_jobs SET status = 'running' WHERE id = ?1",
            [job_id],
        )?;
    }

    let mut ready_items = 0i64;
    let mut failed_items = 0i64;
    for item in &items {
        conn.execute(
            "UPDATE price_update_items SET status = 'prechecking', updated_at = ?1 WHERE id = ?2",
            params![now_shanghai(), item.item_id],
        )?;
        let failure = if item.shop_status != "active" {
            Some((
                "SHOP_NOT_ACTIVE",
                format!("店铺状态为 {}，不能改价", item.shop_status),
            ))
        } else if !item.shop_has_secret {
            Some((
                "SHOP_SECRET_MISSING",
                "店铺未保存 app_secret，不能改价".to_string(),
            ))
        } else if item
            .wechat_product_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            Some((
                "MISSING_WECHAT_PRODUCT_ID",
                "缺少微信 product_id，不能改价".to_string(),
            ))
        } else if item.target_price_cents <= 0 {
            Some(("PRICE_INVALID", "目标售价必须大于 0 分".to_string()))
        } else {
            None
        };

        if let Some((code, summary)) = failure {
            conn.execute(
                "UPDATE price_update_items
                 SET status = 'failed', error_code = ?1, error_summary = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![code, summary, now_shanghai(), item.item_id],
            )?;
            insert_task_log(
                &conn,
                &item.job_id,
                Some(&item.item_id),
                "error",
                &format!(
                    "商品 {} 改价前置校验失败：{}",
                    item.external_product_id, summary
                ),
                None,
            )?;
            failed_items += 1;
        } else {
            conn.execute(
                "UPDATE price_update_items
                 SET status = 'ready_to_update',
                     error_code = NULL,
                     error_summary = '本地改价校验通过，等待提交微信 updateproduct',
                     updated_at = ?1
                 WHERE id = ?2",
                params![now_shanghai(), item.item_id],
            )?;
            ready_items += 1;
        }
    }

    for job_id in &job_ids {
        recompute_price_update_job(&conn, job_id)?;
    }

    Ok(PriceUpdatePrecheckBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items: items.len() as i64,
        ready_items,
        failed_items,
    })
}

#[tauri::command]
pub async fn run_price_update_submit_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PriceUpdateSubmitBatchResult> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let submit_items = {
        let conn = open_connection(&app)?;
        load_ready_price_update_items(&conn, limit)?
    };
    if submit_items.is_empty() {
        return Ok(PriceUpdateSubmitBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            submitted_items: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = submit_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut submitted_items = 0i64;
    let mut failed_items = 0i64;

    for item in submit_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
            conn.execute(
                "UPDATE price_update_jobs SET status = 'running' WHERE id = ?1",
                [item.job_id.as_str()],
            )?;
            conn.execute(
                "UPDATE price_update_items
                 SET status = 'submitting',
                     error_code = NULL,
                     error_summary = '正在获取微信商品详情并提交 updateproduct',
                     updated_at = ?1
                 WHERE id = ?2",
                params![now_shanghai(), item.item_id.as_str()],
            )?;
        }
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始提交微信 updateproduct 改价",
            Some(&serde_json::json!({
                "wechat_product_id": item.wechat_product_id,
                "target_price_cents": item.target_price_cents
            })),
        )?;

        let product_id = match item.wechat_product_id.as_deref().map(str::trim) {
            Some(product_id) if !product_id.is_empty() => product_id.to_string(),
            _ => {
                mark_price_update_item_failed_for_app(
                    &app,
                    &item,
                    "MISSING_WECHAT_PRODUCT_ID",
                    "缺少微信 product_id，不能提交改价",
                )?;
                failed_items += 1;
                continue;
            }
        };

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                mark_price_update_item_failed_for_app(
                    &app,
                    &item,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let get_call = match client.get_product(&access_token, &product_id, 3).await {
            Ok(call) => call,
            Err(error) => {
                mark_price_update_item_failed_for_app(
                    &app,
                    &item,
                    "WECHAT_GETPRODUCT_HTTP_FAILED",
                    &format!("微信 getproduct 请求失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let payload = {
            let conn = open_connection(&app)?;
            match &get_call.result {
                WechatCallResult::Success(info) => {
                    insert_api_call_log(
                        &conn,
                        Some(&item.shop_id),
                        get_call.meta.endpoint,
                        get_call.meta.method,
                        "success",
                        None,
                        None,
                        Some(&format!(
                            "getproduct ok for price update, product_id={product_id}"
                        )),
                    )?;
                    match build_price_update_product_payload(
                        info,
                        &product_id,
                        item.target_price_cents,
                    ) {
                        Ok(payload) => payload,
                        Err(error) => {
                            drop(conn);
                            mark_price_update_item_failed_for_app(
                                &app,
                                &item,
                                "PRICE_UPDATE_PAYLOAD_INVALID",
                                &error,
                            )?;
                            failed_items += 1;
                            continue;
                        }
                    }
                }
                WechatCallResult::ApiError(error) => {
                    insert_api_call_log(
                        &conn,
                        Some(&item.shop_id),
                        get_call.meta.endpoint,
                        get_call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some("getproduct api error before price update"),
                    )?;
                    drop(conn);
                    mark_price_update_item_failed_for_app(
                        &app,
                        &item,
                        &format!("WECHAT_GETPRODUCT_{}", error.errcode),
                        &format!("微信 getproduct 失败：{}", error.errmsg),
                    )?;
                    failed_items += 1;
                    continue;
                }
            }
        };

        let update_call = match client.update_product(&access_token, &payload).await {
            Ok(call) => call,
            Err(error) => {
                mark_price_update_item_failed_for_app(
                    &app,
                    &item,
                    "WECHAT_UPDATEPRODUCT_HTTP_FAILED",
                    &format!("微信 updateproduct 请求失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &update_call.result {
            WechatCallResult::Success(_) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    update_call.meta.endpoint,
                    update_call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "updateproduct ok, product_id={product_id}, target_price_cents={}",
                        item.target_price_cents
                    )),
                )?;
                let now = now_shanghai();
                conn.execute(
                    "UPDATE price_update_items
                     SET status = 'submitted',
                         error_code = NULL,
                         error_summary = '微信 updateproduct 已提交，等待商品状态同步确认价格',
                         updated_at = ?1
                     WHERE id = ?2",
                    params![now, item.item_id],
                )?;
                conn.execute(
                    "UPDATE shop_products
                     SET last_price_update_at = ?1
                     WHERE shop_id = ?2 AND external_product_id = ?3",
                    params![now, item.shop_id, item.external_product_id],
                )?;
                insert_task_log(
                    &conn,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    "微信 updateproduct 已提交，等待后续状态同步确认",
                    Some(&serde_json::json!({
                        "wechat_product_id": product_id,
                        "target_price_cents": item.target_price_cents
                    })),
                )?;
                submitted_items += 1;
            }
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    update_call.meta.endpoint,
                    update_call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("updateproduct api error"),
                )?;
                drop(conn);
                mark_price_update_item_failed_for_app(
                    &app,
                    &item,
                    &format!("WECHAT_UPDATEPRODUCT_{}", error.errcode),
                    &format!("微信 updateproduct 失败：{}", error.errmsg),
                )?;
                failed_items += 1;
            }
        }
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_price_update_job(&conn, job_id)?;
    }

    Ok(PriceUpdateSubmitBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        submitted_items,
        failed_items,
    })
}

#[tauri::command]
pub async fn run_price_update_confirm_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PriceUpdateConfirmBatchResult> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let confirm_items = {
        let conn = open_connection(&app)?;
        load_confirmable_price_update_items(&conn, limit)?
    };
    if confirm_items.is_empty() {
        return Ok(PriceUpdateConfirmBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            confirmed_items: 0,
            pending_items: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = confirm_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut confirmed_items = 0i64;
    let mut pending_items = 0i64;
    let mut failed_items = 0i64;

    for item in confirm_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
            conn.execute(
                "UPDATE price_update_jobs SET status = 'running' WHERE id = ?1",
                [item.job_id.as_str()],
            )?;
            conn.execute(
                "UPDATE price_update_items
                 SET status = 'audit_pending',
                     error_code = NULL,
                     error_summary = '正在同步微信商品详情并确认线上 SKU 价格',
                     updated_at = ?1
                 WHERE id = ?2",
                params![now_shanghai(), item.item_id.as_str()],
            )?;
        }

        let product_id = match item.wechat_product_id.as_deref().map(str::trim) {
            Some(product_id) if !product_id.is_empty() => product_id.to_string(),
            _ => {
                mark_price_update_item_failed_for_app(
                    &app,
                    &item,
                    "MISSING_WECHAT_PRODUCT_ID",
                    "缺少微信 product_id，不能确认改价结果",
                )?;
                failed_items += 1;
                continue;
            }
        };

        if item.shop_status != "active" {
            mark_price_update_item_failed_for_app(
                &app,
                &item,
                "SHOP_NOT_ACTIVE",
                &format!("店铺状态为 {}，不能确认改价结果", item.shop_status),
            )?;
            failed_items += 1;
            continue;
        }
        if !item.shop_has_secret {
            mark_price_update_item_failed_for_app(
                &app,
                &item,
                "SHOP_SECRET_MISSING",
                "店铺未保存 app_secret，不能确认改价结果",
            )?;
            failed_items += 1;
            continue;
        }

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                set_price_update_item_pending_for_app(
                    &app,
                    &item,
                    Some("ACCESS_TOKEN_FAILED"),
                    &format!("获取 access_token 失败，稍后可重试：{error}"),
                )?;
                pending_items += 1;
                continue;
            }
        };

        let get_call = match client.get_product(&access_token, &product_id, 3).await {
            Ok(call) => call,
            Err(error) => {
                set_price_update_item_pending_for_app(
                    &app,
                    &item,
                    Some("WECHAT_GETPRODUCT_HTTP_FAILED"),
                    &format!("微信 getproduct 请求失败，稍后可重试：{error}"),
                )?;
                pending_items += 1;
                continue;
            }
        };

        match &get_call.result {
            WechatCallResult::Success(info) => {
                let conn = open_connection(&app)?;
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    get_call.meta.endpoint,
                    get_call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "getproduct ok for price confirm, product_id={product_id}"
                    )),
                )?;
                drop(conn);

                let resolution = resolve_price_update_confirmation(info, item.target_price_cents);
                match resolution.status {
                    "success" => {
                        set_price_update_item_confirmed_for_app(&app, &item, &resolution.summary)?;
                        insert_task_log_for_app(
                            &app,
                            &item.job_id,
                            Some(&item.item_id),
                            "info",
                            "线上 SKU 价格已确认",
                            Some(&serde_json::json!({
                                "wechat_product_id": product_id,
                                "target_price_cents": item.target_price_cents,
                                "wechat_status": resolution.wechat_status,
                                "wechat_edit_status": resolution.wechat_edit_status
                            })),
                        )?;
                        confirmed_items += 1;
                    }
                    "failed" => {
                        mark_price_update_item_failed_for_app(
                            &app,
                            &item,
                            resolution
                                .error_code
                                .as_deref()
                                .unwrap_or("PRICE_CONFIRM_FAILED"),
                            &resolution.summary,
                        )?;
                        failed_items += 1;
                    }
                    _ => {
                        set_price_update_item_pending_for_app(
                            &app,
                            &item,
                            resolution.error_code.as_deref(),
                            &resolution.summary,
                        )?;
                        pending_items += 1;
                    }
                }
            }
            WechatCallResult::ApiError(error) => {
                let conn = open_connection(&app)?;
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    get_call.meta.endpoint,
                    get_call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("getproduct api error during price confirm"),
                )?;
                drop(conn);
                set_price_update_item_pending_for_app(
                    &app,
                    &item,
                    Some(&format!("WECHAT_GETPRODUCT_{}", error.errcode)),
                    &format!("微信 getproduct 失败，稍后可重试：{}", error.errmsg),
                )?;
                pending_items += 1;
            }
        }
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_price_update_job(&conn, job_id)?;
    }

    Ok(PriceUpdateConfirmBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        confirmed_items,
        pending_items,
        failed_items,
    })
}
