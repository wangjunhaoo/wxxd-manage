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

/// shipments 行幂等 upsert（审查修复）：按归一化物流身份 (order_id, delivery_id, waybill_id)
/// 先查后插，不再依赖 ON CONFLICT——shipments 有 id 主键 + UNIQUE 三元组双约束，
/// ON CONFLICT(id) 撞第二约束会硬性报错（改运单后行 id 与身份脱钩即触发）；
/// 且 SQLite UNIQUE 对 NULL（无需物流）不去重。
/// 已提交成功（wechat_shipped）的行不重置状态：防止重复回填把已发货单拉回待提交队列。
/// 返回 (行 id, 是否因已发货被保护)。
fn upsert_shipment_row(
    conn: &Connection,
    order: &ShipmentOrderRef,
    fields: &ShipmentFields,
    status: &str,
    now: &str,
) -> AppResult<(String, bool)> {
    let delivery_key = fields.delivery_id.as_deref().unwrap_or("");
    let waybill_key = fields.waybill_id.as_deref().unwrap_or("");
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, status FROM shipments
             WHERE order_id = ?1 AND COALESCE(delivery_id, '') = ?2 AND COALESCE(waybill_id, '') = ?3",
            params![order.order_id, delivery_key, waybill_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((id, current_status)) = existing {
        if current_status == "wechat_shipped" {
            return Ok((id, true));
        }
        conn.execute(
            "UPDATE shipments
             SET delivery_name = COALESCE(?1, delivery_name),
                 deliver_type = ?2,
                 status = ?3,
                 error_code = NULL,
                 error_summary = NULL,
                 blocked_reason = NULL,
                 updated_at = ?4
             WHERE id = ?5",
            params![fields.delivery_name.as_deref(), fields.deliver_type, status, now, id],
        )?;
        return Ok((id, false));
    }
    let shipment_id = build_shipment_id(
        &order.order_id,
        fields.deliver_type,
        fields.delivery_id.as_deref(),
        fields.waybill_id.as_deref(),
    );
    // id 撞旧行但身份不同（改运单后历史行 id 仍含旧运单）：已发货行保护，其余清掉重建
    let id_owner_status: Option<String> = conn
        .query_row(
            "SELECT status FROM shipments WHERE id = ?1",
            [shipment_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(owner_status) = id_owner_status {
        if owner_status == "wechat_shipped" {
            return Ok((shipment_id, true));
        }
        conn.execute("DELETE FROM shipments WHERE id = ?1", [shipment_id.as_str()])?;
    }
    conn.execute(
        "INSERT INTO shipments
         (id, order_id, shop_id, wechat_order_id, delivery_id, delivery_name, waybill_id,
          deliver_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
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
    Ok((shipment_id, false))
}

/// 作废不在当前物流事实集合内的本地未提交 shipment 与包裹（审查修复）：
/// 修正运单号后旧身份行会成为幽灵，留在候选队列会把作废运单连同重复商品再次提交给微信。
/// 已提交成功（wechat_shipped）的行保留；包裹只删未提交的 local_send（confirmed 是官方事实不动）。
fn supersede_stale_local_shipments(
    conn: &Connection,
    order_id: &str,
    keep_identities: &BTreeSet<(String, String)>,
) -> AppResult<()> {
    let rows: Vec<(String, String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, COALESCE(delivery_id, ''), COALESCE(waybill_id, ''), status
             FROM shipments WHERE order_id = ?1",
        )?;
        let rows = stmt
            .query_map([order_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    for (shipment_id, delivery_key, waybill_key, status) in rows {
        if status == "wechat_shipped"
            || keep_identities.contains(&(delivery_key.clone(), waybill_key.clone()))
        {
            continue;
        }
        conn.execute("DELETE FROM shipments WHERE id = ?1", [shipment_id.as_str()])?;
        let stale_package: Option<String> = conn
            .query_row(
                "SELECT id FROM order_packages
                 WHERE order_id = ?1 AND COALESCE(delivery_id, '') = ?2 AND COALESCE(waybill_id, '') = ?3
                   AND source = 'local_send' AND status = 'pending_send'",
                params![order_id, delivery_key, waybill_key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(package_id) = stale_package {
            conn.execute(
                "DELETE FROM order_package_items WHERE package_id = ?1",
                [package_id.as_str()],
            )?;
            conn.execute(
                "DELETE FROM order_packages WHERE id = ?1",
                [package_id.as_str()],
            )?;
        }
    }
    Ok(())
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
    let now = now_shanghai();
    let (shipment_id, protected) = upsert_shipment_row(conn, order, fields, status, &now)?;
    if protected {
        return Ok(ShipmentUpsertResult {
            shipment_id,
            status: "wechat_shipped".to_string(),
            auto_send_enabled,
            message: "该运单已提交微信发货，未重复入队；如需修改运单请使用「改运单」".to_string(),
        });
    }
    // 整单语义：当前身份就是该订单的唯一物流事实，作废其它未提交旧行
    let mut keep = BTreeSet::new();
    keep.insert((
        fields.delivery_id.clone().unwrap_or_default(),
        fields.waybill_id.clone().unwrap_or_default(),
    ));
    supersede_stale_local_shipments(conn, &order.order_id, &keep)?;
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped', 'shipping_submitted') THEN status
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

/// 多运单拆包（订单履约三期 §8，feature 开关 delivery.multi_package_enabled）：
/// 同一订单回填了多个供应商物流时，按 (deliver_type, delivery_id, waybill_id) 分组，
/// 每组生成一个 shipment + 一个 local_send 包裹（order_packages/order_package_items），
/// 提交侧再按订单聚合为一次 send 的多元素 delivery_list。
pub(in crate::commands) fn upsert_order_shipments_multi(
    conn: &Connection,
    order: &ShipmentOrderRef,
) -> AppResult<MultiShipmentUpsertResult> {
    // 拉齐该订单全部已回填物流的采购任务，并联出微信商品身份（send 的 product_infos 来源）
    let mut stmt = conn.prepare(
        "SELECT
           COALESCE(pt.supplier_deliver_type, 1),
           COALESCE(pt.supplier_delivery_id, ''),
           COALESCE(pt.supplier_delivery_name, ''),
           COALESCE(pt.supplier_waybill_id, ''),
           pt.order_item_id,
           COALESCE(oi.wechat_product_id, ''),
           COALESCE(oi.wechat_sku_id, ''),
           COALESCE(oi.sku_count, 1)
         FROM purchase_tasks pt
         JOIN order_items oi ON oi.id = pt.order_item_id
         WHERE pt.order_id = ?1
           AND pt.status IN ('supplier_shipped', 'wechat_shipped', 'completed')
         ORDER BY pt.created_at ASC",
    )?;
    let rows = stmt
        .query_map([order.order_id.as_str()], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Err(AppError::Validation(
            "订单没有已回填物流的采购任务，无法生成拆包发货单".to_string(),
        ));
    }

    // 按物流身份分组：BTreeMap 保证遍历顺序稳定（包裹 id、日志可复现）
    let mut groups: BTreeMap<(i64, String, String), (String, Vec<(String, String, String, i64)>)> =
        BTreeMap::new();
    for (deliver_type, delivery_id, delivery_name, waybill_id, item_id, product_id, sku_id, cnt) in
        rows
    {
        if deliver_type == 1 && (delivery_id.is_empty() || waybill_id.is_empty()) {
            return Err(AppError::Validation(
                "存在缺少快递公司或运单号的采购物流，无法拆包发货".to_string(),
            ));
        }
        if product_id.is_empty() || sku_id.is_empty() {
            return Err(AppError::Validation(
                "订单项缺少微信 product_id 或 sku_id，无法拆包发货".to_string(),
            ));
        }
        let entry = groups
            .entry((deliver_type, delivery_id, waybill_id))
            .or_insert_with(|| (delivery_name.clone(), Vec::new()));
        entry
            .1
            .push((item_id, product_id, sku_id, cnt.max(1)));
    }

    let auto_send_enabled = get_bool_setting(conn, AUTO_SEND_DELIVERY_SETTING, false)?;
    let status = if auto_send_enabled {
        "ready_to_send"
    } else {
        "waiting_confirmation"
    };
    let now = now_shanghai();
    let shipment_count = groups.len() as i64;

    for ((deliver_type, delivery_id, waybill_id), (delivery_name, items)) in &groups {
        let delivery_id_opt = (!delivery_id.is_empty()).then_some(delivery_id.as_str());
        let delivery_name_opt = (!delivery_name.is_empty()).then_some(delivery_name.as_str());
        let waybill_id_opt = (!waybill_id.is_empty()).then_some(waybill_id.as_str());
        let group_fields = ShipmentFields {
            delivery_id: delivery_id_opt.map(str::to_string),
            delivery_name: delivery_name_opt.map(str::to_string),
            waybill_id: waybill_id_opt.map(str::to_string),
            deliver_type: *deliver_type,
        };
        upsert_shipment_row(conn, order, &group_fields, status, &now)?;

        // local_send 包裹幂等 upsert：先按归一化身份查再插。
        // 不能依赖 ON CONFLICT(order_id, delivery_id, waybill_id)——SQLite UNIQUE 对 NULL
        // 不去重，无需物流包裹（deliver_type=3，运单列为 NULL）会插出重复行。
        let existing_package: Option<String> = conn
            .query_row(
                "SELECT id FROM order_packages
                 WHERE order_id = ?1 AND COALESCE(delivery_id, '') = ?2 AND COALESCE(waybill_id, '') = ?3",
                params![order.order_id, delivery_id, waybill_id],
                |row| row.get(0),
            )
            .optional()?;
        let package_id = match existing_package {
            Some(package_id) => {
                // 微信镜像已确认（confirmed）的包裹保持确认态，其余回到 pending_send
                conn.execute(
                    "UPDATE order_packages
                     SET delivery_name = COALESCE(?1, delivery_name),
                         deliver_type = ?2,
                         source = 'local_send',
                         status = CASE WHEN status = 'confirmed' THEN status ELSE 'pending_send' END,
                         updated_at = ?3
                     WHERE id = ?4",
                    params![delivery_name_opt, deliver_type, now, package_id],
                )?;
                package_id
            }
            None => {
                let package_id = format!("package-{}-{}", order.order_id, Uuid::new_v4());
                conn.execute(
                    "INSERT INTO order_packages
                     (id, order_id, shop_id, wechat_order_id, delivery_id, delivery_name, waybill_id,
                      deliver_type, source, status, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'local_send', 'pending_send', ?9, ?9)",
                    params![
                        package_id,
                        order.order_id.as_str(),
                        order.shop_id.as_str(),
                        order.wechat_order_id.as_str(),
                        delivery_id_opt,
                        delivery_name_opt,
                        waybill_id_opt,
                        deliver_type,
                        now
                    ],
                )?;
                package_id
            }
        };

        // 包裹内商品映射全量重建（重复回填/换运单后保持与采购任务一致）
        conn.execute(
            "DELETE FROM order_package_items WHERE package_id = ?1",
            [package_id.as_str()],
        )?;
        for (order_item_id, product_id, sku_id, cnt) in items {
            conn.execute(
                "INSERT INTO order_package_items
                 (id, package_id, order_item_id, wechat_product_id, wechat_sku_id, product_cnt, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("package-item-{}", Uuid::new_v4()),
                    package_id,
                    order_item_id,
                    product_id,
                    sku_id,
                    cnt,
                    now
                ],
            )?;
        }
    }

    // 作废不在当前分组集合内的旧 shipment/包裹（修正运单后的幽灵行清理）
    let keep_identities: BTreeSet<(String, String)> = groups
        .keys()
        .map(|(_, delivery_id, waybill_id)| (delivery_id.clone(), waybill_id.clone()))
        .collect();
    supersede_stale_local_shipments(conn, &order.order_id, &keep_identities)?;

    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped', 'shipping_submitted') THEN status
           ELSE 'supplier_shipped'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), order.order_id],
    )?;

    let message = if auto_send_enabled {
        format!("已按运单拆为 {shipment_count} 个包裹，自动发货开关已开启，将整单聚合提交微信发货")
    } else {
        format!("已按运单拆为 {shipment_count} 个包裹，自动发货开关关闭，当前仅进入待确认发货")
    };
    Ok(MultiShipmentUpsertResult {
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

    /// 造一个含 2 个订单项 + 2 个不同运单采购任务的订单（多运单拆包的标准场景）
    fn seed_multi_logistics_order(conn: &Connection) -> ShipmentOrderRef {
        save_synced_order(conn, "shop-1", "9100000001").expect("订单落库");
        let order_id = "order-shop-1-9100000001".to_string();
        let now = now_shanghai();
        for (idx, (product_id, sku_id, delivery, waybill)) in [
            ("wxp-1", "wxs-1", "SF", "SF001"),
            ("wxp-2", "wxs-2", "ZTO", "ZT002"),
        ]
        .iter()
        .enumerate()
        {
            let item_id = format!("item-{idx}");
            conn.execute(
                "INSERT INTO order_items
                 (id, order_id, shop_id, wechat_order_id, wechat_product_id, wechat_sku_id,
                  sku_count, raw_payload, created_at, updated_at)
                 VALUES (?1, ?2, 'shop-1', '9100000001', ?3, ?4, 2, '{}', ?5, ?5)",
                params![item_id, order_id, product_id, sku_id, now],
            )
            .expect("订单项");
            conn.execute(
                "INSERT INTO purchase_tasks
                 (id, order_id, order_item_id, shop_id, status, quantity,
                  supplier_deliver_type, supplier_delivery_id, supplier_delivery_name,
                  supplier_waybill_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'shop-1', 'supplier_shipped', 2, 1, ?4, ?4, ?5, ?6, ?6)",
                params![format!("pt-{idx}"), order_id, item_id, delivery, waybill, now],
            )
            .expect("采购任务");
        }
        ShipmentOrderRef {
            order_id,
            shop_id: "shop-1".to_string(),
            wechat_order_id: "9100000001".to_string(),
        }
    }

    #[test]
    fn multi_split_creates_shipments_packages_and_item_mapping() {
        let conn = test_conn();
        let order = seed_multi_logistics_order(&conn);
        let result = upsert_order_shipments_multi(&conn, &order).expect("拆包");
        assert_eq!(result.status, "waiting_confirmation"); // 自动发货默认关

        let shipment_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shipments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(shipment_count, 2, "两个运单应拆成两个 shipment");

        let package_rows: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT source, status FROM order_packages WHERE order_id = ?1 ORDER BY waybill_id",
                )
                .unwrap();
            let rows = stmt
                .query_map([order.order_id.as_str()], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows
        };
        assert_eq!(package_rows.len(), 2);
        for (source, status) in &package_rows {
            assert_eq!(source, "local_send");
            assert_eq!(status, "pending_send");
        }

        // 每个包裹恰好映射自己运单对应的那 1 个商品
        let sf_items: Vec<(String, i64)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT opi.wechat_product_id, opi.product_cnt
                     FROM order_package_items opi
                     JOIN order_packages op ON op.id = opi.package_id
                     WHERE op.order_id = ?1 AND op.waybill_id = 'SF001'",
                )
                .unwrap();
            let rows = stmt
                .query_map([order.order_id.as_str()], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows
        };
        assert_eq!(sf_items, vec![("wxp-1".to_string(), 2)]);

        // 幂等：重复回填不产生重复行，包裹商品映射全量重建
        upsert_order_shipments_multi(&conn, &order).expect("重复拆包");
        let recount: i64 = conn
            .query_row("SELECT COUNT(*) FROM order_packages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(recount, 2);
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM order_package_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(item_count, 2);
    }

    #[test]
    fn multi_split_is_idempotent_for_virtual_delivery_null_waybill() {
        let conn = test_conn();
        let order = seed_multi_logistics_order(&conn);
        // 第二个采购任务改成无需物流（运单列为 NULL）：
        // SQLite UNIQUE 对 NULL 不去重，必须靠先查后插保证幂等
        conn.execute(
            "UPDATE purchase_tasks
             SET supplier_deliver_type = 3, supplier_delivery_id = NULL,
                 supplier_delivery_name = NULL, supplier_waybill_id = NULL
             WHERE id = 'pt-1'",
            [],
        )
        .unwrap();
        upsert_order_shipments_multi(&conn, &order).expect("拆包");
        upsert_order_shipments_multi(&conn, &order).expect("重复拆包");
        let package_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM order_packages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(package_count, 2, "无需物流包裹重复回填不得插重复行");
    }

    #[test]
    fn multi_split_keeps_shipped_rows_and_supersedes_stale_waybill() {
        let conn = test_conn();
        let order = seed_multi_logistics_order(&conn);
        upsert_order_shipments_multi(&conn, &order).expect("首次拆包");
        // SF001 包裹已提交成功：重复回填不得把它重置回待提交（防重复 send）
        conn.execute(
            "UPDATE shipments SET status = 'wechat_shipped' WHERE waybill_id = 'SF001'",
            [],
        )
        .unwrap();
        upsert_order_shipments_multi(&conn, &order).expect("重复拆包");
        let shipped_status: String = conn
            .query_row(
                "SELECT status FROM shipments WHERE waybill_id = 'SF001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(shipped_status, "wechat_shipped", "已提交成功的行不得被重置");

        // 修正运单 ZT002 → ZT999 后重跑：旧身份 shipment 与包裹必须被作废，
        // 否则聚合提交会把作废运单连同重复商品一起发给微信
        conn.execute(
            "UPDATE purchase_tasks SET supplier_waybill_id = 'ZT999' WHERE id = 'pt-1'",
            [],
        )
        .unwrap();
        upsert_order_shipments_multi(&conn, &order).expect("改运单后重跑");
        let stale_shipment: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM shipments WHERE waybill_id = 'ZT002'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale_shipment, 0, "旧运单 shipment 应被作废删除");
        let stale_package: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM order_packages WHERE waybill_id = 'ZT002'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale_package, 0, "旧运单包裹应连带删除");
        let new_shipment: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM shipments WHERE waybill_id = 'ZT999'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_shipment, 1, "新运单 shipment 应生成");
    }

    #[test]
    fn multi_split_rejects_missing_wechat_identity() {
        let conn = test_conn();
        let order = seed_multi_logistics_order(&conn);
        conn.execute(
            "UPDATE order_items SET wechat_product_id = NULL WHERE id = 'item-0'",
            [],
        )
        .unwrap();
        let error = upsert_order_shipments_multi(&conn, &order).unwrap_err();
        assert!(error.to_string().contains("product_id"), "缺微信身份应拒绝拆包");
    }
}
