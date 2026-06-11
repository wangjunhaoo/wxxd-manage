use super::*;

#[tauri::command]
pub fn list_order_profit_summaries(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<OrderProfitListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let all_items = load_order_profit_views(&conn)?;
    let filtered = all_items
        .into_iter()
        .filter(|item| {
            status
                .as_deref()
                .map(|status| item.profit_status == status || item.order_status == status)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let total = filtered.len() as i64;
    let totals = summarize_order_profit(&filtered);
    let items = filtered.into_iter().take(limit as usize).collect();
    Ok(OrderProfitListResult {
        items,
        total,
        totals,
    })
}

#[tauri::command]
pub fn record_order_profit_adjustment(
    app: AppHandle,
    request: OrderProfitAdjustmentRequest,
) -> AppResult<OrderProfitAdjustmentResult> {
    let order_id = request.order_id.trim();
    if order_id.is_empty() {
        return Err(AppError::Validation("订单 ID 不能为空".to_string()));
    }
    if request.amount_cents < 0 {
        return Err(AppError::Validation("金额不能为负数".to_string()));
    }
    let kind = normalize_profit_adjustment_kind(&request.kind)?;
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let conn = open_connection(&app)?;
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM orders WHERE id = ?1 OR wechat_order_id = ?1 LIMIT 1",
            [order_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !exists {
        return Err(AppError::Validation(
            "订单不存在，不能记录利润调整项".to_string(),
        ));
    }
    let resolved_order_id: String = conn.query_row(
        "SELECT id FROM orders WHERE id = ?1 OR wechat_order_id = ?1 LIMIT 1",
        [order_id],
        |row| row.get(0),
    )?;
    let adjustment_id = format!("profit-adj-{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO order_profit_adjustments
         (id, order_id, kind, amount_cents, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            adjustment_id,
            resolved_order_id,
            kind,
            request.amount_cents,
            note,
            now_shanghai()
        ],
    )?;
    Ok(OrderProfitAdjustmentResult {
        adjustment_id,
        order_id: resolved_order_id,
        kind: kind.to_string(),
        amount_cents: request.amount_cents,
        message: "利润调整项已记录".to_string(),
    })
}

#[tauri::command]
pub fn list_inventory_risks(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<InventoryRiskListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let all_items = build_inventory_risk_views(&conn)?;
    let filtered = all_items
        .into_iter()
        .filter(|item| {
            status
                .as_deref()
                .map(|status| item.risk_status == status)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let total = filtered.len() as i64;
    let low_stock_count = filtered
        .iter()
        .filter(|item| item.risk_status == "low_stock" || item.risk_status == "stock_pressure")
        .count() as i64;
    let out_of_stock_count = filtered
        .iter()
        .filter(|item| item.risk_status == "out_of_stock")
        .count() as i64;
    let issue_count = filtered
        .iter()
        .filter(|item| item.risk_status == "supplier_issue")
        .count() as i64;
    let items = filtered.into_iter().take(limit as usize).collect();
    Ok(InventoryRiskListResult {
        items,
        total,
        low_stock_count,
        out_of_stock_count,
        issue_count,
    })
}

#[tauri::command]
pub fn run_inventory_risk_scan_once(app: AppHandle) -> AppResult<InventoryRiskScanResult> {
    let task_id = format!("inventory-risk-scan-{}", Uuid::new_v4());
    let started_at = now_shanghai();
    let conn = open_connection(&app)?;
    conn.execute(
        "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
         VALUES (?1, 'inventory.scan_risks', 'running', 0, ?2, ?2)",
        params![task_id, started_at],
    )?;
    let items = build_inventory_risk_views(&conn)?;
    let mut notifications_created = 0i64;
    for item in &items {
        if matches!(
            item.risk_status.as_str(),
            "out_of_stock" | "low_stock" | "stock_pressure" | "supplier_issue"
        ) {
            let severity = if item.risk_status == "out_of_stock" {
                "critical"
            } else {
                "warning"
            };
            upsert_notification(
                &conn,
                severity,
                "inventory_risk",
                &item.external_product_id,
                None,
                &format!("库存风控：{}", item.title),
                &format!(
                    "{}；可用库存 {}，采购占用 {}，已铺店铺 {} 个。",
                    item.recommendation,
                    item.available_stock,
                    item.reserved_quantity,
                    item.active_shop_count
                ),
                Some(&serde_json::json!({
                    "external_product_id": &item.external_product_id,
                    "risk_status": &item.risk_status,
                    "available_stock": item.available_stock,
                    "reserved_quantity": item.reserved_quantity,
                    "active_shop_count": item.active_shop_count
                })),
            )?;
            notifications_created += 1;
        }
    }
    let low_stock_products = items
        .iter()
        .filter(|item| item.risk_status == "low_stock" || item.risk_status == "stock_pressure")
        .count() as i64;
    let out_of_stock_products = items
        .iter()
        .filter(|item| item.risk_status == "out_of_stock")
        .count() as i64;
    let issue_products = items
        .iter()
        .filter(|item| item.risk_status == "supplier_issue")
        .count() as i64;
    conn.execute(
        "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
        params![now_shanghai(), task_id],
    )?;
    insert_task_log(
        &conn,
        &task_id,
        None,
        "info",
        &format!(
            "库存风控扫描完成：商品 {} 个，断货 {} 个，低库存/压力 {} 个，供应商异常 {} 个",
            items.len(),
            out_of_stock_products,
            low_stock_products,
            issue_products
        ),
        None,
    )?;
    Ok(InventoryRiskScanResult {
        task_id,
        scanned_products: items.len() as i64,
        low_stock_products,
        out_of_stock_products,
        issue_products,
        notifications_created,
    })
}

#[tauri::command]
pub fn list_product_sales_analysis(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<ProductSalesAnalysisListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let all_items = build_product_sales_analysis_views(&conn)?;
    let filtered = all_items
        .into_iter()
        .filter(|item| {
            status
                .as_deref()
                .map(|status| {
                    item.operation_status == status || item.inventory_risk_status == status
                })
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let total = filtered.len() as i64;
    let totals = summarize_product_sales_analysis(&filtered);
    let items = filtered.into_iter().take(limit as usize).collect();
    Ok(ProductSalesAnalysisListResult {
        items,
        total,
        totals,
    })
}

#[tauri::command]
pub fn list_product_management_items(
    app: AppHandle,
    status: Option<String>,
    shop_id: Option<String>,
    keyword: Option<String>,
    limit: Option<i64>,
) -> AppResult<ProductManagementListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let shop_id = normalize_optional_filter(shop_id);
    let keyword = normalize_optional_filter(keyword).map(|value| value.to_lowercase());
    let source_products = load_product_management_sources(&conn)?;
    let shop_products = load_product_management_shop_views(&conn)?;
    let sales_items = build_product_sales_analysis_views(&conn)?
        .into_iter()
        .map(|item| (item.external_product_id.clone(), item))
        .collect::<BTreeMap<_, _>>();

    let mut product_ids = BTreeSet::new();
    product_ids.extend(source_products.keys().cloned());
    product_ids.extend(shop_products.keys().cloned());
    product_ids.extend(sales_items.keys().cloned());

    let mut items = product_ids
        .into_iter()
        .map(|external_product_id| {
            let source = source_products.get(&external_product_id);
            let sales = sales_items.get(&external_product_id);
            let shops = shop_products
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let title = source
                .map(|item| item.title.trim())
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    sales
                        .map(|item| item.title.trim())
                        .filter(|value| !value.is_empty())
                })
                .unwrap_or(external_product_id.as_str())
                .to_string();
            let inventory_risk_status = sales
                .map(|item| item.inventory_risk_status.clone())
                .unwrap_or_else(|| "not_listed".to_string());
            let operation_status = sales
                .map(|item| item.operation_status.clone())
                .unwrap_or_else(|| "not_listed".to_string());
            let management_status = derive_product_management_status(
                source.and_then(|item| item.publish_status.as_deref()),
                &inventory_risk_status,
                sales.map(|item| item.order_count).unwrap_or_default(),
                sales.map(|item| item.active_shop_count).unwrap_or_default(),
                &shops,
            )
            .to_string();
            ProductManagementView {
                external_product_id,
                title,
                source_url: source.and_then(|item| item.source_url.clone()),
                supplier_name: source
                    .and_then(|item| item.supplier_name.clone())
                    .or_else(|| sales.and_then(|item| item.supplier_name.clone())),
                supplier_product_id: source.and_then(|item| item.supplier_product_id.clone()),
                management_status,
                publish_status: source.and_then(|item| item.publish_status.clone()),
                publish_error_summary: source.and_then(|item| item.publish_error_summary.clone()),
                active_shop_count: sales.map(|item| item.active_shop_count).unwrap_or_default(),
                shop_count: shops.len() as i64,
                order_count: sales.map(|item| item.order_count).unwrap_or_default(),
                units_sold: sales.map(|item| item.units_sold).unwrap_or_default(),
                revenue_cents: sales.map(|item| item.revenue_cents).unwrap_or_default(),
                purchase_task_count: sales
                    .map(|item| item.purchase_task_count)
                    .unwrap_or_default(),
                missing_cost_count: sales
                    .map(|item| item.missing_cost_count)
                    .unwrap_or_default(),
                purchase_cost_cents: sales
                    .map(|item| item.purchase_cost_cents)
                    .unwrap_or_default(),
                related_aftersale_count: sales
                    .map(|item| item.related_aftersale_count)
                    .unwrap_or_default(),
                related_refund_cents: sales
                    .map(|item| item.related_refund_cents)
                    .unwrap_or_default(),
                total_stock: sales.map(|item| item.total_stock).unwrap_or_default(),
                available_stock: sales.map(|item| item.available_stock).unwrap_or_default(),
                inventory_risk_status,
                operation_status,
                recommendation: sales
                    .map(|item| item.recommendation.clone())
                    .unwrap_or_else(|| "尚未形成铺货和订单数据".to_string()),
                last_order_at: sales.and_then(|item| item.last_order_at.clone()),
                updated_at: sales
                    .map(|item| item.updated_at.clone())
                    .or_else(|| source.map(|item| item.updated_at.clone()))
                    .unwrap_or_else(now_shanghai),
                shops,
            }
        })
        .filter(|item| {
            shop_id
                .as_deref()
                .map(|shop_id| item.shops.iter().any(|shop| shop.shop_id == shop_id))
                .unwrap_or(true)
        })
        .filter(|item| {
            status
                .as_deref()
                .map(|status| product_management_status_matches(item, status))
                .unwrap_or(true)
        })
        .filter(|item| {
            keyword
                .as_deref()
                .map(|keyword| product_management_keyword_matches(item, keyword))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        product_management_rank(&left.management_status)
            .cmp(&product_management_rank(&right.management_status))
            .then(right.updated_at.cmp(&left.updated_at))
            .then(left.external_product_id.cmp(&right.external_product_id))
    });
    let total = items.len() as i64;
    let items = items.into_iter().take(limit as usize).collect();
    Ok(ProductManagementListResult { items, total })
}

#[tauri::command]
pub fn list_order_management_items(
    app: AppHandle,
    status: Option<String>,
    shop_id: Option<String>,
    keyword: Option<String>,
    limit: Option<i64>,
) -> AppResult<OrderManagementListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let shop_id = normalize_optional_filter(shop_id);
    let keyword = normalize_optional_filter(keyword).map(|value| value.to_lowercase());
    let metas = load_order_management_meta(&conn)?;
    let items_by_order = load_order_management_items(&conn)?;
    let purchase_aggregates = load_order_management_purchase_aggregates(&conn)?;
    let shipment_aggregates = load_order_management_shipment_aggregates(&conn)?;
    let aftersale_counts = load_order_management_aftersale_counts(&conn)?;

    let mut items = load_order_profit_views(&conn)?
        .into_iter()
        .map(|profit| {
            let meta = metas.get(&profit.order_id).cloned().unwrap_or_default();
            let order_items = items_by_order
                .get(&profit.order_id)
                .cloned()
                .unwrap_or_default();
            let purchase = purchase_aggregates
                .get(&profit.order_id)
                .cloned()
                .unwrap_or_default();
            let shipment = shipment_aggregates
                .get(&profit.order_id)
                .cloned()
                .unwrap_or_default();
            let active_aftersale_count = aftersale_counts
                .get(&profit.order_id)
                .copied()
                .unwrap_or_default();
            let purchase_status = derive_purchase_management_status(
                profit.item_count,
                profit.missing_cost_count,
                &purchase,
            )
            .to_string();
            let shipment_status = derive_shipment_management_status(&shipment).to_string();
            let management_status = derive_order_management_status(
                &profit.order_status,
                profit.detail_synced_at.as_deref(),
                meta.detail_error.as_deref(),
                &purchase_status,
                &shipment_status,
                active_aftersale_count,
            );
            OrderManagementView {
                order_id: profit.order_id,
                wechat_order_id: profit.wechat_order_id,
                shop_id: profit.shop_id,
                shop_name: profit.shop_name,
                wechat_status: meta.wechat_status,
                order_status: profit.order_status,
                management_status,
                item_count: profit.item_count,
                quantity: profit.quantity,
                revenue_cents: profit.revenue_cents,
                purchase_task_count: profit.purchase_task_count,
                missing_cost_count: profit.missing_cost_count,
                purchase_status,
                shipment_count: shipment.shipment_count,
                shipment_status,
                active_aftersale_count,
                profit_status: profit.profit_status,
                estimated_profit_cents: profit.estimated_profit_cents,
                actual_profit_cents: profit.actual_profit_cents,
                detail_synced_at: profit.detail_synced_at,
                detail_error: meta.detail_error,
                synced_at: meta.synced_at,
                order_created_at: meta.order_created_at,
                order_updated_at: meta.order_updated_at,
                updated_at: profit.updated_at.or(meta.updated_at),
                address_under_review: meta.address_under_review,
                change_sku_state: meta.change_sku_state,
                delivery_deadline: meta.delivery_deadline,
                customer_notes: meta.customer_notes,
                merchant_notes: meta.merchant_notes,
                items: order_items,
            }
        })
        .filter(|item| {
            shop_id
                .as_deref()
                .map(|shop_id| item.shop_id == shop_id)
                .unwrap_or(true)
        })
        .filter(|item| {
            status
                .as_deref()
                .map(|status| order_management_status_matches(item, status))
                .unwrap_or(true)
        })
        .filter(|item| {
            keyword
                .as_deref()
                .map(|keyword| order_management_keyword_matches(item, keyword))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        order_management_rank(&left.management_status)
            .cmp(&order_management_rank(&right.management_status))
            .then(
                right
                    .updated_at
                    .as_deref()
                    .unwrap_or("")
                    .cmp(left.updated_at.as_deref().unwrap_or("")),
            )
            .then(left.wechat_order_id.cmp(&right.wechat_order_id))
    });
    let total = items.len() as i64;
    let items = items.into_iter().take(limit as usize).collect();
    Ok(OrderManagementListResult { items, total })
}

#[derive(Debug, Default, Clone)]
struct ProductManagementSource {
    title: String,
    source_url: Option<String>,
    supplier_name: Option<String>,
    supplier_product_id: Option<String>,
    publish_status: Option<String>,
    publish_error_summary: Option<String>,
    updated_at: String,
}

#[derive(Debug, Default, Clone)]
struct OrderManagementMeta {
    wechat_status: Option<i64>,
    synced_at: Option<String>,
    order_created_at: Option<i64>,
    order_updated_at: Option<i64>,
    address_under_review: bool,
    change_sku_state: Option<i64>,
    delivery_deadline: Option<i64>,
    customer_notes: Option<String>,
    merchant_notes: Option<String>,
    detail_error: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Default, Clone)]
struct OrderManagementPurchaseAggregate {
    pending_count: i64,
    needs_mapping_count: i64,
    issue_count: i64,
    shipped_count: i64,
    completed_count: i64,
}

#[derive(Debug, Default, Clone)]
struct OrderManagementShipmentAggregate {
    shipment_count: i64,
    waiting_count: i64,
    ready_count: i64,
    failed_count: i64,
    blocked_count: i64,
    shipped_count: i64,
}

fn load_product_management_sources(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductManagementSource>> {
    let mut stmt = conn.prepare(
        "SELECT external_product_id, title, source_url, raw_payload, status, error_summary, created_at
         FROM publish_products
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut products = BTreeMap::new();
    for (
        external_product_id,
        fallback_title,
        source_url,
        raw_payload,
        publish_status,
        publish_error_summary,
        updated_at,
    ) in rows
    {
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
        products.insert(
            external_product_id,
            ProductManagementSource {
                title,
                source_url: clean_optional_string(source_url),
                supplier_name: parsed
                    .as_ref()
                    .and_then(|product| product.supplier_name.clone()),
                supplier_product_id: parsed
                    .as_ref()
                    .and_then(|product| product.supplier_product_id.clone()),
                publish_status,
                publish_error_summary: clean_optional_string(publish_error_summary),
                updated_at,
            },
        );
    }
    Ok(products)
}

fn load_product_management_shop_views(
    conn: &Connection,
) -> AppResult<BTreeMap<String, Vec<ProductManagementShopView>>> {
    let mut stmt = conn.prepare(
        "SELECT
           sp.external_product_id,
           sp.shop_id,
           COALESCE(s.name, sp.shop_id),
           sp.status,
           sp.wechat_product_id,
           sp.wechat_status,
           sp.wechat_edit_status,
           sp.current_price_cents,
           sp.last_status_sync_at,
           sp.last_price_update_at,
           sp.audit_summary,
           sp.source_url
         FROM shop_products sp
         LEFT JOIN shops s ON s.id = sp.shop_id
         WHERE sp.external_product_id IS NOT NULL AND TRIM(sp.external_product_id) != ''
         ORDER BY sp.created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductManagementShopView {
                    shop_id: row.get(1)?,
                    shop_name: row.get(2)?,
                    status: row.get(3)?,
                    wechat_product_id: row.get(4)?,
                    wechat_status: row.get(5)?,
                    wechat_edit_status: row.get(6)?,
                    current_price_cents: row.get(7)?,
                    last_status_sync_at: row.get(8)?,
                    last_price_update_at: row.get(9)?,
                    audit_summary: row.get(10)?,
                    source_url: row.get(11)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut grouped: BTreeMap<String, Vec<ProductManagementShopView>> = BTreeMap::new();
    for (external_product_id, item) in rows {
        grouped.entry(external_product_id).or_default().push(item);
    }
    Ok(grouped)
}

fn load_order_management_meta(
    conn: &Connection,
) -> AppResult<BTreeMap<String, OrderManagementMeta>> {
    let mut stmt = conn.prepare(
        "SELECT
           id,
           wechat_status,
           synced_at,
           order_created_at,
           order_updated_at,
           detail_error,
           updated_at,
           COALESCE(address_under_review, 0),
           change_sku_state,
           delivery_deadline,
           customer_notes,
           merchant_notes
         FROM orders",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                OrderManagementMeta {
                    wechat_status: row.get(1)?,
                    synced_at: row.get(2)?,
                    order_created_at: row.get(3)?,
                    order_updated_at: row.get(4)?,
                    detail_error: row.get(5)?,
                    updated_at: row.get(6)?,
                    address_under_review: row.get::<_, i64>(7)? == 1,
                    change_sku_state: row.get(8)?,
                    delivery_deadline: row.get(9)?,
                    customer_notes: row.get(10)?,
                    merchant_notes: row.get(11)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

fn load_order_management_items(
    conn: &Connection,
) -> AppResult<BTreeMap<String, Vec<OrderManagementItemView>>> {
    let mut stmt = conn.prepare(
        "SELECT
           order_id,
           id,
           wechat_product_id,
           wechat_sku_id,
           out_product_id,
           out_sku_id,
           title,
           sku_count,
           sale_price,
           real_price
         FROM order_items
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                OrderManagementItemView {
                    id: row.get(1)?,
                    wechat_product_id: row.get(2)?,
                    wechat_sku_id: row.get(3)?,
                    external_product_id: row.get(4)?,
                    external_sku_id: row.get(5)?,
                    title: row.get(6)?,
                    quantity: row.get(7)?,
                    sale_price: row.get(8)?,
                    real_price: row.get(9)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut grouped: BTreeMap<String, Vec<OrderManagementItemView>> = BTreeMap::new();
    for (order_id, item) in rows {
        grouped.entry(order_id).or_default().push(item);
    }
    Ok(grouped)
}

fn load_order_management_purchase_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, OrderManagementPurchaseAggregate>> {
    let mut stmt = conn.prepare(
        "SELECT
           order_id,
           COALESCE(SUM(CASE WHEN status = 'pending_purchase' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status = 'needs_mapping' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE
             WHEN status IN ('supplier_out_of_stock', 'supplier_price_changed',
                             'supplier_cancelled', 'supplier_quality_risk', 'supplier_exception')
             THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status = 'supplier_shipped' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status IN ('wechat_shipped', 'completed') THEN 1 ELSE 0 END), 0)
         FROM purchase_tasks
         GROUP BY order_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                OrderManagementPurchaseAggregate {
                    pending_count: row.get(1)?,
                    needs_mapping_count: row.get(2)?,
                    issue_count: row.get(3)?,
                    shipped_count: row.get(4)?,
                    completed_count: row.get(5)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

fn load_order_management_shipment_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, OrderManagementShipmentAggregate>> {
    let mut stmt = conn.prepare(
        "SELECT
           order_id,
           COUNT(*),
           COALESCE(SUM(CASE WHEN status = 'waiting_confirmation' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status = 'ready_to_send' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status = 'send_failed' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status = 'blocked' THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN status = 'wechat_shipped' THEN 1 ELSE 0 END), 0)
         FROM shipments
         GROUP BY order_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                OrderManagementShipmentAggregate {
                    shipment_count: row.get(1)?,
                    waiting_count: row.get(2)?,
                    ready_count: row.get(3)?,
                    failed_count: row.get(4)?,
                    blocked_count: row.get(5)?,
                    shipped_count: row.get(6)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

fn load_order_management_aftersale_counts(conn: &Connection) -> AppResult<BTreeMap<String, i64>> {
    // 售后 + 纠纷一并计为「售后活跃」：旧实现靠 orders.status='aftersale_active' 间接覆盖纠纷，
    // 重构后该状态不再写入（改为 has_active_aftersale 标志），这里直接把纠纷单纳入计数。
    let mut stmt = conn.prepare(
        "SELECT order_id, SUM(cnt) FROM (
           SELECT order_id, COUNT(*) AS cnt
           FROM aftersales
           WHERE order_id IS NOT NULL
             AND UPPER(status) NOT IN (
               'USER_CANCELD', 'USER_CANCELLED', 'RETURN_CLOSED',
               'MERCHANT_REFUND_SUCCESS', 'MERCHANT_RETURN_SUCCESS',
               'MERCHANT_REFUND_RETRY_FAIL', 'MERCHANT_FAIL',
               'MERCHANT_EXCHANGE_SUCCESS', 'SYNC_FAILED'
             )
           GROUP BY order_id
           UNION ALL
           SELECT order_id, COUNT(*) AS cnt
           FROM guarantee_orders
           WHERE order_id IS NOT NULL
             AND UPPER(status) NOT IN (
               'STATUS_NO_NEED_PAY', 'STATUS_PAY_SUCC', 'STATUS_USER_CANCEL', 'SYNC_FAILED'
             )
           GROUP BY order_id
         )
         GROUP BY order_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

fn derive_product_management_status(
    publish_status: Option<&str>,
    inventory_risk_status: &str,
    order_count: i64,
    active_shop_count: i64,
    shops: &[ProductManagementShopView],
) -> &'static str {
    if publish_status.map(is_failed_status).unwrap_or(false)
        || shops.iter().any(|shop| is_failed_status(&shop.status))
    {
        return "publish_failed";
    }
    if matches!(
        inventory_risk_status,
        "out_of_stock" | "supplier_issue" | "stock_pressure" | "low_stock"
    ) {
        return "inventory_risk";
    }
    if active_shop_count == 0 || shops.is_empty() {
        return "not_listed";
    }
    if order_count > 0 {
        "listed_sold"
    } else {
        "listed_unsold"
    }
}

fn derive_purchase_management_status(
    item_count: i64,
    missing_cost_count: i64,
    aggregate: &OrderManagementPurchaseAggregate,
) -> &'static str {
    if aggregate.needs_mapping_count > 0 {
        "needs_mapping"
    } else if aggregate.issue_count > 0 {
        "purchase_issue"
    } else if aggregate.pending_count > 0 {
        "pending_purchase"
    } else if missing_cost_count > 0 {
        "missing_cost"
    } else if aggregate.shipped_count > 0 {
        "supplier_shipped"
    } else if aggregate.completed_count > 0 {
        "completed"
    } else if item_count > 0 {
        "missing_purchase_task"
    } else {
        "none"
    }
}

fn derive_shipment_management_status(aggregate: &OrderManagementShipmentAggregate) -> &'static str {
    if aggregate.shipment_count == 0 {
        "none"
    } else if aggregate.failed_count > 0 {
        "send_failed"
    } else if aggregate.blocked_count > 0 {
        // 守卫拦截（改址/换SKU/售后在途）：优先于待提交展示，提醒先去收件箱裁决
        "blocked"
    } else if aggregate.ready_count > 0 {
        "ready_to_send"
    } else if aggregate.waiting_count > 0 {
        "waiting_confirmation"
    } else if aggregate.shipped_count > 0 {
        "wechat_shipped"
    } else {
        "pending"
    }
}

fn derive_order_management_status(
    order_status: &str,
    detail_synced_at: Option<&str>,
    detail_error: Option<&str>,
    purchase_status: &str,
    shipment_status: &str,
    active_aftersale_count: i64,
) -> String {
    if detail_error
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        return "detail_failed".to_string();
    }
    if detail_synced_at.is_none() {
        return "needs_detail".to_string();
    }
    if matches!(
        purchase_status,
        "needs_mapping"
            | "purchase_issue"
            | "pending_purchase"
            | "missing_cost"
            | "missing_purchase_task"
    ) {
        return "needs_purchase".to_string();
    }
    // partially_shipped（官方 21 部分发货）仍有剩余商品待发，与待发货同入「需发货」队列
    if matches!(order_status, "pending_shipment" | "partially_shipped")
        && shipment_status != "wechat_shipped"
    {
        return "needs_shipment".to_string();
    }
    if active_aftersale_count > 0 || order_status == "aftersale_active" {
        return "aftersale_active".to_string();
    }
    order_status.to_string()
}

fn product_management_status_matches(item: &ProductManagementView, status: &str) -> bool {
    item.management_status == status
        || item.inventory_risk_status == status
        || item.operation_status == status
        || item.publish_status.as_deref() == Some(status)
        || item.shops.iter().any(|shop| shop.status == status)
}

fn order_management_status_matches(item: &OrderManagementView, status: &str) -> bool {
    item.management_status == status
        || item.order_status == status
        || item.purchase_status == status
        || item.shipment_status == status
        || item.profit_status == status
}

fn product_management_keyword_matches(item: &ProductManagementView, keyword: &str) -> bool {
    string_contains(&item.external_product_id, keyword)
        || string_contains(&item.title, keyword)
        || option_contains(&item.source_url, keyword)
        || option_contains(&item.supplier_name, keyword)
        || option_contains(&item.supplier_product_id, keyword)
        || item.shops.iter().any(|shop| {
            option_contains(&shop.wechat_product_id, keyword)
                || string_contains(&shop.shop_name, keyword)
        })
}

fn order_management_keyword_matches(item: &OrderManagementView, keyword: &str) -> bool {
    string_contains(&item.order_id, keyword)
        || string_contains(&item.wechat_order_id, keyword)
        || string_contains(&item.shop_name, keyword)
        || item.items.iter().any(|order_item| {
            option_contains(&order_item.title, keyword)
                || option_contains(&order_item.external_product_id, keyword)
                || option_contains(&order_item.external_sku_id, keyword)
                || option_contains(&order_item.wechat_product_id, keyword)
                || option_contains(&order_item.wechat_sku_id, keyword)
        })
}

fn product_management_rank(status: &str) -> i64 {
    match status {
        "publish_failed" => 0,
        "inventory_risk" => 1,
        "not_listed" => 2,
        "listed_sold" => 3,
        "listed_unsold" => 4,
        _ => 5,
    }
}

fn order_management_rank(status: &str) -> i64 {
    match status {
        "detail_failed" => 0,
        "needs_detail" => 1,
        "needs_purchase" => 2,
        "needs_shipment" => 3,
        "aftersale_active" => 4,
        _ => 5,
    }
}

fn is_failed_status(status: &str) -> bool {
    matches!(status, "failed" | "sync_failed" | "send_failed")
}

fn clean_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn string_contains(value: &str, keyword: &str) -> bool {
    value.to_lowercase().contains(keyword)
}

fn option_contains(value: &Option<String>, keyword: &str) -> bool {
    value
        .as_deref()
        .map(|value| string_contains(value, keyword))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_management_status_prioritizes_failures_and_risks() {
        let failed_shop = ProductManagementShopView {
            shop_id: "shop-1".to_string(),
            shop_name: "店铺".to_string(),
            status: "failed".to_string(),
            wechat_product_id: None,
            wechat_status: None,
            wechat_edit_status: None,
            current_price_cents: None,
            last_status_sync_at: None,
            last_price_update_at: None,
            audit_summary: None,
            source_url: None,
        };
        assert_eq!(
            derive_product_management_status(None, "healthy", 1, 1, &[failed_shop]),
            "publish_failed"
        );
        assert_eq!(
            derive_product_management_status(None, "low_stock", 1, 1, &[]),
            "inventory_risk"
        );
        assert_eq!(
            derive_product_management_status(None, "healthy", 0, 0, &[]),
            "not_listed"
        );
    }

    #[test]
    fn order_management_status_prioritizes_actionable_work() {
        assert_eq!(
            derive_order_management_status(
                "pending_shipment",
                Some("2026-05-28 10:00:00"),
                Some("失败"),
                "completed",
                "wechat_shipped",
                0,
            ),
            "detail_failed"
        );
        assert_eq!(
            derive_order_management_status(
                "pending_shipment",
                Some("2026-05-28 10:00:00"),
                None,
                "pending_purchase",
                "none",
                0,
            ),
            "needs_purchase"
        );
        assert_eq!(
            derive_order_management_status(
                "pending_shipment",
                Some("2026-05-28 10:00:00"),
                None,
                "completed",
                "none",
                0,
            ),
            "needs_shipment"
        );
    }
}
