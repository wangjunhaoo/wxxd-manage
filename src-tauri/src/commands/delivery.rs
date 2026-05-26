use super::*;

#[tauri::command]
pub fn get_delivery_settings(app: AppHandle) -> AppResult<DeliverySettings> {
    let conn = open_connection(&app)?;
    Ok(DeliverySettings {
        auto_send_delivery: get_bool_setting(&conn, AUTO_SEND_DELIVERY_SETTING, false)?,
    })
}

#[tauri::command]
pub fn list_delivery_companies(
    app: AppHandle,
    shop_id: Option<String>,
) -> AppResult<Vec<DeliveryCompanyView>> {
    let conn = open_connection(&app)?;
    let shop_id = normalize_optional_filter(shop_id);
    load_delivery_company_views(&conn, shop_id.as_deref())
}

#[tauri::command]
pub async fn sync_delivery_companies(
    app: AppHandle,
    shop_id: String,
    ewaybill_only: Option<bool>,
) -> AppResult<DeliveryCompanySyncResult> {
    let shop_id = shop_id.trim().to_string();
    if shop_id.is_empty() {
        return Err(AppError::Validation("店铺 ID 不能为空".to_string()));
    }
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let task_id = format!("delivery-company-sync-{}", Uuid::new_v4());
    let started_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'delivery.sync_company_list', 'running', 0, ?2, ?2)",
            params![task_id, started_at],
        )?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "开始同步微信快递公司列表",
            Some(&serde_json::json!({
                "shop_id": &shop_id,
                "ewaybill_only": ewaybill_only.unwrap_or(false)
            })),
        )?;
    }

    let mut result = DeliveryCompanySyncResult {
        task_id: task_id.clone(),
        shop_id: shop_id.clone(),
        synced_companies: 0,
        failed_steps: Vec::new(),
    };
    let call = client
        .get_delivery_company_list(&access_token, ewaybill_only.unwrap_or(false))
        .await?;
    match &call.result {
        WechatCallResult::Success(raw) => {
            let mut conn = open_connection(&app)?;
            result.synced_companies =
                upsert_delivery_companies(&mut conn, &shop_id, &raw.raw_payload)?;
            insert_success_api_and_task_log(
                &conn,
                &task_id,
                &shop_id,
                call.meta.endpoint,
                call.meta.method,
                &format!("快递公司列表同步完成：{} 家", result.synced_companies),
            )?;
        }
        WechatCallResult::ApiError(error) => {
            result
                .failed_steps
                .push(format!("快递公司列表同步失败：{}", error.errmsg));
            insert_api_error_and_task_log(&app, &task_id, &shop_id, &call.meta, error)?;
        }
    }
    let final_status = if result.failed_steps.is_empty() {
        "success"
    } else {
        "failed"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;
    Ok(result)
}

#[tauri::command]
pub fn set_auto_send_delivery(app: AppHandle, enabled: bool) -> AppResult<DeliverySettings> {
    let conn = open_connection(&app)?;
    set_bool_setting(&conn, AUTO_SEND_DELIVERY_SETTING, enabled)?;
    if enabled {
        conn.execute(
            "UPDATE shipments
             SET status = 'ready_to_send', updated_at = ?1
             WHERE status = 'waiting_confirmation'",
            [now_shanghai()],
        )?;
    }
    Ok(DeliverySettings {
        auto_send_delivery: enabled,
    })
}

#[tauri::command]
pub fn record_order_shipment(
    app: AppHandle,
    request: ShipmentRecordRequest,
) -> AppResult<ShipmentRecordResult> {
    let conn = open_connection(&app)?;
    let order = resolve_shipment_order(&conn, &request)?;
    let fields = normalize_shipment_fields(
        request.deliver_type,
        request.delivery_id.as_deref(),
        request.delivery_name.as_deref(),
        request.waybill_id.as_deref(),
    )?;
    let shipment = upsert_order_shipment(&conn, &order, &fields)?;
    Ok(ShipmentRecordResult {
        shipment_id: shipment.shipment_id,
        order_id: order.order_id,
        status: shipment.status,
        auto_send_enabled: shipment.auto_send_enabled,
        message: shipment.message,
    })
}

#[tauri::command]
pub fn list_delivery_shipments(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<ShipmentListResult> {
    let conn = open_connection(&app)?;
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let status = normalize_optional_filter(status);
    let total = count_delivery_shipments(&conn, status.as_deref())?;
    let items = load_delivery_shipment_views(&conn, status.as_deref(), limit)?;
    Ok(ShipmentListResult { items, total })
}

#[tauri::command]
pub fn retry_delivery_shipment(
    app: AppHandle,
    shipment_id: String,
) -> AppResult<ShipmentRetryResult> {
    let shipment_id = shipment_id.trim();
    if shipment_id.is_empty() {
        return Err(AppError::Validation("物流单 ID 不能为空".to_string()));
    }

    let conn = open_connection(&app)?;
    let current_status = conn
        .query_row(
            "SELECT status FROM shipments WHERE id = ?1",
            [shipment_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("物流单不存在".to_string()))?;
    if current_status == "wechat_shipped" {
        return Err(AppError::Validation(
            "已发货成功的物流单不能重试".to_string(),
        ));
    }

    let auto_send_enabled = get_bool_setting(&conn, AUTO_SEND_DELIVERY_SETTING, false)?;
    let next_status = if auto_send_enabled {
        "ready_to_send"
    } else {
        "waiting_confirmation"
    };
    let now = now_shanghai();
    conn.execute(
        "UPDATE shipments
         SET status = ?1,
             error_code = NULL,
             error_summary = NULL,
             updated_at = ?2
         WHERE id = ?3",
        params![next_status, now, shipment_id],
    )?;
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'supplier_shipped'
         END,
         updated_at = ?1
         WHERE id = (SELECT order_id FROM shipments WHERE id = ?2)",
        params![now_shanghai(), shipment_id],
    )?;

    let message = if auto_send_enabled {
        "物流单已重新进入待提交微信发货队列".to_string()
    } else {
        "自动发货开关关闭，物流单已回到待确认状态".to_string()
    };
    Ok(ShipmentRetryResult {
        shipment_id: shipment_id.to_string(),
        status: next_status.to_string(),
        auto_send_enabled,
        message,
    })
}

#[tauri::command]
pub async fn run_delivery_submission_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<DeliverySubmitBatchResult> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let task_id = format!("delivery-submit-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'delivery.submit_wechat_shipment', 'running', 0, ?2, ?2)",
            params![task_id, created_at],
        )?;
        if !get_bool_setting(&conn, AUTO_SEND_DELIVERY_SETTING, false)? {
            insert_task_log(
                &conn,
                &task_id,
                None,
                "warning",
                "自动发货开关关闭，未提交微信发货",
                None,
            )?;
            conn.execute(
                "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
                params![now_shanghai(), task_id],
            )?;
            return Ok(DeliverySubmitBatchResult {
                task_id,
                processed_shipments: 0,
                submitted_shipments: 0,
                failed_shipments: 0,
            });
        }
    }

    let shipments = {
        let conn = open_connection(&app)?;
        load_delivery_submission_candidates(&conn, limit)?
    };
    if shipments.is_empty() {
        let conn = open_connection(&app)?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "没有待提交微信发货的物流单",
            None,
        )?;
        conn.execute(
            "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
            params![now_shanghai(), task_id],
        )?;
        return Ok(DeliverySubmitBatchResult {
            task_id,
            processed_shipments: 0,
            submitted_shipments: 0,
            failed_shipments: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_shipments = shipments.len() as i64;
    let mut submitted_shipments = 0i64;
    let mut failed_shipments = 0i64;

    for shipment in shipments {
        let payload = {
            let conn = open_connection(&app)?;
            match build_send_delivery_payload(&conn, &shipment) {
                Ok(payload) => payload,
                Err(error) => {
                    failed_shipments += 1;
                    mark_shipment_failed(
                        &conn,
                        &shipment,
                        "DELIVERY_PAYLOAD_INVALID",
                        &error.to_string(),
                    )?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shipment.shipment_id),
                        "error",
                        &format!("物流单 {} 发货参数无效：{error}", shipment.shipment_id),
                        None,
                    )?;
                    continue;
                }
            }
        };

        let access_token = match ensure_access_token(&app, &shipment.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_shipments += 1;
                let conn = open_connection(&app)?;
                mark_shipment_failed(
                    &conn,
                    &shipment,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&shipment.shipment_id),
                    "error",
                    &format!(
                        "物流单 {} 获取 access_token 失败：{error}",
                        shipment.shipment_id
                    ),
                    None,
                )?;
                continue;
            }
        };

        let call = match client.send_delivery(&access_token, &payload).await {
            Ok(call) => call,
            Err(error) => {
                failed_shipments += 1;
                let conn = open_connection(&app)?;
                mark_shipment_failed(
                    &conn,
                    &shipment,
                    "SEND_DELIVERY_REQUEST_FAILED",
                    &format!("微信发货请求失败：{error}"),
                )?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&shipment.shipment_id),
                    "error",
                    &format!("物流单 {} 微信发货请求失败：{error}", shipment.shipment_id),
                    None,
                )?;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(result) => {
                insert_api_call_log(
                    &conn,
                    Some(&shipment.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some("senddelivery ok"),
                )?;
                mark_shipment_submitted(&conn, &shipment, &payload, &result.raw_payload)?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&shipment.shipment_id),
                    "info",
                    &format!("订单 {} 已提交微信发货", shipment.wechat_order_id),
                    None,
                )?;
                submitted_shipments += 1;
            }
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&shipment.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("senddelivery api error"),
                )?;
                mark_shipment_failed(&conn, &shipment, &error.errcode.to_string(), &error.errmsg)?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&shipment.shipment_id),
                    "error",
                    &format!(
                        "订单 {} 微信发货失败：{}",
                        shipment.wechat_order_id, error.errmsg
                    ),
                    Some(&serde_json::json!({
                        "errcode": error.errcode
                    })),
                )?;
                failed_shipments += 1;
            }
        }
    }

    let final_status = if failed_shipments == 0 {
        "success"
    } else if failed_shipments == processed_shipments {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;

    Ok(DeliverySubmitBatchResult {
        task_id,
        processed_shipments,
        submitted_shipments,
        failed_shipments,
    })
}
