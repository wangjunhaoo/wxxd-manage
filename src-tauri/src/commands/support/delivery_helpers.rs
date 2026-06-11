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
           sh.blocked_reason,
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
            blocked_reason: row.get(12)?,
            submitted_at: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
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
    // blocked（守卫拦截）的物流单也纳入候选：每轮重查守卫条件，条件解除即自动恢复提交（自愈）。
    // 审查修复①：分页按「订单」而非按「shipment 行」——行级 LIMIT 会把同一订单的多包裹
    // 切到不同批次造成部分提交（官方对剩余包裹可能永久拒收），改为先取 LIMIT 个订单再取其全部候选行。
    // 审查修复②：submitting（send 在途）行 10 分钟后回收——提交方 tick 中途死亡时自愈。
    let stale_submitting_cutoff = format_shanghai(Utc::now() - Duration::minutes(10));
    let mut stmt = conn.prepare(
        "SELECT id, order_id, shop_id, wechat_order_id, delivery_id, waybill_id, deliver_type, status
         FROM shipments
         WHERE (status IN ('ready_to_send', 'blocked')
                OR (status = 'submitting' AND updated_at < ?2))
           AND order_id IN (
             SELECT order_id FROM shipments
             WHERE status IN ('ready_to_send', 'blocked')
                OR (status = 'submitting' AND updated_at < ?2)
             GROUP BY order_id
             ORDER BY MIN(created_at) ASC
             LIMIT ?1
           )
         ORDER BY order_id ASC, created_at ASC",
    )?;
    let items = stmt
        .query_map(params![limit, stale_submitting_cutoff], |row| {
            Ok(ShipmentCandidate {
                shipment_id: row.get(0)?,
                order_id: row.get(1)?,
                shop_id: row.get(2)?,
                wechat_order_id: row.get(3)?,
                delivery_id: row.get(4)?,
                waybill_id: row.get(5)?,
                deliver_type: row.get(6)?,
                status: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// 发货前置守卫（订单履约重设计 §8.1）：把官方守卫错误码前移到本地检查，绝不盲调 API。
/// 返回 Some(拦截原因) 表示必须拦截；None 表示放行。
pub(in crate::commands) fn check_shipment_send_guard(
    conn: &Connection,
    shipment: &ShipmentCandidate,
) -> AppResult<Option<String>> {
    let row: Option<(Option<i64>, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT address_under_review, change_sku_state, has_active_aftersale
             FROM orders WHERE id = ?1",
            params![shipment.order_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((address_under_review, change_sku_state, has_active_aftersale)) = row else {
        return Ok(Some("订单不存在，无法发货".to_string()));
    };
    if address_under_review == Some(1) {
        return Ok(Some(
            "有买家改址申请待处理（对应微信错误码 10020115），请先到申请收件箱裁决".to_string(),
        ));
    }
    if change_sku_state == Some(3) {
        return Ok(Some(
            "有发货前换SKU申请待处理（对应微信错误码 10020276），请先到申请收件箱裁决".to_string(),
        ));
    }
    if has_active_aftersale == Some(1) {
        return Ok(Some(
            "订单存在未完成的售后/纠纷（对应微信错误码 108009），售后终结后自动恢复提交".to_string(),
        ));
    }
    Ok(None)
}

/// 守卫拦截落库：物流单转 blocked 并记录原因；条件解除后下一轮自动恢复（见候选查询）。
pub(in crate::commands) fn mark_shipment_blocked(
    conn: &Connection,
    shipment: &ShipmentCandidate,
    reason: &str,
    was_blocked: bool,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE shipments
         SET status = 'blocked', blocked_reason = ?1, updated_at = ?2
         WHERE id = ?3",
        params![reason, now, shipment.shipment_id],
    )?;
    if !was_blocked {
        upsert_notification(
            conn,
            "warning",
            "delivery_blocked",
            &shipment.shipment_id,
            Some(&shipment.shop_id),
            "发货被前置守卫拦截",
            &format!(
                "订单 {} 的物流单未提交微信发货：{reason}",
                shipment.wechat_order_id
            ),
            Some(&serde_json::json!({
                "order_id": &shipment.order_id,
                "wechat_order_id": &shipment.wechat_order_id,
                "shipment_id": &shipment.shipment_id
            })),
        )?;
    }
    Ok(())
}

/// 整单订单项 → send 的 product_infos（单包裹发货 / 无包裹映射时的兜底）。
fn load_whole_order_product_infos(conn: &Connection, order_id: &str) -> AppResult<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(wechat_product_id, ''), COALESCE(wechat_sku_id, ''), sku_count
         FROM order_items
         WHERE order_id = ?1
         ORDER BY created_at ASC",
    )?;
    let products = stmt
        .query_map([order_id], |row| {
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
    Ok(products
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "product_id": item.product_id,
                "sku_id": item.sku_id,
                "product_cnt": item.product_cnt.max(1)
            })
        })
        .collect())
}

/// local_send 包裹的商品映射 → send 的 product_infos（多包裹拆单发货的商品归属来源）。
fn load_package_product_infos(
    conn: &Connection,
    shipment: &ShipmentCandidate,
) -> AppResult<Vec<Value>> {
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
        .query_map(
            params![
                shipment.order_id,
                shipment.delivery_id.as_deref().unwrap_or(""),
                shipment.waybill_id.as_deref().unwrap_or("")
            ],
            |row| {
                Ok(ShipmentProductInfo {
                    product_id: row.get(0)?,
                    sku_id: row.get(1)?,
                    product_cnt: row.get(2)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if rows
        .iter()
        .any(|item| item.product_id.trim().is_empty() || item.sku_id.trim().is_empty())
    {
        return Err(AppError::Validation(
            "包裹商品映射缺少微信 product_id 或 sku_id，无法提交发货".to_string(),
        ));
    }
    Ok(rows
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "product_id": item.product_id,
                "sku_id": item.sku_id,
                "product_cnt": item.product_cnt.max(1)
            })
        })
        .collect())
}

/// 按订单聚合构造一次 send 的 payload（订单履约三期 §8）：
/// 同一订单的多个 shipment 合成多元素 delivery_list 一次提交（官方拆单发货语义）；
/// 单 shipment 且无包裹映射时回退整单订单项（与历史行为一致）。
pub(in crate::commands) fn build_group_send_delivery_payload(
    conn: &Connection,
    group: &[ShipmentCandidate],
) -> AppResult<Value> {
    let first = group
        .first()
        .ok_or_else(|| AppError::Validation("发货分组为空".to_string()))?;
    let mut delivery_list = Vec::with_capacity(group.len());
    for shipment in group {
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
        let package_infos = load_package_product_infos(conn, shipment)?;
        let product_infos = if !package_infos.is_empty() {
            package_infos
        } else if group.len() == 1 {
            load_whole_order_product_infos(conn, &shipment.order_id)?
        } else {
            return Err(AppError::Validation(
                "多包裹发货缺少包裹商品映射，无法确定各运单包含的商品".to_string(),
            ));
        };
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
        delivery_list.push(delivery);
    }

    Ok(serde_json::json!({
        "order_id": first.wechat_order_id,
        "delivery_list": delivery_list
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
             blocked_reason = NULL,
             updated_at = ?3
         WHERE id = ?4",
        params![error_code, error_summary, now, shipment.shipment_id],
    )?;
    // 审查修复：官方已发货（wechat_shipped，rank 40）的订单不得被失败回退打成 exception——
    // 微信侧是事实源，外部已发货后本地 send 失败只是重复提交被拒
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'exception'
         END,
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
             blocked_reason = NULL,
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
    // 三期双轴语义：本地提交成功只到 shipping_submitted（rank 38），
    // wechat_shipped（rank 40）由详情回刷镜像官方 status=30 时经 apply_wechat_status 确认
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'shipping_submitted'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), shipment.order_id],
    )?;
    // 对应 local_send 包裹推进到 submitted（镜像确认后变 confirmed）
    conn.execute(
        "UPDATE order_packages
         SET status = 'submitted', updated_at = ?1
         WHERE order_id = ?2
           AND COALESCE(delivery_id, '') = ?3
           AND COALESCE(waybill_id, '') = ?4
           AND status != 'confirmed'",
        params![
            now_shanghai(),
            shipment.order_id,
            shipment.delivery_id.as_deref().unwrap_or(""),
            shipment.waybill_id.as_deref().unwrap_or("")
        ],
    )?;
    Ok(())
}

/// 发货成功后镜像进微信商家备注的摘要（merchantnotes/update，best-effort 不阻断主流程）：
/// 货源（供应商名#采购商品号）+ 各包裹运单摘要，按字符安全截断防超官方备注长度上限。
pub(in crate::commands) fn build_merchant_notes_summary(
    conn: &Connection,
    order_id: &str,
    group: &[ShipmentCandidate],
) -> AppResult<String> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT COALESCE(supplier_name, ''), COALESCE(supplier_product_id, '')
         FROM purchase_tasks
         WHERE order_id = ?1
           AND status IN ('supplier_shipped', 'wechat_shipped', 'completed')",
    )?;
    let suppliers = stmt
        .query_map([order_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let supplier_part = suppliers
        .iter()
        .filter(|(name, pid)| !name.is_empty() || !pid.is_empty())
        .map(|(name, pid)| {
            if pid.is_empty() {
                name.clone()
            } else if name.is_empty() {
                pid.clone()
            } else {
                format!("{name}#{pid}")
            }
        })
        .collect::<Vec<_>>()
        .join("、");
    let waybill_part = group
        .iter()
        .map(|shipment| {
            let company = shipment.delivery_id.as_deref().unwrap_or("无需物流");
            match shipment.waybill_id.as_deref() {
                Some(waybill) if !waybill.is_empty() => format!("{company} {waybill}"),
                _ => company.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("；");
    let mut notes = if supplier_part.is_empty() {
        format!("[代发] 已提交 {} 个包裹：{}", group.len(), waybill_part)
    } else {
        format!(
            "[代发] 货源：{supplier_part}；已提交 {} 个包裹：{}",
            group.len(),
            waybill_part
        )
    };
    if notes.chars().count() > 450 {
        notes = notes.chars().take(450).collect();
    }
    Ok(notes)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("内存库");
        conn.pragma_update(None, "foreign_keys", "OFF")
            .expect("关闭外键");
        crate::storage::migrate(&conn).expect("迁移");
        conn
    }

    fn seed_order_with_items(conn: &Connection) -> String {
        save_synced_order(conn, "shop-1", "9100000002").expect("订单落库");
        let order_id = "order-shop-1-9100000002".to_string();
        let now = now_shanghai();
        for (idx, (product_id, sku_id)) in
            [("wxp-1", "wxs-1"), ("wxp-2", "wxs-2")].iter().enumerate()
        {
            conn.execute(
                "INSERT INTO order_items
                 (id, order_id, shop_id, wechat_order_id, wechat_product_id, wechat_sku_id,
                  sku_count, raw_payload, created_at, updated_at)
                 VALUES (?1, ?2, 'shop-1', '9100000002', ?3, ?4, 1, '{}', ?5, ?5)",
                params![format!("item-{idx}"), order_id, product_id, sku_id, now],
            )
            .expect("订单项");
        }
        order_id
    }

    fn seed_package(
        conn: &Connection,
        order_id: &str,
        delivery_id: &str,
        waybill_id: &str,
        product_id: &str,
        sku_id: &str,
    ) {
        let now = now_shanghai();
        let package_id = format!("package-{order_id}-{waybill_id}");
        conn.execute(
            "INSERT INTO order_packages
             (id, order_id, shop_id, wechat_order_id, delivery_id, waybill_id,
              deliver_type, source, status, created_at, updated_at)
             VALUES (?1, ?2, 'shop-1', '9100000002', ?3, ?4, 1, 'local_send', 'pending_send', ?5, ?5)",
            params![package_id, order_id, delivery_id, waybill_id, now],
        )
        .expect("包裹");
        conn.execute(
            "INSERT INTO order_package_items
             (id, package_id, wechat_product_id, wechat_sku_id, product_cnt, created_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)",
            params![format!("pkg-item-{waybill_id}"), package_id, product_id, sku_id, now],
        )
        .expect("包裹商品");
    }

    fn candidate(order_id: &str, delivery_id: &str, waybill_id: &str) -> ShipmentCandidate {
        ShipmentCandidate {
            shipment_id: build_shipment_id(order_id, 1, Some(delivery_id), Some(waybill_id)),
            order_id: order_id.to_string(),
            shop_id: "shop-1".to_string(),
            wechat_order_id: "9100000002".to_string(),
            delivery_id: Some(delivery_id.to_string()),
            waybill_id: Some(waybill_id.to_string()),
            deliver_type: 1,
            status: "ready_to_send".to_string(),
        }
    }

    #[test]
    fn group_payload_uses_package_mapping_for_multi_packages() {
        let conn = test_conn();
        let order_id = seed_order_with_items(&conn);
        seed_package(&conn, &order_id, "SF", "SF001", "wxp-1", "wxs-1");
        seed_package(&conn, &order_id, "ZTO", "ZT002", "wxp-2", "wxs-2");
        let group = vec![
            candidate(&order_id, "SF", "SF001"),
            candidate(&order_id, "ZTO", "ZT002"),
        ];
        let payload = build_group_send_delivery_payload(&conn, &group).expect("payload");
        let delivery_list = payload["delivery_list"].as_array().expect("数组");
        assert_eq!(delivery_list.len(), 2, "两个包裹应合成两元素 delivery_list");
        assert_eq!(
            delivery_list[0]["product_infos"].as_array().unwrap().len(),
            1,
            "包裹商品来自包裹映射而非整单"
        );
        assert_eq!(
            delivery_list[0]["product_infos"][0]["product_id"],
            serde_json::json!("wxp-1")
        );
        assert_eq!(delivery_list[1]["waybill_id"], serde_json::json!("ZT002"));
        assert_eq!(payload["order_id"], serde_json::json!("9100000002"));
    }

    #[test]
    fn group_payload_falls_back_to_whole_order_for_single_unmapped() {
        let conn = test_conn();
        let order_id = seed_order_with_items(&conn);
        let group = vec![candidate(&order_id, "SF", "SF001")];
        let payload = build_group_send_delivery_payload(&conn, &group).expect("payload");
        let delivery_list = payload["delivery_list"].as_array().expect("数组");
        assert_eq!(delivery_list.len(), 1);
        assert_eq!(
            delivery_list[0]["product_infos"].as_array().unwrap().len(),
            2,
            "无包裹映射的单包裹应回退整单订单项"
        );
    }

    #[test]
    fn group_payload_rejects_multi_packages_without_mapping() {
        let conn = test_conn();
        let order_id = seed_order_with_items(&conn);
        let group = vec![
            candidate(&order_id, "SF", "SF001"),
            candidate(&order_id, "ZTO", "ZT002"),
        ];
        let error = build_group_send_delivery_payload(&conn, &group).unwrap_err();
        assert!(
            error.to_string().contains("包裹商品映射"),
            "多包裹无映射必须拒绝，不能瞎猜商品归属"
        );
    }

    #[test]
    fn mark_submitted_sets_shipping_submitted_and_package_submitted() {
        let conn = test_conn();
        let order_id = seed_order_with_items(&conn);
        seed_package(&conn, &order_id, "SF", "SF001", "wxp-1", "wxs-1");
        let shipment = candidate(&order_id, "SF", "SF001");
        let now = now_shanghai();
        conn.execute(
            "INSERT INTO shipments
             (id, order_id, shop_id, wechat_order_id, delivery_id, waybill_id,
              deliver_type, status, created_at, updated_at)
             VALUES (?1, ?2, 'shop-1', '9100000002', 'SF', 'SF001', 1, 'ready_to_send', ?3, ?3)",
            params![shipment.shipment_id, order_id, now],
        )
        .unwrap();
        mark_shipment_submitted(
            &conn,
            &shipment,
            &serde_json::json!({}),
            &serde_json::json!({}),
        )
        .expect("提交落库");
        let order_status: String = conn
            .query_row(
                "SELECT status FROM orders WHERE id = ?1",
                [order_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            order_status, "shipping_submitted",
            "本地提交成功只到 shipping_submitted，wechat_shipped 由镜像确认"
        );
        let package_status: String = conn
            .query_row(
                "SELECT status FROM order_packages WHERE order_id = ?1",
                [order_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(package_status, "submitted");
    }
}
