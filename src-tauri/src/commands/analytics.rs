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
