use super::*;

pub(in crate::commands) fn load_purchase_task_shipment_ref(
    conn: &Connection,
    purchase_task_id: &str,
) -> AppResult<PurchaseTaskShipmentRef> {
    let purchase = conn
        .query_row(
            "SELECT pt.id, pt.order_id, pt.shop_id, COALESCE(o.wechat_order_id, '')
             FROM purchase_tasks pt
             JOIN orders o ON o.id = pt.order_id
             WHERE pt.id = ?1",
            [purchase_task_id],
            |row| {
                Ok(PurchaseTaskShipmentRef {
                    purchase_task_id: row.get(0)?,
                    order: ShipmentOrderRef {
                        order_id: row.get(1)?,
                        shop_id: row.get(2)?,
                        wechat_order_id: row.get(3)?,
                    },
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("采购任务不存在".to_string()))?;
    if purchase.order.wechat_order_id.trim().is_empty() {
        return Err(AppError::Validation(
            "采购任务关联订单缺少微信订单号，无法生成发货单".to_string(),
        ));
    }
    Ok(purchase)
}

pub(in crate::commands) fn normalize_shipment_fields(
    deliver_type: Option<i64>,
    delivery_id: Option<&str>,
    delivery_name: Option<&str>,
    waybill_id: Option<&str>,
) -> AppResult<ShipmentFields> {
    let deliver_type = deliver_type.unwrap_or(1);
    if ![1, 3].contains(&deliver_type) {
        return Err(AppError::Validation(
            "发货方式只支持 1 自寄快递或 3 虚拟商品无需物流".to_string(),
        ));
    }
    let delivery_id = delivery_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let delivery_name = delivery_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let waybill_id = waybill_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if deliver_type == 1 && (delivery_id.is_none() || waybill_id.is_none()) {
        return Err(AppError::Validation(
            "自寄快递发货必须填写快递公司 ID 和快递单号".to_string(),
        ));
    }
    Ok(ShipmentFields {
        delivery_id,
        delivery_name,
        waybill_id,
        deliver_type,
    })
}

pub(in crate::commands) fn count_distinct_order_purchase_logistics(
    conn: &Connection,
    order_id: &str,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM (
           SELECT
             COALESCE(supplier_deliver_type, 1) AS deliver_type,
             COALESCE(supplier_delivery_id, '') AS delivery_id,
             COALESCE(supplier_waybill_id, '') AS waybill_id
           FROM purchase_tasks
           WHERE order_id = ?1
             AND status IN ('supplier_shipped', 'wechat_shipped', 'completed')
           GROUP BY deliver_type, delivery_id, waybill_id
         )",
        [order_id],
        |row| row.get(0),
    )?)
}

pub(in crate::commands) fn upsert_order_shipment(
    conn: &Connection,
    order: &ShipmentOrderRef,
    fields: &ShipmentFields,
) -> AppResult<ShipmentUpsertResult> {
    let auto_send_enabled = get_bool_setting(conn, AUTO_SEND_DELIVERY_SETTING, false)?;
    let status = if auto_send_enabled {
        "ready_to_send"
    } else {
        "waiting_confirmation"
    };
    let shipment_id = build_shipment_id(
        &order.order_id,
        fields.deliver_type,
        fields.delivery_id.as_deref(),
        fields.waybill_id.as_deref(),
    );
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO shipments
         (id, order_id, shop_id, wechat_order_id, delivery_id, delivery_name, waybill_id,
          deliver_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(id) DO UPDATE SET
           delivery_id = excluded.delivery_id,
           delivery_name = excluded.delivery_name,
           waybill_id = excluded.waybill_id,
           deliver_type = excluded.deliver_type,
           status = excluded.status,
           error_code = NULL,
           error_summary = NULL,
           updated_at = excluded.updated_at",
        params![
            shipment_id.as_str(),
            order.order_id.as_str(),
            order.shop_id.as_str(),
            order.wechat_order_id.as_str(),
            fields.delivery_id.as_deref(),
            fields.delivery_name.as_deref(),
            fields.waybill_id.as_deref(),
            fields.deliver_type,
            status,
            now
        ],
    )?;
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'supplier_shipped'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), order.order_id],
    )?;
    let message = if auto_send_enabled {
        "物流已回填，自动发货开关已开启，等待提交微信发货".to_string()
    } else {
        "物流已回填，自动发货开关关闭，当前仅进入待确认发货".to_string()
    };
    Ok(ShipmentUpsertResult {
        shipment_id,
        status: status.to_string(),
        auto_send_enabled,
        message,
    })
}

pub(in crate::commands) fn csv_escape(value: &str) -> String {
    if value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
