use super::*;

#[derive(Debug)]
struct PendingOrderPriceAdjustmentItem {
    item_id: String,
    job_id: String,
    shop_id: String,
    shop_name: String,
    wechat_order_id: String,
    change_order_infos_json: String,
    change_express: bool,
    express_fee_cents: Option<i64>,
}

#[tauri::command]
pub fn create_order_price_adjustment_job(
    app: AppHandle,
    request: OrderPriceAdjustmentJobRequest,
) -> AppResult<OrderPriceAdjustmentJobCreated> {
    validate_order_price_adjustment_request(&request)?;
    let mut conn = open_connection(&app)?;
    let existing_request: Option<String> = conn
        .query_row(
            "SELECT id FROM order_price_adjustment_jobs WHERE request_id = ?1",
            [request.request_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if existing_request.is_some() {
        return Err(AppError::Validation(
            "request_id 已存在，不能重复创建订单改价任务".to_string(),
        ));
    }

    let tx = conn.transaction()?;
    let job_id = format!("order-price-{}", Uuid::new_v4().simple());
    let created_at = now_shanghai();
    let mut failed_items = 0usize;

    tx.execute(
        "INSERT INTO order_price_adjustment_jobs
         (id, request_id, status, accepted_order_count, created_at, updated_at)
         VALUES (?1, ?2, 'queued', ?3, ?4, ?4)",
        params![
            job_id,
            request.request_id,
            request.orders.len() as i64,
            created_at
        ],
    )?;
    tx.execute(
        "INSERT INTO task_runs (id, task_type, status, progress, created_at)
         VALUES (?1, 'orders.change_order_price', 'pending', 0, ?2)",
        params![job_id, created_at],
    )?;

    for order in &request.orders {
        let shop = tx
            .query_row(
                "SELECT s.name, s.status, CASE WHEN c.shop_id IS NULL THEN 0 ELSE 1 END
                 FROM shops s
                 LEFT JOIN shop_credentials c ON c.shop_id = s.id
                 WHERE s.id = ?1",
                [order.shop_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? == 1,
                    ))
                },
            )
            .optional()?;

        let local_order = tx
            .query_row(
                "SELECT id, wechat_status
                 FROM orders
                 WHERE shop_id = ?1 AND wechat_order_id = ?2",
                params![order.shop_id, order.wechat_order_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()?;

        let mut status = "pending";
        let mut error_code: Option<&str> = None;
        let mut error_summary: Option<String> = None;
        let shop_name: String;
        let mut order_id: Option<String> = None;
        let mut wechat_status: Option<i64> = None;

        match shop {
            None => {
                failed_items += 1;
                status = "failed";
                error_code = Some("SHOP_NOT_FOUND");
                error_summary = Some("目标店铺不存在".to_string());
                shop_name = order.shop_id.clone();
            }
            Some((name, shop_status, has_secret)) => {
                shop_name = name;
                if shop_status != "active" {
                    failed_items += 1;
                    status = "failed";
                    error_code = Some("SHOP_NOT_ACTIVE");
                    error_summary = Some("目标店铺不是 active 状态".to_string());
                } else if !has_secret {
                    failed_items += 1;
                    status = "failed";
                    error_code = Some("SHOP_SECRET_MISSING");
                    error_summary = Some("目标店铺缺少 app_secret，不能调用微信改价".to_string());
                } else if let Some((local_order_id, local_wechat_status)) = local_order {
                    order_id = Some(local_order_id.clone());
                    wechat_status = local_wechat_status;
                    if local_wechat_status != Some(10) {
                        failed_items += 1;
                        status = "failed";
                        error_code = Some("ORDER_NOT_UNPAID");
                        error_summary =
                            Some("本地订单状态不是待付款，请先同步待付款订单".to_string());
                    } else if let Some((code, summary)) =
                        validate_order_price_lines(&tx, &local_order_id, order)?
                    {
                        failed_items += 1;
                        status = "failed";
                        error_code = Some(code);
                        error_summary = Some(summary);
                    }
                } else {
                    failed_items += 1;
                    status = "failed";
                    error_code = Some("ORDER_NOT_SYNCED");
                    error_summary = Some("本地未找到该待付款订单，请先同步待付款订单".to_string());
                }
            }
        }

        tx.execute(
            "INSERT INTO order_price_adjustment_items
             (id, job_id, shop_id, shop_name, order_id, wechat_order_id, wechat_status,
              change_order_infos_json, change_express, express_fee_cents, note, status,
              error_code, error_summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                format!("order-price-item-{}", Uuid::new_v4()),
                job_id,
                order.shop_id,
                shop_name,
                order_id,
                order.wechat_order_id,
                wechat_status,
                serde_json::to_string(&order.lines).unwrap_or_else(|_| "[]".to_string()),
                if order.change_express { 1 } else { 0 },
                order.express_fee_cents,
                order.note,
                status,
                error_code,
                error_summary,
                created_at,
            ],
        )?;
    }

    let job_status = if failed_items == request.orders.len() {
        "failed"
    } else if failed_items > 0 {
        "partial_success"
    } else {
        "queued"
    };
    tx.execute(
        "UPDATE order_price_adjustment_jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![job_status, now_shanghai(), job_id],
    )?;
    tx.execute(
        "UPDATE task_runs SET status = ?1 WHERE id = ?2",
        params![job_status, job_id],
    )?;
    tx.commit()?;

    Ok(OrderPriceAdjustmentJobCreated {
        task_id: job_id,
        status: job_status.to_string(),
        accepted_order_count: request.orders.len(),
    })
}

#[tauri::command]
pub fn get_order_price_adjustment_job(
    app: AppHandle,
    task_id: String,
) -> AppResult<OrderPriceAdjustmentJobView> {
    let conn = open_connection(&app)?;
    let mut job = conn
        .query_row(
            "SELECT id, request_id, status, accepted_order_count, created_at, updated_at
             FROM order_price_adjustment_jobs WHERE id = ?1",
            [task_id.as_str()],
            |row| {
                Ok(OrderPriceAdjustmentJobView {
                    id: row.get(0)?,
                    request_id: row.get(1)?,
                    status: row.get(2)?,
                    accepted_order_count: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    items: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("订单改价任务不存在".to_string()))?;
    job.items = load_order_price_adjustment_items(&conn, &job.id)?;
    Ok(job)
}

#[tauri::command]
pub async fn run_order_price_adjustment_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<OrderPriceAdjustmentBatchResult> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let items = {
        let conn = open_connection(&app)?;
        load_pending_order_price_adjustment_items(&conn, limit)?
    };
    if items.is_empty() {
        return Ok(OrderPriceAdjustmentBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            success_items: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let mut processed_jobs = BTreeSet::new();
    let mut success_items = 0i64;
    let mut failed_items = 0i64;

    for item in items {
        processed_jobs.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE order_price_adjustment_jobs SET status = 'running', updated_at = ?1 WHERE id = ?2",
                params![now_shanghai(), item.job_id],
            )?;
            conn.execute(
                "UPDATE order_price_adjustment_items SET status = 'submitting', updated_at = ?1 WHERE id = ?2",
                params![now_shanghai(), item.item_id],
            )?;
        }

        let lines = match parse_order_price_adjustment_lines(&item.change_order_infos_json) {
            Ok(lines) => lines,
            Err(error) => {
                failed_items += 1;
                mark_order_price_adjustment_failed(
                    &app,
                    &item,
                    "INVALID_LINES",
                    &format!("订单改价商品行解析失败：{error}"),
                )?;
                continue;
            }
        };
        let change_infos = lines
            .into_iter()
            .map(|line| OrderPriceUpdateInfo {
                product_id: line.product_id,
                sku_id: line.sku_id,
                change_price: line.change_price_cents,
            })
            .collect::<Vec<_>>();

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_items += 1;
                mark_order_price_adjustment_failed(
                    &app,
                    &item,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                continue;
            }
        };

        let call = match client
            .change_order_price(
                &access_token,
                &item.wechat_order_id,
                &change_infos,
                item.change_express,
                item.express_fee_cents,
            )
            .await
        {
            Ok(call) => call,
            Err(error) => {
                failed_items += 1;
                mark_order_price_adjustment_failed(
                    &app,
                    &item,
                    "WECHAT_REQUEST_FAILED",
                    &format!("微信订单改价请求失败：{error}"),
                )?;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(_) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "changeorderprice ok, order_id={}",
                        item.wechat_order_id
                    )),
                )?;
                let now = now_shanghai();
                conn.execute(
                    "UPDATE order_price_adjustment_items
                     SET status = 'success',
                         error_code = NULL,
                         error_summary = '微信已接受未付款订单改价',
                         submitted_at = ?1,
                         updated_at = ?1
                     WHERE id = ?2",
                    params![now, item.item_id],
                )?;
                insert_task_log(
                    &conn,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    &format!("订单 {} 改价已提交微信", item.wechat_order_id),
                    Some(&serde_json::json!({
                        "shop_id": item.shop_id,
                        "wechat_order_id": item.wechat_order_id,
                        "line_count": change_infos.len(),
                        "change_express": item.change_express,
                        "has_express_fee": item.express_fee_cents.is_some()
                    })),
                )?;
                success_items += 1;
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
                    Some("changeorderprice api error"),
                )?;
                drop(conn);
                failed_items += 1;
                mark_order_price_adjustment_failed(
                    &app,
                    &item,
                    &error.errcode.to_string(),
                    &format!("微信订单改价失败：{}", error.errmsg),
                )?;
            }
        }
    }

    let conn = open_connection(&app)?;
    for job_id in &processed_jobs {
        recompute_order_price_adjustment_job(&conn, job_id)?;
    }

    Ok(OrderPriceAdjustmentBatchResult {
        processed_jobs: processed_jobs.len() as i64,
        processed_items: success_items + failed_items,
        success_items,
        failed_items,
    })
}

fn validate_order_price_adjustment_request(
    request: &OrderPriceAdjustmentJobRequest,
) -> AppResult<()> {
    if request.request_id.trim().is_empty() {
        return Err(AppError::Validation("request_id 不能为空".to_string()));
    }
    if request.orders.is_empty() {
        return Err(AppError::Validation("orders 不能为空".to_string()));
    }
    for order in &request.orders {
        if order.shop_id.trim().is_empty() {
            return Err(AppError::Validation("shop_id 不能为空".to_string()));
        }
        if order.wechat_order_id.trim().is_empty() {
            return Err(AppError::Validation("wechat_order_id 不能为空".to_string()));
        }
        if order.lines.is_empty() {
            return Err(AppError::Validation(format!(
                "订单 {} 至少需要一个商品改价行",
                order.wechat_order_id
            )));
        }
        if order.change_express && order.express_fee_cents.unwrap_or(0) < 0 {
            return Err(AppError::Validation(format!(
                "订单 {} 的运费不能小于 0",
                order.wechat_order_id
            )));
        }
        for line in &order.lines {
            if line.product_id.trim().is_empty() || line.sku_id.trim().is_empty() {
                return Err(AppError::Validation(format!(
                    "订单 {} 的 product_id 和 sku_id 不能为空",
                    order.wechat_order_id
                )));
            }
            if line.change_price_cents <= 0 {
                return Err(AppError::Validation(format!(
                    "订单 {} 的商品目标总价必须大于 0 分",
                    order.wechat_order_id
                )));
            }
            if line.change_price_cents > 10_000_000 {
                return Err(AppError::Validation(format!(
                    "订单 {} 的商品目标总价超过安全上限",
                    order.wechat_order_id
                )));
            }
        }
    }
    Ok(())
}

fn validate_order_price_lines(
    tx: &rusqlite::Transaction<'_>,
    local_order_id: &str,
    order: &OrderPriceAdjustmentOrderInput,
) -> AppResult<Option<(&'static str, String)>> {
    let item_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM order_items WHERE order_id = ?1",
        [local_order_id],
        |row| row.get(0),
    )?;
    if item_count == 0 {
        return Ok(Some((
            "ORDER_DETAIL_NOT_SYNCED",
            "本地订单缺少商品明细，请先同步订单详情".to_string(),
        )));
    }

    for line in &order.lines {
        let current_price = tx
            .query_row(
                "SELECT sale_price, real_price
                 FROM order_items
                 WHERE order_id = ?1 AND wechat_product_id = ?2 AND wechat_sku_id = ?3
                 LIMIT 1",
                params![local_order_id, line.product_id, line.sku_id],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()?;
        let Some((sale_price, real_price)) = current_price else {
            return Ok(Some((
                "ORDER_ITEM_NOT_FOUND",
                format!(
                    "订单内未找到商品 {} / SKU {}，请先同步订单详情",
                    line.product_id, line.sku_id
                ),
            )));
        };
        if let Some(reference_price) = real_price.or(sale_price) {
            if line.change_price_cents > reference_price {
                return Ok(Some((
                    "ORDER_PRICE_CAN_ONLY_DECREASE",
                    format!(
                        "商品 {} / SKU {} 目标总价高于当前订单价，微信只支持改低",
                        line.product_id, line.sku_id
                    ),
                )));
            }
        }
    }
    Ok(None)
}

fn load_order_price_adjustment_items(
    conn: &Connection,
    job_id: &str,
) -> AppResult<Vec<OrderPriceAdjustmentItemView>> {
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, shop_name, order_id, wechat_order_id, wechat_status,
                change_order_infos_json, change_express, express_fee_cents, note,
                status, error_code, error_summary, created_at, updated_at, submitted_at
         FROM order_price_adjustment_items
         WHERE job_id = ?1
         ORDER BY created_at ASC",
    )?;
    let items = stmt
        .query_map([job_id], |row| {
            let lines_json: String = row.get(6)?;
            Ok(OrderPriceAdjustmentItemView {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                shop_name: row.get(2)?,
                order_id: row.get(3)?,
                wechat_order_id: row.get(4)?,
                wechat_status: row.get(5)?,
                change_order_infos: parse_order_price_adjustment_lines(&lines_json)
                    .unwrap_or_default(),
                change_express: row.get::<_, i64>(7)? == 1,
                express_fee_cents: row.get(8)?,
                note: row.get(9)?,
                status: row.get(10)?,
                error_code: row.get(11)?,
                error_summary: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
                submitted_at: row.get(15)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

fn load_pending_order_price_adjustment_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingOrderPriceAdjustmentItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, job_id, shop_id, shop_name, wechat_order_id,
                change_order_infos_json, change_express, express_fee_cents
         FROM order_price_adjustment_items
         WHERE status = 'pending'
         ORDER BY created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingOrderPriceAdjustmentItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                shop_id: row.get(2)?,
                shop_name: row.get(3)?,
                wechat_order_id: row.get(4)?,
                change_order_infos_json: row.get(5)?,
                change_express: row.get::<_, i64>(6)? == 1,
                express_fee_cents: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

fn parse_order_price_adjustment_lines(
    value: &str,
) -> AppResult<Vec<OrderPriceAdjustmentLineInput>> {
    serde_json::from_str(value)
        .map_err(|error| AppError::Validation(format!("订单改价商品行 JSON 不合法：{error}")))
}

fn mark_order_price_adjustment_failed(
    app: &AppHandle,
    item: &PendingOrderPriceAdjustmentItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE order_price_adjustment_items
         SET status = 'failed', error_code = ?1, error_summary = ?2, updated_at = ?3
         WHERE id = ?4",
        params![error_code, error_summary, now_shanghai(), item.item_id],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "error",
        &format!(
            "店铺 {} 订单 {} 改价失败：{error_summary}",
            item.shop_name, item.wechat_order_id
        ),
        Some(&serde_json::json!({
            "error_code": error_code,
            "shop_id": item.shop_id,
            "wechat_order_id": item.wechat_order_id
        })),
    )?;
    upsert_notification(
        &conn,
        "critical",
        "order_price_adjustment",
        &item.item_id,
        Some(&item.shop_id),
        "订单改价失败",
        &format!("订单 {} 改价失败：{}", item.wechat_order_id, error_summary),
        Some(&serde_json::json!({
            "job_id": item.job_id,
            "item_id": item.item_id,
            "shop_id": item.shop_id,
            "wechat_order_id": item.wechat_order_id,
            "error_code": error_code
        })),
    )?;
    Ok(())
}

fn recompute_order_price_adjustment_job(conn: &Connection, job_id: &str) -> AppResult<()> {
    let (total, pending, success, failed): (i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*),
                SUM(CASE WHEN status IN ('pending', 'submitting') THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END)
         FROM order_price_adjustment_items
         WHERE job_id = ?1",
        [job_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            ))
        },
    )?;
    let status = if total == 0 || failed == total {
        "failed"
    } else if success == total {
        "success"
    } else if pending > 0 && success == 0 && failed == 0 {
        "queued"
    } else {
        "partial_success"
    };
    let progress = if total == 0 {
        0
    } else {
        ((success + failed) * 100 / total).clamp(0, 100)
    };
    let now = now_shanghai();
    conn.execute(
        "UPDATE order_price_adjustment_jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, job_id],
    )?;
    conn.execute(
        "UPDATE task_runs
         SET status = ?1,
             progress = ?2,
             pending_count = ?3,
             ready_count = 0,
             failed_count = ?4,
             finished_at = CASE WHEN ?3 = 0 THEN ?5 ELSE finished_at END
         WHERE id = ?6",
        params![status, progress, pending, failed, now, job_id],
    )?;
    Ok(())
}
