use super::*;

pub(in crate::commands) fn resolve_shipment_order(
    conn: &Connection,
    request: &ShipmentRecordRequest,
) -> AppResult<ShipmentOrderRef> {
    if let Some(order_id) = request
        .order_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return conn
            .query_row(
                "SELECT id, shop_id, wechat_order_id
                 FROM orders
                 WHERE id = ?1",
                [order_id],
                |row| {
                    Ok(ShipmentOrderRef {
                        order_id: row.get(0)?,
                        shop_id: row.get(1)?,
                        wechat_order_id: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("订单不存在，无法回填物流".to_string()));
    }

    let shop_id = request
        .shop_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("缺少 shop_id 或 order_id".to_string()))?;
    let wechat_order_id = request
        .wechat_order_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("缺少 wechat_order_id 或 order_id".to_string()))?;

    conn.query_row(
        "SELECT id, shop_id, wechat_order_id
         FROM orders
         WHERE shop_id = ?1 AND wechat_order_id = ?2",
        params![shop_id, wechat_order_id],
        |row| {
            Ok(ShipmentOrderRef {
                order_id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_order_id: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Validation("订单不存在，无法回填物流".to_string()))
}

pub(in crate::commands) fn count_delivery_shipments(
    conn: &Connection,
    status: Option<&str>,
) -> AppResult<i64> {
    if let Some(status) = status {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM shipments WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?)
    } else {
        Ok(conn.query_row("SELECT COUNT(*) FROM shipments", [], |row| row.get(0))?)
    }
}

pub(in crate::commands) fn load_delivery_shipment_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<ShipmentView>> {
    let sql = format!(
        "SELECT
           sh.id,
           sh.order_id,
           sh.shop_id,
           COALESCE(s.name, sh.shop_id),
           sh.wechat_order_id,
           sh.delivery_id,
           sh.delivery_name,
           sh.waybill_id,
           sh.deliver_type,
           sh.status,
           sh.error_code,
           sh.error_summary,
           sh.submitted_at,
           sh.created_at,
           sh.updated_at
         FROM shipments sh
         LEFT JOIN shops s ON s.id = sh.shop_id
         {}
         ORDER BY sh.updated_at DESC, sh.created_at DESC
         LIMIT ?{}",
        if status.is_some() {
            "WHERE sh.status = ?1"
        } else {
            ""
        },
        if status.is_some() { 2 } else { 1 }
    );
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(ShipmentView {
            id: row.get(0)?,
            order_id: row.get(1)?,
            shop_id: row.get(2)?,
            shop_name: row.get(3)?,
            wechat_order_id: row.get(4)?,
            delivery_id: row.get(5)?,
            delivery_name: row.get(6)?,
            waybill_id: row.get(7)?,
            deliver_type: row.get(8)?,
            status: row.get(9)?,
            error_code: row.get(10)?,
            error_summary: row.get(11)?,
            submitted_at: row.get(12)?,
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    };
    let rows = if let Some(status) = status {
        stmt.query_map(params![status, limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub(in crate::commands) fn load_delivery_submission_candidates(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<ShipmentCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT id, order_id, shop_id, wechat_order_id, delivery_id, waybill_id, deliver_type
         FROM shipments
         WHERE status = 'ready_to_send'
         ORDER BY created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(ShipmentCandidate {
                shipment_id: row.get(0)?,
                order_id: row.get(1)?,
                shop_id: row.get(2)?,
                wechat_order_id: row.get(3)?,
                delivery_id: row.get(4)?,
                waybill_id: row.get(5)?,
                deliver_type: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(in crate::commands) fn build_send_delivery_payload(
    conn: &Connection,
    shipment: &ShipmentCandidate,
) -> AppResult<Value> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(wechat_product_id, ''), COALESCE(wechat_sku_id, ''), sku_count
         FROM order_items
         WHERE order_id = ?1
         ORDER BY created_at ASC",
    )?;
    let products = stmt
        .query_map([shipment.order_id.as_str()], |row| {
            Ok(ShipmentProductInfo {
                product_id: row.get(0)?,
                sku_id: row.get(1)?,
                product_cnt: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if products.is_empty() {
        return Err(AppError::Validation(
            "订单缺少订单项，无法构造发货商品列表".to_string(),
        ));
    }
    if products
        .iter()
        .any(|item| item.product_id.trim().is_empty() || item.sku_id.trim().is_empty())
    {
        return Err(AppError::Validation(
            "订单项缺少微信 product_id 或 sku_id，无法提交发货".to_string(),
        ));
    }
    if shipment.deliver_type == 1
        && (shipment
            .delivery_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
            || shipment
                .waybill_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty())
    {
        return Err(AppError::Validation(
            "自寄快递发货缺少快递公司 ID 或快递单号".to_string(),
        ));
    }

    let product_infos = products
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "product_id": item.product_id,
                "sku_id": item.sku_id,
                "product_cnt": item.product_cnt.max(1)
            })
        })
        .collect::<Vec<_>>();
    let mut delivery = serde_json::json!({
        "deliver_type": shipment.deliver_type,
        "product_infos": product_infos
    });
    if shipment.deliver_type == 1 {
        if let Some(delivery_id) = &shipment.delivery_id {
            delivery["delivery_id"] = serde_json::json!(delivery_id);
        }
        if let Some(waybill_id) = &shipment.waybill_id {
            delivery["waybill_id"] = serde_json::json!(waybill_id);
        }
    }

    Ok(serde_json::json!({
        "order_id": shipment.wechat_order_id,
        "delivery_list": [delivery]
    }))
}

pub(in crate::commands) fn mark_shipment_failed(
    conn: &Connection,
    shipment: &ShipmentCandidate,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE shipments
         SET status = 'send_failed',
             error_code = ?1,
             error_summary = ?2,
             updated_at = ?3
         WHERE id = ?4",
        params![error_code, error_summary, now, shipment.shipment_id],
    )?;
    conn.execute(
        "UPDATE orders
         SET status = CASE WHEN status IN ('completed', 'cancelled') THEN status ELSE 'exception' END,
             updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), shipment.order_id],
    )?;
    upsert_notification(
        conn,
        "critical",
        "delivery_shipment",
        &shipment.shipment_id,
        Some(&shipment.shop_id),
        "微信发货提交失败",
        &format!(
            "订单 {} 的物流单提交微信失败：{}（{}）",
            shipment.wechat_order_id, error_summary, error_code
        ),
        Some(&serde_json::json!({
            "order_id": &shipment.order_id,
            "wechat_order_id": &shipment.wechat_order_id,
            "shipment_id": &shipment.shipment_id,
            "error_code": error_code
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn mark_shipment_submitted(
    conn: &Connection,
    shipment: &ShipmentCandidate,
    payload: &Value,
    response: &Value,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE shipments
         SET status = 'wechat_shipped',
             send_payload = ?1,
             error_code = NULL,
             error_summary = NULL,
             submitted_at = ?2,
             updated_at = ?2
         WHERE id = ?3",
        params![
            serde_json::json!({
                "request": payload,
                "response": response
            })
            .to_string(),
            now,
            shipment.shipment_id
        ],
    )?;
    conn.execute(
        "UPDATE orders SET status = 'wechat_shipped', updated_at = ?1 WHERE id = ?2",
        params![now_shanghai(), shipment.order_id],
    )?;
    Ok(())
}

pub(in crate::commands) fn build_shipment_id(
    order_id: &str,
    deliver_type: i64,
    delivery_id: Option<&str>,
    waybill_id: Option<&str>,
) -> String {
    let delivery = delivery_id.unwrap_or("virtual").trim().replace('/', "_");
    let waybill = waybill_id.unwrap_or("no-waybill").trim().replace('/', "_");
    format!("shipment-{order_id}-{deliver_type}-{delivery}-{waybill}")
}
