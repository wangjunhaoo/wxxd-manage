use super::*;

#[tauri::command]
pub async fn run_order_sync_once(
    app: AppHandle,
    lookback_days: Option<i64>,
    page_size: Option<i64>,
) -> AppResult<OrderSyncBatchResult> {
    let lookback_days = lookback_days.unwrap_or(1).clamp(1, 7);
    let page_size = page_size.unwrap_or(100).clamp(1, 100);
    let sync_shops = {
        let conn = open_connection(&app)?;
        load_order_sync_shops(&conn)?
    };
    let task_id = format!("order-sync-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'orders.sync_shop_orders', 'running', 0, ?2, ?2)",
            params![task_id, created_at],
        )?;
    }

    if sync_shops.is_empty() {
        let conn = open_connection(&app)?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "没有可同步订单的 active 店铺",
            None,
        )?;
        conn.execute(
            "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
            params![now_shanghai(), task_id],
        )?;
        return Ok(OrderSyncBatchResult {
            task_id,
            processed_shops: 0,
            synced_orders: 0,
            failed_shops: 0,
        });
    }

    let client = WechatShopClient::default();
    let end_time = Utc::now().timestamp();
    let start_time = (Utc::now() - Duration::days(lookback_days)).timestamp();
    let mut processed_shops = 0i64;
    let mut synced_orders = 0i64;
    let mut failed_shops = 0i64;

    for shop in sync_shops {
        processed_shops += 1;
        let access_token = match ensure_access_token(&app, &shop.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_shops += 1;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(&shop.shop_id),
                    "error",
                    &format!("店铺 {} 获取 access_token 失败：{error}", shop.shop_name),
                    None,
                )?;
                continue;
            }
        };

        let mut next_key = String::new();
        let mut page_count = 0;
        loop {
            page_count += 1;
            let call = match client
                .get_order_list(
                    &access_token,
                    start_time,
                    end_time,
                    Some(20),
                    page_size,
                    &next_key,
                )
                .await
            {
                Ok(call) => call,
                Err(error) => {
                    failed_shops += 1;
                    insert_task_log_for_app(
                        &app,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 获取订单列表请求失败：{error}", shop.shop_name),
                        None,
                    )?;
                    break;
                }
            };

            let conn = open_connection(&app)?;
            match &call.result {
                WechatCallResult::Success(result) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "success",
                        None,
                        None,
                        Some(&format!(
                            "order list ok, count={}, has_more={}",
                            result.order_id_list.len(),
                            result.has_more
                        )),
                    )?;
                    for order_id in &result.order_id_list {
                        save_synced_order(&conn, &shop.shop_id, order_id, 20)?;
                        synced_orders += 1;
                    }
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "info",
                        &format!(
                            "店铺 {} 同步待发货订单 {} 个",
                            shop.shop_name,
                            result.order_id_list.len()
                        ),
                        Some(&serde_json::json!({
                            "lookback_days": lookback_days,
                            "page": page_count,
                            "has_more": result.has_more
                        })),
                    )?;
                    if !result.has_more || page_count >= 5 {
                        break;
                    }
                    next_key = result.next_key.clone().unwrap_or_default();
                    if next_key.trim().is_empty() {
                        break;
                    }
                }
                WechatCallResult::ApiError(error) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some("order list api error"),
                    )?;
                    failed_shops += 1;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 订单列表同步失败：{}", shop.shop_name, error.errmsg),
                        Some(&serde_json::json!({
                            "errcode": error.errcode
                        })),
                    )?;
                    upsert_notification(
                        &conn,
                        "critical",
                        "aftersale_sync",
                        &shop.shop_id,
                        Some(&shop.shop_id),
                        "店铺售后列表同步失败",
                        &format!("店铺 {} 售后列表同步失败：{}", shop.shop_name, error.errmsg),
                        Some(&serde_json::json!({
                            "shop_id": &shop.shop_id,
                            "errcode": error.errcode
                        })),
                    )?;
                    break;
                }
            }
        }
    }

    let final_status = if failed_shops == 0 {
        "success"
    } else if failed_shops == processed_shops {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;

    Ok(OrderSyncBatchResult {
        task_id,
        processed_shops,
        synced_orders,
        failed_shops,
    })
}

#[tauri::command]
pub async fn run_order_detail_sync_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<OrderDetailSyncBatchResult> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let detail_items = {
        let conn = open_connection(&app)?;
        load_order_detail_sync_items(&conn, limit)?
    };
    let task_id = format!("order-detail-sync-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'orders.sync_order_details', 'running', 0, ?2, ?2)",
            params![task_id, created_at],
        )?;
    }

    if detail_items.is_empty() {
        let conn = open_connection(&app)?;
        insert_task_log(&conn, &task_id, None, "info", "没有待同步详情的订单", None)?;
        conn.execute(
            "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
            params![now_shanghai(), task_id],
        )?;
        return Ok(OrderDetailSyncBatchResult {
            task_id,
            processed_orders: 0,
            synced_orders: 0,
            created_items: 0,
            failed_orders: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_orders = detail_items.len() as i64;
    let mut synced_orders = 0i64;
    let mut created_items = 0i64;
    let mut failed_orders = 0i64;

    for item in detail_items {
        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_orders += 1;
                mark_order_detail_failed(&app, &item, &format!("获取 access_token 失败：{error}"))?;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(&item.order_id),
                    "error",
                    &format!(
                        "订单 {} 获取 access_token 失败：{error}",
                        item.wechat_order_id
                    ),
                    None,
                )?;
                continue;
            }
        };

        let call = match client.get_order(&access_token, &item.wechat_order_id).await {
            Ok(call) => call,
            Err(error) => {
                failed_orders += 1;
                mark_order_detail_failed(&app, &item, &format!("微信 getorder 请求失败：{error}"))?;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(&item.order_id),
                    "error",
                    &format!("订单 {} 详情请求失败：{error}", item.wechat_order_id),
                    None,
                )?;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(result) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!("getorder ok, order_id={}", item.wechat_order_id)),
                )?;
                let item_count = save_order_detail(&conn, &item, &result.order)?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&item.order_id),
                    "info",
                    &format!(
                        "订单 {} 详情已同步，订单项 {} 个",
                        item.wechat_order_id, item_count
                    ),
                    Some(&serde_json::json!({
                        "wechat_order_id": item.wechat_order_id
                    })),
                )?;
                synced_orders += 1;
                created_items += item_count;
            }
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("getorder api error"),
                )?;
                drop(conn);
                failed_orders += 1;
                mark_order_detail_failed(
                    &app,
                    &item,
                    &format!("微信 getorder 失败：{}", error.errmsg),
                )?;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(&item.order_id),
                    "error",
                    &format!(
                        "订单 {} 详情同步失败：{}",
                        item.wechat_order_id, error.errmsg
                    ),
                    Some(&serde_json::json!({
                        "errcode": error.errcode
                    })),
                )?;
            }
        }
    }

    let final_status = if failed_orders == 0 {
        "success"
    } else if failed_orders == processed_orders {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;

    Ok(OrderDetailSyncBatchResult {
        task_id,
        processed_orders,
        synced_orders,
        created_items,
        failed_orders,
    })
}
