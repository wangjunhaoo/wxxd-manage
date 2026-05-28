use super::*;

pub(in crate::commands) fn save_synced_order(
    conn: &Connection,
    shop_id: &str,
    wechat_order_id: &str,
    wechat_status: i64,
) -> AppResult<()> {
    let now = now_shanghai();
    let internal_status = match wechat_status {
        10 => "unpaid",
        20 | 21 => "pending_shipment",
        30 => "wechat_shipped",
        100 => "completed",
        250 => "cancelled",
        _ => "synced",
    };
    conn.execute(
        "INSERT INTO orders
         (id, shop_id, wechat_order_id, wechat_status, status, raw_payload, created_at, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7)
         ON CONFLICT(shop_id, wechat_order_id) DO UPDATE SET
           wechat_status = excluded.wechat_status,
           status = excluded.status,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("order-{}-{}", shop_id, wechat_order_id),
            shop_id,
            wechat_order_id,
            wechat_status,
            internal_status,
            serde_json::json!({
                "order_id": wechat_order_id,
                "status": wechat_status
            })
            .to_string(),
            now
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn mark_order_detail_failed(
    app: &AppHandle,
    item: &OrderDetailSyncItem,
    detail_error: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE orders SET detail_error = ?1, updated_at = ?2 WHERE id = ?3",
        params![detail_error, now_shanghai(), item.order_id],
    )?;
    upsert_notification(
        &conn,
        "critical",
        "order_detail_sync",
        &item.order_id,
        Some(&item.shop_id),
        "订单详情同步失败",
        &format!(
            "订单 {} 详情同步失败：{}",
            item.wechat_order_id, detail_error
        ),
        Some(&serde_json::json!({
            "order_id": &item.order_id,
            "wechat_order_id": &item.wechat_order_id,
            "shop_id": &item.shop_id
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn save_order_detail(
    conn: &Connection,
    item: &OrderDetailSyncItem,
    order: &Value,
) -> AppResult<i64> {
    let now = now_shanghai();
    let wechat_status = order.get("status").and_then(Value::as_i64);
    let order_created_at = order.get("create_time").and_then(Value::as_i64);
    let order_updated_at = order.get("update_time").and_then(Value::as_i64);
    let internal_status = wechat_status
        .map(order_status_from_wechat)
        .unwrap_or("pending_shipment");
    let summary_payload = sanitize_order_payload(order);

    conn.execute(
        "UPDATE orders
         SET wechat_status = COALESCE(?1, wechat_status),
             status = ?2,
             raw_payload = ?3,
             order_created_at = ?4,
             order_updated_at = ?5,
             detail_synced_at = ?6,
             detail_error = NULL,
             updated_at = ?6
         WHERE id = ?7",
        params![
            wechat_status,
            internal_status,
            summary_payload.to_string(),
            order_created_at,
            order_updated_at,
            now,
            item.order_id
        ],
    )?;

    let product_infos = order
        .pointer("/order_detail/product_infos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut saved_items = 0i64;
    for (index, product) in product_infos.iter().enumerate() {
        let wechat_product_id = json_value_to_string(product.get("product_id"));
        let wechat_sku_id = json_value_to_string(product.get("sku_id"));
        let out_product_id = product
            .get("out_product_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let out_sku_id = product
            .get("out_sku_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let title = product
            .get("title")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let sku_count = product
            .get("sku_cnt")
            .or_else(|| product.get("sku_count"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let sale_price = product.get("sale_price").and_then(Value::as_i64);
        let real_price = product.get("real_price").and_then(Value::as_i64);
        let item_id = format!("order-item-{}-{}", item.order_id, index);
        conn.execute(
            "INSERT INTO order_items
             (id, order_id, shop_id, wechat_order_id, wechat_product_id, wechat_sku_id,
              out_product_id, out_sku_id, title, sku_count, sale_price, real_price,
              raw_payload, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
             ON CONFLICT(id) DO UPDATE SET
               wechat_product_id = excluded.wechat_product_id,
               wechat_sku_id = excluded.wechat_sku_id,
               out_product_id = excluded.out_product_id,
               out_sku_id = excluded.out_sku_id,
               title = excluded.title,
               sku_count = excluded.sku_count,
               sale_price = excluded.sale_price,
               real_price = excluded.real_price,
               raw_payload = excluded.raw_payload,
               updated_at = excluded.updated_at",
            params![
                item_id,
                item.order_id,
                item.shop_id,
                item.wechat_order_id,
                wechat_product_id,
                wechat_sku_id,
                out_product_id,
                out_sku_id,
                title,
                sku_count,
                sale_price,
                real_price,
                sanitize_order_item_payload(product).to_string(),
                now
            ],
        )?;
        saved_items += 1;
    }
    Ok(saved_items)
}

pub(in crate::commands) fn save_failed_aftersale(
    conn: &Connection,
    shop_id: &str,
    wechat_aftersale_id: &str,
    reason: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO aftersales
         (id, shop_id, wechat_aftersale_id, status, reason, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'sync_failed', ?4, ?5, ?6, ?6)
         ON CONFLICT(shop_id, wechat_aftersale_id) DO UPDATE SET
           status = excluded.status,
           reason = excluded.reason,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("aftersale-{shop_id}-{wechat_aftersale_id}"),
            shop_id,
            wechat_aftersale_id,
            reason,
            serde_json::json!({
                "after_sale_order_id": wechat_aftersale_id,
                "error": reason
            })
            .to_string(),
            now
        ],
    )?;
    upsert_notification(
        conn,
        "critical",
        "aftersale",
        wechat_aftersale_id,
        Some(shop_id),
        "售后详情同步失败",
        &format!("售后单 {wechat_aftersale_id} 同步失败：{reason}"),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "wechat_aftersale_id": wechat_aftersale_id,
            "reason": reason
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn save_synced_aftersale(
    conn: &Connection,
    shop_id: &str,
    after_sale_order: &Value,
) -> AppResult<()> {
    let wechat_aftersale_id = json_value_to_string(after_sale_order.get("after_sale_order_id"))
        .or_else(|| json_value_to_string(after_sale_order.get("aftersale_order_id")))
        .ok_or_else(|| AppError::Validation("售后详情缺少 after_sale_order_id".to_string()))?;
    let status = json_value_to_string(after_sale_order.get("status"))
        .unwrap_or_else(|| "unknown".to_string());
    let aftersale_type = json_value_to_string(after_sale_order.get("type"))
        .or_else(|| json_value_to_string(after_sale_order.get("after_sale_type")));
    let reason = json_value_to_string(after_sale_order.get("reason_text"))
        .or_else(|| json_value_to_string(after_sale_order.get("reason")));
    let wechat_order_id = json_value_to_string(after_sale_order.get("order_id"));
    let local_order_id = match &wechat_order_id {
        Some(wechat_order_id) => conn
            .query_row(
                "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2 LIMIT 1",
                params![shop_id, wechat_order_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let refund_amount_cents = extract_refund_amount_cents(after_sale_order);
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO aftersales
         (id, shop_id, wechat_aftersale_id, order_id, wechat_order_id, status,
          aftersale_type, reason, refund_amount_cents, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(shop_id, wechat_aftersale_id) DO UPDATE SET
           order_id = excluded.order_id,
           wechat_order_id = excluded.wechat_order_id,
           status = excluded.status,
           aftersale_type = excluded.aftersale_type,
           reason = excluded.reason,
           refund_amount_cents = excluded.refund_amount_cents,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("aftersale-{shop_id}-{wechat_aftersale_id}"),
            shop_id,
            wechat_aftersale_id,
            local_order_id,
            wechat_order_id,
            status,
            aftersale_type,
            reason,
            refund_amount_cents,
            sanitize_aftersale_payload(after_sale_order).to_string(),
            now
        ],
    )?;

    if is_active_aftersale_status(&status) {
        upsert_notification(
            conn,
            "warning",
            "aftersale",
            &wechat_aftersale_id,
            Some(shop_id),
            "微信售后待处理",
            &format!(
                "售后单 {} 当前状态为 {}，需人工判断是否同意、拒绝或补充凭证。",
                wechat_aftersale_id, status
            ),
            Some(&serde_json::json!({
                "shop_id": shop_id,
                "wechat_aftersale_id": &wechat_aftersale_id,
                "wechat_order_id": &wechat_order_id,
                "status": &status,
                "reason": &reason
            })),
        )?;
    }

    let local_order_id: Option<String> = conn
        .query_row(
            "SELECT order_id FROM aftersales WHERE shop_id = ?1 AND wechat_aftersale_id = ?2",
            params![shop_id, wechat_aftersale_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if let Some(order_id) = local_order_id {
        if is_active_aftersale_status(&status) {
            conn.execute(
                "UPDATE orders
                 SET status = 'aftersale_active', updated_at = ?1
                 WHERE id = ?2 AND status != 'cancelled'",
                params![now_shanghai(), order_id],
            )?;
        }
        if is_refund_success_aftersale_status(&status) {
            if let Some(amount_cents) = refund_amount_cents.filter(|value| *value > 0) {
                conn.execute(
                    "INSERT INTO order_profit_adjustments
                     (id, order_id, kind, amount_cents, note, created_at)
                     VALUES (?1, ?2, 'refund', ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET
                       amount_cents = excluded.amount_cents,
                       note = excluded.note,
                       created_at = excluded.created_at",
                    params![
                        format!("aftersale-refund-{shop_id}-{wechat_aftersale_id}"),
                        order_id,
                        amount_cents,
                        format!("微信售后退款：{wechat_aftersale_id}"),
                        now_shanghai()
                    ],
                )?;
            }
        }
    }
    Ok(())
}

pub(in crate::commands) fn save_failed_guarantee_order(
    conn: &Connection,
    shop_id: &str,
    guarantee_order_id: &str,
    reason: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO guarantee_orders
         (id, shop_id, guarantee_order_id, status, apply_reason, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'sync_failed', ?4, ?5, ?6, ?6)
         ON CONFLICT(shop_id, guarantee_order_id) DO UPDATE SET
           status = excluded.status,
           apply_reason = excluded.apply_reason,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("guarantee-{shop_id}-{guarantee_order_id}"),
            shop_id,
            guarantee_order_id,
            reason,
            serde_json::json!({
                "guarantee_order_id": guarantee_order_id,
                "error": reason
            })
            .to_string(),
            now
        ],
    )?;
    upsert_notification(
        conn,
        "critical",
        "guarantee_sync",
        guarantee_order_id,
        Some(shop_id),
        "纠纷单详情同步失败",
        &format!("纠纷单 {guarantee_order_id} 同步失败：{reason}"),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "guarantee_order_id": guarantee_order_id,
            "reason": reason
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn save_synced_guarantee_order(
    conn: &Connection,
    shop_id: &str,
    guarantee_order: &Value,
) -> AppResult<()> {
    let guarantee_order_id = json_value_to_string(guarantee_order.get("guarantee_order_id"))
        .ok_or_else(|| AppError::Validation("纠纷单详情缺少 guarantee_order_id".to_string()))?;
    let status = json_value_to_string(guarantee_order.get("status"))
        .unwrap_or_else(|| "unknown".to_string());
    let guarantee_type = json_value_to_i64(guarantee_order.get("type"));
    let wechat_order_id = json_value_to_string(guarantee_order.get("order_id"));
    let local_order_id = match &wechat_order_id {
        Some(wechat_order_id) => conn
            .query_row(
                "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2 LIMIT 1",
                params![shop_id, wechat_order_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let apply_reason = json_value_to_string(guarantee_order.get("apply_reason"))
        .or_else(|| json_value_to_string(guarantee_order.get("refund_reason_text")));
    let pay_amount_cents = json_value_to_i64(guarantee_order.get("pay_amount"))
        .or_else(|| json_value_to_i64(guarantee_order.pointer("/bad_pay_info/pay_fee")))
        .or_else(|| {
            json_value_to_i64(guarantee_order.pointer("/fake_one_pay_four_info/total_pay_fee"))
        });
    let merchant_refuse_reason =
        json_value_to_string(guarantee_order.get("merchant_refuse_reason"));
    let created_time = json_value_to_i64(guarantee_order.get("create_time"));
    let updated_time = json_value_to_i64(guarantee_order.get("update_time"));
    let expire_time = json_value_to_i64(guarantee_order.get("expire_time"));
    let complete_time = json_value_to_i64(guarantee_order.get("complete_time"));
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO guarantee_orders
         (id, shop_id, guarantee_order_id, order_id, wechat_order_id, guarantee_type,
          status, apply_reason, pay_amount_cents, merchant_refuse_reason, raw_payload,
          created_time, updated_time, expire_time, complete_time, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)
         ON CONFLICT(shop_id, guarantee_order_id) DO UPDATE SET
           order_id = excluded.order_id,
           wechat_order_id = excluded.wechat_order_id,
           guarantee_type = excluded.guarantee_type,
           status = excluded.status,
           apply_reason = excluded.apply_reason,
           pay_amount_cents = excluded.pay_amount_cents,
           merchant_refuse_reason = excluded.merchant_refuse_reason,
           raw_payload = excluded.raw_payload,
           created_time = excluded.created_time,
           updated_time = excluded.updated_time,
           expire_time = excluded.expire_time,
           complete_time = excluded.complete_time,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("guarantee-{shop_id}-{guarantee_order_id}"),
            shop_id,
            guarantee_order_id,
            local_order_id,
            wechat_order_id,
            guarantee_type,
            status,
            apply_reason,
            pay_amount_cents,
            merchant_refuse_reason,
            sanitize_aftersale_payload(guarantee_order).to_string(),
            created_time,
            updated_time,
            expire_time,
            complete_time,
            now
        ],
    )?;

    if is_active_guarantee_status(&status) {
        upsert_notification(
            conn,
            "critical",
            "guarantee_order",
            &guarantee_order_id,
            Some(shop_id),
            "微信纠纷单待处理",
            &format!(
                "纠纷单 {} 当前状态为 {}（{}），需人工核对凭证、责任方和处理时限。",
                guarantee_order_id,
                status,
                guarantee_status_text(&status)
            ),
            Some(&serde_json::json!({
                "shop_id": shop_id,
                "guarantee_order_id": &guarantee_order_id,
                "wechat_order_id": &wechat_order_id,
                "status": &status,
                "status_text": guarantee_status_text(&status),
                "apply_reason": &apply_reason,
                "pay_amount_cents": pay_amount_cents
            })),
        )?;
    }

    if let Some(order_id) = conn
        .query_row(
            "SELECT order_id FROM guarantee_orders WHERE shop_id = ?1 AND guarantee_order_id = ?2",
            params![shop_id, guarantee_order_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
    {
        if is_active_guarantee_status(&status) {
            conn.execute(
                "UPDATE orders
                 SET status = 'aftersale_active', updated_at = ?1
                 WHERE id = ?2 AND status != 'cancelled'",
                params![now_shanghai(), order_id],
            )?;
        }
    }
    Ok(())
}

pub(in crate::commands) fn order_status_from_wechat(status: i64) -> &'static str {
    match status {
        10 => "unpaid",
        20 | 21 => "pending_shipment",
        30 => "wechat_shipped",
        100 => "completed",
        250 => "cancelled",
        _ => "synced",
    }
}

pub(in crate::commands) fn sanitize_order_payload(order: &Value) -> Value {
    serde_json::json!({
        "order_id": json_value_to_string(order.get("order_id")),
        "status": order.get("status").and_then(Value::as_i64),
        "create_time": order.get("create_time").and_then(Value::as_i64),
        "update_time": order.get("update_time").and_then(Value::as_i64),
        "product_infos": order
            .pointer("/order_detail/product_infos")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(sanitize_order_item_payload).collect::<Vec<_>>())
            .unwrap_or_default()
    })
}

pub(in crate::commands) fn sanitize_order_item_payload(product: &Value) -> Value {
    serde_json::json!({
        "product_id": product.get("product_id").cloned().unwrap_or(Value::Null),
        "sku_id": product.get("sku_id").cloned().unwrap_or(Value::Null),
        "out_product_id": product.get("out_product_id").cloned().unwrap_or(Value::Null),
        "out_sku_id": product.get("out_sku_id").cloned().unwrap_or(Value::Null),
        "title": product.get("title").cloned().unwrap_or(Value::Null),
        "sku_cnt": product.get("sku_cnt").cloned().unwrap_or(Value::Null),
        "sale_price": product.get("sale_price").cloned().unwrap_or(Value::Null),
        "real_price": product.get("real_price").cloned().unwrap_or(Value::Null),
        "thumb_img": product.get("thumb_img").cloned().unwrap_or(Value::Null)
    })
}

pub(in crate::commands) fn sanitize_aftersale_payload(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (key, child) in map {
                if is_sensitive_json_key(key) {
                    sanitized.insert(key.clone(), Value::String("[redacted]".to_string()));
                } else {
                    sanitized.insert(key.clone(), sanitize_aftersale_payload(child));
                }
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_aftersale_payload).collect()),
        _ => value.clone(),
    }
}

pub(in crate::commands) fn is_sensitive_json_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "openid", "open_id", "unionid", "name", "tel", "mobile", "phone", "address", "receiver",
        "contact",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(in crate::commands) fn json_value_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

pub(in crate::commands) fn json_value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(value)) => value.as_i64(),
        Some(Value::String(value)) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(in crate::commands) fn json_value_to_bool(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Number(value)) => value.as_i64().map(|value| value != 0),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(in crate::commands) fn json_value_to_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(in crate::commands) fn extract_refund_amount_cents(after_sale_order: &Value) -> Option<i64> {
    [
        "/refund_info/refund_fee",
        "/refund_info/refund_amount",
        "/refund_info/refund_price",
        "/refund_resp/refund_fee",
        "/refund_resp/refund_amount",
        "/refund_amount",
        "/refund_fee",
    ]
    .iter()
    .find_map(|path| json_value_to_i64(after_sale_order.pointer(path)))
    .or_else(|| {
        after_sale_order
            .pointer("/refund_info/refund_product_infos")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        json_value_to_i64(item.get("refund_amount"))
                            .or_else(|| json_value_to_i64(item.get("refund_fee")))
                    })
                    .sum::<i64>()
            })
            .filter(|sum| *sum > 0)
    })
}

pub(in crate::commands) fn is_active_aftersale_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_uppercase().as_str(),
        "USER_CANCELD"
            | "USER_CANCELLED"
            | "RETURN_CLOSED"
            | "MERCHANT_REFUND_SUCCESS"
            | "MERCHANT_RETURN_SUCCESS"
            | "MERCHANT_REFUND_RETRY_FAIL"
            | "MERCHANT_FAIL"
            | "MERCHANT_EXCHANGE_SUCCESS"
            | "SYNC_FAILED"
    )
}

pub(in crate::commands) fn is_refund_success_aftersale_status(status: &str) -> bool {
    matches!(
        status.to_ascii_uppercase().as_str(),
        "MERCHANT_REFUND_SUCCESS" | "MERCHANT_RETURN_SUCCESS"
    )
}

pub(in crate::commands) fn is_active_guarantee_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_uppercase().as_str(),
        "STATUS_NO_NEED_PAY" | "STATUS_PAY_SUCC" | "STATUS_USER_CANCEL" | "SYNC_FAILED"
    )
}

pub(in crate::commands) fn guarantee_type_text(guarantee_type: Option<i64>) -> &'static str {
    match guarantee_type {
        Some(1) => "假一赔三/四",
        Some(2) => "坏损包退",
        Some(0) => "全部类型",
        _ => "未知类型",
    }
}

pub(in crate::commands) fn guarantee_status_text(status: &str) -> &'static str {
    match status.to_ascii_uppercase().as_str() {
        "STATUS_WAIT_MERCHANT_HANDLE" => "等待商家处理",
        "STATUS_WAIT_PLATFORM_HANDLE" => "等待平台处理",
        "STATUS_WAIT_USER_CONFIRM" => "等待用户确认",
        "STATUS_WAIT_MERCHANT_PROOF" => "等待商家举证",
        "STATUS_WAIT_USER_PROOF" => "等待用户举证",
        "STATUS_WAIT_BOTH_PROOF" => "等待双方举证",
        "STATUS_WAIT_OP_COMFIRM" => "等待 OP 确认",
        "STATUS_WAIT_PAYSCORE_DONE" => "等待支付分付款",
        "STATUS_NO_NEED_PAY" => "无需赔付",
        "STATUS_PAYING" => "赔付中",
        "STATUS_PAY_BLOCK" => "赔付金额异常待确认",
        "STATUS_PAY_SUCC" => "赔付成功",
        "STATUS_PAY_FAIL" => "赔付失败",
        "STATUS_USER_CANCEL" => "用户取消申请",
        "SYNC_FAILED" => "同步失败",
        _ => "未知状态",
    }
}

pub(in crate::commands) fn normalize_guarantee_handling_status(
    status: &str,
) -> AppResult<&'static str> {
    match status.trim() {
        "pending" => Ok("pending"),
        "in_progress" => Ok("in_progress"),
        "waiting_supplier" => Ok("waiting_supplier"),
        "evidence_ready" => Ok("evidence_ready"),
        "resolved" => Ok("resolved"),
        "ignored" => Ok("ignored"),
        _ => Err(AppError::Validation(
            "纠纷跟进状态必须是 pending/in_progress/waiting_supplier/evidence_ready/resolved/ignored"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn guarantee_handling_status_text(status: &str) -> &'static str {
    match status {
        "pending" => "待跟进",
        "in_progress" => "跟进中",
        "waiting_supplier" => "等供应商",
        "evidence_ready" => "凭证已整理",
        "resolved" => "已处理",
        "ignored" => "无需处理",
        _ => "未知跟进状态",
    }
}

pub(in crate::commands) fn aftersale_evidence_type_text(evidence_type: &str) -> &'static str {
    match evidence_type {
        "image" => "图片",
        "text" => "文字说明",
        "chat_record" => "沟通记录",
        "logistics" => "物流凭证",
        "supplier_proof" => "供应商凭证",
        "quality_check" => "质检凭证",
        "other" => "其他",
        _ => "未知凭证",
    }
}

pub(in crate::commands) fn aftersale_evidence_status_text(status: &str) -> &'static str {
    match status {
        "draft" => "草稿",
        "ready" => "已整理",
        "used" => "已使用",
        "archived" => "已归档",
        _ => "未知状态",
    }
}

pub(in crate::commands) fn supplier_aftersale_followup_type_text(
    followup_type: &str,
) -> &'static str {
    match followup_type {
        "contact" => "联系供应商",
        "evidence_request" => "索要凭证",
        "evidence_received" => "收到凭证",
        "compensation" => "赔付沟通",
        "return_refund" => "退货退款",
        "other" => "其他协同",
        _ => "未知协同",
    }
}

pub(in crate::commands) fn supplier_aftersale_followup_status_text(status: &str) -> &'static str {
    match status {
        "pending" => "待处理",
        "contacted" => "已联系",
        "waiting_supplier" => "等供应商",
        "evidence_ready" => "凭证已备",
        "compensation_pending" => "赔付待确认",
        "closed" => "已关闭",
        _ => "未知状态",
    }
}
