use super::*;

/// 每店增量同步水位的 setting key（值为 unix 秒字符串：该店 update_time ≤ 水位的订单已全部入库）
fn order_sync_watermark_key(shop_id: &str) -> String {
    format!("orders.sync_watermark.{shop_id}")
}

/// 水位回看重叠：容忍微信侧写入延迟/时钟漂移，宁可重拉（skeleton upsert 幂等）不可漏单
const SYNC_WATERMARK_OVERLAP_SECS: i64 = 300;
/// 增量窗口右边界回退：避开 update_time 正在持续写入的最新边缘
const SYNC_WINDOW_RIGHT_MARGIN_SECS: i64 = 60;
/// 官方硬约束：单时间窗跨度 ≤7 天
const SYNC_WINDOW_MAX_SPAN_SECS: i64 = 7 * 86_400;
/// 每店每轮最多消化的子窗口数（31042/翻页溢出对半切后的总预算，防极端数据量拖死单轮）
const SYNC_MAX_SUBWINDOWS_PER_SHOP: i64 = 8;
/// 每个子窗口最多翻页数（沿用旧实现钳制；官方 606006 要求翻页期间参数冻结，本实现天然满足）
const SYNC_MAX_PAGES_PER_WINDOW: i64 = 5;
/// 对半切的最小窗口粒度：低于该跨度仍溢出时接受截断并告警（10 分钟同店 >500 单变更属极端异常）
const SYNC_MIN_SPLIT_SPAN_SECS: i64 = 600;

/// 单个时间窗的翻页同步结果
enum WindowSyncOutcome {
    /// 整窗翻页完成
    Done { synced: i64 },
    /// 结果集过大：官方 31042「请求内订单过多」或翻满页数仍 has_more，需要对半切窗
    NeedsSplit { synced: i64 },
    /// 请求/接口失败，本店本轮中止
    Failed { synced: i64, error: String },
}

/// 翻一个时间窗的订单列表并落库（skeleton upsert，不写状态——见 save_synced_order）。
#[allow(clippy::too_many_arguments)]
async fn sync_order_list_window(
    app: &AppHandle,
    client: &WechatShopClient,
    shop: &OrderSyncShop,
    access_token: &str,
    time_field: OrderListTimeField,
    order_status: Option<i64>,
    start_time: i64,
    end_time: i64,
    page_size: i64,
) -> AppResult<WindowSyncOutcome> {
    let mut next_key = String::new();
    let mut page_count = 0i64;
    let mut synced = 0i64;
    loop {
        page_count += 1;
        let call = match client
            .get_order_list(
                access_token,
                time_field,
                start_time,
                end_time,
                order_status,
                page_size,
                &next_key,
            )
            .await
        {
            Ok(call) => call,
            Err(error) => {
                return Ok(WindowSyncOutcome::Failed {
                    synced,
                    error: format!("订单列表请求失败：{error}"),
                })
            }
        };
        let conn = open_connection(app)?;
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
                        "order list ok, count={}, has_more={}, window=[{start_time},{end_time}]",
                        result.order_id_list.len(),
                        result.has_more
                    )),
                )?;
                for order_id in &result.order_id_list {
                    save_synced_order(&conn, &shop.shop_id, order_id)?;
                    synced += 1;
                }
                if !result.has_more {
                    return Ok(WindowSyncOutcome::Done { synced });
                }
                if page_count >= SYNC_MAX_PAGES_PER_WINDOW {
                    return Ok(WindowSyncOutcome::NeedsSplit { synced });
                }
                next_key = result.next_key.clone().unwrap_or_default();
                if next_key.trim().is_empty() {
                    return Ok(WindowSyncOutcome::Done { synced });
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
                // 31042：请求内订单过多，官方建议缩短时间范围 → 上抛切窗
                if error.errcode == 31042 {
                    return Ok(WindowSyncOutcome::NeedsSplit { synced });
                }
                return Ok(WindowSyncOutcome::Failed {
                    synced,
                    error: format!("微信返回错误 {}：{}", error.errcode, error.errmsg),
                });
            }
        }
    }
}

/// 单店同步一个大窗口 [start, end]：用「对半切」工作栈消化 31042/翻页溢出，
/// 子窗按时间顺序处理、整窗完成才推进 completed_until（不丢页）。
/// 返回 (synced, completed_until, warnings, hard_error)。
#[allow(clippy::too_many_arguments)]
async fn sync_shop_window_driven(
    app: &AppHandle,
    client: &WechatShopClient,
    shop: &OrderSyncShop,
    access_token: &str,
    time_field: OrderListTimeField,
    order_status: Option<i64>,
    start: i64,
    end: i64,
    page_size: i64,
) -> AppResult<(i64, i64, Vec<String>, Option<String>)> {
    let mut synced_total = 0i64;
    let mut completed_until = start;
    let mut warnings: Vec<String> = Vec::new();
    let mut budget = SYNC_MAX_SUBWINDOWS_PER_SHOP;
    // LIFO 栈按时间顺序弹出：切窗时后推右半、再推左半
    let mut stack: Vec<(i64, i64)> = vec![(start, end)];
    while let Some((ws, we)) = stack.pop() {
        if budget <= 0 {
            warnings.push(format!(
                "子窗口预算耗尽，窗口 [{ws}, {we}] 留待下一轮继续（水位不前进，不丢单）"
            ));
            return Ok((synced_total, completed_until, warnings, None));
        }
        budget -= 1;
        match sync_order_list_window(
            app,
            client,
            shop,
            access_token,
            time_field,
            order_status,
            ws,
            we,
            page_size,
        )
        .await?
        {
            WindowSyncOutcome::Done { synced } => {
                synced_total += synced;
                completed_until = we;
            }
            WindowSyncOutcome::NeedsSplit { synced } => {
                synced_total += synced;
                if we - ws <= SYNC_MIN_SPLIT_SPAN_SECS {
                    // 极端密度：最小粒度窗口仍溢出，接受截断推进并告警，避免水位永久卡死
                    warnings.push(format!(
                        "窗口 [{ws}, {we}] 在最小切分粒度下订单仍超量，已截断推进（可能有漏单，请用兜底补漏模式核对）"
                    ));
                    completed_until = we;
                } else {
                    let mid = ws + (we - ws) / 2;
                    stack.push((mid, we));
                    stack.push((ws, mid));
                }
            }
            WindowSyncOutcome::Failed { synced, error } => {
                synced_total += synced;
                return Ok((synced_total, completed_until, warnings, Some(error)));
            }
        }
    }
    Ok((synced_total, completed_until, warnings, None))
}

/// 订单列表同步（订单履约重设计 §3.1/§3.2）。两种模式：
/// - **增量主轴**（不传 order_status）：按店水位 + update_time_range、不传 status 拉全状态变化
///   （改价/取消/完成/发货都触达 update_time），整窗翻完才推进水位；
/// - **兜底/专项**（传 order_status）：create_time_range + status 过滤，保留「同步待付款(10)」
///   按钮语义，每日 status=20 补漏也走此路径（对冲「不传 status 行为未明」的文档风险）。
/// 命中订单一律只做 skeleton upsert + detail_dirty=1，真实状态由详情回刷经守卫落定。
#[tauri::command]
pub async fn run_order_sync_once(
    app: AppHandle,
    lookback_days: Option<i64>,
    page_size: Option<i64>,
    order_status: Option<i64>,
) -> AppResult<OrderSyncBatchResult> {
    let lookback_days = lookback_days.unwrap_or(1).clamp(1, 7);
    let page_size = page_size.unwrap_or(100).clamp(1, 100);
    let incremental = order_status.is_none();
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

        let now_ts = Utc::now().timestamp();
        let right_edge = now_ts - SYNC_WINDOW_RIGHT_MARGIN_SECS;
        let mut shop_failed = false;
        let mut shop_synced = 0i64;

        if incremental {
            // ===== 增量主轴：update_time + 每店水位，>7 天切片（每轮最多 2 片）=====
            let watermark_key = order_sync_watermark_key(&shop.shop_id);
            let watermark = {
                let conn = open_connection(&app)?;
                get_string_setting(&conn, &watermark_key)?.and_then(|v| v.parse::<i64>().ok())
            };
            let mut window_start = match watermark {
                Some(value) => (value - SYNC_WATERMARK_OVERLAP_SECS).max(0),
                None => now_ts - lookback_days * 86_400,
            };
            for _ in 0..2 {
                if window_start >= right_edge {
                    break;
                }
                let slice_end = (window_start + SYNC_WINDOW_MAX_SPAN_SECS).min(right_edge);
                let (synced, completed_until, warnings, hard_error) = sync_shop_window_driven(
                    &app,
                    &client,
                    &shop,
                    &access_token,
                    OrderListTimeField::Update,
                    None,
                    window_start,
                    slice_end,
                    page_size,
                )
                .await?;
                shop_synced += synced;
                {
                    let conn = open_connection(&app)?;
                    for warning in &warnings {
                        insert_task_log(
                            &conn,
                            &task_id,
                            Some(&shop.shop_id),
                            "warning",
                            &format!("店铺 {} 增量同步：{warning}", shop.shop_name),
                            None,
                        )?;
                    }
                    // 整窗（或其已完成前缀）落库成功才推进水位——不丢页
                    if completed_until > window_start {
                        set_string_setting(&conn, &watermark_key, &completed_until.to_string())?;
                    }
                    if let Some(error) = &hard_error {
                        insert_task_log(
                            &conn,
                            &task_id,
                            Some(&shop.shop_id),
                            "error",
                            &format!("店铺 {} 订单增量同步失败：{error}", shop.shop_name),
                            None,
                        )?;
                        upsert_notification(
                            &conn,
                            "critical",
                            "order_sync",
                            &shop.shop_id,
                            Some(&shop.shop_id),
                            "店铺订单列表同步失败",
                            &format!("店铺 {} 订单列表同步失败：{error}", shop.shop_name),
                            Some(&serde_json::json!({
                                "shop_id": &shop.shop_id
                            })),
                        )?;
                    }
                }
                if hard_error.is_some() {
                    shop_failed = true;
                    break;
                }
                if completed_until < slice_end {
                    // 本片没消化完（预算耗尽），下一轮从水位续跑
                    break;
                }
                window_start = completed_until;
            }
        } else {
            // ===== 兜底/专项模式：create_time 窗口 + status 过滤（不动水位）=====
            let status = order_status;
            let start_time = now_ts - lookback_days * 86_400;
            let (synced, _completed_until, warnings, hard_error) = sync_shop_window_driven(
                &app,
                &client,
                &shop,
                &access_token,
                OrderListTimeField::Create,
                status,
                start_time,
                now_ts,
                page_size,
            )
            .await?;
            shop_synced += synced;
            let conn = open_connection(&app)?;
            for warning in &warnings {
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&shop.shop_id),
                    "warning",
                    &format!("店铺 {} 兜底同步：{warning}", shop.shop_name),
                    None,
                )?;
            }
            if let Some(error) = &hard_error {
                shop_failed = true;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&shop.shop_id),
                    "error",
                    &format!("店铺 {} 订单列表同步失败：{error}", shop.shop_name),
                    None,
                )?;
                upsert_notification(
                    &conn,
                    "critical",
                    "order_sync",
                    &shop.shop_id,
                    Some(&shop.shop_id),
                    "店铺订单列表同步失败",
                    &format!("店铺 {} 订单列表同步失败：{error}", shop.shop_name),
                    Some(&serde_json::json!({
                        "shop_id": &shop.shop_id
                    })),
                )?;
            }
        }

        synced_orders += shop_synced;
        if shop_failed {
            failed_shops += 1;
        }
        let conn = open_connection(&app)?;
        insert_task_log(
            &conn,
            &task_id,
            Some(&shop.shop_id),
            "info",
            &format!(
                "店铺 {} {}同步命中订单 {} 个",
                shop.shop_name,
                if incremental { "增量" } else { "兜底" },
                shop_synced
            ),
            Some(&serde_json::json!({
                "mode": if incremental { "incremental" } else { "backfill" },
                "order_status": order_status,
                "lookback_days": lookback_days
            })),
        )?;
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
