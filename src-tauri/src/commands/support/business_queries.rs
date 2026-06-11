use super::*;

pub(in crate::commands) fn load_order_sync_shops(
    conn: &Connection,
) -> AppResult<Vec<OrderSyncShop>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name
         FROM shops s
         JOIN shop_credentials c ON c.shop_id = s.id
         WHERE s.status = 'active'
         ORDER BY s.created_at ASC",
    )?;
    let shops = stmt
        .query_map([], |row| {
            Ok(OrderSyncShop {
                shop_id: row.get(0)?,
                shop_name: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(shops)
}

/// 详情回刷候选（订单履约重设计 §3.3）——替换旧的「detail_synced_at IS NULL 拉一次就永不重拉」。
/// 四档优先级：
///   0 新单（详情从未拉过）
///   1 增量同步命中的脏单（detail_dirty=1，含取消/改价/退款等一切 update_time 变化）
///   2 发货前敏感单 30 分钟回刷（改址 12h 自动同意、换SKU 有 ddl，风险集中在发货前）
///   3 其余非终态单 6 小时安全网
/// 终态单（completed/cancelled）只会经 0/1 档最后回刷一次锁定最终金额，之后不再进入候选。
pub(in crate::commands) fn load_order_detail_sync_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<OrderDetailSyncItem>> {
    let sensitive_cutoff = format_shanghai(Utc::now() - Duration::minutes(30));
    let stale_cutoff = format_shanghai(Utc::now() - Duration::hours(6));
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, wechat_order_id,
           CASE
             WHEN detail_synced_at IS NULL OR detail_synced_at = '' THEN 0
             WHEN detail_dirty = 1 THEN 1
             WHEN status IN ('pending_shipment', 'pending_purchase', 'supplier_shipped',
                             'partially_shipped', 'shipping_submitted', 'exception')
                  AND detail_synced_at < ?1 THEN 2
             ELSE 3
           END AS priority
         FROM orders
         WHERE wechat_order_id IS NOT NULL
           AND shop_id IS NOT NULL
           AND (
             (detail_synced_at IS NULL OR detail_synced_at = '')
             OR detail_dirty = 1
             OR (status IN ('pending_shipment', 'pending_purchase', 'supplier_shipped',
                            'partially_shipped', 'shipping_submitted', 'exception')
                 AND detail_synced_at < ?1)
             OR (status NOT IN ('completed', 'cancelled') AND detail_synced_at < ?2)
           )
         ORDER BY priority ASC, COALESCE(detail_synced_at, '') ASC, synced_at ASC, created_at ASC
         LIMIT ?3",
    )?;
    let items = stmt
        .query_map(params![sensitive_cutoff, stale_cutoff, limit], |row| {
            Ok(OrderDetailSyncItem {
                order_id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_order_id: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(in crate::commands) fn count_aftersales(
    conn: &Connection,
    status: Option<&str>,
) -> AppResult<i64> {
    match status {
        Some("active") => Ok(conn.query_row(
            "SELECT COUNT(*)
             FROM aftersales
             WHERE status NOT IN (
               'USER_CANCELD', 'USER_CANCELLED', 'RETURN_CLOSED',
               'MERCHANT_REFUND_SUCCESS', 'MERCHANT_RETURN_SUCCESS',
               'MERCHANT_REFUND_RETRY_FAIL', 'MERCHANT_FAIL',
               'MERCHANT_EXCHANGE_SUCCESS', 'sync_failed'
             )",
            [],
            |row| row.get(0),
        )?),
        Some(status) => Ok(conn.query_row(
            "SELECT COUNT(*) FROM aftersales WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?),
        None => Ok(conn.query_row("SELECT COUNT(*) FROM aftersales", [], |row| row.get(0))?),
    }
}

pub(in crate::commands) fn load_aftersale_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AftersaleView>> {
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(AftersaleView {
            id: row.get(0)?,
            shop_id: row.get(1)?,
            shop_name: row.get(2)?,
            order_id: row.get(3)?,
            wechat_order_id: row.get(4)?,
            wechat_aftersale_id: row.get(5)?,
            status: row.get(6)?,
            aftersale_type: row.get(7)?,
            reason: row.get(8)?,
            refund_amount_cents: row.get(9)?,
            responsibility_party: row.get(10)?,
            responsibility_note: row.get(11)?,
            supplier_compensation_cents: row.get(12)?,
            handled_at: row.get(13)?,
            last_action: row.get(14)?,
            last_action_status: row.get(15)?,
            last_action_error: row.get(16)?,
            last_action_note: row.get(17)?,
            last_action_at: row.get(18)?,
            evidence_count: row.get(19)?,
            synced_at: row.get(20)?,
            updated_at: row.get(21)?,
        })
    };
    let base_select = "SELECT
           a.id,
           a.shop_id,
           COALESCE(s.name, a.shop_id),
           a.order_id,
           a.wechat_order_id,
           a.wechat_aftersale_id,
           a.status,
           a.aftersale_type,
           a.reason,
           a.refund_amount_cents,
           a.responsibility_party,
           a.responsibility_note,
           COALESCE(a.supplier_compensation_cents, 0),
           a.handled_at,
           a.last_action,
           a.last_action_status,
           a.last_action_error,
           a.last_action_note,
           a.last_action_at,
           (
             SELECT COUNT(*)
             FROM aftersale_evidence_records e
             WHERE e.target_type = 'aftersale'
               AND (e.target_id = a.id OR e.external_target_id = a.wechat_aftersale_id)
           ),
           a.synced_at,
           a.updated_at
         FROM aftersales a
         LEFT JOIN shops s ON s.id = a.shop_id";
    match status {
        Some("active") => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE a.status NOT IN (
                   'USER_CANCELD', 'USER_CANCELLED', 'RETURN_CLOSED',
                   'MERCHANT_REFUND_SUCCESS', 'MERCHANT_RETURN_SUCCESS',
                   'MERCHANT_REFUND_RETRY_FAIL', 'MERCHANT_FAIL',
                   'MERCHANT_EXCHANGE_SUCCESS', 'sync_failed'
                 )
                 ORDER BY a.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        Some(status) => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE a.status = ?1
                 ORDER BY a.updated_at DESC
                 LIMIT ?2"
            ))?;
            let rows = stmt
                .query_map(params![status, limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 ORDER BY a.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

pub(in crate::commands) fn count_aftersale_evidence(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM aftersale_evidence_records
         WHERE (?1 IS NULL OR target_type = ?1)
           AND (?2 IS NULL OR target_id = ?2 OR external_target_id = ?2)
           AND (?3 IS NULL OR status = ?3)",
        params![target_type, target_id, status],
        |row| row.get(0),
    )?)
}

pub(in crate::commands) fn load_aftersale_evidence_views(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AftersaleEvidenceView>> {
    let mut stmt = conn.prepare(
        "SELECT
           e.id,
           e.target_type,
           e.target_id,
           e.shop_id,
           COALESCE(s.name, e.shop_id),
           e.external_target_id,
           e.evidence_type,
           e.title,
           e.content_text,
           e.local_file_path,
           e.source_url,
           e.status,
           e.created_at,
           e.updated_at
         FROM aftersale_evidence_records e
         LEFT JOIN shops s ON s.id = e.shop_id
         WHERE (?1 IS NULL OR e.target_type = ?1)
           AND (?2 IS NULL OR e.target_id = ?2 OR e.external_target_id = ?2)
           AND (?3 IS NULL OR e.status = ?3)
         ORDER BY e.updated_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(params![target_type, target_id, status, limit], |row| {
            let evidence_type: String = row.get(6)?;
            let status: String = row.get(11)?;
            Ok(AftersaleEvidenceView {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_name: row.get(4)?,
                external_target_id: row.get(5)?,
                evidence_type: evidence_type.clone(),
                evidence_type_text: aftersale_evidence_type_text(&evidence_type).to_string(),
                title: row.get(7)?,
                content_text: row.get(8)?,
                local_file_path: row.get(9)?,
                source_url: row.get(10)?,
                status: status.clone(),
                status_text: aftersale_evidence_status_text(&status).to_string(),
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(in crate::commands) fn count_supplier_aftersale_followups(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM supplier_aftersale_followups
         WHERE (?1 IS NULL OR target_type = ?1)
           AND (?2 IS NULL OR target_id = ?2 OR external_target_id = ?2)
           AND (?3 IS NULL OR status = ?3)",
        params![target_type, target_id, status],
        |row| row.get(0),
    )?)
}

pub(in crate::commands) fn load_supplier_aftersale_followup_views(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<SupplierAftersaleFollowupView>> {
    let mut stmt = conn.prepare(
        "SELECT
           f.id,
           f.target_type,
           f.target_id,
           f.shop_id,
           COALESCE(s.name, f.shop_id),
           f.external_target_id,
           f.purchase_task_id,
           f.supplier_name,
           f.followup_type,
           f.status,
           f.note,
           f.created_at,
           f.updated_at
         FROM supplier_aftersale_followups f
         LEFT JOIN shops s ON s.id = f.shop_id
         WHERE (?1 IS NULL OR f.target_type = ?1)
           AND (?2 IS NULL OR f.target_id = ?2 OR f.external_target_id = ?2)
           AND (?3 IS NULL OR f.status = ?3)
         ORDER BY f.updated_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(params![target_type, target_id, status, limit], |row| {
            let followup_type: String = row.get(8)?;
            let status: String = row.get(9)?;
            Ok(SupplierAftersaleFollowupView {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_name: row.get(4)?,
                external_target_id: row.get(5)?,
                purchase_task_id: row.get(6)?,
                supplier_name: row.get(7)?,
                followup_type: followup_type.clone(),
                followup_type_text: supplier_aftersale_followup_type_text(&followup_type)
                    .to_string(),
                status: status.clone(),
                status_text: supplier_aftersale_followup_status_text(&status).to_string(),
                note: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(in crate::commands) fn count_guarantee_orders(
    conn: &Connection,
    status: Option<&str>,
) -> AppResult<i64> {
    match status {
        Some("active") => Ok(conn.query_row(
            "SELECT COUNT(*)
             FROM guarantee_orders
             WHERE status NOT IN (
               'STATUS_NO_NEED_PAY', 'STATUS_PAY_SUCC', 'STATUS_USER_CANCEL', 'sync_failed'
             )",
            [],
            |row| row.get(0),
        )?),
        Some(status) => Ok(conn.query_row(
            "SELECT COUNT(*) FROM guarantee_orders WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?),
        None => Ok(
            conn.query_row("SELECT COUNT(*) FROM guarantee_orders", [], |row| {
                row.get(0)
            })?,
        ),
    }
}

pub(in crate::commands) fn load_guarantee_order_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<GuaranteeOrderView>> {
    let mapper = |row: &rusqlite::Row<'_>| {
        let guarantee_type: Option<i64> = row.get(6)?;
        let status: String = row.get(7)?;
        Ok(GuaranteeOrderView {
            id: row.get(0)?,
            shop_id: row.get(1)?,
            shop_name: row.get(2)?,
            order_id: row.get(3)?,
            wechat_order_id: row.get(4)?,
            guarantee_order_id: row.get(5)?,
            guarantee_type,
            guarantee_type_text: guarantee_type_text(guarantee_type).to_string(),
            status: status.clone(),
            status_text: guarantee_status_text(&status).to_string(),
            apply_reason: row.get(8)?,
            pay_amount_cents: row.get(9)?,
            merchant_refuse_reason: row.get(10)?,
            handling_status: row.get(11)?,
            handling_note: row.get(12)?,
            responsibility_party: row.get(13)?,
            supplier_compensation_cents: row.get(14)?,
            handled_at: row.get(15)?,
            created_time: row.get(16)?,
            updated_time_unix: row.get(17)?,
            expire_time: row.get(18)?,
            complete_time: row.get(19)?,
            evidence_count: row.get(20)?,
            synced_at: row.get(21)?,
            updated_at: row.get(22)?,
        })
    };
    let base_select = "SELECT
           g.id,
           g.shop_id,
           COALESCE(s.name, g.shop_id),
           g.order_id,
           g.wechat_order_id,
           g.guarantee_order_id,
           g.guarantee_type,
           g.status,
           g.apply_reason,
           g.pay_amount_cents,
           g.merchant_refuse_reason,
           g.handling_status,
           g.handling_note,
           g.responsibility_party,
           COALESCE(g.supplier_compensation_cents, 0),
           g.handled_at,
           g.created_time,
           g.updated_time,
           g.expire_time,
           g.complete_time,
           (
             SELECT COUNT(*)
             FROM aftersale_evidence_records e
             WHERE e.target_type = 'guarantee'
               AND (e.target_id = g.id OR e.external_target_id = g.guarantee_order_id)
           ),
           g.synced_at,
           g.updated_at
         FROM guarantee_orders g
         LEFT JOIN shops s ON s.id = g.shop_id";
    match status {
        Some("active") => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE g.status NOT IN (
                   'STATUS_NO_NEED_PAY', 'STATUS_PAY_SUCC', 'STATUS_USER_CANCEL', 'sync_failed'
                 )
                 ORDER BY g.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        Some(status) => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE g.status = ?1
                 ORDER BY g.updated_at DESC
                 LIMIT ?2"
            ))?;
            let rows = stmt
                .query_map(params![status, limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 ORDER BY g.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

pub(in crate::commands) fn load_purchase_candidates(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PurchaseCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT
           oi.order_id,
           oi.id,
           oi.shop_id,
           oi.out_product_id,
           oi.out_sku_id,
           COALESCE(
             sp.source_url,
             (
               SELECT pp.source_url
               FROM publish_products pp
               WHERE pp.external_product_id = oi.out_product_id
               ORDER BY pp.created_at DESC
               LIMIT 1
             )
           ),
           oi.sku_count
         FROM order_items oi
         JOIN orders o ON o.id = oi.order_id
         LEFT JOIN purchase_tasks pt ON pt.order_item_id = oi.id
         LEFT JOIN shop_products sp
           ON sp.shop_id = oi.shop_id
          AND sp.external_product_id = oi.out_product_id
         WHERE pt.id IS NULL
           AND o.status IN ('pending_shipment', 'pending_purchase', 'synced')
         ORDER BY oi.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PurchaseCandidate {
                order_id: row.get(0)?,
                order_item_id: row.get(1)?,
                shop_id: row.get(2)?,
                out_product_id: row.get(3)?,
                out_sku_id: row.get(4)?,
                source_url: row.get(5)?,
                quantity: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(in crate::commands) fn count_purchase_tasks(
    conn: &Connection,
    status: Option<&str>,
) -> AppResult<i64> {
    if let Some(status) = status {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM purchase_tasks WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?)
    } else {
        Ok(conn.query_row("SELECT COUNT(*) FROM purchase_tasks", [], |row| row.get(0))?)
    }
}

pub(in crate::commands) fn load_purchase_task_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<PurchaseTaskView>> {
    let sql = format!(
        "SELECT
           pt.id,
           pt.order_id,
           COALESCE(o.wechat_order_id, ''),
           pt.shop_id,
           COALESCE(s.name, pt.shop_id),
           pt.status,
           pt.external_product_id,
           pt.external_sku_id,
           pt.source_url,
           oi.title,
           pt.quantity,
           oi.sale_price,
           oi.real_price,
           COALESCE(oi.real_price, oi.sale_price * pt.quantity),
           pt.estimated_cost,
           CASE
             WHEN pt.estimated_cost IS NULL THEN NULL
             ELSE (COALESCE(oi.real_price, oi.sale_price * pt.quantity) / 100.0) - pt.estimated_cost
           END,
           pt.supplier_name,
           pt.supplier_product_id,
           pt.supplier_delivery_id,
           pt.supplier_delivery_name,
           pt.supplier_waybill_id,
           pt.supplier_deliver_type,
           pt.supplier_shipped_at,
           pt.error_summary,
           pt.purchased_at,
           CASE WHEN d.decoded_at IS NOT NULL THEN 1 ELSE 0 END,
           CASE WHEN d.decoded_at IS NOT NULL
                THEN TRIM(COALESCE(d.province, '') || ' ' || COALESCE(d.city, '') || ' ' || COALESCE(d.county, ''))
                ELSE NULL END,
           pt.created_at,
           pt.updated_at
         FROM purchase_tasks pt
         JOIN orders o ON o.id = pt.order_id
         JOIN order_items oi ON oi.id = pt.order_item_id
         LEFT JOIN shops s ON s.id = pt.shop_id
         LEFT JOIN order_decoded_addresses d ON d.order_id = pt.order_id
         {}
         ORDER BY pt.created_at DESC
         LIMIT ?{}",
        if status.is_some() {
            "WHERE pt.status = ?1"
        } else {
            ""
        },
        if status.is_some() { 2 } else { 1 }
    );
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(PurchaseTaskView {
            id: row.get(0)?,
            order_id: row.get(1)?,
            wechat_order_id: row.get(2)?,
            shop_id: row.get(3)?,
            shop_name: row.get(4)?,
            status: row.get(5)?,
            external_product_id: row.get(6)?,
            external_sku_id: row.get(7)?,
            source_url: row.get(8)?,
            title: row.get(9)?,
            quantity: row.get(10)?,
            sale_price: row.get(11)?,
            real_price: row.get(12)?,
            estimated_revenue: row.get(13)?,
            estimated_cost: row.get(14)?,
            estimated_profit: row.get(15)?,
            supplier_name: row.get(16)?,
            supplier_product_id: row.get(17)?,
            supplier_delivery_id: row.get(18)?,
            supplier_delivery_name: row.get(19)?,
            supplier_waybill_id: row.get(20)?,
            supplier_deliver_type: row.get(21)?,
            supplier_shipped_at: row.get(22)?,
            error_summary: row.get(23)?,
            purchased_at: row.get(24)?,
            has_decoded_address: row.get::<_, i64>(25)? == 1,
            decoded_region: row
                .get::<_, Option<String>>(26)?
                .filter(|value| !value.trim().is_empty()),
            created_at: row.get(27)?,
            updated_at: row.get(28)?,
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

pub(in crate::commands) fn load_order_profit_views(
    conn: &Connection,
) -> AppResult<Vec<OrderProfitView>> {
    let mut stmt = conn.prepare(
        "WITH item_totals AS (
           SELECT
             order_id,
             COUNT(*) AS item_count,
             COALESCE(SUM(sku_count), 0) AS quantity,
             COALESCE(SUM(COALESCE(real_price, sale_price * sku_count, 0)), 0) AS revenue_cents
           FROM order_items
           GROUP BY order_id
         ),
         purchase_totals AS (
           SELECT
             order_id,
             COUNT(*) AS purchase_task_count,
             COALESCE(SUM(CASE WHEN estimated_cost IS NULL THEN 1 ELSE 0 END), 0) AS missing_cost_count,
             COALESCE(SUM(CASE
               WHEN estimated_cost IS NULL THEN 0
               ELSE CAST(ROUND(estimated_cost * 100) AS INTEGER)
             END), 0) AS purchase_cost_cents
           FROM purchase_tasks
           GROUP BY order_id
         ),
         adjustment_totals AS (
           SELECT
             order_id,
             COALESCE(SUM(CASE WHEN kind = 'purchase_freight' THEN amount_cents ELSE 0 END), 0) AS purchase_freight_cents,
             COALESCE(SUM(CASE WHEN kind = 'refund' THEN amount_cents ELSE 0 END), 0) AS refund_cents,
             COALESCE(SUM(CASE WHEN kind = 'aftersale_compensation' THEN amount_cents ELSE 0 END), 0) AS aftersale_compensation_cents,
             COALESCE(SUM(CASE WHEN kind = 'other_cost' THEN amount_cents ELSE 0 END), 0) AS other_cost_cents,
             COALESCE(SUM(CASE WHEN kind = 'other_income' THEN amount_cents ELSE 0 END), 0) AS other_income_cents
           FROM order_profit_adjustments
           GROUP BY order_id
         )
         SELECT
           o.id,
           COALESCE(o.wechat_order_id, ''),
           COALESCE(o.shop_id, ''),
           COALESCE(s.name, o.shop_id, ''),
           o.status,
           COALESCE(it.item_count, 0),
           COALESCE(it.quantity, 0),
           COALESCE(it.revenue_cents, 0),
           COALESCE(pt.purchase_task_count, 0),
           COALESCE(pt.missing_cost_count, 0),
           COALESCE(pt.purchase_cost_cents, 0),
           COALESCE(at.purchase_freight_cents, 0),
           COALESCE(at.refund_cents, 0),
           COALESCE(at.aftersale_compensation_cents, 0),
           COALESCE(at.other_cost_cents, 0),
           COALESCE(at.other_income_cents, 0),
           o.detail_synced_at,
           o.updated_at
         FROM orders o
         LEFT JOIN shops s ON s.id = o.shop_id
         LEFT JOIN item_totals it ON it.order_id = o.id
         LEFT JOIN purchase_totals pt ON pt.order_id = o.id
         LEFT JOIN adjustment_totals at ON at.order_id = o.id
         ORDER BY COALESCE(o.updated_at, o.created_at) DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let revenue_cents = row.get::<_, i64>(7)?;
            let purchase_task_count = row.get::<_, i64>(8)?;
            let missing_cost_count = row.get::<_, i64>(9)?;
            let purchase_cost_cents = row.get::<_, i64>(10)?;
            let purchase_freight_cents = row.get::<_, i64>(11)?;
            let refund_cents = row.get::<_, i64>(12)?;
            let aftersale_compensation_cents = row.get::<_, i64>(13)?;
            let other_cost_cents = row.get::<_, i64>(14)?;
            let other_income_cents = row.get::<_, i64>(15)?;
            let estimated_profit_cents = revenue_cents + other_income_cents
                - purchase_cost_cents
                - purchase_freight_cents
                - refund_cents
                - aftersale_compensation_cents
                - other_cost_cents;
            let actual_profit_cents = if purchase_task_count > 0 && missing_cost_count == 0 {
                Some(estimated_profit_cents)
            } else {
                None
            };
            Ok(OrderProfitView {
                order_id: row.get(0)?,
                wechat_order_id: row.get(1)?,
                shop_id: row.get(2)?,
                shop_name: row.get(3)?,
                order_status: row.get(4)?,
                item_count: row.get(5)?,
                quantity: row.get(6)?,
                revenue_cents,
                purchase_task_count,
                missing_cost_count,
                purchase_cost_cents,
                purchase_freight_cents,
                refund_cents,
                aftersale_compensation_cents,
                other_cost_cents,
                other_income_cents,
                estimated_profit_cents,
                actual_profit_cents,
                profit_status: resolve_profit_status(
                    purchase_task_count,
                    missing_cost_count,
                    actual_profit_cents,
                )
                .to_string(),
                detail_synced_at: row.get(16)?,
                updated_at: row.get(17)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(in crate::commands) fn summarize_order_profit(items: &[OrderProfitView]) -> OrderProfitTotals {
    OrderProfitTotals {
        order_count: items.len() as i64,
        revenue_cents: items.iter().map(|item| item.revenue_cents).sum(),
        purchase_cost_cents: items.iter().map(|item| item.purchase_cost_cents).sum(),
        purchase_freight_cents: items.iter().map(|item| item.purchase_freight_cents).sum(),
        refund_cents: items.iter().map(|item| item.refund_cents).sum(),
        aftersale_compensation_cents: items
            .iter()
            .map(|item| item.aftersale_compensation_cents)
            .sum(),
        other_cost_cents: items.iter().map(|item| item.other_cost_cents).sum(),
        other_income_cents: items.iter().map(|item| item.other_income_cents).sum(),
        estimated_profit_cents: items.iter().map(|item| item.estimated_profit_cents).sum(),
        actual_profit_cents: items
            .iter()
            .filter_map(|item| item.actual_profit_cents)
            .sum(),
        unknown_actual_order_count: items
            .iter()
            .filter(|item| item.actual_profit_cents.is_none())
            .count() as i64,
    }
}

pub(in crate::commands) fn resolve_profit_status(
    purchase_task_count: i64,
    missing_cost_count: i64,
    actual_profit_cents: Option<i64>,
) -> &'static str {
    if purchase_task_count == 0 {
        "missing_purchase_task"
    } else if missing_cost_count > 0 {
        "missing_cost"
    } else if actual_profit_cents.unwrap_or_default() < 0 {
        "loss"
    } else {
        "profitable"
    }
}

pub(in crate::commands) fn normalize_profit_adjustment_kind(kind: &str) -> AppResult<&'static str> {
    match kind.trim() {
        "purchase_freight" => Ok("purchase_freight"),
        "refund" => Ok("refund"),
        "aftersale_compensation" => Ok("aftersale_compensation"),
        "other_cost" => Ok("other_cost"),
        "other_income" => Ok("other_income"),
        _ => Err(AppError::Validation(
            "调整类型必须是 purchase_freight/refund/aftersale_compensation/other_cost/other_income"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_aftersale_responsibility_party(
    party: &str,
) -> AppResult<&'static str> {
    match party.trim() {
        "supplier" => Ok("supplier"),
        "merchant" => Ok("merchant"),
        "customer" => Ok("customer"),
        "platform" => Ok("platform"),
        "unknown" => Ok("unknown"),
        _ => Err(AppError::Validation(
            "责任方必须是 supplier/merchant/customer/platform/unknown".to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_aftersale_evidence_target_type(
    target_type: &str,
) -> AppResult<&'static str> {
    match target_type.trim() {
        "aftersale" => Ok("aftersale"),
        "guarantee" => Ok("guarantee"),
        _ => Err(AppError::Validation(
            "凭证目标类型必须是 aftersale 或 guarantee".to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_aftersale_evidence_type(
    evidence_type: &str,
) -> AppResult<&'static str> {
    match evidence_type.trim() {
        "image" => Ok("image"),
        "text" => Ok("text"),
        "chat_record" => Ok("chat_record"),
        "logistics" => Ok("logistics"),
        "supplier_proof" => Ok("supplier_proof"),
        "quality_check" => Ok("quality_check"),
        "other" => Ok("other"),
        _ => Err(AppError::Validation(
            "凭证类型必须是 image/text/chat_record/logistics/supplier_proof/quality_check/other"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_aftersale_evidence_status(
    status: &str,
) -> AppResult<&'static str> {
    match status.trim() {
        "draft" => Ok("draft"),
        "ready" => Ok("ready"),
        "used" => Ok("used"),
        "archived" => Ok("archived"),
        _ => Err(AppError::Validation(
            "凭证状态必须是 draft/ready/used/archived".to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_supplier_aftersale_followup_type(
    followup_type: &str,
) -> AppResult<&'static str> {
    match followup_type.trim() {
        "contact" => Ok("contact"),
        "evidence_request" => Ok("evidence_request"),
        "evidence_received" => Ok("evidence_received"),
        "compensation" => Ok("compensation"),
        "return_refund" => Ok("return_refund"),
        "other" => Ok("other"),
        _ => Err(AppError::Validation(
            "供应商协同类型必须是 contact/evidence_request/evidence_received/compensation/return_refund/other"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_supplier_aftersale_followup_status(
    status: &str,
) -> AppResult<&'static str> {
    match status.trim() {
        "pending" => Ok("pending"),
        "contacted" => Ok("contacted"),
        "waiting_supplier" => Ok("waiting_supplier"),
        "evidence_ready" => Ok("evidence_ready"),
        "compensation_pending" => Ok("compensation_pending"),
        "closed" => Ok("closed"),
        _ => Err(AppError::Validation(
            "供应商协同状态必须是 pending/contacted/waiting_supplier/evidence_ready/compensation_pending/closed"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_optional_note(
    note: Option<String>,
    max_len: usize,
    label: &str,
) -> AppResult<Option<String>> {
    let note = note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if note.as_deref().map(str::len).unwrap_or_default() > max_len {
        return Err(AppError::Validation(format!(
            "{label}不能超过 {max_len} 个字符"
        )));
    }
    Ok(note)
}

pub(in crate::commands) fn resolve_aftersale_evidence_target(
    conn: &Connection,
    target_type: &str,
    target_id: &str,
) -> AppResult<(String, String, String)> {
    match target_type {
        "aftersale" => conn
            .query_row(
                "SELECT id, shop_id, wechat_aftersale_id
                 FROM aftersales
                 WHERE id = ?1 OR wechat_aftersale_id = ?1
                 LIMIT 1",
                [target_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("售后单不存在".to_string())),
        "guarantee" => conn
            .query_row(
                "SELECT id, shop_id, guarantee_order_id
                 FROM guarantee_orders
                 WHERE id = ?1 OR guarantee_order_id = ?1
                 LIMIT 1",
                [target_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("纠纷单不存在".to_string())),
        _ => Err(AppError::Validation("凭证目标类型不合法".to_string())),
    }
}

pub(in crate::commands) fn load_aftersale_action_target(
    app: &AppHandle,
    aftersale_id: &str,
) -> AppResult<AftersaleActionTarget> {
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT id, shop_id, wechat_aftersale_id, status
         FROM aftersales
         WHERE id = ?1 OR wechat_aftersale_id = ?1
         LIMIT 1",
        [aftersale_id],
        |row| {
            Ok(AftersaleActionTarget {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_aftersale_id: row.get(2)?,
                status: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Validation("售后单不存在".to_string()))
}

pub(in crate::commands) fn validate_aftersale_action_status(
    target: &AftersaleActionTarget,
) -> AppResult<()> {
    if !is_active_aftersale_status(&target.status) {
        return Err(AppError::Validation(format!(
            "售后单当前状态为 {}，不允许提交同意或拒绝动作",
            target.status
        )));
    }
    Ok(())
}

pub(in crate::commands) fn handle_aftersale_action_call(
    app: &AppHandle,
    target: AftersaleActionTarget,
    call: WechatRawCall,
    action: &str,
    note: Option<&str>,
) -> AppResult<AftersaleActionResult> {
    let conn = open_connection(app)?;
    match &call.result {
        WechatCallResult::Success(_) => {
            insert_api_call_log(
                &conn,
                Some(&target.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "aftersale {} ok, id={}",
                    action, target.wechat_aftersale_id
                )),
            )?;
            update_aftersale_action_state(&conn, &target.id, action, "success", None, note)?;
            upsert_notification(
                &conn,
                "info",
                "aftersale_action",
                &target.id,
                Some(&target.shop_id),
                &format!("售后{}已提交", aftersale_action_label(action)),
                &format!(
                    "售后单 {} 已提交{}动作，后续通过售后同步确认平台状态。",
                    target.wechat_aftersale_id,
                    aftersale_action_label(action)
                ),
                Some(&serde_json::json!({
                    "aftersale_id": &target.id,
                    "wechat_aftersale_id": &target.wechat_aftersale_id,
                    "action": action,
                    "note": note
                })),
            )?;
            Ok(AftersaleActionResult {
                aftersale_id: target.id,
                wechat_aftersale_id: target.wechat_aftersale_id,
                action: action.to_string(),
                status: "success".to_string(),
                errcode: None,
                errmsg: None,
                message: "售后动作已提交，等待后续同步确认平台状态".to_string(),
            })
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&target.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some(&format!("aftersale {} api error", action)),
            )?;
            update_aftersale_action_state(
                &conn,
                &target.id,
                action,
                "failed",
                Some(&error.errmsg),
                note,
            )?;
            upsert_notification(
                &conn,
                "warning",
                "aftersale_action",
                &target.id,
                Some(&target.shop_id),
                &format!("售后{}失败", aftersale_action_label(action)),
                &format!(
                    "售后单 {} 提交{}失败：{}",
                    target.wechat_aftersale_id,
                    aftersale_action_label(action),
                    error.errmsg
                ),
                Some(&serde_json::json!({
                    "aftersale_id": &target.id,
                    "wechat_aftersale_id": &target.wechat_aftersale_id,
                    "action": action,
                    "errcode": error.errcode
                })),
            )?;
            Ok(AftersaleActionResult {
                aftersale_id: target.id,
                wechat_aftersale_id: target.wechat_aftersale_id,
                action: action.to_string(),
                status: "failed".to_string(),
                errcode: Some(error.errcode),
                errmsg: Some(error.errmsg.clone()),
                message: format!("售后动作提交失败：{}", error.errmsg),
            })
        }
    }
}

pub(in crate::commands) fn update_aftersale_action_state(
    conn: &Connection,
    aftersale_id: &str,
    action: &str,
    status: &str,
    error: Option<&str>,
    note: Option<&str>,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE aftersales
         SET last_action = ?1,
             last_action_status = ?2,
             last_action_error = ?3,
             last_action_note = ?4,
             last_action_at = ?5,
             updated_at = ?5
         WHERE id = ?6",
        params![action, status, error, note, now, aftersale_id],
    )?;
    Ok(())
}

pub(in crate::commands) fn aftersale_action_label(action: &str) -> &'static str {
    match action {
        "accept" => "同意",
        "reject" => "拒绝",
        _ => "处理",
    }
}

pub(in crate::commands) fn normalize_purchase_issue_type(
    issue_type: &str,
) -> AppResult<(&'static str, &'static str)> {
    match issue_type.trim() {
        "out_of_stock" => Ok(("supplier_out_of_stock", "供应商缺货")),
        "price_changed" => Ok(("supplier_price_changed", "供应商涨价")),
        "supplier_cancelled" => Ok(("supplier_cancelled", "供应商取消")),
        "quality_risk" => Ok(("supplier_quality_risk", "供应商质量风险")),
        "other" => Ok(("supplier_exception", "供应商其他异常")),
        _ => Err(AppError::Validation(
            "采购异常类型必须是 out_of_stock/price_changed/supplier_cancelled/quality_risk/other"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "all")
}

pub(in crate::commands) fn count_notifications(
    conn: &Connection,
    status: Option<&str>,
    severity: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM notifications
         WHERE (?1 IS NULL OR status = ?1)
           AND (?2 IS NULL OR severity = ?2)",
        params![status, severity],
        |row| row.get(0),
    )?)
}

pub(in crate::commands) fn load_notification_views(
    conn: &Connection,
    status: Option<&str>,
    severity: Option<&str>,
    limit: i64,
) -> AppResult<Vec<NotificationView>> {
    let mut stmt = conn.prepare(
        "SELECT
           n.id,
           n.severity,
           n.source_type,
           n.source_id,
           n.shop_id,
           s.name,
           n.title,
           n.body,
           n.status,
           n.data_json,
           n.read_at,
           n.created_at,
           n.updated_at
         FROM notifications n
         LEFT JOIN shops s ON s.id = n.shop_id
         WHERE (?1 IS NULL OR n.status = ?1)
           AND (?2 IS NULL OR n.severity = ?2)
         ORDER BY
           CASE n.status WHEN 'unread' THEN 0 ELSE 1 END,
           CASE n.severity WHEN 'critical' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END,
           n.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![status, severity, limit], |row| {
            Ok(NotificationView {
                id: row.get(0)?,
                severity: row.get(1)?,
                source_type: row.get(2)?,
                source_id: row.get(3)?,
                shop_id: row.get(4)?,
                shop_name: row.get(5)?,
                title: row.get(6)?,
                body: row.get(7)?,
                status: row.get(8)?,
                data_json: row.get(9)?,
                read_at: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(in crate::commands) fn upsert_notification(
    conn: &Connection,
    severity: &str,
    source_type: &str,
    source_id: &str,
    shop_id: Option<&str>,
    title: &str,
    body: &str,
    data: Option<&Value>,
) -> AppResult<()> {
    let dedupe_key = format!("{source_type}:{source_id}");
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO notifications
         (id, severity, source_type, source_id, shop_id, title, body, status,
          dedupe_key, data_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'unread', ?8, ?9, ?10, ?10)
         ON CONFLICT(dedupe_key) DO UPDATE SET
           severity = excluded.severity,
           shop_id = excluded.shop_id,
           title = excluded.title,
           body = excluded.body,
           status = CASE
             WHEN notifications.status = 'read' AND notifications.body = excluded.body THEN notifications.status
             ELSE 'unread'
           END,
           read_at = CASE
             WHEN notifications.status = 'read' AND notifications.body = excluded.body THEN notifications.read_at
             ELSE NULL
           END,
           data_json = excluded.data_json,
           updated_at = excluded.updated_at",
        params![
            format!("notification-{}", Uuid::new_v4()),
            severity,
            source_type,
            source_id,
            shop_id,
            title,
            body,
            dedupe_key,
            data.map(|value| value.to_string()),
            now
        ],
    )?;
    Ok(())
}

/// 国内常用快递公司白名单（微信 delivery_id），按常用程度排序。
/// 微信接口会返回全球 1500+ 家快递公司且无国内/国际标志，
/// 业务只做国内代发，故读取时按此名单过滤（库中仍保留全量原始数据）。
const DOMESTIC_DELIVERY_IDS: &[&str] = &[
    "SF",    // 顺丰速运
    "ZTO",   // 中通快递
    "YTO",   // 圆通速递
    "STO",   // 申通快递
    "YUNDA", // 韵达速递
    "JTSD",  // 极兔速递
    "JD",    // 京东快递
    "EMS",   // 中国邮政
    "YZPY",  // 邮政快递包裹
    "YZBK",  // 邮政国内标快
    "CNSD",  // 菜鸟速递(丹鸟)
    "DNWL",  // 丹鸟物流
    "DBKD",  // 德邦快递
    "KYSY",  // 跨越速运
    "ZJS",   // 宅急送
    "SXJD",  // 顺心捷达
    "ZTOKY", // 中通快运
    "YDKY",  // 韵达快运
    "JDKY",  // 京东快运
    "FWX",   // 丰网速运
    "SURE",  // 速尔快递
    "UC",    // 优速快递
];

pub(in crate::commands) fn load_delivery_company_views(
    conn: &Connection,
    shop_id: Option<&str>,
) -> AppResult<Vec<DeliveryCompanyView>> {
    let sql = if shop_id.is_some() {
        "SELECT shop_id, delivery_id, delivery_name, synced_at
         FROM delivery_companies
         WHERE shop_id = ?1
         ORDER BY delivery_name ASC, delivery_id ASC"
    } else {
        "SELECT shop_id, delivery_id, delivery_name, synced_at
         FROM delivery_companies
         ORDER BY delivery_name ASC, delivery_id ASC"
    };
    let mut stmt = conn.prepare(sql)?;
    let mut rows = if let Some(shop_id) = shop_id {
        stmt.query_map([shop_id], delivery_company_from_row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], delivery_company_from_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    rows.retain(|company| DOMESTIC_DELIVERY_IDS.contains(&company.delivery_id.as_str()));
    rows.sort_by_key(|company| {
        DOMESTIC_DELIVERY_IDS
            .iter()
            .position(|id| *id == company.delivery_id)
            .unwrap_or(usize::MAX)
    });
    Ok(rows)
}

pub(in crate::commands) fn delivery_company_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DeliveryCompanyView> {
    Ok(DeliveryCompanyView {
        shop_id: row.get(0)?,
        delivery_id: row.get(1)?,
        delivery_name: row.get(2)?,
        synced_at: row.get(3)?,
    })
}

pub(in crate::commands) fn load_aftersale_reject_reason_views(
    conn: &Connection,
    shop_id: Option<&str>,
    reject_scene: Option<i64>,
) -> AppResult<Vec<AftersaleRejectReasonView>> {
    let mut sql = "SELECT shop_id,
                          reject_reason_type,
                          reject_reason_type_text,
                          reject_reason,
                          reject_scene,
                          synced_at
                   FROM aftersale_reject_reasons"
        .to_string();
    let mut conditions = Vec::new();
    let mut params_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(shop_id) = shop_id {
        conditions.push("shop_id = ?".to_string());
        params_values.push(Box::new(shop_id.to_string()));
    }
    if let Some(reject_scene) = reject_scene {
        conditions.push("reject_scene = ?".to_string());
        params_values.push(Box::new(reject_scene));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY shop_id ASC, reject_scene ASC, reject_reason_type ASC");

    let params_ref = params_values
        .iter()
        .map(|value| value.as_ref() as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_ref.as_slice(), |row| {
            let reject_scene: Option<i64> = row.get(4)?;
            Ok(AftersaleRejectReasonView {
                shop_id: row.get(0)?,
                reject_reason_type: row.get(1)?,
                reject_reason_type_text: row.get(2)?,
                reject_reason: row.get(3)?,
                reject_scene,
                reject_scene_text: reject_scene_text(reject_scene).to_string(),
                synced_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(in crate::commands) fn upsert_aftersale_reject_reasons(
    conn: &mut Connection,
    shop_id: &str,
    raw_payload: &Value,
) -> AppResult<i64> {
    let reason_list = raw_payload
        .get("reason_list")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::Validation("微信售后拒绝原因响应缺少 reason_list".to_string()))?;
    let now = now_shanghai();
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM aftersale_reject_reasons WHERE shop_id = ?1",
        [shop_id],
    )?;
    let mut synced = 0i64;
    for reason in reason_list {
        let reject_reason_type = json_value_to_i64(reason.get("reject_reason_type"));
        let reject_reason_type_text = json_value_to_string(reason.get("reject_reason_type_text"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let reject_reason = json_value_to_string(reason.get("reject_reason"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (Some(reject_reason_type), Some(reject_reason_type_text), Some(reject_reason)) =
            (reject_reason_type, reject_reason_type_text, reject_reason)
        else {
            continue;
        };
        tx.execute(
            "INSERT INTO aftersale_reject_reasons
             (shop_id, reject_reason_type, reject_reason_type_text, reject_reason,
              reject_scene, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(shop_id, reject_reason_type) DO UPDATE SET
               reject_reason_type_text = excluded.reject_reason_type_text,
               reject_reason = excluded.reject_reason,
               reject_scene = excluded.reject_scene,
               raw_payload = excluded.raw_payload,
               synced_at = excluded.synced_at",
            params![
                shop_id,
                reject_reason_type,
                reject_reason_type_text,
                reject_reason,
                json_value_to_i64(reason.get("reject_scene")),
                reason.to_string(),
                now
            ],
        )?;
        synced += 1;
    }
    tx.commit()?;
    Ok(synced)
}

pub(in crate::commands) fn reject_scene_text(reject_scene: Option<i64>) -> &'static str {
    match reject_scene {
        Some(1) => "拒绝仅退款",
        Some(4) => "拒绝退货退款",
        Some(5) => "拒绝换货",
        Some(6) => "拒绝换货发新商品",
        Some(7) => "极速换货收货处理",
        _ => "未知场景",
    }
}

pub(in crate::commands) fn upsert_delivery_companies(
    conn: &mut Connection,
    shop_id: &str,
    raw_payload: &Value,
) -> AppResult<i64> {
    let company_list = raw_payload
        .get("company_list")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::Validation("微信快递公司列表响应缺少 company_list".to_string()))?;
    let now = now_shanghai();
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM delivery_companies WHERE shop_id = ?1",
        [shop_id],
    )?;
    let mut synced = 0i64;
    for company in company_list {
        let delivery_id = json_value_to_string(company.get("delivery_id"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let delivery_name = json_value_to_string(company.get("delivery_name"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (Some(delivery_id), Some(delivery_name)) = (delivery_id, delivery_name) else {
            continue;
        };
        tx.execute(
            "INSERT INTO delivery_companies
             (shop_id, delivery_id, delivery_name, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(shop_id, delivery_id) DO UPDATE SET
               delivery_name = excluded.delivery_name,
               raw_payload = excluded.raw_payload,
               synced_at = excluded.synced_at",
            params![
                shop_id,
                delivery_id,
                delivery_name,
                company.to_string(),
                now
            ],
        )?;
        synced += 1;
    }
    tx.commit()?;
    Ok(synced)
}
