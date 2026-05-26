use super::*;

#[tauri::command]
pub fn run_purchase_task_generation_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PurchaseTaskBatchResult> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let candidates = {
        let conn = open_connection(&app)?;
        load_purchase_candidates(&conn, limit)?
    };
    let task_id = format!("purchase-task-gen-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    let conn = open_connection(&app)?;
    conn.execute(
        "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
         VALUES (?1, 'procurement.create_purchase_tasks', 'running', 0, ?2, ?2)",
        params![task_id, created_at],
    )?;

    if candidates.is_empty() {
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "没有待生成采购任务的订单项",
            None,
        )?;
        conn.execute(
            "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
            params![now_shanghai(), task_id],
        )?;
        return Ok(PurchaseTaskBatchResult {
            task_id,
            processed_items: 0,
            created_tasks: 0,
            skipped_items: 0,
        });
    }

    let mut created_tasks = 0i64;
    let mut skipped_items = 0i64;
    for candidate in &candidates {
        let purchase_id = format!("purchase-{}", Uuid::new_v4());
        let now = now_shanghai();
        let has_external_mapping = candidate
            .out_product_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
            && candidate
                .out_sku_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
        let status = if has_external_mapping {
            "pending_purchase"
        } else {
            "needs_mapping"
        };
        let error_summary = if status == "needs_mapping" {
            Some("缺少外部商品映射，需要人工补齐 out_product_id/out_sku_id")
        } else {
            None
        };
        if status == "needs_mapping" {
            skipped_items += 1;
        } else {
            created_tasks += 1;
        }
        conn.execute(
            "INSERT INTO purchase_tasks
             (id, order_id, order_item_id, shop_id, status, external_product_id, external_sku_id, source_url, quantity, error_summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
             ON CONFLICT(order_item_id) DO UPDATE SET
               status = excluded.status,
               external_product_id = excluded.external_product_id,
               external_sku_id = excluded.external_sku_id,
               source_url = COALESCE(excluded.source_url, purchase_tasks.source_url),
               quantity = excluded.quantity,
               error_summary = excluded.error_summary,
               updated_at = excluded.updated_at",
            params![
                purchase_id,
                candidate.order_id,
                candidate.order_item_id,
                candidate.shop_id,
                status,
                candidate.out_product_id,
                candidate.out_sku_id,
                candidate.source_url,
                candidate.quantity,
                error_summary,
                now
            ],
        )?;
        if status == "needs_mapping" {
            upsert_notification(
                &conn,
                "warning",
                "purchase_mapping",
                &candidate.order_item_id,
                Some(&candidate.shop_id),
                "采购任务缺少外部商品映射",
                &format!(
                    "订单 {} 的订单项缺少外部商品 ID 或外部 SKU，需人工补齐映射后再继续采购。",
                    candidate.order_id
                ),
                Some(&serde_json::json!({
                    "order_id": &candidate.order_id,
                    "order_item_id": &candidate.order_item_id,
                    "shop_id": &candidate.shop_id,
                    "has_source_url": candidate.source_url.is_some()
                })),
            )?;
        }
        conn.execute(
            "UPDATE orders SET status = CASE WHEN ?1 = 'pending_purchase' THEN 'pending_purchase' ELSE status END, updated_at = ?2 WHERE id = ?3",
            params![status, now, candidate.order_id],
        )?;
    }

    insert_task_log(
        &conn,
        &task_id,
        None,
        "info",
        &format!(
            "采购任务生成完成：新建/更新 {} 个，待映射 {} 个",
            created_tasks, skipped_items
        ),
        None,
    )?;
    conn.execute(
        "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
        params![now_shanghai(), task_id],
    )?;

    Ok(PurchaseTaskBatchResult {
        task_id,
        processed_items: candidates.len() as i64,
        created_tasks,
        skipped_items,
    })
}

#[tauri::command]
pub fn list_purchase_tasks(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<PurchaseTaskListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let total = count_purchase_tasks(&conn, status.as_deref())?;
    let items = load_purchase_task_views(&conn, status.as_deref(), limit)?;
    Ok(PurchaseTaskListResult { items, total })
}

#[tauri::command]
pub fn resolve_purchase_task_mapping(
    app: AppHandle,
    request: PurchaseTaskMappingRequest,
) -> AppResult<PurchaseTaskMappingResult> {
    let purchase_task_id = request.purchase_task_id.trim();
    let external_product_id = request.external_product_id.trim();
    let external_sku_id = request.external_sku_id.trim();
    if purchase_task_id.is_empty() {
        return Err(AppError::Validation("采购任务 ID 不能为空".to_string()));
    }
    if external_product_id.is_empty() {
        return Err(AppError::Validation("外部商品 ID 不能为空".to_string()));
    }
    if external_sku_id.is_empty() {
        return Err(AppError::Validation("外部 SKU 不能为空".to_string()));
    }
    if external_product_id.len() > 200 || external_sku_id.len() > 200 {
        return Err(AppError::Validation(
            "外部商品 ID 和外部 SKU 不能超过 200 个字符".to_string(),
        ));
    }
    let supplier_name = normalize_optional_note(request.supplier_name, 200, "供应商名称")?;
    let supplier_product_id =
        normalize_optional_note(request.supplier_product_id, 200, "供应商商品 ID")?;
    let source_url = normalize_optional_note(request.source_url, 1000, "货源链接")?;
    if let Some(value) = source_url.as_deref() {
        let parsed = Url::parse(value)
            .map_err(|_| AppError::Validation("货源链接必须是有效 URL".to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::Validation(
                "货源链接只支持 http 或 https".to_string(),
            ));
        }
        let has_private_param = parsed.query_pairs().any(|(key, _)| {
            let key = key.as_ref().to_ascii_lowercase();
            matches!(
                key.as_str(),
                "access_token" | "token" | "api_key" | "apikey" | "app_secret" | "password"
            ) || key.contains("cookie")
                || key.contains("session")
        });
        if has_private_param {
            return Err(AppError::Validation(
                "货源链接不能包含 token、Cookie、session 或密钥类私密参数".to_string(),
            ));
        }
    }
    let note = normalize_optional_note(request.note, 1000, "映射备注")?;
    if let Some(cost) = request.estimated_cost {
        if cost < 0.0 {
            return Err(AppError::Validation("采购成本不能为负数".to_string()));
        }
    }

    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    let purchase = tx
        .query_row(
            "SELECT pt.id, pt.order_id, pt.order_item_id, pt.shop_id, pt.status, COALESCE(o.wechat_order_id, '')
             FROM purchase_tasks pt
             JOIN orders o ON o.id = pt.order_id
             WHERE pt.id = ?1
             LIMIT 1",
            [purchase_task_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("采购任务不存在".to_string()))?;
    let (
        resolved_purchase_task_id,
        order_id,
        order_item_id,
        shop_id,
        current_status,
        wechat_order_id,
    ) = purchase;
    if !matches!(
        current_status.as_str(),
        "needs_mapping" | "pending_purchase"
    ) {
        return Err(AppError::Validation(
            "只有待映射或待采购的采购任务允许补齐商品映射".to_string(),
        ));
    }

    let now = now_shanghai();
    tx.execute(
        "UPDATE order_items
         SET out_product_id = ?1,
             out_sku_id = ?2,
             updated_at = ?3
         WHERE id = ?4",
        params![external_product_id, external_sku_id, now, order_item_id],
    )?;
    tx.execute(
        "UPDATE purchase_tasks
         SET status = 'pending_purchase',
             external_product_id = ?1,
             external_sku_id = ?2,
             supplier_name = COALESCE(?3, supplier_name),
             supplier_product_id = COALESCE(?4, supplier_product_id),
             estimated_cost = COALESCE(?5, estimated_cost),
             source_url = COALESCE(?6, source_url),
             error_summary = NULL,
             updated_at = ?7
         WHERE id = ?8",
        params![
            external_product_id,
            external_sku_id,
            supplier_name.as_deref(),
            supplier_product_id.as_deref(),
            request.estimated_cost,
            source_url.as_deref(),
            now,
            resolved_purchase_task_id
        ],
    )?;
    tx.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'pending_purchase'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now, order_id],
    )?;
    tx.execute(
        "UPDATE notifications
         SET status = 'read',
             read_at = COALESCE(read_at, ?1),
             updated_at = ?1
         WHERE source_type = 'purchase_mapping'
           AND source_id IN (?2, ?3)
           AND status = 'unread'",
        params![now, order_item_id, resolved_purchase_task_id],
    )?;
    upsert_notification(
        &tx,
        "info",
        "purchase_mapping_resolved",
        &resolved_purchase_task_id,
        Some(&shop_id),
        "采购任务映射已补齐",
        &format!(
            "订单 {} 的采购任务已补齐外部商品映射，重新进入待采购。",
            wechat_order_id
        ),
        Some(&serde_json::json!({
            "purchase_task_id": &resolved_purchase_task_id,
            "order_id": &order_id,
            "wechat_order_id": &wechat_order_id,
            "has_supplier_name": supplier_name.is_some(),
            "has_supplier_product_id": supplier_product_id.is_some(),
            "has_source_url": source_url.is_some(),
            "has_estimated_cost": request.estimated_cost.is_some(),
            "has_note": note.is_some()
        })),
    )?;
    tx.commit()?;

    Ok(PurchaseTaskMappingResult {
        purchase_task_id: resolved_purchase_task_id,
        order_id,
        status: "pending_purchase".to_string(),
        external_product_id: external_product_id.to_string(),
        external_sku_id: external_sku_id.to_string(),
        message: "采购任务映射已补齐，已重新进入待采购".to_string(),
    })
}

#[tauri::command]
pub fn mark_purchase_task_issue(
    app: AppHandle,
    request: PurchaseTaskIssueRequest,
) -> AppResult<PurchaseTaskIssueResult> {
    let purchase_task_id = request.purchase_task_id.trim();
    if purchase_task_id.is_empty() {
        return Err(AppError::Validation("采购任务 ID 不能为空".to_string()));
    }
    let (status, label) = normalize_purchase_issue_type(&request.issue_type)?;
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if note.as_deref().map(str::len).unwrap_or_default() > 1000 {
        return Err(AppError::Validation(
            "采购异常备注不能超过 1000 个字符".to_string(),
        ));
    }

    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    let purchase = tx
        .query_row(
            "SELECT pt.id, pt.order_id, pt.shop_id, pt.status, o.wechat_order_id
             FROM purchase_tasks pt
             JOIN orders o ON o.id = pt.order_id
             WHERE pt.id = ?1
             LIMIT 1",
            [purchase_task_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("采购任务不存在".to_string()))?;
    let (resolved_purchase_task_id, order_id, shop_id, current_status, wechat_order_id) = purchase;
    if matches!(
        current_status.as_str(),
        "supplier_shipped" | "wechat_shipped" | "completed"
    ) {
        return Err(AppError::Validation(
            "已发货或已完成的采购任务不能标记供应商异常".to_string(),
        ));
    }

    let summary = note
        .as_deref()
        .map(|value| format!("{label}：{value}"))
        .unwrap_or_else(|| label.to_string());
    let now = now_shanghai();
    tx.execute(
        "UPDATE purchase_tasks
         SET status = ?1,
             error_summary = ?2,
             updated_at = ?3
         WHERE id = ?4",
        params![status, summary, now, resolved_purchase_task_id],
    )?;
    tx.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'exception'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now, order_id],
    )?;
    upsert_notification(
        &tx,
        "warning",
        "purchase_issue",
        &resolved_purchase_task_id,
        Some(&shop_id),
        "采购任务供应商异常",
        &format!(
            "订单 {} 的采购任务已标记为{}，需人工处理；系统不会自动换供应商或取消订单。",
            wechat_order_id, label
        ),
        Some(&serde_json::json!({
            "purchase_task_id": &resolved_purchase_task_id,
            "order_id": &order_id,
            "wechat_order_id": &wechat_order_id,
            "issue_type": request.issue_type,
            "status": status
        })),
    )?;
    tx.commit()?;

    Ok(PurchaseTaskIssueResult {
        purchase_task_id: resolved_purchase_task_id,
        order_id,
        status: status.to_string(),
        message: format!("{label}已记录，订单已进入异常待处理"),
    })
}

#[tauri::command]
pub fn export_purchase_tasks(
    app: AppHandle,
    status: Option<String>,
) -> AppResult<PurchaseTaskExportResult> {
    let conn = open_connection(&app)?;
    let status = normalize_optional_filter(status);
    let rows = load_purchase_task_views(&conn, status.as_deref(), 10_000)?;
    let export_dir = database_path(&app)?
        .parent()
        .ok_or_else(|| AppError::Validation("无法定位应用数据目录".to_string()))?
        .join("exports");
    fs::create_dir_all(&export_dir)?;
    let file_path = export_dir.join(format!(
        "purchase-tasks-{}.csv",
        now_shanghai()
            .replace(':', "")
            .replace('+', "")
            .replace('-', "")
    ));
    let mut csv = String::from("\u{feff}");
    csv.push_str("采购任务ID,状态,店铺,微信订单号,外部商品ID,货源链接,外部SKU,商品标题,数量,成交金额(分),预估成本,预估毛利,供应商,供应商商品ID,供应商物流公司,供应商物流单号,供应商发货时间,失败原因,创建时间,更新时间\n");
    for row in &rows {
        let fields = [
            row.id.clone(),
            row.status.clone(),
            row.shop_name.clone(),
            row.wechat_order_id.clone(),
            row.external_product_id.clone().unwrap_or_default(),
            row.source_url.clone().unwrap_or_default(),
            row.external_sku_id.clone().unwrap_or_default(),
            row.title.clone().unwrap_or_default(),
            row.quantity.to_string(),
            row.estimated_revenue
                .map(|value| value.to_string())
                .unwrap_or_default(),
            row.estimated_cost
                .map(|value| format!("{value:.2}"))
                .unwrap_or_default(),
            row.estimated_profit
                .map(|value| format!("{value:.2}"))
                .unwrap_or_default(),
            row.supplier_name.clone().unwrap_or_default(),
            row.supplier_product_id.clone().unwrap_or_default(),
            row.supplier_delivery_name
                .clone()
                .or_else(|| row.supplier_delivery_id.clone())
                .unwrap_or_default(),
            row.supplier_waybill_id.clone().unwrap_or_default(),
            row.supplier_shipped_at.clone().unwrap_or_default(),
            row.error_summary.clone().unwrap_or_default(),
            row.created_at.clone(),
            row.updated_at.clone(),
        ];
        csv.push_str(
            &fields
                .iter()
                .map(|value| csv_escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }
    fs::write(&file_path, csv)?;

    Ok(PurchaseTaskExportResult {
        file_path: file_path.display().to_string(),
        exported_count: rows.len() as i64,
    })
}

#[tauri::command]
pub fn export_supplier_agent_tasks(
    app: AppHandle,
    status: Option<String>,
    format: Option<String>,
) -> AppResult<SupplierAgentExportResult> {
    let format = normalize_supplier_agent_export_format(format)?;
    let conn = open_connection(&app)?;
    let status = normalize_optional_filter(status);
    let rows = load_purchase_task_views(&conn, status.as_deref(), 10_000)?;
    let records = rows
        .iter()
        .map(supplier_agent_task_json)
        .collect::<Vec<_>>();
    let export_dir = database_path(&app)?
        .parent()
        .ok_or_else(|| AppError::Validation("无法定位应用数据目录".to_string()))?
        .join("exports");
    fs::create_dir_all(&export_dir)?;
    let suffix = if format == "md" { "md" } else { &format };
    let file_path = export_dir.join(format!(
        "supplier-agent-purchase-tasks-{}.{}",
        now_shanghai()
            .replace(':', "")
            .replace('+', "")
            .replace('-', ""),
        suffix
    ));
    let content = match format.as_str() {
        "jsonl" => {
            records
                .iter()
                .map(|record| {
                    serde_json::to_string(record)
                        .map_err(|error| AppError::Validation(format!("JSON 序列化失败：{error}")))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("\n")
                + "\n"
        }
        "json" => {
            serde_json::to_string_pretty(&records)
                .map_err(|error| AppError::Validation(format!("JSON 序列化失败：{error}")))?
                + "\n"
        }
        "md" => render_supplier_agent_markdown(&records),
        _ => unreachable!("format normalized"),
    };
    fs::write(&file_path, content)?;

    Ok(SupplierAgentExportResult {
        file_path: file_path.display().to_string(),
        exported_count: rows.len() as i64,
        format,
        sensitive_fields: "excluded".to_string(),
    })
}

#[tauri::command]
pub fn get_supplier_agent_result_template() -> String {
    [
        serde_json::json!({
            "purchase_task_id": "purchase-task-id",
            "action": "shipment",
            "delivery_id": "SF",
            "delivery_name": "顺丰速运",
            "waybill_id": "SF1234567890",
            "deliver_type": 1,
            "estimated_cost": 18.8
        }),
        serde_json::json!({
            "purchase_task_id": "purchase-task-id",
            "action": "issue",
            "issue_type": "out_of_stock",
            "note": "supplier reported no stock"
        }),
        serde_json::json!({
            "purchase_task_id": "purchase-task-id",
            "action": "mapping",
            "external_product_id": "external-product-id",
            "external_sku_id": "external-sku-id",
            "source_url": "https://example.com/source-product",
            "supplier_name": "supplier",
            "supplier_product_id": "supplier-product-id",
            "estimated_cost": 18.8,
            "note": "operator confirmed mapping"
        }),
    ]
    .iter()
    .map(serde_json::to_string)
    .collect::<Result<Vec<_>, _>>()
    .unwrap_or_default()
    .join("\n")
        + "\n"
}

#[tauri::command]
pub fn apply_supplier_agent_results(
    app: AppHandle,
    request: SupplierAgentApplyRequest,
) -> AppResult<SupplierAgentApplyResult> {
    let records = parse_supplier_agent_result_records(&request.raw_results)?;
    let mut results = Vec::new();

    for (index, record) in records.iter().enumerate() {
        let item_index = index as i64 + 1;
        let result = match build_supplier_agent_action(record) {
            Ok(action) => {
                if request.dry_run {
                    SupplierAgentApplyItemResult {
                        index: item_index,
                        purchase_task_id: Some(action.purchase_task_id),
                        action: Some(action.action),
                        status: "dry_run".to_string(),
                        error: None,
                        response: None,
                    }
                } else {
                    apply_supplier_agent_action(&app, action, item_index)
                }
            }
            Err(error) => SupplierAgentApplyItemResult {
                index: item_index,
                purchase_task_id: supplier_agent_value_string(record.get("purchase_task_id")),
                action: supplier_agent_value_string(record.get("action")),
                status: "failed".to_string(),
                error: Some(error.to_string()),
                response: None,
            },
        };
        let failed = result.status == "failed";
        results.push(result);
        if failed && !request.continue_on_error {
            break;
        }
    }

    let succeeded = results
        .iter()
        .filter(|item| item.status == "success" || item.status == "dry_run")
        .count() as i64;
    let failed = results
        .iter()
        .filter(|item| item.status == "failed")
        .count() as i64;
    Ok(SupplierAgentApplyResult {
        dry_run: request.dry_run,
        processed: results.len() as i64,
        succeeded,
        failed,
        results,
    })
}

#[tauri::command]
pub fn record_purchase_task_shipment(
    app: AppHandle,
    request: PurchaseTaskShipmentRequest,
) -> AppResult<PurchaseTaskShipmentResult> {
    let purchase_task_id = request.purchase_task_id.trim();
    if purchase_task_id.is_empty() {
        return Err(AppError::Validation("采购任务 ID 不能为空".to_string()));
    }
    if let Some(cost) = request.estimated_cost {
        if cost < 0.0 {
            return Err(AppError::Validation("采购成本不能为负数".to_string()));
        }
    }

    let mut conn = open_connection(&app)?;
    let purchase = load_purchase_task_shipment_ref(&conn, purchase_task_id)?;
    let fields = normalize_shipment_fields(
        request.deliver_type,
        request.delivery_id.as_deref(),
        request.delivery_name.as_deref(),
        request.waybill_id.as_deref(),
    )?;
    let now = now_shanghai();
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE purchase_tasks
         SET status = 'supplier_shipped',
             supplier_delivery_id = ?1,
             supplier_delivery_name = ?2,
             supplier_waybill_id = ?3,
             supplier_deliver_type = ?4,
             estimated_cost = COALESCE(?5, estimated_cost),
             error_summary = NULL,
             supplier_shipped_at = ?6,
             updated_at = ?6
         WHERE id = ?7",
        params![
            fields.delivery_id.as_deref(),
            fields.delivery_name.as_deref(),
            fields.waybill_id.as_deref(),
            fields.deliver_type,
            request.estimated_cost,
            now,
            purchase.purchase_task_id
        ],
    )?;
    tx.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'supplier_shipped'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), purchase.order.order_id],
    )?;

    let auto_send_enabled = get_bool_setting(&tx, AUTO_SEND_DELIVERY_SETTING, false)?;
    let pending_count: i64 = tx.query_row(
        "SELECT COUNT(*)
         FROM purchase_tasks
         WHERE order_id = ?1
           AND status NOT IN ('supplier_shipped', 'wechat_shipped', 'completed')",
        [purchase.order.order_id.as_str()],
        |row| row.get(0),
    )?;
    if pending_count > 0 {
        tx.commit()?;
        return Ok(PurchaseTaskShipmentResult {
            purchase_task_id: purchase.purchase_task_id,
            order_id: purchase.order.order_id,
            purchase_status: "supplier_shipped".to_string(),
            shipment_id: None,
            shipment_status: None,
            auto_send_enabled,
            message: format!(
                "采购物流已回填；同一订单还有 {pending_count} 个采购任务未回填，暂不进入微信发货队列"
            ),
        });
    }

    let distinct_logistics =
        count_distinct_order_purchase_logistics(&tx, &purchase.order.order_id)?;
    if distinct_logistics > 1 {
        upsert_notification(
            &tx,
            "warning",
            "delivery_order",
            &purchase.order.order_id,
            Some(&purchase.order.shop_id),
            "同一订单存在多个供应商物流",
            &format!(
                "订单 {} 已回填多个供应商物流，需到履约发货页人工确认整单发货。",
                purchase.order.wechat_order_id
            ),
            Some(&serde_json::json!({
                "order_id": &purchase.order.order_id,
                "wechat_order_id": &purchase.order.wechat_order_id,
                "distinct_logistics": distinct_logistics
            })),
        )?;
        tx.commit()?;
        return Ok(PurchaseTaskShipmentResult {
            purchase_task_id: purchase.purchase_task_id,
            order_id: purchase.order.order_id,
            purchase_status: "supplier_shipped".to_string(),
            shipment_id: None,
            shipment_status: None,
            auto_send_enabled,
            message:
                "采购物流已回填；同一订单存在多个供应商物流，当前需到履约发货页人工确认整单发货"
                    .to_string(),
        });
    }

    let shipment = upsert_order_shipment(&tx, &purchase.order, &fields)?;
    tx.commit()?;
    Ok(PurchaseTaskShipmentResult {
        purchase_task_id: purchase.purchase_task_id,
        order_id: purchase.order.order_id,
        purchase_status: "supplier_shipped".to_string(),
        shipment_id: Some(shipment.shipment_id),
        shipment_status: Some(shipment.status),
        auto_send_enabled,
        message: shipment.message,
    })
}
