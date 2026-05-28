use super::*;

pub(in crate::commands) fn build_inventory_risk_views(
    conn: &Connection,
) -> AppResult<Vec<InventoryRiskView>> {
    let mut products = load_inventory_source_products(conn)?;
    let purchase_aggregates = load_inventory_purchase_aggregates(conn)?;
    let shop_counts = load_inventory_shop_counts(conn)?;
    for external_product_id in purchase_aggregates.keys().chain(shop_counts.keys()) {
        products
            .entry(external_product_id.clone())
            .or_insert_with(|| InventorySourceProduct {
                title: external_product_id.clone(),
                updated_at: now_shanghai(),
                ..InventorySourceProduct::default()
            });
    }

    let mut items = products
        .into_iter()
        .map(|(external_product_id, product)| {
            let purchase = purchase_aggregates
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let active_shop_count = shop_counts
                .get(&external_product_id)
                .copied()
                .unwrap_or_default();
            let available_stock = product.total_stock - purchase.reserved_quantity;
            let (risk_status, recommendation) = resolve_inventory_risk(
                product.total_stock,
                purchase.reserved_quantity,
                available_stock,
                purchase.pending_purchase_quantity,
                purchase.supplier_issue_count,
                active_shop_count,
            );
            InventoryRiskView {
                external_product_id,
                title: product.title,
                supplier_name: product.supplier_name,
                supplier_product_id: product.supplier_product_id,
                total_stock: product.total_stock,
                reserved_quantity: purchase.reserved_quantity,
                available_stock,
                active_shop_count,
                pending_purchase_quantity: purchase.pending_purchase_quantity,
                supplier_issue_count: purchase.supplier_issue_count,
                risk_status: risk_status.to_string(),
                recommendation: recommendation.to_string(),
                updated_at: product.updated_at,
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        inventory_risk_rank(&left.risk_status)
            .cmp(&inventory_risk_rank(&right.risk_status))
            .then(left.available_stock.cmp(&right.available_stock))
            .then(left.external_product_id.cmp(&right.external_product_id))
    });
    Ok(items)
}

pub(in crate::commands) fn load_inventory_source_products(
    conn: &Connection,
) -> AppResult<BTreeMap<String, InventorySourceProduct>> {
    let mut stmt = conn.prepare(
        "SELECT external_product_id, title, raw_payload, created_at
         FROM publish_products
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut products = BTreeMap::new();
    for (external_product_id, fallback_title, raw_payload, created_at) in rows {
        if products.contains_key(&external_product_id) {
            continue;
        }
        let parsed = serde_json::from_str::<ExternalProductInput>(&raw_payload).ok();
        let title = parsed
            .as_ref()
            .map(|product| product.title.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback_title.trim())
            .to_string();
        let total_stock = parsed
            .as_ref()
            .map(|product| product.skus.iter().map(|sku| sku.stock.max(0)).sum())
            .unwrap_or_default();
        products.insert(
            external_product_id,
            InventorySourceProduct {
                title,
                supplier_name: parsed
                    .as_ref()
                    .and_then(|product| product.supplier_name.clone()),
                supplier_product_id: parsed
                    .as_ref()
                    .and_then(|product| product.supplier_product_id.clone()),
                total_stock,
                updated_at: created_at,
            },
        );
    }
    Ok(products)
}

pub(in crate::commands) fn load_inventory_purchase_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, InventoryPurchaseAggregate>> {
    let mut stmt = conn.prepare(
        "SELECT
           external_product_id,
           COALESCE(SUM(CASE
             WHEN status IN ('pending_purchase', 'supplier_shipped', 'wechat_shipped', 'completed')
             THEN quantity ELSE 0 END), 0) AS reserved_quantity,
           COALESCE(SUM(CASE WHEN status = 'pending_purchase' THEN quantity ELSE 0 END), 0)
             AS pending_purchase_quantity,
           COALESCE(SUM(CASE
             WHEN status IN ('supplier_out_of_stock', 'supplier_price_changed',
                             'supplier_cancelled', 'supplier_quality_risk', 'supplier_exception')
             THEN 1 ELSE 0 END), 0) AS supplier_issue_count
         FROM purchase_tasks
         WHERE external_product_id IS NOT NULL AND TRIM(external_product_id) != ''
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                InventoryPurchaseAggregate {
                    reserved_quantity: row.get(1)?,
                    pending_purchase_quantity: row.get(2)?,
                    supplier_issue_count: row.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(in crate::commands) fn load_inventory_shop_counts(
    conn: &Connection,
) -> AppResult<BTreeMap<String, i64>> {
    let mut stmt = conn.prepare(
        "SELECT external_product_id, COUNT(DISTINCT shop_id)
         FROM shop_products
         WHERE external_product_id IS NOT NULL AND TRIM(external_product_id) != ''
           AND status NOT IN ('deleted', 'failed')
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(in crate::commands) fn resolve_inventory_risk(
    total_stock: i64,
    reserved_quantity: i64,
    available_stock: i64,
    pending_purchase_quantity: i64,
    supplier_issue_count: i64,
    active_shop_count: i64,
) -> (&'static str, &'static str) {
    if supplier_issue_count > 0 {
        return (
            "supplier_issue",
            "存在供应商异常采购任务，先人工处理供应商缺货、涨价或取消",
        );
    }
    if total_stock <= 0 || available_stock <= 0 {
        return (
            "out_of_stock",
            "货源库存已不足，建议暂停继续铺货并评估下架或换源",
        );
    }
    if pending_purchase_quantity > available_stock {
        return (
            "stock_pressure",
            "待采购数量超过可用库存，建议优先补货或减少铺货范围",
        );
    }
    if available_stock <= 5 {
        return ("low_stock", "可用库存偏低，建议补库存或降低新店铺铺货节奏");
    }
    if active_shop_count == 0 && reserved_quantity == 0 {
        return ("not_listed", "尚未形成有效店铺商品，可继续进入铺货任务");
    }
    ("healthy", "库存风险正常，可继续观察真实订单和售后表现")
}

pub(in crate::commands) fn inventory_risk_rank(status: &str) -> i64 {
    match status {
        "out_of_stock" => 0,
        "supplier_issue" => 1,
        "stock_pressure" => 2,
        "low_stock" => 3,
        "not_listed" => 4,
        _ => 5,
    }
}

pub(in crate::commands) fn build_product_sales_analysis_views(
    conn: &Connection,
) -> AppResult<Vec<ProductSalesAnalysisView>> {
    let source_products = load_inventory_source_products(conn)?;
    let inventory_risks = build_inventory_risk_views(conn)?
        .into_iter()
        .map(|item| (item.external_product_id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let sales = load_product_sales_aggregates(conn)?;
    let purchase_costs = load_product_purchase_cost_aggregates(conn)?;
    let aftersales = load_product_aftersale_aggregates(conn)?;

    let mut product_ids = BTreeSet::new();
    product_ids.extend(source_products.keys().cloned());
    product_ids.extend(inventory_risks.keys().cloned());
    product_ids.extend(sales.keys().cloned());
    product_ids.extend(purchase_costs.keys().cloned());
    product_ids.extend(aftersales.keys().cloned());

    let mut items = product_ids
        .into_iter()
        .map(|external_product_id| {
            let source = source_products.get(&external_product_id);
            let inventory = inventory_risks.get(&external_product_id);
            let sale = sales.get(&external_product_id).cloned().unwrap_or_default();
            let purchase = purchase_costs
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let aftersale = aftersales
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let title = source
                .map(|product| product.title.trim())
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    inventory
                        .map(|item| item.title.trim())
                        .filter(|value| !value.is_empty())
                })
                .or_else(|| (!sale.title.trim().is_empty()).then_some(sale.title.trim()))
                .unwrap_or(external_product_id.as_str())
                .to_string();
            let total_stock = inventory.map(|item| item.total_stock).unwrap_or_default();
            let available_stock = inventory
                .map(|item| item.available_stock)
                .unwrap_or_default();
            let active_shop_count = inventory
                .map(|item| item.active_shop_count)
                .unwrap_or_default();
            let inventory_risk_status = inventory
                .map(|item| item.risk_status.clone())
                .unwrap_or_else(|| "not_listed".to_string());
            let gross_profit_cents =
                sale.revenue_cents - purchase.purchase_cost_cents - aftersale.related_refund_cents;
            let (operation_status, recommendation) = resolve_product_operation_status(
                sale.units_sold,
                sale.order_count,
                active_shop_count,
                available_stock,
                gross_profit_cents,
                purchase.purchase_task_count,
                purchase.missing_cost_count,
                aftersale.related_aftersale_count,
                &inventory_risk_status,
            );
            ProductSalesAnalysisView {
                external_product_id,
                title,
                supplier_name: source.and_then(|product| product.supplier_name.clone()),
                active_shop_count,
                order_count: sale.order_count,
                units_sold: sale.units_sold,
                revenue_cents: sale.revenue_cents,
                purchase_task_count: purchase.purchase_task_count,
                missing_cost_count: purchase.missing_cost_count,
                purchase_cost_cents: purchase.purchase_cost_cents,
                related_aftersale_count: aftersale.related_aftersale_count,
                related_refund_cents: aftersale.related_refund_cents,
                total_stock,
                available_stock,
                inventory_risk_status,
                operation_status: operation_status.to_string(),
                recommendation: recommendation.to_string(),
                last_order_at: sale.last_order_at,
                updated_at: source
                    .map(|product| product.updated_at.clone())
                    .or_else(|| inventory.map(|item| item.updated_at.clone()))
                    .unwrap_or_else(now_shanghai),
            }
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        product_operation_rank(&left.operation_status)
            .cmp(&product_operation_rank(&right.operation_status))
            .then(right.units_sold.cmp(&left.units_sold))
            .then(right.revenue_cents.cmp(&left.revenue_cents))
            .then(left.external_product_id.cmp(&right.external_product_id))
    });
    Ok(items)
}

pub(in crate::commands) fn load_product_sales_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductSalesAggregate>> {
    let mut stmt = conn.prepare(
        "WITH product_order_items AS (
           SELECT
             COALESCE(NULLIF(TRIM(oi.out_product_id), ''), NULLIF(TRIM(pt.external_product_id), '')) AS external_product_id,
             oi.order_id,
             COALESCE(NULLIF(TRIM(oi.title), ''), '') AS title,
             COALESCE(oi.sku_count, 0) AS sku_count,
             COALESCE(oi.real_price, oi.sale_price * oi.sku_count, 0) AS revenue_cents,
             COALESCE(o.updated_at, o.synced_at, o.created_at) AS order_time,
             COALESCE(o.status, '') AS order_status
           FROM order_items oi
           LEFT JOIN orders o ON o.id = oi.order_id
           LEFT JOIN purchase_tasks pt ON pt.order_item_id = oi.id
         )
         SELECT
           external_product_id,
           MAX(title),
           COUNT(DISTINCT order_id),
           COALESCE(SUM(sku_count), 0),
           COALESCE(SUM(revenue_cents), 0),
           MAX(order_time)
         FROM product_order_items
         WHERE external_product_id IS NOT NULL
           AND external_product_id != ''
           AND order_status NOT IN ('cancelled')
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductSalesAggregate {
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    order_count: row.get(2)?,
                    units_sold: row.get(3)?,
                    revenue_cents: row.get(4)?,
                    last_order_at: row.get(5)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(in crate::commands) fn load_product_purchase_cost_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductPurchaseCostAggregate>> {
    let mut stmt = conn.prepare(
        "SELECT
           external_product_id,
           COUNT(*) AS purchase_task_count,
           COALESCE(SUM(CASE WHEN estimated_cost IS NULL THEN 1 ELSE 0 END), 0) AS missing_cost_count,
           COALESCE(SUM(CASE
             WHEN estimated_cost IS NULL THEN 0
             ELSE CAST(ROUND(estimated_cost * 100) AS INTEGER)
           END), 0) AS purchase_cost_cents
         FROM purchase_tasks
         WHERE external_product_id IS NOT NULL AND TRIM(external_product_id) != ''
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductPurchaseCostAggregate {
                    purchase_task_count: row.get(1)?,
                    missing_cost_count: row.get(2)?,
                    purchase_cost_cents: row.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(in crate::commands) fn load_product_aftersale_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductAftersaleAggregate>> {
    let mut stmt = conn.prepare(
        "WITH product_orders AS (
           SELECT DISTINCT
             COALESCE(NULLIF(TRIM(oi.out_product_id), ''), NULLIF(TRIM(pt.external_product_id), '')) AS external_product_id,
             oi.order_id,
             oi.wechat_order_id
           FROM order_items oi
           LEFT JOIN purchase_tasks pt ON pt.order_item_id = oi.id
         ),
         product_aftersales AS (
           SELECT DISTINCT
             po.external_product_id,
             a.id AS aftersale_id,
             COALESCE(a.refund_amount_cents, 0) AS refund_amount_cents
           FROM product_orders po
           JOIN aftersales a
             ON a.order_id = po.order_id
             OR (
               a.wechat_order_id IS NOT NULL
               AND po.wechat_order_id IS NOT NULL
               AND a.wechat_order_id = po.wechat_order_id
             )
           WHERE po.external_product_id IS NOT NULL AND po.external_product_id != ''
         )
         SELECT
           external_product_id,
           COUNT(aftersale_id),
           COALESCE(SUM(refund_amount_cents), 0)
         FROM product_aftersales
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductAftersaleAggregate {
                    related_aftersale_count: row.get(1)?,
                    related_refund_cents: row.get(2)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(in crate::commands) fn summarize_product_sales_analysis(
    items: &[ProductSalesAnalysisView],
) -> ProductSalesAnalysisTotals {
    ProductSalesAnalysisTotals {
        product_count: items.len() as i64,
        sold_product_count: items.iter().filter(|item| item.units_sold > 0).count() as i64,
        total_units_sold: items.iter().map(|item| item.units_sold).sum(),
        revenue_cents: items.iter().map(|item| item.revenue_cents).sum(),
        purchase_cost_cents: items.iter().map(|item| item.purchase_cost_cents).sum(),
        gross_profit_cents: items
            .iter()
            .map(|item| item.revenue_cents - item.purchase_cost_cents - item.related_refund_cents)
            .sum(),
        missing_cost_product_count: items
            .iter()
            .filter(|item| item.missing_cost_count > 0)
            .count() as i64,
        scale_candidate_count: items
            .iter()
            .filter(|item| item.operation_status == "scale_candidate")
            .count() as i64,
        risk_product_count: items
            .iter()
            .filter(|item| {
                matches!(
                    item.operation_status.as_str(),
                    "stock_risk" | "margin_risk" | "aftersale_watch"
                )
            })
            .count() as i64,
    }
}

pub(in crate::commands) fn resolve_product_operation_status(
    units_sold: i64,
    order_count: i64,
    active_shop_count: i64,
    available_stock: i64,
    gross_profit_cents: i64,
    purchase_task_count: i64,
    missing_cost_count: i64,
    related_aftersale_count: i64,
    inventory_risk_status: &str,
) -> (&'static str, &'static str) {
    if matches!(
        inventory_risk_status,
        "out_of_stock" | "supplier_issue" | "stock_pressure" | "low_stock"
    ) {
        return (
            "stock_risk",
            "库存或供应商风险已触发，先处理补货、换源或暂停继续铺货",
        );
    }
    if units_sold > 0
        && purchase_task_count > 0
        && missing_cost_count == 0
        && gross_profit_cents < 0
    {
        return (
            "margin_risk",
            "已有真实订单但毛利为负，优先核对成本并进入批量改价",
        );
    }
    if units_sold > 0 && related_aftersale_count > 0 {
        return (
            "aftersale_watch",
            "已有订单关联售后，先观察原因和责任方再扩大铺货",
        );
    }
    if units_sold >= 3 && order_count >= 2 && gross_profit_cents > 0 && available_stock > 5 {
        return (
            "scale_candidate",
            "真实订单、毛利和库存都较健康，可优先追加店铺或测试放量",
        );
    }
    if active_shop_count > 0 && units_sold == 0 {
        return (
            "no_sales",
            "已铺货但暂无真实订单，建议检查价格、标题、素材或下架节奏",
        );
    }
    if active_shop_count == 0 {
        return ("not_listed", "尚未形成有效店铺商品，可进入铺货候选池");
    }
    if units_sold > 0 {
        return ("steady", "已有真实订单，继续观察毛利、库存和售后变化");
    }
    ("observe", "数据不足，先保持观察，不做自动动销动作")
}

pub(in crate::commands) fn product_operation_rank(status: &str) -> i64 {
    match status {
        "stock_risk" => 0,
        "margin_risk" => 1,
        "aftersale_watch" => 2,
        "scale_candidate" => 3,
        "no_sales" => 4,
        "not_listed" => 5,
        "steady" => 6,
        _ => 7,
    }
}
