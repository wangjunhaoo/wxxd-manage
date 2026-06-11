use super::*;

#[tauri::command]
pub fn get_delivery_settings(app: AppHandle) -> AppResult<DeliverySettings> {
    let conn = open_connection(&app)?;
    Ok(DeliverySettings {
        auto_send_delivery: get_bool_setting(&conn, AUTO_SEND_DELIVERY_SETTING, false)?,
        multi_package_enabled: get_bool_setting(&conn, MULTI_PACKAGE_SETTING, false)?,
    })
}

#[tauri::command]
pub fn set_multi_package_enabled(app: AppHandle, enabled: bool) -> AppResult<DeliverySettings> {
    let conn = open_connection(&app)?;
    set_bool_setting(&conn, MULTI_PACKAGE_SETTING, enabled)?;
    Ok(DeliverySettings {
        auto_send_delivery: get_bool_setting(&conn, AUTO_SEND_DELIVERY_SETTING, false)?,
        multi_package_enabled: enabled,
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
        multi_package_enabled: get_bool_setting(&conn, MULTI_PACKAGE_SETTING, false)?,
    })
}

#[tauri::command]
pub fn record_order_shipment(
    app: AppHandle,
    request: ShipmentRecordRequest,
) -> AppResult<ShipmentRecordResult> {
    let conn = open_connection(&app)?;
    let order = resolve_shipment_order(&conn, &request)?;
    // 审查修复：与采购侧回填同款守卫——已提交/已发货订单拒绝重复回填
    let order_status: String = conn.query_row(
        "SELECT status FROM orders WHERE id = ?1",
        [order.order_id.as_str()],
        |row| row.get(0),
    )?;
    match order_status.as_str() {
        "shipping_submitted" | "wechat_shipped" | "completed" => {
            return Err(AppError::Validation(
                "订单已提交微信发货，不能重复回填物流；修改运单请使用「改运单」，漏发补寄请使用「补发」".to_string(),
            ));
        }
        "cancelled" => {
            return Err(AppError::Validation("订单已取消，无需回填物流".to_string()));
        }
        _ => {}
    }
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
    let (current_status, order_status) = conn
        .query_row(
            "SELECT sh.status, o.status
             FROM shipments sh JOIN orders o ON o.id = sh.order_id
             WHERE sh.id = ?1",
            [shipment_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("物流单不存在".to_string()))?;
    if current_status == "wechat_shipped" {
        return Err(AppError::Validation(
            "已发货成功的物流单不能重试".to_string(),
        ));
    }
    // 审查修复：官方已发货/已完成的订单不得把物流单重新入队（重复 send 必被拒）
    if matches!(order_status.as_str(), "wechat_shipped" | "completed" | "cancelled") {
        return Err(AppError::Validation(format!(
            "订单当前状态为 {order_status}，微信侧已是终局，不能重新提交发货"
        )));
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
             blocked_reason = NULL,
             updated_at = ?2
         WHERE id = ?3",
        params![next_status, now, shipment_id],
    )?;
    // shipping_submitted（同单其它包裹已提交）不回退，避免双轴降级
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped', 'shipping_submitted') THEN status
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
                blocked_shipments: 0,
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
            blocked_shipments: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_shipments = shipments.len() as i64;
    let mut submitted_shipments = 0i64;
    let mut failed_shipments = 0i64;
    let mut blocked_shipments = 0i64;

    // 三期 §8：按订单聚合候选，多包裹合成一次 send 的多元素 delivery_list（官方拆单发货语义）
    let mut groups: Vec<Vec<ShipmentCandidate>> = Vec::new();
    {
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        for shipment in shipments {
            match index.get(&shipment.order_id) {
                Some(&position) => groups[position].push(shipment),
                None => {
                    index.insert(shipment.order_id.clone(), groups.len());
                    groups.push(vec![shipment]);
                }
            }
        }
    }

    for group in groups {
        let first = &group[0];
        {
            let conn = open_connection(&app)?;
            // 审查修复：官方侧已发货/已完成（外部工具或商家后台先行发货，详情镜像已确认）——
            // 不再盲调 send，把组内未提交行直接对齐为已发货
            let order_status: String = conn.query_row(
                "SELECT status FROM orders WHERE id = ?1",
                [first.order_id.as_str()],
                |row| row.get(0),
            )?;
            if matches!(order_status.as_str(), "wechat_shipped" | "completed") {
                for shipment in &group {
                    conn.execute(
                        "UPDATE shipments
                         SET status = 'wechat_shipped',
                             error_code = NULL,
                             error_summary = NULL,
                             blocked_reason = NULL,
                             submitted_at = COALESCE(submitted_at, ?1),
                             updated_at = ?1
                         WHERE id = ?2 AND status != 'wechat_shipped'",
                        params![now_shanghai(), shipment.shipment_id],
                    )?;
                }
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&first.shipment_id),
                    "info",
                    &format!(
                        "订单 {} 微信侧已发货（外部发货），物流单已对齐为已发货，未重复提交",
                        first.wechat_order_id
                    ),
                    None,
                )?;
                continue;
            }
            // 同订单还有组外未就绪的兄弟物流单（待确认/发货失败）：本轮整组跳过——
            // 部分提交后官方对剩余包裹可能永久拒收（设计 §13 V2 未经真机验证前不冒险）
            let unready_siblings: i64 = conn.query_row(
                "SELECT COUNT(*) FROM shipments
                 WHERE order_id = ?1 AND status IN ('waiting_confirmation', 'send_failed')",
                [first.order_id.as_str()],
                |row| row.get(0),
            )?;
            if unready_siblings > 0 {
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&first.shipment_id),
                    "info",
                    &format!(
                        "订单 {} 还有 {unready_siblings} 个待确认/发货失败的兄弟物流单，本轮整组暂不提交（失败的请全部重试后一并提交）",
                        first.wechat_order_id
                    ),
                    None,
                )?;
                continue;
            }

            // 发货前置守卫（订单级）：改址/换SKU/售后在途 → 整组拦截进 blocked（条件解除自动恢复）
            match check_shipment_send_guard(&conn, first)? {
                Some(reason) => {
                    for shipment in &group {
                        blocked_shipments += 1;
                        let was_blocked = shipment.status == "blocked";
                        mark_shipment_blocked(&conn, shipment, &reason, was_blocked)?;
                        if !was_blocked {
                            insert_task_log(
                                &conn,
                                &task_id,
                                Some(&shipment.shipment_id),
                                "warning",
                                &format!(
                                    "订单 {} 发货被守卫拦截：{reason}",
                                    shipment.wechat_order_id
                                ),
                                None,
                            )?;
                        }
                    }
                    continue;
                }
                None => {
                    for shipment in &group {
                        if shipment.status == "blocked" {
                            // 拦截条件已解除：恢复为待提交并继续走本轮提交
                            conn.execute(
                                "UPDATE shipments
                                 SET status = 'ready_to_send', blocked_reason = NULL, updated_at = ?1
                                 WHERE id = ?2",
                                params![now_shanghai(), shipment.shipment_id],
                            )?;
                            insert_task_log(
                                &conn,
                                &task_id,
                                Some(&shipment.shipment_id),
                                "info",
                                &format!(
                                    "订单 {} 守卫拦截条件已解除，恢复提交",
                                    shipment.wechat_order_id
                                ),
                                None,
                            )?;
                        }
                    }
                }
            }
        }

        // 组级失败统一落库：组内每个物流单都标失败（一次 send 整组成败一体）
        let mark_group_failed =
            |conn: &Connection, code: &str, summary: &str| -> AppResult<()> {
                for shipment in &group {
                    mark_shipment_failed(conn, shipment, code, summary)?;
                }
                Ok(())
            };

        let payload = {
            let conn = open_connection(&app)?;
            match build_group_send_delivery_payload(&conn, &group) {
                Ok(payload) => payload,
                Err(error) => {
                    failed_shipments += group.len() as i64;
                    mark_group_failed(&conn, "DELIVERY_PAYLOAD_INVALID", &error.to_string())?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&first.shipment_id),
                        "error",
                        &format!("订单 {} 发货参数无效：{error}", first.wechat_order_id),
                        None,
                    )?;
                    continue;
                }
            }
        };

        // 审查修复（发货幂等护栏）：send 在途期间把整组置 submitting，候选查询不再捞取，
        // 防止僵尸 tick 与新 tick 并发时对同一订单重复 senddelivery；
        // 若本 tick 中途死亡，候选查询会在 10 分钟后把 submitting 行回收重查守卫
        {
            let conn = open_connection(&app)?;
            for shipment in &group {
                conn.execute(
                    "UPDATE shipments SET status = 'submitting', updated_at = ?1 WHERE id = ?2",
                    params![now_shanghai(), shipment.shipment_id],
                )?;
            }
        }

        let access_token = match ensure_access_token(&app, &first.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_shipments += group.len() as i64;
                let conn = open_connection(&app)?;
                mark_group_failed(
                    &conn,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&first.shipment_id),
                    "error",
                    &format!(
                        "订单 {} 获取 access_token 失败：{error}",
                        first.wechat_order_id
                    ),
                    None,
                )?;
                continue;
            }
        };

        let call = match client.send_delivery(&access_token, &payload).await {
            Ok(call) => call,
            Err(error) => {
                failed_shipments += group.len() as i64;
                let conn = open_connection(&app)?;
                mark_group_failed(
                    &conn,
                    "SEND_DELIVERY_REQUEST_FAILED",
                    &format!("微信发货请求失败：{error}"),
                )?;
                insert_task_log(
                    &conn,
                    &task_id,
                    Some(&first.shipment_id),
                    "error",
                    &format!(
                        "订单 {} 微信发货请求失败：{error}",
                        first.wechat_order_id
                    ),
                    None,
                )?;
                continue;
            }
        };

        let group_submitted = {
            let conn = open_connection(&app)?;
            match &call.result {
                WechatCallResult::Success(result) => {
                    insert_api_call_log(
                        &conn,
                        Some(&first.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "success",
                        None,
                        None,
                        Some("senddelivery ok"),
                    )?;
                    for shipment in &group {
                        mark_shipment_submitted(&conn, shipment, &payload, &result.raw_payload)?;
                    }
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&first.shipment_id),
                        "info",
                        &format!(
                            "订单 {} 已提交微信发货（{} 个包裹）",
                            first.wechat_order_id,
                            group.len()
                        ),
                        None,
                    )?;
                    submitted_shipments += group.len() as i64;
                    true
                }
                WechatCallResult::ApiError(error) => {
                    insert_api_call_log(
                        &conn,
                        Some(&first.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some("senddelivery api error"),
                    )?;
                    mark_group_failed(&conn, &error.errcode.to_string(), &error.errmsg)?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&first.shipment_id),
                        "error",
                        &format!(
                            "订单 {} 微信发货失败：{}",
                            first.wechat_order_id, error.errmsg
                        ),
                        Some(&serde_json::json!({
                            "errcode": error.errcode
                        })),
                    )?;
                    failed_shipments += group.len() as i64;
                    false
                }
            }
        };

        // 发货成功后把采购货源+运单摘要镜像进微信商家备注（best-effort，失败只记日志）
        if group_submitted {
            let notes = {
                let conn = open_connection(&app)?;
                build_merchant_notes_summary(&conn, &first.order_id, &group)?
            };
            match client
                .update_merchant_notes(&access_token, &first.wechat_order_id, &notes)
                .await
            {
                Ok(call) => {
                    if let WechatCallResult::ApiError(error) = &call.result {
                        let conn = open_connection(&app)?;
                        insert_task_log(
                            &conn,
                            &task_id,
                            Some(&first.shipment_id),
                            "info",
                            &format!(
                                "订单 {} 商家备注镜像未成功（不影响发货）：{}",
                                first.wechat_order_id, error.errmsg
                            ),
                            None,
                        )?;
                    }
                }
                Err(error) => {
                    let conn = open_connection(&app)?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&first.shipment_id),
                        "info",
                        &format!(
                            "订单 {} 商家备注镜像请求失败（不影响发货）：{error}",
                            first.wechat_order_id
                        ),
                        None,
                    )?;
                }
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
        blocked_shipments,
    })
}
