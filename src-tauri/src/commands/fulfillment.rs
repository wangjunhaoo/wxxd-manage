use super::*;

// ============================================================================
// 订单履约二期（docs/order-fulfillment-redesign.md §5/§6/§7）：
// 申请收件箱（改址/换SKU）扫描与裁决、收货地址解密、采购双节点。
// ============================================================================

/// 从微信原始 payload 中提取订单号数组（兼容文档与示例的字段名分歧：order_id_list / orders）
fn extract_order_ids(payload: &Value) -> Vec<String> {
    ["order_id_list", "order_ids", "orders"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|value| json_value_to_string(Some(value)))
        .collect()
}

/// upsert 申请收件箱记录。返回 true 表示「新出现的待处理申请」（新建或终态被重新激活），
/// 调用方据此发通知；pending 中的既有申请只刷新 payload/updated_at 不重复通知。
fn upsert_order_request(
    conn: &Connection,
    shop_id: &str,
    order_id: Option<&str>,
    wechat_order_id: &str,
    kind: &str,
    payload: Option<&Value>,
) -> AppResult<bool> {
    let now = now_shanghai();
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, state FROM order_requests
             WHERE shop_id = ?1 AND wechat_order_id = ?2 AND kind = ?3",
            params![shop_id, wechat_order_id, kind],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match existing {
        None => {
            conn.execute(
                "INSERT INTO order_requests
                 (id, shop_id, order_id, wechat_order_id, kind, state, payload_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?7)",
                params![
                    format!("order-request-{}", Uuid::new_v4()),
                    shop_id,
                    order_id,
                    wechat_order_id,
                    kind,
                    payload.map(|value| value.to_string()),
                    now
                ],
            )?;
            Ok(true)
        }
        Some((id, state)) => {
            let reactivated = state != "pending";
            conn.execute(
                "UPDATE order_requests
                 SET state = 'pending',
                     order_id = COALESCE(?1, order_id),
                     payload_json = COALESCE(?2, payload_json),
                     resolution = CASE WHEN ?3 THEN NULL ELSE resolution END,
                     resolved_at = CASE WHEN ?3 THEN NULL ELSE resolved_at END,
                     deadline_at = CASE WHEN ?3 THEN NULL ELSE deadline_at END,
                     updated_at = ?4
                 WHERE id = ?5",
                params![
                    order_id,
                    payload.map(|value| value.to_string()),
                    reactivated,
                    now,
                    id
                ],
            )?;
            Ok(reactivated)
        }
    }
}

fn request_kind_title(kind: &str) -> &'static str {
    match kind {
        "address_change" => "买家改址申请待处理",
        "sku_change" => "发货前换SKU申请待处理",
        _ => "订单申请待处理",
    }
}

fn notify_new_request(
    conn: &Connection,
    shop_id: &str,
    wechat_order_id: &str,
    kind: &str,
    has_purchase_task: bool,
) -> AppResult<()> {
    let extra = if kind == "address_change" {
        if has_purchase_task {
            "注意：该单已生成采购任务，同意改址可能导致错发，建议拒绝并与买家协商。12 小时不处理将被系统自动同意！"
        } else {
            "注意：12 小时不处理将被系统自动同意。"
        }
    } else {
        "超过截止时间将被系统自动拒绝，请尽快裁决。"
    };
    upsert_notification(
        conn,
        "critical",
        "order_request",
        &format!("{kind}:{shop_id}:{wechat_order_id}"),
        Some(shop_id),
        request_kind_title(kind),
        &format!("订单 {wechat_order_id} 有{}。{extra}", request_kind_title(kind)),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "wechat_order_id": wechat_order_id,
            "kind": kind,
            "has_purchase_task": has_purchase_task
        })),
    )?;
    Ok(())
}

/// 申请专项扫描（订单履约重设计 §3.5）：
/// ① preshipmentchangesku/get 批量发现待处理换SKU申请；
/// ② order/search {address_under_review:true} 批量发现改址待审单（官方唯一批量入口）；
/// ③ 本地 reconcile：按订单详情镜像列回填 deadline、解决已被外部定性的申请、临期升级通知、
///    过期强制置脏回刷定性。命中订单一律 skeleton upsert + detail_dirty=1。
#[tauri::command]
pub async fn run_order_negotiation_scan_once(
    app: AppHandle,
    page_size: Option<i64>,
) -> AppResult<NegotiationScanResult> {
    let page_size = page_size.unwrap_or(100).clamp(1, 100);
    let sync_shops = {
        let conn = open_connection(&app)?;
        load_order_sync_shops(&conn)?
    };
    let task_id = format!("negotiation-scan-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'orders.negotiation_scan', 'running', 0, ?2, ?2)",
            params![task_id, created_at],
        )?;
    }

    let client = WechatShopClient::default();
    let mut result = NegotiationScanResult {
        task_id: task_id.clone(),
        processed_shops: 0,
        address_requests: 0,
        sku_requests: 0,
        reconciled_requests: 0,
        failed_shops: 0,
    };

    for shop in sync_shops {
        result.processed_shops += 1;
        let access_token = match ensure_access_token(&app, &shop.shop_id, &client).await {
            Ok(token) => token,
            Err(error) => {
                result.failed_shops += 1;
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

        let mut shop_failed = false;
        // ① 发货前换SKU待处理申请
        let mut next_key = String::new();
        for _page in 0..3 {
            let call = match client
                .preshipment_changesku_get(&access_token, page_size, &next_key)
                .await
            {
                Ok(call) => call,
                Err(error) => {
                    shop_failed = true;
                    insert_task_log_for_app(
                        &app,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 换SKU申请扫描请求失败：{error}", shop.shop_name),
                        None,
                    )?;
                    break;
                }
            };
            let conn = open_connection(&app)?;
            match &call.result {
                WechatCallResult::Success(raw) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "success",
                        None,
                        None,
                        Some("changesku get ok"),
                    )?;
                    let order_ids = extract_order_ids(&raw.raw_payload);
                    for wechat_order_id in &order_ids {
                        result.sku_requests += register_pending_request(
                            &conn,
                            &shop.shop_id,
                            wechat_order_id,
                            "sku_change",
                        )?;
                    }
                    let has_more = raw
                        .raw_payload
                        .get("has_more")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    next_key = raw
                        .raw_payload
                        .get("next_key")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if !has_more || next_key.trim().is_empty() {
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
                        Some("changesku get api error"),
                    )?;
                    shop_failed = true;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!(
                            "店铺 {} 换SKU申请扫描失败：{}",
                            shop.shop_name, error.errmsg
                        ),
                        Some(&serde_json::json!({ "errcode": error.errcode })),
                    )?;
                    break;
                }
            }
        }

        // ② 改址待审单（order/search address_under_review=true）
        let mut next_key = String::new();
        for _page in 0..3 {
            let call = match client
                .search_orders(
                    &access_token,
                    &serde_json::json!({ "address_under_review": true }),
                    None,
                    page_size,
                    &next_key,
                )
                .await
            {
                Ok(call) => call,
                Err(error) => {
                    shop_failed = true;
                    insert_task_log_for_app(
                        &app,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 改址申请扫描请求失败：{error}", shop.shop_name),
                        None,
                    )?;
                    break;
                }
            };
            let conn = open_connection(&app)?;
            match &call.result {
                WechatCallResult::Success(raw) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "success",
                        None,
                        None,
                        Some("order search (address_under_review) ok"),
                    )?;
                    let order_ids = extract_order_ids(&raw.raw_payload);
                    for wechat_order_id in &order_ids {
                        result.address_requests += register_pending_request(
                            &conn,
                            &shop.shop_id,
                            wechat_order_id,
                            "address_change",
                        )?;
                    }
                    let has_more = raw
                        .raw_payload
                        .get("has_more")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    next_key = raw
                        .raw_payload
                        .get("next_key")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if !has_more || next_key.trim().is_empty() {
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
                        Some("order search api error"),
                    )?;
                    shop_failed = true;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!(
                            "店铺 {} 改址申请扫描失败：{}",
                            shop.shop_name, error.errmsg
                        ),
                        Some(&serde_json::json!({ "errcode": error.errcode })),
                    )?;
                    break;
                }
            }
        }
        if shop_failed {
            result.failed_shops += 1;
        }
    }

    // ③ 本地 reconcile
    {
        let conn = open_connection(&app)?;
        result.reconciled_requests = reconcile_order_requests(&conn)?;
    }

    let final_status = if result.failed_shops == 0 {
        "success"
    } else if result.failed_shops == result.processed_shops {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;
    Ok(result)
}

/// 把扫描命中的申请登记进收件箱：skeleton upsert 订单 + 置脏回刷 + 新申请发 critical 通知。
/// 返回 1 表示登记了新申请，0 表示既有 pending 刷新。
fn register_pending_request(
    conn: &Connection,
    shop_id: &str,
    wechat_order_id: &str,
    kind: &str,
) -> AppResult<i64> {
    // skeleton upsert（同时置 detail_dirty=1，详情回刷会带回 apply_time/ddl 等镜像）
    save_synced_order(conn, shop_id, wechat_order_id)?;
    let order_id: Option<String> = conn
        .query_row(
            "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2",
            params![shop_id, wechat_order_id],
            |row| row.get(0),
        )
        .optional()?;
    let is_new = upsert_order_request(
        conn,
        shop_id,
        order_id.as_deref(),
        wechat_order_id,
        kind,
        None,
    )?;
    if is_new {
        let has_purchase_task = order_id
            .as_deref()
            .map(|order_id| {
                conn.query_row(
                    "SELECT COUNT(*) FROM purchase_tasks WHERE order_id = ?1 AND status != 'cancelled'",
                    params![order_id],
                    |row| row.get::<_, i64>(0),
                )
            })
            .transpose()?
            .unwrap_or(0)
            > 0;
        notify_new_request(conn, shop_id, wechat_order_id, kind, has_purchase_task)?;
    }
    Ok(is_new as i64)
}

/// 收件箱本地对账：回填 deadline、解决外部定性申请、临期升级、过期强制回刷。
fn reconcile_order_requests(conn: &Connection) -> AppResult<i64> {
    let now = now_shanghai();
    let now_ts = Utc::now().timestamp();
    let mut reconciled = 0i64;
    let mut stmt = conn.prepare(
        "SELECT r.id, r.kind, r.order_id, r.created_at, r.deadline_at,
                o.address_under_review, o.address_apply_time,
                o.change_sku_state, o.change_sku_ddl,
                o.detail_dirty, o.detail_synced_at, o.wechat_order_id, o.shop_id
         FROM order_requests r
         LEFT JOIN orders o ON o.id = r.order_id
         WHERE r.state = 'pending'",
    )?;
    struct PendingRow {
        id: String,
        kind: String,
        order_id: Option<String>,
        created_at: String,
        deadline_at: Option<i64>,
        address_under_review: Option<i64>,
        address_apply_time: Option<i64>,
        change_sku_state: Option<i64>,
        change_sku_ddl: Option<i64>,
        detail_dirty: Option<i64>,
        detail_synced_at: Option<String>,
        wechat_order_id: Option<String>,
        shop_id: Option<String>,
    }
    let rows = stmt
        .query_map([], |row| {
            Ok(PendingRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                order_id: row.get(2)?,
                created_at: row.get(3)?,
                deadline_at: row.get(4)?,
                address_under_review: row.get(5)?,
                address_apply_time: row.get(6)?,
                change_sku_state: row.get(7)?,
                change_sku_ddl: row.get(8)?,
                detail_dirty: row.get(9)?,
                detail_synced_at: row.get(10)?,
                wechat_order_id: row.get(11)?,
                shop_id: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for row in rows {
        let detail_fresh = row.detail_dirty == Some(0)
            && row
                .detail_synced_at
                .as_deref()
                .map(|synced| synced > row.created_at.as_str())
                .unwrap_or(false);
        let mut resolved: Option<&str> = None;
        let mut deadline: Option<i64> = row.deadline_at;
        match row.kind.as_str() {
            "address_change" => {
                if row.address_under_review == Some(1) {
                    if let Some(apply_time) = row.address_apply_time.filter(|value| *value > 0) {
                        // 官方规则：申请发起 12 小时未处理 = 自动同意
                        deadline = Some(apply_time + 12 * 3600);
                    }
                } else if detail_fresh {
                    // 详情已是申请之后的新快照且不再有待审改址：被外部定性（人工后台处理/超时自动同意/买家取消）
                    resolved = Some("external_or_timeout");
                }
            }
            "sku_change" => match row.change_sku_state {
                Some(3) => {
                    deadline = row.change_sku_ddl.or(deadline);
                }
                Some(4) => resolved = Some("external_accepted"),
                Some(5) => resolved = Some("external_rejected"),
                Some(6) => resolved = Some("user_cancelled"),
                Some(7) => resolved = Some("auto_timeout"),
                _ => {
                    if detail_fresh {
                        resolved = Some("external_or_timeout");
                    }
                }
            },
            _ => {}
        }

        if let Some(resolution) = resolved {
            conn.execute(
                "UPDATE order_requests
                 SET state = 'resolved_external', resolution = ?1, resolved_at = ?2, updated_at = ?2
                 WHERE id = ?3",
                params![resolution, now, row.id],
            )?;
            reconciled += 1;
            continue;
        }
        if deadline != row.deadline_at {
            conn.execute(
                "UPDATE order_requests SET deadline_at = ?1, updated_at = ?2 WHERE id = ?3",
                params![deadline, now, row.id],
            )?;
        }
        if let Some(deadline) = deadline {
            if now_ts > deadline {
                // 已过截止时间：强制回刷详情定性实际结果（下一轮 reconcile 落终态）
                if let Some(order_id) = &row.order_id {
                    conn.execute(
                        "UPDATE orders SET detail_dirty = 1, updated_at = ?1 WHERE id = ?2",
                        params![now, order_id],
                    )?;
                }
            } else if deadline - now_ts < 2 * 3600 {
                // 临期（<2h）升级提醒
                if let (Some(shop_id), Some(wechat_order_id)) =
                    (row.shop_id.as_deref(), row.wechat_order_id.as_deref())
                {
                    upsert_notification(
                        conn,
                        "critical",
                        "order_request_deadline",
                        &row.id,
                        Some(shop_id),
                        "申请即将超时，请立即处理",
                        &format!(
                            "订单 {wechat_order_id} 的{}剩余不足 2 小时（{}）。",
                            request_kind_title(&row.kind),
                            if row.kind == "address_change" {
                                "超时将自动同意，已采购订单可能错发"
                            } else {
                                "超时将自动拒绝"
                            }
                        ),
                        Some(&serde_json::json!({
                            "request_id": &row.id,
                            "kind": &row.kind,
                            "deadline_at": deadline
                        })),
                    )?;
                }
            }
        }
    }
    Ok(reconciled)
}

/// 收件箱列表（pending 按倒计时升序置顶）
#[tauri::command]
pub fn list_order_requests(
    app: AppHandle,
    state: Option<String>,
    limit: Option<i64>,
) -> AppResult<OrderRequestListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let state = normalize_optional_filter(state);
    let sql = format!(
        "SELECT r.id, r.shop_id, COALESCE(s.name, r.shop_id), r.order_id, r.wechat_order_id,
                r.kind, r.state, r.deadline_at, r.payload_json, r.resolution, r.resolved_at,
                r.created_at, r.updated_at,
                (SELECT COUNT(*) FROM purchase_tasks pt
                  WHERE pt.order_id = r.order_id AND pt.status != 'cancelled')
         FROM order_requests r
         LEFT JOIN shops s ON s.id = r.shop_id
         {}
         ORDER BY CASE r.state WHEN 'pending' THEN 0 ELSE 1 END,
                  COALESCE(r.deadline_at, 9223372036854775807) ASC,
                  r.updated_at DESC
         LIMIT ?{}",
        if state.is_some() {
            "WHERE r.state = ?1"
        } else {
            ""
        },
        if state.is_some() { 2 } else { 1 }
    );
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(OrderRequestView {
            id: row.get(0)?,
            shop_id: row.get(1)?,
            shop_name: row.get(2)?,
            order_id: row.get(3)?,
            wechat_order_id: row.get(4)?,
            kind: row.get(5)?,
            state: row.get(6)?,
            deadline_at: row.get(7)?,
            payload_json: row.get(8)?,
            resolution: row.get(9)?,
            resolved_at: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
            purchase_task_count: row.get(13)?,
        })
    };
    let items = if let Some(state) = &state {
        stmt.query_map(params![state, limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    };
    let total = if let Some(state) = &state {
        conn.query_row(
            "SELECT COUNT(*) FROM order_requests WHERE state = ?1",
            [state.as_str()],
            |row| row.get(0),
        )?
    } else {
        conn.query_row("SELECT COUNT(*) FROM order_requests", [], |row| row.get(0))?
    };
    Ok(OrderRequestListResult { items, total })
}

/// 人工裁决申请（同意/拒绝），调用对应微信接口并更新收件箱与订单镜像。
#[tauri::command]
pub async fn decide_order_request(
    app: AppHandle,
    request_id: String,
    approve: bool,
) -> AppResult<OrderRequestDecisionResult> {
    let request_id = request_id.trim().to_string();
    if request_id.is_empty() {
        return Err(AppError::Validation("申请 ID 不能为空".to_string()));
    }
    let (shop_id, order_id, wechat_order_id, kind, state) = {
        let conn = open_connection(&app)?;
        conn.query_row(
            "SELECT shop_id, order_id, wechat_order_id, kind, state
             FROM order_requests WHERE id = ?1",
            params![request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("申请不存在".to_string()))?
    };
    if state != "pending" {
        return Err(AppError::Validation(format!(
            "申请当前状态为 {state}，仅待处理申请可裁决"
        )));
    }

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = match kind.as_str() {
        "address_change" => {
            client
                .address_modify_decide(&access_token, &wechat_order_id, approve)
                .await?
        }
        "sku_change" => {
            client
                .preshipment_changesku_decide(&access_token, &wechat_order_id, approve)
                .await?
        }
        other => {
            return Err(AppError::Validation(format!(
                "申请类型 {other} 暂不支持系统内裁决"
            )))
        }
    };

    let conn = open_connection(&app)?;
    match &call.result {
        WechatCallResult::Success(_) => {
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!("{kind} decide ok, approve={approve}")),
            )?;
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("request decide api error"),
            )?;
            return Err(AppError::Validation(format!(
                "微信返回错误 {}：{}",
                error.errcode, error.errmsg
            )));
        }
    }

    let now = now_shanghai();
    let (new_state, resolution) = if approve {
        ("accepted", "manual_accept")
    } else {
        ("rejected", "manual_reject")
    };
    conn.execute(
        "UPDATE order_requests
         SET state = ?1, resolution = ?2, resolved_at = ?3, updated_at = ?3
         WHERE id = ?4",
        params![new_state, resolution, now, request_id],
    )?;

    if let Some(order_id) = &order_id {
        match kind.as_str() {
            "address_change" => {
                conn.execute(
                    "UPDATE orders
                     SET address_under_review = 0, detail_dirty = 1, updated_at = ?1
                     WHERE id = ?2",
                    params![now, order_id],
                )?;
                if approve {
                    // 同意改址后地址已变：清掉解密缓存，解密队列会自动重解
                    // （官方：同一订单重复解密不重复计额度，免费刷新）
                    conn.execute(
                        "DELETE FROM order_decoded_addresses WHERE order_id = ?1",
                        params![order_id],
                    )?;
                }
            }
            "sku_change" => {
                conn.execute(
                    "UPDATE orders
                     SET change_sku_state = ?1, detail_dirty = 1, updated_at = ?2
                     WHERE id = ?3",
                    params![if approve { 4 } else { 5 }, now, order_id],
                )?;
                if approve {
                    upsert_notification(
                        &conn,
                        "warning",
                        "order_sku_changed",
                        &wechat_order_id,
                        Some(&shop_id),
                        "换SKU已同意，请核对采购任务",
                        &format!(
                            "订单 {wechat_order_id} 的换SKU申请已同意，订单商品行即将变化；\
                             若已生成采购任务请核对规格是否需要重新采购。"
                        ),
                        Some(&serde_json::json!({
                            "order_id": order_id,
                            "wechat_order_id": &wechat_order_id
                        })),
                    )?;
                }
            }
            _ => {}
        }
    }

    Ok(OrderRequestDecisionResult {
        request_id,
        kind,
        state: new_state.to_string(),
        message: if approve {
            "已同意申请".to_string()
        } else {
            "已拒绝申请".to_string()
        },
    })
}

// ============================================================================
// 收货地址解密（订单履约重设计 §5）
// ============================================================================

/// 解密错误分类
enum DecodeFailure {
    /// 10020198 解密过快：本轮停止，下轮重试
    RateLimited,
    /// 额度类（当月/当日达限）：当日熔断
    QuotaExhausted(String),
    /// 状态/类型不支持：永久跳过
    Skip(String),
    /// 平台限制（须官方工具发货）：跳过 + critical 通知
    PlatformBlocked(String),
    /// 其他错误：记录后退避重试
    Other(String),
}

fn classify_decode_error(errcode: i64, errmsg: &str) -> DecodeFailure {
    match errcode {
        10020198 => DecodeFailure::RateLimited,
        10020123 => DecodeFailure::QuotaExhausted(format!("{errcode}: {errmsg}")),
        10020503 | 10020124 | 10020248 | 10020506 => {
            DecodeFailure::Skip(format!("{errcode}: {errmsg}"))
        }
        10020501 | 10020502 => DecodeFailure::PlatformBlocked(format!("{errcode}: {errmsg}")),
        _ => DecodeFailure::Other(format!("{errcode}: {errmsg}")),
    }
}

fn encrypt_field(app: &AppHandle, value: Option<&str>) -> AppResult<Option<String>> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(plain) => {
            let encrypted = crate::crypto::encrypt_secret(app, plain)?;
            Ok(Some(
                serde_json::json!({
                    "ciphertext": encrypted.ciphertext,
                    "nonce": encrypted.nonce
                })
                .to_string(),
            ))
        }
        None => Ok(None),
    }
}

fn decrypt_field(app: &AppHandle, stored: Option<&str>) -> Option<String> {
    let raw = stored?;
    let value: Value = serde_json::from_str(raw).ok()?;
    let ciphertext = value.get("ciphertext")?.as_str()?;
    let nonce = value.get("nonce")?.as_str()?;
    crate::crypto::decrypt_secret(app, ciphertext, nonce).ok()
}

/// 解密单个订单的收货信息并加密落库。成功返回 Ok(true)，可跳过返回 Ok(false)。
async fn decode_one_order(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    order_id: &str,
    shop_id: &str,
    wechat_order_id: &str,
) -> AppResult<Result<bool, DecodeFailure>> {
    let call = match client
        .decode_order_sensitive_info(access_token, wechat_order_id)
        .await
    {
        Ok(call) => call,
        Err(error) => return Ok(Err(DecodeFailure::Other(format!("请求失败：{error}")))),
    };
    let conn = open_connection(app)?;
    let now = now_shanghai();
    match &call.result {
        WechatCallResult::Success(raw) => {
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("sensitiveinfo decode ok"),
            )?;
            let payload = &raw.raw_payload;
            let address = payload.get("address_info");
            let pick = |keys: &[&str]| -> Option<String> {
                address.and_then(|info| {
                    keys.iter()
                        .find_map(|key| json_value_to_string(info.get(*key)))
                        .filter(|value| !value.trim().is_empty())
                })
            };
            let user_name = pick(&["user_name"]);
            // 官方语义（api_decodesensitiveinfo）：买家开启号码保护时 tel_number 只返回
            // 脱敏号（187****7735），可拨打的联系方式在 virtual_number_info；
            // virtual_order_tel_number 是虚拟发货订单的联系方式候选
            let tel_number = pick(&["tel_number", "virtual_order_tel_number"]);
            let province = pick(&["province_name", "province"]);
            let city = pick(&["city_name", "city"]);
            let county = pick(&["county_name", "county"]);
            let detail = {
                let detail_info = pick(&["detail_info"]);
                let house_number = pick(&["house_number"]);
                match (detail_info, house_number) {
                    (Some(detail), Some(house)) => Some(format!("{detail} {house}")),
                    (Some(detail), None) => Some(detail),
                    (None, Some(house)) => Some(house),
                    (None, None) => None,
                }
            };
            let virtual_info = payload.get("virtual_number_info");
            let virtual_number =
                virtual_info.and_then(|info| json_value_to_string(info.get("virtual_number")));
            let virtual_extension =
                virtual_info.and_then(|info| json_value_to_string(info.get("extension")));
            let virtual_expiration =
                virtual_info.and_then(|info| json_value_to_i64(info.get("expiration")));
            let hash_code = address.and_then(|info| json_value_to_string(info.get("hash_code")));

            let user_name_enc = encrypt_field(app, user_name.as_deref())?;
            let tel_number_enc = encrypt_field(app, tel_number.as_deref())?;
            let detail_info_enc = encrypt_field(app, detail.as_deref())?;
            conn.execute(
                "INSERT INTO order_decoded_addresses
                 (order_id, shop_id, wechat_order_id, user_name_enc, tel_number_enc, detail_info_enc,
                  province, city, county, virtual_number, virtual_extension, virtual_expiration,
                  hash_code, decode_error, decode_skip_reason, decoded_at, purged_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL, NULL, ?14, NULL, ?14, ?14)
                 ON CONFLICT(order_id) DO UPDATE SET
                   user_name_enc = excluded.user_name_enc,
                   tel_number_enc = excluded.tel_number_enc,
                   detail_info_enc = excluded.detail_info_enc,
                   province = excluded.province,
                   city = excluded.city,
                   county = excluded.county,
                   virtual_number = excluded.virtual_number,
                   virtual_extension = excluded.virtual_extension,
                   virtual_expiration = excluded.virtual_expiration,
                   hash_code = excluded.hash_code,
                   decode_error = NULL,
                   decode_skip_reason = NULL,
                   decoded_at = excluded.decoded_at,
                   purged_at = NULL,
                   updated_at = excluded.updated_at",
                params![
                    order_id,
                    shop_id,
                    wechat_order_id,
                    user_name_enc,
                    tel_number_enc,
                    detail_info_enc,
                    province,
                    city,
                    county,
                    virtual_number,
                    virtual_extension,
                    virtual_expiration,
                    hash_code,
                    now
                ],
            )?;
            Ok(Ok(true))
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("sensitiveinfo decode api error"),
            )?;
            Ok(Err(classify_decode_error(error.errcode, &error.errmsg)))
        }
    }
}

fn write_decode_failure(
    conn: &Connection,
    order_id: &str,
    shop_id: &str,
    wechat_order_id: &str,
    error: Option<&str>,
    skip_reason: Option<&str>,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO order_decoded_addresses
         (order_id, shop_id, wechat_order_id, decode_error, decode_skip_reason, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(order_id) DO UPDATE SET
           decode_error = excluded.decode_error,
           decode_skip_reason = excluded.decode_skip_reason,
           updated_at = excluded.updated_at",
        params![order_id, shop_id, wechat_order_id, error, skip_reason, now],
    )?;
    Ok(())
}

fn decode_circuit_open_today(conn: &Connection) -> AppResult<bool> {
    let today = now_shanghai()[..10].to_string();
    Ok(get_string_setting(conn, ORDER_DECODE_CIRCUIT_SETTING)? == Some(today))
}

/// 解密队列单轮（订单履约重设计 §5.1/§5.3）：候选=进入待采购且尚无可用解密结果的订单；
/// 全局串行、相邻调用间隔 ≥2s；额度熔断按日，次日 0 点后自动恢复（driver 步骤会自然补跑）。
#[tauri::command]
pub async fn run_address_decode_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<AddressDecodeBatchResult> {
    let limit = limit.unwrap_or(5).clamp(1, 20);
    let task_id = format!("address-decode-{}", Uuid::new_v4());
    let mut result = AddressDecodeBatchResult {
        task_id: task_id.clone(),
        processed_orders: 0,
        decoded_orders: 0,
        skipped_orders: 0,
        failed_orders: 0,
        circuit_open: false,
    };
    let candidates = {
        let conn = open_connection(&app)?;
        if decode_circuit_open_today(&conn)? {
            result.circuit_open = true;
            return Ok(result);
        }
        // 失败行 1 小时退避后自动重试；skip/已解密的不再进队
        let retry_cutoff = format_shanghai(Utc::now() - Duration::hours(1));
        let mut stmt = conn.prepare(
            "SELECT o.id, o.shop_id, o.wechat_order_id
             FROM orders o
             LEFT JOIN order_decoded_addresses d ON d.order_id = o.id
             WHERE o.status = 'pending_purchase'
               AND (d.order_id IS NULL
                    OR (d.decoded_at IS NULL AND d.decode_skip_reason IS NULL AND d.updated_at < ?1))
             ORDER BY o.updated_at ASC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![retry_cutoff, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if candidates.is_empty() {
        return Ok(result);
    }

    let client = WechatShopClient::default();
    let mut tokens: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (index, (order_id, shop_id, wechat_order_id)) in candidates.iter().enumerate() {
        if index > 0 {
            // 全局串行 + ≥2s 间隔（10020198 限速的预防）
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        result.processed_orders += 1;
        let access_token = match tokens.get(shop_id) {
            Some(token) => token.clone(),
            None => match ensure_access_token(&app, shop_id, &client).await {
                Ok(token) => {
                    tokens.insert(shop_id.clone(), token.clone());
                    token
                }
                Err(error) => {
                    result.failed_orders += 1;
                    let conn = open_connection(&app)?;
                    write_decode_failure(
                        &conn,
                        order_id,
                        shop_id,
                        wechat_order_id,
                        Some(&format!("获取 access_token 失败：{error}")),
                        None,
                    )?;
                    continue;
                }
            },
        };
        match decode_one_order(
            &app,
            &client,
            &access_token,
            order_id,
            shop_id,
            wechat_order_id,
        )
        .await?
        {
            Ok(_) => result.decoded_orders += 1,
            Err(DecodeFailure::RateLimited) => {
                result.failed_orders += 1;
                let conn = open_connection(&app)?;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(order_id),
                    "warning",
                    "解密过快（10020198），本轮停止，下轮自动重试",
                    None,
                )
                .ok();
                drop(conn);
                break;
            }
            Err(DecodeFailure::QuotaExhausted(reason)) => {
                result.failed_orders += 1;
                result.circuit_open = true;
                let conn = open_connection(&app)?;
                let today = now_shanghai()[..10].to_string();
                set_string_setting(&conn, ORDER_DECODE_CIRCUIT_SETTING, &today)?;
                write_decode_failure(
                    &conn,
                    order_id,
                    shop_id,
                    wechat_order_id,
                    Some(&reason),
                    None,
                )?;
                upsert_notification(
                    &conn,
                    "warning",
                    "address_decode_quota",
                    &today,
                    Some(shop_id),
                    "收货信息解密额度受限",
                    &format!(
                        "解密额度达到限制（{reason}），今日解密已熔断；可在微信小店商家后台申请临时额度，次日 0 点自动恢复。"
                    ),
                    None,
                )?;
                break;
            }
            Err(DecodeFailure::Skip(reason)) => {
                result.skipped_orders += 1;
                let conn = open_connection(&app)?;
                write_decode_failure(
                    &conn,
                    order_id,
                    shop_id,
                    wechat_order_id,
                    None,
                    Some(&reason),
                )?;
            }
            Err(DecodeFailure::PlatformBlocked(reason)) => {
                result.skipped_orders += 1;
                let conn = open_connection(&app)?;
                write_decode_failure(
                    &conn,
                    order_id,
                    shop_id,
                    wechat_order_id,
                    None,
                    Some(&reason),
                )?;
                upsert_notification(
                    &conn,
                    "critical",
                    "address_decode_blocked",
                    wechat_order_id,
                    Some(shop_id),
                    "订单收货信息被平台限制解密",
                    &format!(
                        "订单 {wechat_order_id} 被平台限制解密（{reason}），请到微信小店违规中心查询，并使用官方发货工具发货。"
                    ),
                    None,
                )?;
            }
            Err(DecodeFailure::Other(reason)) => {
                result.failed_orders += 1;
                let conn = open_connection(&app)?;
                write_decode_failure(
                    &conn,
                    order_id,
                    shop_id,
                    wechat_order_id,
                    Some(&reason),
                    None,
                )?;
            }
        }
    }
    Ok(result)
}

/// 手动解密单个订单（详情页/采购队列兜底按钮；无视 1 小时退避，但仍尊重当日熔断之外的人工意愿）
#[tauri::command]
pub async fn decode_order_address(
    app: AppHandle,
    order_id: String,
) -> AppResult<DecodedOrderAddressView> {
    let order_id = order_id.trim().to_string();
    let (shop_id, wechat_order_id) = {
        let conn = open_connection(&app)?;
        conn.query_row(
            "SELECT shop_id, wechat_order_id FROM orders WHERE id = ?1",
            params![order_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("订单不存在".to_string()))?
    };
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    match decode_one_order(
        &app,
        &client,
        &access_token,
        &order_id,
        &shop_id,
        &wechat_order_id,
    )
    .await?
    {
        Ok(_) => {}
        Err(DecodeFailure::RateLimited) => {
            return Err(AppError::Validation(
                "解密过快（10020198），请稍后重试".to_string(),
            ))
        }
        Err(DecodeFailure::QuotaExhausted(reason)) => {
            let conn = open_connection(&app)?;
            let today = now_shanghai()[..10].to_string();
            set_string_setting(&conn, ORDER_DECODE_CIRCUIT_SETTING, &today)?;
            return Err(AppError::Validation(format!("解密额度受限：{reason}")));
        }
        Err(DecodeFailure::Skip(reason)) | Err(DecodeFailure::PlatformBlocked(reason)) => {
            let conn = open_connection(&app)?;
            write_decode_failure(
                &conn,
                &order_id,
                &shop_id,
                &wechat_order_id,
                None,
                Some(&reason),
            )?;
            return Err(AppError::Validation(format!("该订单不支持解密：{reason}")));
        }
        Err(DecodeFailure::Other(reason)) => {
            let conn = open_connection(&app)?;
            write_decode_failure(
                &conn,
                &order_id,
                &shop_id,
                &wechat_order_id,
                Some(&reason),
                None,
            )?;
            return Err(AppError::Validation(format!("解密失败：{reason}")));
        }
    }
    get_decoded_order_address(app, order_id)
}

/// 读取解密后的收货信息（解密展示给操作员；密文仅在本命令内解开，不落日志）
#[tauri::command]
pub fn get_decoded_order_address(
    app: AppHandle,
    order_id: String,
) -> AppResult<DecodedOrderAddressView> {
    let conn = open_connection(&app)?;
    let row = conn
        .query_row(
            "SELECT order_id, wechat_order_id, user_name_enc, tel_number_enc, detail_info_enc,
                    province, city, county, virtual_number, virtual_extension, virtual_expiration,
                    decode_error, decode_skip_reason, decoded_at
             FROM order_decoded_addresses WHERE order_id = ?1",
            params![order_id.trim()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("该订单尚未解密收货信息".to_string()))?;
    Ok(DecodedOrderAddressView {
        order_id: row.0,
        wechat_order_id: row.1,
        user_name: decrypt_field(&app, row.2.as_deref()),
        tel_number: decrypt_field(&app, row.3.as_deref()),
        detail_info: decrypt_field(&app, row.4.as_deref()),
        province: row.5,
        city: row.6,
        county: row.7,
        virtual_number: row.8,
        virtual_extension: row.9,
        virtual_expiration: row.10,
        decode_error: row.11,
        decode_skip_reason: row.12,
        decoded_at: row.13,
    })
}

// ============================================================================
// 采购双节点（订单履约重设计 §7）：「待采购」与「待回运单」
// ============================================================================

/// 标记/取消标记采购任务「已在上游下单」。purchased=true 进入「待回运单」队列。
// ============================================================================
// 订单履约三期（docs/order-fulfillment-redesign.md §8）：改运单、补发、
// 虚拟号保活巡检、解密明文 GC。
// ============================================================================

/// 整单订单项 → product_infos JSON（改运单/补发的兜底商品列表）。
fn whole_order_product_infos_json(conn: &Connection, order_id: &str) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(wechat_product_id, ''), COALESCE(wechat_sku_id, ''), sku_count
         FROM order_items
         WHERE order_id = ?1
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([order_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Err(AppError::Validation(
            "订单缺少订单项，无法构造商品列表".to_string(),
        ));
    }
    if rows
        .iter()
        .any(|(product_id, sku_id, _)| product_id.is_empty() || sku_id.is_empty())
    {
        return Err(AppError::Validation(
            "订单项缺少微信 product_id 或 sku_id".to_string(),
        ));
    }
    Ok(rows
        .into_iter()
        .map(|(product_id, sku_id, cnt)| {
            serde_json::json!({
                "product_id": product_id,
                "sku_id": sku_id,
                "product_cnt": cnt.max(1)
            })
        })
        .collect())
}

/// 本地包裹（按运单定位）的商品映射 → product_infos JSON；包裹不存在或映射为空返回 None。
fn package_product_infos_json(
    conn: &Connection,
    order_id: &str,
    delivery_id: &str,
    waybill_id: &str,
) -> AppResult<Option<Vec<Value>>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(opi.wechat_product_id, ''), COALESCE(opi.wechat_sku_id, ''), opi.product_cnt
         FROM order_package_items opi
         JOIN order_packages op ON op.id = opi.package_id
         WHERE op.order_id = ?1
           AND COALESCE(op.delivery_id, '') = ?2
           AND COALESCE(op.waybill_id, '') = ?3
         ORDER BY opi.created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![order_id, delivery_id, waybill_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Ok(None);
    }
    if rows
        .iter()
        .any(|(product_id, sku_id, _)| product_id.is_empty() || sku_id.is_empty())
    {
        return Err(AppError::Validation(
            "包裹商品映射缺少微信 product_id 或 sku_id".to_string(),
        ));
    }
    Ok(Some(
        rows.into_iter()
            .map(|(product_id, sku_id, cnt)| {
                serde_json::json!({
                    "product_id": product_id,
                    "sku_id": sku_id,
                    "product_cnt": cnt.max(1)
                })
            })
            .collect(),
    ))
}

/// 改运单（deliveryinfo/update，三期 §8）。官方双模式：
/// old_waybill_id 给出 → change_infos 包裹级 old→new（拆单发货也支持）；
/// 否则 → delivery_list 整单重报（拆单发货的订单官方不支持，本地多包裹时直接拒绝 606040 前移）。
/// 未完成订单 ≤3 次（606041），本地 delivery_change_count 计数提前禁用。
#[tauri::command]
pub async fn change_shipment_delivery_info(
    app: AppHandle,
    request: DeliveryChangeRequest,
) -> AppResult<DeliveryChangeResult> {
    let new_delivery_id = request.delivery_id.trim().to_string();
    let new_waybill_id = request.waybill_id.trim().to_string();
    if new_delivery_id.is_empty() || new_waybill_id.is_empty() {
        return Err(AppError::Validation(
            "新快递公司 ID 和新运单号不能为空".to_string(),
        ));
    }
    let order_id = request.order_id.trim().to_string();
    let old_waybill_id = request
        .old_waybill_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let old_delivery_id = request
        .old_delivery_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let (shop_id, wechat_order_id, change_count, order_status) = {
        let conn = open_connection(&app)?;
        conn.query_row(
            "SELECT shop_id, COALESCE(wechat_order_id, ''), COALESCE(delivery_change_count, 0), status
             FROM orders WHERE id = ?1",
            [order_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("订单不存在".to_string()))?
    };
    if wechat_order_id.is_empty() {
        return Err(AppError::Validation("订单缺少微信订单号".to_string()));
    }
    if matches!(order_status.as_str(), "completed" | "cancelled") {
        return Err(AppError::Validation(
            "订单已终态，微信不允许修改物流".to_string(),
        ));
    }
    if change_count >= 3 {
        return Err(AppError::Validation(
            "该订单修改物流次数已达微信上限（3 次），无法再次修改".to_string(),
        ));
    }
    // 官方 order_id 为数字类型（与 send 的字符串不同），转换失败直接报参数错误
    let numeric_order_id: i64 = wechat_order_id.parse().map_err(|_| {
        AppError::Validation(format!("微信订单号 {wechat_order_id} 不是数字，无法调用改运单接口"))
    })?;

    let (payload, mode, old_delivery_for_update, old_waybill_for_update) = {
        let conn = open_connection(&app)?;
        match &old_waybill_id {
            Some(old_waybill) => {
                // 包裹模式：old 的商品映射取本地包裹；old 无包裹行（历史单）时仅在
                // 单包裹订单允许整单兜底——拆单订单整单兜底会与微信侧包裹记录不符（审查修复，与 send 路径同约定）
                let old_delivery = old_delivery_id.clone().unwrap_or_default();
                let product_infos =
                    match package_product_infos_json(&conn, &order_id, &old_delivery, old_waybill)? {
                        Some(infos) => infos,
                        None => {
                            let package_count: i64 = conn.query_row(
                                "SELECT COUNT(*) FROM order_packages WHERE order_id = ?1",
                                [order_id.as_str()],
                                |row| row.get(0),
                            )?;
                            if package_count > 1 {
                                return Err(AppError::Validation(
                                    "该订单为拆单发货且本地缺少原包裹商品映射，请先同步订单详情后重试".to_string(),
                                ));
                            }
                            whole_order_product_infos_json(&conn, &order_id)?
                        }
                    };
                let payload = serde_json::json!({
                    "order_id": numeric_order_id,
                    "change_infos": [{
                        "old": {
                            "delivery_id": old_delivery,
                            "waybill_id": old_waybill,
                            "product_infos": product_infos
                        },
                        "new": {
                            "delivery_id": new_delivery_id,
                            "waybill_id": new_waybill_id,
                            "product_infos": product_infos
                        }
                    }]
                });
                (
                    payload,
                    "package".to_string(),
                    old_delivery,
                    old_waybill.clone(),
                )
            }
            None => {
                // 整单模式：本地存在多个包裹说明已拆单，官方 606040 不支持，前移拦截
                let package_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM order_packages WHERE order_id = ?1",
                    [order_id.as_str()],
                    |row| row.get(0),
                )?;
                if package_count > 1 {
                    return Err(AppError::Validation(
                        "该订单已拆单发货，整单改运单不被微信支持；请指定原运单号走包裹级修改".to_string(),
                    ));
                }
                let product_infos = whole_order_product_infos_json(&conn, &order_id)?;
                let payload = serde_json::json!({
                    "order_id": numeric_order_id,
                    "delivery_list": [{
                        "deliver_type": 1,
                        "delivery_id": new_delivery_id,
                        "waybill_id": new_waybill_id,
                        "product_infos": product_infos
                    }]
                });
                (payload, "whole_order".to_string(), String::new(), String::new())
            }
        }
    };

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client
        .update_delivery_info(
            &access_token,
            numeric_order_id,
            payload.get("delivery_list"),
            payload.get("change_infos"),
        )
        .await?;

    let mut conn = open_connection(&app)?;
    match &call.result {
        WechatCallResult::Success(_) => {
            // 审查修复：成功后的本地镜像整体进事务，杜绝「计数已 +1 但快照未更新」的部分提交
            let tx = conn.transaction()?;
            insert_api_call_log(
                &tx,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("deliveryinfo update ok"),
            )?;
            let now = now_shanghai();
            let new_count = change_count + 1;
            // 本地计数 + 触发详情回刷核对官方包裹快照
            tx.execute(
                "UPDATE orders
                 SET delivery_change_count = ?1, detail_dirty = 1, updated_at = ?2
                 WHERE id = ?3",
                params![new_count, now, order_id],
            )?;
            // 回填本地快照：shipments / order_packages / purchase_tasks 的运单字段跟随更新。
            // 审查修复：purchase_tasks 是 shipment/包裹重建的事实源（重新回填按它分组），
            // 不同步会让后续重回填把新运单静默写回旧运单。
            if mode == "package" {
                tx.execute(
                    "UPDATE shipments
                     SET delivery_id = ?1, delivery_name = COALESCE(?2, delivery_name),
                         waybill_id = ?3, updated_at = ?4
                     WHERE order_id = ?5
                       AND COALESCE(delivery_id, '') = ?6
                       AND COALESCE(waybill_id, '') = ?7",
                    params![
                        new_delivery_id,
                        request.delivery_name.as_deref(),
                        new_waybill_id,
                        now,
                        order_id,
                        old_delivery_for_update,
                        old_waybill_for_update
                    ],
                )?;
                tx.execute(
                    "UPDATE order_packages
                     SET delivery_id = ?1, delivery_name = COALESCE(?2, delivery_name),
                         waybill_id = ?3, updated_at = ?4
                     WHERE order_id = ?5
                       AND COALESCE(delivery_id, '') = ?6
                       AND COALESCE(waybill_id, '') = ?7",
                    params![
                        new_delivery_id,
                        request.delivery_name.as_deref(),
                        new_waybill_id,
                        now,
                        order_id,
                        old_delivery_for_update,
                        old_waybill_for_update
                    ],
                )?;
                tx.execute(
                    "UPDATE purchase_tasks
                     SET supplier_delivery_id = ?1,
                         supplier_delivery_name = COALESCE(?2, supplier_delivery_name),
                         supplier_waybill_id = ?3,
                         updated_at = ?4
                     WHERE order_id = ?5
                       AND COALESCE(supplier_delivery_id, '') = ?6
                       AND COALESCE(supplier_waybill_id, '') = ?7",
                    params![
                        new_delivery_id,
                        request.delivery_name.as_deref(),
                        new_waybill_id,
                        now,
                        order_id,
                        old_delivery_for_update,
                        old_waybill_for_update
                    ],
                )?;
            } else {
                tx.execute(
                    "UPDATE shipments
                     SET delivery_id = ?1, delivery_name = COALESCE(?2, delivery_name),
                         waybill_id = ?3, updated_at = ?4
                     WHERE order_id = ?5 AND status IN ('wechat_shipped', 'send_failed')",
                    params![
                        new_delivery_id,
                        request.delivery_name.as_deref(),
                        new_waybill_id,
                        now,
                        order_id
                    ],
                )?;
                tx.execute(
                    "UPDATE order_packages
                     SET delivery_id = ?1, delivery_name = COALESCE(?2, delivery_name),
                         waybill_id = ?3, updated_at = ?4
                     WHERE order_id = ?5",
                    params![
                        new_delivery_id,
                        request.delivery_name.as_deref(),
                        new_waybill_id,
                        now,
                        order_id
                    ],
                )?;
                tx.execute(
                    "UPDATE purchase_tasks
                     SET supplier_delivery_id = ?1,
                         supplier_delivery_name = COALESCE(?2, supplier_delivery_name),
                         supplier_waybill_id = ?3,
                         updated_at = ?4
                     WHERE order_id = ?5
                       AND status IN ('supplier_shipped', 'wechat_shipped', 'completed')",
                    params![
                        new_delivery_id,
                        request.delivery_name.as_deref(),
                        new_waybill_id,
                        now,
                        order_id
                    ],
                )?;
            }
            tx.commit()?;
            Ok(DeliveryChangeResult {
                order_id,
                wechat_order_id,
                mode,
                delivery_change_count: new_count,
                message: format!("改运单成功（已用 {new_count}/3 次）"),
            })
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("deliveryinfo update api error"),
            )?;
            // 606041=次数超限：同步本地计数到上限，防止重复盲调
            if error.errcode == 606041 {
                conn.execute(
                    "UPDATE orders SET delivery_change_count = 3, updated_at = ?1 WHERE id = ?2",
                    params![now_shanghai(), order_id],
                )?;
            }
            Err(AppError::Validation(format!(
                "微信改运单失败（{}）：{}",
                error.errcode, error.errmsg
            )))
        }
    }
}

/// 补发包裹（delivery/compensation，三期 §8）。官方约束前移：
/// reason 枚举 1漏发/2拆包/3坏损/4赠品；前置 SKU 已全部发货且在售后期内；一单 ≤10 次；
/// deliver_type 枚举 1=快递/6=无需物流（与 send 不同），本系统补发固定走快递。
#[tauri::command]
pub async fn compensate_order_delivery(
    app: AppHandle,
    request: CompensateDeliveryRequest,
) -> AppResult<CompensateDeliveryResult> {
    if !(1..=4).contains(&request.reason) {
        return Err(AppError::Validation(
            "补发原因只支持 1 漏发 / 2 拆包 / 3 坏损 / 4 赠品".to_string(),
        ));
    }
    let delivery_id = request.delivery_id.trim().to_string();
    let waybill_id = request.waybill_id.trim().to_string();
    if delivery_id.is_empty() || waybill_id.is_empty() {
        return Err(AppError::Validation(
            "补发的快递公司 ID 和运单号不能为空".to_string(),
        ));
    }
    let order_id = request.order_id.trim().to_string();

    let (shop_id, wechat_order_id, compensation_count, order_status) = {
        let conn = open_connection(&app)?;
        conn.query_row(
            "SELECT shop_id, COALESCE(wechat_order_id, ''), COALESCE(compensation_count, 0), status
             FROM orders WHERE id = ?1",
            [order_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("订单不存在".to_string()))?
    };
    if wechat_order_id.is_empty() {
        return Err(AppError::Validation("订单缺少微信订单号".to_string()));
    }
    if order_status == "cancelled" {
        return Err(AppError::Validation("订单已取消，无法补发".to_string()));
    }
    // 审查修复（官方约束前移）：补发前置要求 SKU 已全部发货——
    // 仅官方确认已发货/已完成的订单放行，部分发货/未发货直接拦截不盲调 API
    if !matches!(order_status.as_str(), "wechat_shipped" | "completed") {
        return Err(AppError::Validation(format!(
            "订单当前状态为 {order_status}，微信要求商品已全部发货后才能补发；剩余商品请走正常发货流程"
        )));
    }
    if compensation_count >= 10 {
        return Err(AppError::Validation(
            "该订单补发次数已达微信上限（10 个包裹），无法再补发".to_string(),
        ));
    }
    let numeric_order_id: i64 = wechat_order_id.parse().map_err(|_| {
        AppError::Validation(format!("微信订单号 {wechat_order_id} 不是数字，无法调用补发接口"))
    })?;

    let product_infos: Vec<Value> = match &request.product_infos {
        Some(infos) if !infos.is_empty() => infos
            .iter()
            .map(|info| {
                serde_json::json!({
                    "product_id": info.product_id,
                    "sku_id": info.sku_id,
                    "product_cnt": info.product_cnt.max(1)
                })
            })
            .collect(),
        _ => {
            let conn = open_connection(&app)?;
            whole_order_product_infos_json(&conn, &order_id)?
        }
    };
    let delivery_list = serde_json::json!([{
        "deliver_type": 1,
        "delivery_id": delivery_id,
        "waybill_id": waybill_id,
        "product_infos": product_infos
    }]);

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client
        .compensate_delivery(&access_token, numeric_order_id, &delivery_list, request.reason)
        .await?;

    let mut conn = open_connection(&app)?;
    match &call.result {
        WechatCallResult::Success(_) => {
            // 审查修复：成功后的本地镜像整体进事务（防计数与包裹/商品行部分提交）
            let tx = conn.transaction()?;
            insert_api_call_log(
                &tx,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("delivery compensation ok"),
            )?;
            let now = now_shanghai();
            let new_count = compensation_count + 1;
            tx.execute(
                "UPDATE orders
                 SET compensation_count = ?1, detail_dirty = 1, updated_at = ?2
                 WHERE id = ?3",
                params![new_count, now, order_id],
            )?;
            // 补发包裹落本地包裹表（source=compensation），商品行随补发清单
            tx.execute(
                "INSERT INTO order_packages
                 (id, order_id, shop_id, wechat_order_id, delivery_id, delivery_name, waybill_id,
                  deliver_type, source, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 'compensation', 'submitted', ?8, ?8)
                 ON CONFLICT(order_id, delivery_id, waybill_id) DO UPDATE SET
                   source = 'compensation',
                   delivery_name = COALESCE(excluded.delivery_name, order_packages.delivery_name),
                   updated_at = excluded.updated_at",
                params![
                    format!("package-{}-{}", order_id, Uuid::new_v4()),
                    order_id,
                    shop_id,
                    wechat_order_id,
                    delivery_id,
                    request.delivery_name.as_deref(),
                    waybill_id,
                    now
                ],
            )?;
            let package_id: String = tx.query_row(
                "SELECT id FROM order_packages
                 WHERE order_id = ?1 AND COALESCE(delivery_id, '') = ?2 AND COALESCE(waybill_id, '') = ?3",
                params![order_id, delivery_id, waybill_id],
                |row| row.get(0),
            )?;
            tx.execute(
                "DELETE FROM order_package_items WHERE package_id = ?1",
                [package_id.as_str()],
            )?;
            if let Some(infos) = delivery_list[0].get("product_infos").and_then(Value::as_array) {
                for info in infos {
                    tx.execute(
                        "INSERT INTO order_package_items
                         (id, package_id, order_item_id, wechat_product_id, wechat_sku_id, product_cnt, created_at)
                         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6)",
                        params![
                            format!("package-item-{}", Uuid::new_v4()),
                            package_id,
                            info.get("product_id").and_then(Value::as_str),
                            info.get("sku_id").and_then(Value::as_str),
                            info.get("product_cnt").and_then(Value::as_i64).unwrap_or(1),
                            now
                        ],
                    )?;
                }
            }
            tx.commit()?;
            Ok(CompensateDeliveryResult {
                order_id,
                wechat_order_id,
                compensation_count: new_count,
                message: format!("补发提交成功（已用 {new_count}/10 个补发包裹）"),
            })
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("delivery compensation api error"),
            )?;
            Err(AppError::Validation(format!(
                "微信补发失败（{}）：{}",
                error.errcode, error.errmsg
            )))
        }
    }
}

/// 虚拟号保活巡检（driver 每日一次，三期）。
/// 官方语义（审查修复后核实）：tel_number_ext_info.has_delay_times = 「已延期次数」，
/// 因此只对「从未延期过（=0）」的窗口单自动首延；已延期过的单不再自动续（官方可延次数有限，
/// 且剩余次数只有 delay 接口返回的 available_extend_num 才是权威值）。
/// 调用后（无论成败）本地把 virtual_tel_delay_times 预记为 1 并置 detail_dirty，
/// 防当日/次日重复盲调；详情回刷会用官方真值覆盖。
#[tauri::command]
pub async fn run_virtual_number_delay_scan_once(
    app: AppHandle,
) -> AppResult<VirtualTelDelayScanResult> {
    let mut result = VirtualTelDelayScanResult {
        scanned_orders: 0,
        delayed_orders: 0,
        failed_orders: 0,
    };
    let now_epoch = Utc::now().timestamp();
    let window_end = now_epoch + 7 * 24 * 3600;
    let candidates = {
        let conn = open_connection(&app)?;
        // 批次 8（审查修复）：每日步骤与常规步骤共享 120s tick 预算，靠 7 天窗口多轮收敛
        let mut stmt = conn.prepare(
            "SELECT id, shop_id, COALESCE(wechat_order_id, '')
             FROM orders
             WHERE use_tel_number = 1
               AND COALESCE(virtual_tel_delay_times, 0) = 0
               AND virtual_tel_expire_time IS NOT NULL
               AND virtual_tel_expire_time BETWEEN ?1 AND ?2
               AND status NOT IN ('completed', 'cancelled')
             ORDER BY virtual_tel_expire_time ASC
             LIMIT 8",
        )?;
        let rows = stmt
            .query_map(params![now_epoch, window_end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if candidates.is_empty() {
        return Ok(result);
    }

    let client = WechatShopClient::default();
    // token 负缓存（审查修复）：失败店铺也记账，同店剩余候选不再重复打认证 API
    let mut tokens: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for (order_id, shop_id, wechat_order_id) in &candidates {
        result.scanned_orders += 1;
        if wechat_order_id.is_empty() {
            result.failed_orders += 1;
            continue;
        }
        let access_token = match tokens.get(shop_id) {
            Some(Some(token)) => token.clone(),
            Some(None) => {
                result.failed_orders += 1;
                continue;
            }
            None => match ensure_access_token(&app, shop_id, &client).await {
                Ok(token) => {
                    tokens.insert(shop_id.clone(), Some(token.clone()));
                    token
                }
                Err(_) => {
                    tokens.insert(shop_id.clone(), None);
                    result.failed_orders += 1;
                    continue;
                }
            },
        };
        match client
            .delay_virtual_tel_number(&access_token, wechat_order_id)
            .await
        {
            Ok(call) => {
                // 审查修复：远端已调用成功后，本地落库失败不得中止全扫描（丢防重标记
                // 比单条降级更糟——次日重复盲调烧配额），降级为失败计数
                let mark_attempted = |success: bool| -> AppResult<()> {
                    let conn = open_connection(&app)?;
                    if let WechatCallResult::ApiError(error) = &call.result {
                        insert_api_call_log(
                            &conn,
                            Some(shop_id),
                            call.meta.endpoint,
                            call.meta.method,
                            "api_error",
                            Some(error.errcode),
                            Some(&error.errmsg),
                            Some("virtualnumber delay api error"),
                        )?;
                    }
                    // 成败都预记已延期 1 次 + 置脏：防重复盲调，官方真值由详情回刷覆盖
                    conn.execute(
                        "UPDATE orders
                         SET virtual_tel_delay_times = 1, detail_dirty = 1, updated_at = ?1
                         WHERE id = ?2",
                        params![now_shanghai(), order_id],
                    )?;
                    let _ = success;
                    Ok(())
                };
                let success = matches!(&call.result, WechatCallResult::Success(_));
                match mark_attempted(success) {
                    Ok(()) => {
                        if success {
                            result.delayed_orders += 1;
                        } else {
                            result.failed_orders += 1;
                        }
                    }
                    Err(error) => {
                        eprintln!("[virtual-delay] 订单 {order_id} 防重标记落库失败：{error}");
                        result.failed_orders += 1;
                    }
                }
            }
            Err(_) => {
                result.failed_orders += 1;
            }
        }
    }
    Ok(result)
}

/// 解密地址明文 GC（driver 每日一次，三期）：订单终态满 30 天后清掉密文与虚拟号字段，
/// 保留行与 purged_at 作审计痕迹（合规要求：明文不长存）。
#[tauri::command]
pub fn run_decoded_address_gc_once(app: AppHandle) -> AppResult<DecodedAddressGcResult> {
    let conn = open_connection(&app)?;
    let cutoff = format_shanghai(Utc::now() - Duration::days(30));
    let purged = conn.execute(
        "UPDATE order_decoded_addresses
         SET user_name_enc = NULL,
             tel_number_enc = NULL,
             detail_info_enc = NULL,
             virtual_number = NULL,
             virtual_extension = NULL,
             purged_at = ?1,
             updated_at = ?1
         WHERE purged_at IS NULL
           AND order_id IN (
             SELECT id FROM orders
             WHERE status IN ('completed', 'cancelled') AND updated_at < ?2
           )",
        params![now_shanghai(), cutoff],
    )?;
    Ok(DecodedAddressGcResult {
        purged_addresses: purged as i64,
    })
}

#[tauri::command]
pub fn mark_purchase_task_purchased(
    app: AppHandle,
    task_id: String,
    purchased: bool,
) -> AppResult<PurchaseTaskPurchasedResult> {
    let task_id = task_id.trim().to_string();
    let conn = open_connection(&app)?;
    let status: String = conn
        .query_row(
            "SELECT status FROM purchase_tasks WHERE id = ?1",
            params![task_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("采购任务不存在".to_string()))?;
    if purchased && status != "pending_purchase" {
        return Err(AppError::Validation(format!(
            "采购任务当前状态为 {status}，仅待采购任务可标记已下单"
        )));
    }
    let now = now_shanghai();
    let purchased_at = if purchased { Some(now.clone()) } else { None };
    conn.execute(
        "UPDATE purchase_tasks SET purchased_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![purchased_at, now, task_id],
    )?;
    Ok(PurchaseTaskPurchasedResult {
        task_id,
        purchased_at,
    })
}
