use super::*;

#[tauri::command]
pub fn list_aftersales(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<AftersaleListResult> {
    let status = normalize_optional_filter(status);
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let conn = open_connection(&app)?;
    Ok(AftersaleListResult {
        items: load_aftersale_views(&conn, status.as_deref(), limit)?,
        total: count_aftersales(&conn, status.as_deref())?,
    })
}

#[tauri::command]
pub fn list_aftersale_evidence(
    app: AppHandle,
    target_type: Option<String>,
    target_id: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<AftersaleEvidenceListResult> {
    let target_type = match normalize_optional_filter(target_type) {
        Some(value) => Some(normalize_aftersale_evidence_target_type(&value)?.to_string()),
        None => None,
    };
    let status = match normalize_optional_filter(status) {
        Some(value) => Some(normalize_aftersale_evidence_status(&value)?.to_string()),
        None => None,
    };
    let target_id = normalize_optional_filter(target_id);
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let conn = open_connection(&app)?;
    Ok(AftersaleEvidenceListResult {
        items: load_aftersale_evidence_views(
            &conn,
            target_type.as_deref(),
            target_id.as_deref(),
            status.as_deref(),
            limit,
        )?,
        total: count_aftersale_evidence(
            &conn,
            target_type.as_deref(),
            target_id.as_deref(),
            status.as_deref(),
        )?,
    })
}

#[tauri::command]
pub fn record_aftersale_evidence(
    app: AppHandle,
    request: AftersaleEvidenceRecordRequest,
) -> AppResult<AftersaleEvidenceRecordResult> {
    let target_type = normalize_aftersale_evidence_target_type(&request.target_type)?;
    let target_id = request.target_id.trim();
    if target_id.is_empty() {
        return Err(AppError::Validation(
            "售后/纠纷目标 ID 不能为空".to_string(),
        ));
    }
    let evidence_type = normalize_aftersale_evidence_type(&request.evidence_type)?;
    let status = match request.status.as_deref() {
        Some(value) => normalize_aftersale_evidence_status(value)?,
        None => "draft",
    };
    let title = request.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("凭证标题不能为空".to_string()));
    }
    if title.chars().count() > 100 {
        return Err(AppError::Validation(
            "凭证标题不能超过 100 个字符".to_string(),
        ));
    }
    let content_text = normalize_optional_note(request.content_text, 2000, "凭证说明")?;
    let local_file_path = normalize_optional_note(request.local_file_path, 1000, "本地文件路径")?;
    let source_url = normalize_optional_note(request.source_url, 1000, "来源链接")?;
    if let Some(source_url) = source_url.as_deref() {
        if !source_url.starts_with("http://") && !source_url.starts_with("https://") {
            return Err(AppError::Validation(
                "来源链接必须以 http:// 或 https:// 开头".to_string(),
            ));
        }
    }
    if content_text.is_none() && local_file_path.is_none() && source_url.is_none() {
        return Err(AppError::Validation(
            "凭证说明、本地文件路径和来源链接至少填写一项".to_string(),
        ));
    }

    let mut conn = open_connection(&app)?;
    let (resolved_target_id, shop_id, external_target_id) =
        resolve_aftersale_evidence_target(&conn, target_type, target_id)?;
    let tx = conn.transaction()?;
    let evidence_id = format!("evidence-{}", Uuid::new_v4());
    let now = now_shanghai();
    tx.execute(
        "INSERT INTO aftersale_evidence_records
         (id, target_type, target_id, shop_id, external_target_id, evidence_type, title,
          content_text, local_file_path, source_url, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![
            evidence_id,
            target_type,
            resolved_target_id,
            shop_id,
            external_target_id,
            evidence_type,
            title,
            content_text,
            local_file_path,
            source_url,
            status,
            now
        ],
    )?;
    upsert_notification(
        &tx,
        "info",
        "aftersale_evidence",
        &evidence_id,
        Some(&shop_id),
        if target_type == "guarantee" {
            "纠纷凭证资料已记录"
        } else {
            "售后凭证资料已记录"
        },
        &format!(
            "{} {} 已记录本地凭证资料：{}。",
            if target_type == "guarantee" {
                "纠纷单"
            } else {
                "售后单"
            },
            external_target_id,
            title
        ),
        Some(&serde_json::json!({
            "evidence_id": &evidence_id,
            "target_type": target_type,
            "target_id": &resolved_target_id,
            "external_target_id": &external_target_id,
            "evidence_type": evidence_type,
            "status": status,
            "has_content_text": content_text.is_some(),
            "has_local_file_path": local_file_path.is_some(),
            "has_source_url": source_url.is_some()
        })),
    )?;
    tx.commit()?;

    Ok(AftersaleEvidenceRecordResult {
        evidence_id,
        target_type: target_type.to_string(),
        target_id: resolved_target_id,
        evidence_type: evidence_type.to_string(),
        status: status.to_string(),
        message: "本地凭证资料已记录，尚未上传微信或提交平台处理".to_string(),
    })
}

#[tauri::command]
pub fn update_aftersale_evidence_status(
    app: AppHandle,
    request: AftersaleEvidenceStatusUpdateRequest,
) -> AppResult<AftersaleEvidenceStatusUpdateResult> {
    let evidence_id = request.evidence_id.trim();
    if evidence_id.is_empty() {
        return Err(AppError::Validation("凭证 ID 不能为空".to_string()));
    }
    let status = normalize_aftersale_evidence_status(&request.status)?;
    let status_text = aftersale_evidence_status_text(status);

    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    let (target_type, target_id, shop_id, external_target_id, title): (
        String,
        String,
        String,
        String,
        String,
    ) = tx
        .query_row(
            "SELECT target_type, target_id, shop_id, external_target_id, title
             FROM aftersale_evidence_records
             WHERE id = ?1
             LIMIT 1",
            [evidence_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("凭证资料不存在".to_string()))?;
    let now = now_shanghai();
    tx.execute(
        "UPDATE aftersale_evidence_records
         SET status = ?1, updated_at = ?2
         WHERE id = ?3",
        params![status, now, evidence_id],
    )?;
    let target_label = if target_type == "guarantee" {
        "纠纷单"
    } else {
        "售后单"
    };
    upsert_notification(
        &tx,
        "info",
        "aftersale_evidence",
        evidence_id,
        Some(&shop_id),
        "本地凭证状态已更新",
        &format!(
            "{target_label} {external_target_id} 的本地凭证「{title}」已标记为{status_text}。"
        ),
        Some(&serde_json::json!({
            "evidence_id": evidence_id,
            "target_type": &target_type,
            "target_id": &target_id,
            "external_target_id": &external_target_id,
            "status": status
        })),
    )?;
    tx.commit()?;

    Ok(AftersaleEvidenceStatusUpdateResult {
        evidence_id: evidence_id.to_string(),
        status: status.to_string(),
        status_text: status_text.to_string(),
        message: "本地凭证状态已更新，尚未上传微信或提交平台处理".to_string(),
    })
}

#[tauri::command]
pub fn export_aftersale_evidence(
    app: AppHandle,
    target_type: Option<String>,
    target_id: Option<String>,
    status: Option<String>,
    format: Option<String>,
) -> AppResult<AftersaleEvidenceExportResult> {
    let target_type = match normalize_optional_filter(target_type) {
        Some(value) => Some(normalize_aftersale_evidence_target_type(&value)?.to_string()),
        None => None,
    };
    let status = match normalize_optional_filter(status) {
        Some(value) => Some(normalize_aftersale_evidence_status(&value)?.to_string()),
        None => None,
    };
    let target_id = normalize_optional_filter(target_id);
    let format = normalize_aftersale_evidence_export_format(format)?;
    let conn = open_connection(&app)?;
    let rows = load_aftersale_evidence_views(
        &conn,
        target_type.as_deref(),
        target_id.as_deref(),
        status.as_deref(),
        10_000,
    )?;
    let export_dir = database_path(&app)?
        .parent()
        .ok_or_else(|| AppError::Validation("无法定位应用数据目录".to_string()))?
        .join("exports");
    fs::create_dir_all(&export_dir)?;
    let file_path = export_dir.join(format!(
        "aftersale-evidence-{}.{}",
        now_shanghai()
            .replace(':', "")
            .replace('+', "")
            .replace('-', ""),
        if format == "md" { "md" } else { &format }
    ));
    let records = rows
        .iter()
        .map(aftersale_evidence_export_json)
        .collect::<Vec<_>>();
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
        "md" => render_aftersale_evidence_markdown(&rows),
        _ => unreachable!("format normalized"),
    };
    fs::write(&file_path, content)?;

    Ok(AftersaleEvidenceExportResult {
        file_path: file_path.display().to_string(),
        exported_count: rows.len() as i64,
        format,
        sensitive_fields: "recipient_info_and_file_contents_excluded".to_string(),
    })
}

#[tauri::command]
pub fn list_supplier_aftersale_followups(
    app: AppHandle,
    target_type: Option<String>,
    target_id: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<SupplierAftersaleFollowupListResult> {
    let target_type = match normalize_optional_filter(target_type) {
        Some(value) => Some(normalize_aftersale_evidence_target_type(&value)?.to_string()),
        None => None,
    };
    let status = match normalize_optional_filter(status) {
        Some(value) => Some(normalize_supplier_aftersale_followup_status(&value)?.to_string()),
        None => None,
    };
    let target_id = normalize_optional_filter(target_id);
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let conn = open_connection(&app)?;
    Ok(SupplierAftersaleFollowupListResult {
        items: load_supplier_aftersale_followup_views(
            &conn,
            target_type.as_deref(),
            target_id.as_deref(),
            status.as_deref(),
            limit,
        )?,
        total: count_supplier_aftersale_followups(
            &conn,
            target_type.as_deref(),
            target_id.as_deref(),
            status.as_deref(),
        )?,
    })
}

#[tauri::command]
pub fn record_supplier_aftersale_followup(
    app: AppHandle,
    request: SupplierAftersaleFollowupRecordRequest,
) -> AppResult<SupplierAftersaleFollowupRecordResult> {
    let target_type = normalize_aftersale_evidence_target_type(&request.target_type)?;
    let target_id = request.target_id.trim();
    if target_id.is_empty() {
        return Err(AppError::Validation(
            "售后/纠纷目标 ID 不能为空".to_string(),
        ));
    }
    let followup_type = normalize_supplier_aftersale_followup_type(&request.followup_type)?;
    let status = normalize_supplier_aftersale_followup_status(&request.status)?;
    let note = request.note.trim();
    if note.is_empty() {
        return Err(AppError::Validation("供应商协同备注不能为空".to_string()));
    }
    if note.chars().count() > 1000 {
        return Err(AppError::Validation(
            "供应商协同备注不能超过 1000 个字符".to_string(),
        ));
    }
    let purchase_task_id = normalize_optional_note(request.purchase_task_id, 120, "采购任务 ID")?;
    let supplier_name = normalize_optional_note(request.supplier_name, 120, "供应商名称")?;

    let mut conn = open_connection(&app)?;
    let (resolved_target_id, shop_id, external_target_id) =
        resolve_aftersale_evidence_target(&conn, target_type, target_id)?;
    if let Some(purchase_task_id) = purchase_task_id.as_deref() {
        let exists = conn
            .query_row(
                "SELECT 1 FROM purchase_tasks WHERE id = ?1 LIMIT 1",
                [purchase_task_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(AppError::Validation("采购任务不存在".to_string()));
        }
    }

    let tx = conn.transaction()?;
    let followup_id = format!("supplier-aftersale-followup-{}", Uuid::new_v4());
    let now = now_shanghai();
    tx.execute(
        "INSERT INTO supplier_aftersale_followups
         (id, target_type, target_id, shop_id, external_target_id, purchase_task_id,
          supplier_name, followup_type, status, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        params![
            followup_id,
            target_type,
            resolved_target_id,
            shop_id,
            external_target_id,
            purchase_task_id,
            supplier_name,
            followup_type,
            status,
            note,
            now
        ],
    )?;
    upsert_notification(
        &tx,
        "info",
        "supplier_aftersale_followup",
        &followup_id,
        Some(&shop_id),
        "供应商售后协同已记录",
        &format!(
            "{} {} 已记录供应商协同：{}，状态为{}。",
            if target_type == "guarantee" {
                "纠纷单"
            } else {
                "售后单"
            },
            external_target_id,
            supplier_aftersale_followup_type_text(followup_type),
            supplier_aftersale_followup_status_text(status)
        ),
        Some(&serde_json::json!({
            "followup_id": &followup_id,
            "target_type": target_type,
            "target_id": &resolved_target_id,
            "external_target_id": &external_target_id,
            "purchase_task_id_present": purchase_task_id.is_some(),
            "supplier_name_present": supplier_name.is_some(),
            "followup_type": followup_type,
            "status": status,
            "has_note": true
        })),
    )?;
    tx.commit()?;

    Ok(SupplierAftersaleFollowupRecordResult {
        followup_id,
        target_type: target_type.to_string(),
        target_id: resolved_target_id,
        status: status.to_string(),
        message: "供应商售后协同已记录，只保存本地脱敏备注，不调用供应商平台或微信处理接口"
            .to_string(),
    })
}

#[tauri::command]
pub fn list_aftersale_reject_reasons(
    app: AppHandle,
    shop_id: Option<String>,
    reject_scene: Option<i64>,
) -> AppResult<Vec<AftersaleRejectReasonView>> {
    let conn = open_connection(&app)?;
    let shop_id = normalize_optional_filter(shop_id);
    load_aftersale_reject_reason_views(&conn, shop_id.as_deref(), reject_scene)
}

#[tauri::command]
pub async fn sync_aftersale_reject_reasons(
    app: AppHandle,
    shop_id: String,
) -> AppResult<AftersaleRejectReasonSyncResult> {
    let shop_id = shop_id.trim().to_string();
    if shop_id.is_empty() {
        return Err(AppError::Validation("店铺 ID 不能为空".to_string()));
    }

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let task_id = format!("aftersale-reject-reason-sync-{}", Uuid::new_v4());
    let started_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'aftersales.sync_reject_reasons', 'running', 0, ?2, ?2)",
            params![task_id, started_at],
        )?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "开始同步微信售后拒绝原因",
            Some(&serde_json::json!({ "shop_id": &shop_id })),
        )?;
    }

    let mut result = AftersaleRejectReasonSyncResult {
        task_id: task_id.clone(),
        shop_id: shop_id.clone(),
        synced_reasons: 0,
        failed_steps: Vec::new(),
    };
    let call = client.get_aftersale_reject_reasons(&access_token).await?;
    match &call.result {
        WechatCallResult::Success(raw) => {
            let mut conn = open_connection(&app)?;
            result.synced_reasons =
                upsert_aftersale_reject_reasons(&mut conn, &shop_id, &raw.raw_payload)?;
            insert_success_api_and_task_log(
                &conn,
                &task_id,
                &shop_id,
                call.meta.endpoint,
                call.meta.method,
                &format!("售后拒绝原因同步完成：{} 条", result.synced_reasons),
            )?;
        }
        WechatCallResult::ApiError(error) => {
            result
                .failed_steps
                .push(format!("售后拒绝原因同步失败：{}", error.errmsg));
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
pub fn record_aftersale_responsibility(
    app: AppHandle,
    request: AftersaleResponsibilityRequest,
) -> AppResult<AftersaleResponsibilityResult> {
    let aftersale_id = request.aftersale_id.trim();
    if aftersale_id.is_empty() {
        return Err(AppError::Validation("售后单 ID 不能为空".to_string()));
    }
    let party = normalize_aftersale_responsibility_party(&request.responsibility_party)?;
    let note = request
        .responsibility_note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if note.as_deref().map(str::len).unwrap_or_default() > 1000 {
        return Err(AppError::Validation(
            "售后处理备注不能超过 1000 个字符".to_string(),
        ));
    }
    let supplier_compensation_cents = request.supplier_compensation_cents.unwrap_or(0);
    if supplier_compensation_cents < 0 {
        return Err(AppError::Validation("供应商赔付金额不能为负数".to_string()));
    }
    if supplier_compensation_cents > 0 && party != "supplier" {
        return Err(AppError::Validation(
            "只有责任方为 supplier 时才能记录供应商赔付金额".to_string(),
        ));
    }

    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    let aftersale = tx
        .query_row(
            "SELECT id, shop_id, wechat_aftersale_id, order_id, wechat_order_id,
                    supplier_compensation_adjustment_id
             FROM aftersales
             WHERE id = ?1 OR wechat_aftersale_id = ?1
             LIMIT 1",
            [aftersale_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("售后单不存在".to_string()))?;

    let (
        resolved_aftersale_id,
        shop_id,
        wechat_aftersale_id,
        order_id,
        wechat_order_id,
        old_adjustment_id,
    ) = aftersale;
    let adjustment_note = note
        .as_deref()
        .map(|value| format!("售后供应商赔付：{}；{}", wechat_aftersale_id, value))
        .unwrap_or_else(|| format!("售后供应商赔付：{}", wechat_aftersale_id));

    let profit_adjustment_id = if supplier_compensation_cents > 0 {
        match order_id.as_deref() {
            Some(order_id) => {
                let adjustment_id = match old_adjustment_id.as_deref() {
                    Some(existing_id) => {
                        let updated = tx.execute(
                            "UPDATE order_profit_adjustments
                             SET amount_cents = ?1, note = ?2
                             WHERE id = ?3 AND kind = 'other_income'",
                            params![supplier_compensation_cents, adjustment_note, existing_id],
                        )?;
                        if updated > 0 {
                            existing_id.to_string()
                        } else {
                            let new_id = format!("profit-adj-{}", Uuid::new_v4());
                            tx.execute(
                                "INSERT INTO order_profit_adjustments
                                 (id, order_id, kind, amount_cents, note, created_at)
                                 VALUES (?1, ?2, 'other_income', ?3, ?4, ?5)",
                                params![
                                    new_id,
                                    order_id,
                                    supplier_compensation_cents,
                                    adjustment_note,
                                    now_shanghai()
                                ],
                            )?;
                            new_id
                        }
                    }
                    None => {
                        let new_id = format!("profit-adj-{}", Uuid::new_v4());
                        tx.execute(
                            "INSERT INTO order_profit_adjustments
                             (id, order_id, kind, amount_cents, note, created_at)
                             VALUES (?1, ?2, 'other_income', ?3, ?4, ?5)",
                            params![
                                new_id,
                                order_id,
                                supplier_compensation_cents,
                                adjustment_note,
                                now_shanghai()
                            ],
                        )?;
                        new_id
                    }
                };
                Some(adjustment_id)
            }
            None => None,
        }
    } else {
        if let Some(existing_id) = old_adjustment_id.as_deref() {
            tx.execute(
                "DELETE FROM order_profit_adjustments
                 WHERE id = ?1 AND kind = 'other_income'",
                [existing_id],
            )?;
        }
        None
    };

    let now = now_shanghai();
    tx.execute(
        "UPDATE aftersales
         SET responsibility_party = ?1,
             responsibility_note = ?2,
             supplier_compensation_cents = ?3,
             supplier_compensation_adjustment_id = ?4,
             handled_at = ?5,
             updated_at = ?5
         WHERE id = ?6",
        params![
            party,
            note,
            supplier_compensation_cents,
            profit_adjustment_id.as_deref(),
            now,
            resolved_aftersale_id
        ],
    )?;

    if party == "supplier" {
        upsert_notification(
            &tx,
            "info",
            "aftersale_responsibility",
            &resolved_aftersale_id,
            Some(&shop_id),
            "售后已标记供应商责任",
            &format!(
                "售后单 {} 已归因为供应商责任{}。",
                wechat_aftersale_id,
                if supplier_compensation_cents > 0 {
                    "，供应商赔付已计入利润回款"
                } else {
                    ""
                }
            ),
            Some(&serde_json::json!({
                "aftersale_id": &resolved_aftersale_id,
                "wechat_aftersale_id": &wechat_aftersale_id,
                "wechat_order_id": &wechat_order_id,
                "supplier_compensation_cents": supplier_compensation_cents,
                "profit_adjustment_id": &profit_adjustment_id
            })),
        )?;
    }
    tx.commit()?;

    let message = if supplier_compensation_cents > 0 && profit_adjustment_id.is_none() {
        "售后责任已记录；该售后未关联本地订单，供应商赔付暂未计入利润".to_string()
    } else if supplier_compensation_cents > 0 {
        "售后责任已记录，供应商赔付已计入订单利润回款".to_string()
    } else {
        "售后责任已记录".to_string()
    };

    Ok(AftersaleResponsibilityResult {
        aftersale_id: resolved_aftersale_id,
        responsibility_party: party.to_string(),
        supplier_compensation_cents,
        profit_adjustment_id,
        message,
    })
}

#[tauri::command]
pub async fn accept_aftersale(
    app: AppHandle,
    request: AftersaleAcceptRequest,
) -> AppResult<AftersaleActionResult> {
    let aftersale_id = request.aftersale_id.trim();
    if aftersale_id.is_empty() {
        return Err(AppError::Validation("售后单 ID 不能为空".to_string()));
    }
    let address_id = request
        .address_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if let Some(accept_type) = request.accept_type {
        if !matches!(accept_type, 1 | 2) {
            return Err(AppError::Validation(
                "同意类型 accept_type 只能是 1 或 2".to_string(),
            ));
        }
    }
    let note = normalize_optional_note(request.note, 1000, "售后处理备注")?;
    let target = load_aftersale_action_target(&app, aftersale_id)?;
    validate_aftersale_action_status(&target)?;

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &target.shop_id, &client).await?;
    let call = match client
        .accept_aftersale(
            &access_token,
            &target.wechat_aftersale_id,
            address_id.as_deref(),
            request.accept_type,
        )
        .await
    {
        Ok(call) => call,
        Err(error) => {
            let message = format!("同意售后请求失败：{error}");
            let conn = open_connection(&app)?;
            update_aftersale_action_state(
                &conn,
                &target.id,
                "accept",
                "failed",
                Some(&message),
                note.as_deref(),
            )?;
            return Err(error);
        }
    };
    handle_aftersale_action_call(&app, target, call, "accept", note.as_deref())
}

#[tauri::command]
pub async fn reject_aftersale(
    app: AppHandle,
    request: AftersaleRejectRequest,
) -> AppResult<AftersaleActionResult> {
    let aftersale_id = request.aftersale_id.trim();
    if aftersale_id.is_empty() {
        return Err(AppError::Validation("售后单 ID 不能为空".to_string()));
    }
    if request.reject_reason_type <= 0 {
        return Err(AppError::Validation(
            "拒绝原因类型 reject_reason_type 必须大于 0".to_string(),
        ));
    }
    let reject_reason = request
        .reject_reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if reject_reason.as_deref().map(str::len).unwrap_or_default() > 1000 {
        return Err(AppError::Validation(
            "拒绝原因不能超过 1000 个字符".to_string(),
        ));
    }
    let note = normalize_optional_note(request.note, 1000, "售后处理备注")?;
    let target = load_aftersale_action_target(&app, aftersale_id)?;
    validate_aftersale_action_status(&target)?;

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &target.shop_id, &client).await?;
    let call = match client
        .reject_aftersale(
            &access_token,
            &target.wechat_aftersale_id,
            request.reject_reason_type,
            reject_reason.as_deref(),
        )
        .await
    {
        Ok(call) => call,
        Err(error) => {
            let message = format!("拒绝售后请求失败：{error}");
            let conn = open_connection(&app)?;
            update_aftersale_action_state(
                &conn,
                &target.id,
                "reject",
                "failed",
                Some(&message),
                note.as_deref(),
            )?;
            return Err(error);
        }
    };
    handle_aftersale_action_call(&app, target, call, "reject", note.as_deref())
}

#[tauri::command]
pub async fn run_aftersale_sync_once(
    app: AppHandle,
    lookback_hours: Option<i64>,
    limit: Option<i64>,
) -> AppResult<AftersaleSyncBatchResult> {
    let lookback_hours = lookback_hours.unwrap_or(24).clamp(1, 24);
    let limit = limit.unwrap_or(200).clamp(1, 1000);
    let sync_shops = {
        let conn = open_connection(&app)?;
        load_order_sync_shops(&conn)?
    };
    let task_id = format!("aftersale-sync-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'aftersales.sync_shop_aftersales', 'running', 0, ?2, ?2)",
            params![task_id, created_at],
        )?;
    }

    if sync_shops.is_empty() {
        let conn = open_connection(&app)?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "没有可同步售后的 active 店铺",
            None,
        )?;
        conn.execute(
            "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
            params![now_shanghai(), task_id],
        )?;
        return Ok(AftersaleSyncBatchResult {
            task_id,
            processed_shops: 0,
            synced_aftersales: 0,
            failed_shops: 0,
            failed_aftersales: 0,
        });
    }

    let client = WechatShopClient::default();
    let end_time = Utc::now().timestamp();
    let start_time = (Utc::now() - Duration::hours(lookback_hours)).timestamp();
    let mut processed_shops = 0i64;
    let mut synced_aftersales = 0i64;
    let mut failed_shops = 0i64;
    let mut failed_aftersales = 0i64;

    for shop in sync_shops {
        if synced_aftersales + failed_aftersales >= limit {
            break;
        }
        processed_shops += 1;
        let access_token = match ensure_access_token(&app, &shop.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_shops += 1;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(&shop.shop_id),
                    "error",
                    &format!("店铺 {} 获取 access_token 失败：{error}", shop.shop_name),
                    None,
                )?;
                continue;
            }
        };

        let mut next_key: Option<String> = None;
        let mut page_count = 0;
        loop {
            if synced_aftersales + failed_aftersales >= limit {
                break;
            }
            page_count += 1;
            let call = match client
                .get_aftersale_list(
                    &access_token,
                    start_time,
                    end_time,
                    next_key.as_deref().unwrap_or(""),
                )
                .await
            {
                Ok(call) => call,
                Err(error) => {
                    failed_shops += 1;
                    insert_task_log_for_app(
                        &app,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 获取售后列表请求失败：{error}", shop.shop_name),
                        None,
                    )?;
                    break;
                }
            };

            let conn = open_connection(&app)?;
            match &call.result {
                WechatCallResult::Success(result) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "success",
                        None,
                        None,
                        Some(&format!(
                            "aftersale list ok, count={}, has_more={}",
                            result.after_sale_order_id_list.len(),
                            result.has_more
                        )),
                    )?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "info",
                        &format!(
                            "店铺 {} 获取售后单 {} 个",
                            shop.shop_name,
                            result.after_sale_order_id_list.len()
                        ),
                        Some(&serde_json::json!({
                            "lookback_hours": lookback_hours,
                            "page": page_count,
                            "has_more": result.has_more
                        })),
                    )?;
                    drop(conn);

                    for aftersale_id in &result.after_sale_order_id_list {
                        if synced_aftersales + failed_aftersales >= limit {
                            break;
                        }
                        let detail_call = match client
                            .get_aftersale_order(&access_token, aftersale_id)
                            .await
                        {
                            Ok(call) => call,
                            Err(error) => {
                                failed_aftersales += 1;
                                let conn = open_connection(&app)?;
                                save_failed_aftersale(
                                    &conn,
                                    &shop.shop_id,
                                    aftersale_id,
                                    &format!("微信售后详情请求失败：{error}"),
                                )?;
                                insert_task_log(
                                    &conn,
                                    &task_id,
                                    Some(&shop.shop_id),
                                    "error",
                                    &format!("售后单 {} 详情请求失败：{error}", aftersale_id),
                                    None,
                                )?;
                                continue;
                            }
                        };

                        let conn = open_connection(&app)?;
                        match &detail_call.result {
                            WechatCallResult::Success(detail) => {
                                insert_api_call_log(
                                    &conn,
                                    Some(&shop.shop_id),
                                    detail_call.meta.endpoint,
                                    detail_call.meta.method,
                                    "success",
                                    None,
                                    None,
                                    Some(&format!("get aftersale ok, id={aftersale_id}")),
                                )?;
                                save_synced_aftersale(
                                    &conn,
                                    &shop.shop_id,
                                    &detail.after_sale_order,
                                )?;
                                synced_aftersales += 1;
                            }
                            WechatCallResult::ApiError(error) => {
                                insert_api_call_log(
                                    &conn,
                                    Some(&shop.shop_id),
                                    detail_call.meta.endpoint,
                                    detail_call.meta.method,
                                    "api_error",
                                    Some(error.errcode),
                                    Some(&error.errmsg),
                                    Some("get aftersale api error"),
                                )?;
                                save_failed_aftersale(
                                    &conn,
                                    &shop.shop_id,
                                    aftersale_id,
                                    &format!("微信售后详情失败：{}", error.errmsg),
                                )?;
                                failed_aftersales += 1;
                                insert_task_log(
                                    &conn,
                                    &task_id,
                                    Some(&shop.shop_id),
                                    "error",
                                    &format!(
                                        "售后单 {} 详情同步失败：{}",
                                        aftersale_id, error.errmsg
                                    ),
                                    Some(&serde_json::json!({
                                        "errcode": error.errcode
                                    })),
                                )?;
                            }
                        }
                    }

                    if !result.has_more || page_count >= 5 {
                        break;
                    }
                    next_key = result
                        .next_key
                        .clone()
                        .filter(|value| !value.trim().is_empty());
                    if next_key.is_none() {
                        break;
                    }
                }
                WechatCallResult::ApiError(error) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some("aftersale list api error"),
                    )?;
                    failed_shops += 1;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 售后列表同步失败：{}", shop.shop_name, error.errmsg),
                        Some(&serde_json::json!({
                            "errcode": error.errcode
                        })),
                    )?;
                    break;
                }
            }
        }
    }

    let final_status = if failed_shops == 0 && failed_aftersales == 0 {
        "success"
    } else if processed_shops > 0 && failed_shops == processed_shops && synced_aftersales == 0 {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;

    Ok(AftersaleSyncBatchResult {
        task_id,
        processed_shops,
        synced_aftersales,
        failed_shops,
        failed_aftersales,
    })
}

#[tauri::command]
pub fn list_guarantee_orders(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<GuaranteeOrderListResult> {
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let status = status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(str::to_string);
    let conn = open_connection(&app)?;
    Ok(GuaranteeOrderListResult {
        items: load_guarantee_order_views(&conn, status.as_deref(), limit)?,
        total: count_guarantee_orders(&conn, status.as_deref())?,
    })
}

#[tauri::command]
pub fn record_guarantee_followup(
    app: AppHandle,
    request: GuaranteeFollowupRequest,
) -> AppResult<GuaranteeFollowupResult> {
    let guarantee_order_id = request.guarantee_order_id.trim();
    if guarantee_order_id.is_empty() {
        return Err(AppError::Validation("纠纷单 ID 不能为空".to_string()));
    }
    let handling_status = normalize_guarantee_handling_status(&request.handling_status)?;
    let responsibility_party = match request
        .responsibility_party
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => Some(normalize_aftersale_responsibility_party(value)?.to_string()),
        None => None,
    };
    let note = normalize_optional_note(request.handling_note, 1000, "纠纷跟进备注")?;
    let supplier_compensation_cents = request.supplier_compensation_cents.unwrap_or(0);
    if supplier_compensation_cents < 0 {
        return Err(AppError::Validation("供应商赔付金额不能为负数".to_string()));
    }
    if supplier_compensation_cents > 0 && responsibility_party.as_deref() != Some("supplier") {
        return Err(AppError::Validation(
            "只有责任方为 supplier 时才能记录纠纷供应商赔付金额".to_string(),
        ));
    }

    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    let guarantee = tx
        .query_row(
            "SELECT id, shop_id, guarantee_order_id, order_id, wechat_order_id,
                    supplier_compensation_adjustment_id
             FROM guarantee_orders
             WHERE id = ?1 OR guarantee_order_id = ?1
             LIMIT 1",
            [guarantee_order_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("纠纷单不存在".to_string()))?;
    let (
        resolved_id,
        shop_id,
        resolved_guarantee_order_id,
        order_id,
        wechat_order_id,
        old_adjustment_id,
    ) = guarantee;
    let adjustment_note = note
        .as_deref()
        .map(|value| format!("纠纷供应商赔付：{}；{}", resolved_guarantee_order_id, value))
        .unwrap_or_else(|| format!("纠纷供应商赔付：{}", resolved_guarantee_order_id));
    let now = now_shanghai();

    let profit_adjustment_id = if supplier_compensation_cents > 0 {
        match order_id.as_deref() {
            Some(order_id) => {
                let adjustment_id = match old_adjustment_id.as_deref() {
                    Some(existing_id) => {
                        let updated = tx.execute(
                            "UPDATE order_profit_adjustments
                             SET amount_cents = ?1, note = ?2
                             WHERE id = ?3 AND kind = 'other_income'",
                            params![supplier_compensation_cents, adjustment_note, existing_id],
                        )?;
                        if updated > 0 {
                            existing_id.to_string()
                        } else {
                            let new_id = format!("profit-adj-{}", Uuid::new_v4());
                            tx.execute(
                                "INSERT INTO order_profit_adjustments
                                 (id, order_id, kind, amount_cents, note, created_at)
                                 VALUES (?1, ?2, 'other_income', ?3, ?4, ?5)",
                                params![
                                    new_id,
                                    order_id,
                                    supplier_compensation_cents,
                                    adjustment_note,
                                    now
                                ],
                            )?;
                            new_id
                        }
                    }
                    None => {
                        let new_id = format!("profit-adj-{}", Uuid::new_v4());
                        tx.execute(
                            "INSERT INTO order_profit_adjustments
                             (id, order_id, kind, amount_cents, note, created_at)
                             VALUES (?1, ?2, 'other_income', ?3, ?4, ?5)",
                            params![
                                new_id,
                                order_id,
                                supplier_compensation_cents,
                                adjustment_note,
                                now
                            ],
                        )?;
                        new_id
                    }
                };
                Some(adjustment_id)
            }
            None => None,
        }
    } else {
        if let Some(existing_id) = old_adjustment_id.as_deref() {
            tx.execute(
                "DELETE FROM order_profit_adjustments
                 WHERE id = ?1 AND kind = 'other_income'",
                [existing_id],
            )?;
        }
        None
    };

    tx.execute(
        "UPDATE guarantee_orders
         SET handling_status = ?1,
             handling_note = ?2,
             responsibility_party = ?3,
             supplier_compensation_cents = ?4,
             supplier_compensation_adjustment_id = ?5,
             handled_at = ?6,
             updated_at = ?6
         WHERE id = ?7",
        params![
            handling_status,
            note,
            responsibility_party.as_deref(),
            supplier_compensation_cents,
            profit_adjustment_id.as_deref(),
            now,
            resolved_id
        ],
    )?;

    let severity = match handling_status {
        "resolved" | "ignored" => "info",
        _ => "warning",
    };
    let compensation_suffix = if supplier_compensation_cents > 0 && profit_adjustment_id.is_some() {
        "，供应商赔付已计入利润回款"
    } else if supplier_compensation_cents > 0 {
        "，该纠纷未关联本地订单，赔付暂未计入利润"
    } else {
        ""
    };
    upsert_notification(
        &tx,
        severity,
        "guarantee_followup",
        &resolved_id,
        Some(&shop_id),
        "纠纷单跟进已更新",
        &format!(
            "纠纷单 {} 已标记为{}{}。",
            resolved_guarantee_order_id,
            guarantee_handling_status_text(handling_status),
            compensation_suffix
        ),
        Some(&serde_json::json!({
            "guarantee_id": &resolved_id,
            "guarantee_order_id": &resolved_guarantee_order_id,
            "wechat_order_id": &wechat_order_id,
            "handling_status": handling_status,
            "responsibility_party": &responsibility_party,
            "has_note": note.is_some(),
            "supplier_compensation_cents": supplier_compensation_cents,
            "profit_adjustment_id": &profit_adjustment_id
        })),
    )?;
    tx.commit()?;

    let message = if supplier_compensation_cents > 0 && profit_adjustment_id.is_none() {
        "纠纷跟进已记录；该纠纷未关联本地订单，供应商赔付暂未计入利润".to_string()
    } else if supplier_compensation_cents > 0 {
        "纠纷跟进已记录，供应商赔付已计入订单利润回款".to_string()
    } else {
        "纠纷跟进已记录".to_string()
    };
    Ok(GuaranteeFollowupResult {
        guarantee_order_id: resolved_guarantee_order_id,
        handling_status: handling_status.to_string(),
        responsibility_party,
        supplier_compensation_cents,
        profit_adjustment_id,
        message,
    })
}

#[tauri::command]
pub async fn run_guarantee_sync_once(
    app: AppHandle,
    lookback_hours: Option<i64>,
    limit: Option<i64>,
) -> AppResult<GuaranteeSyncBatchResult> {
    let lookback_hours = lookback_hours.unwrap_or(24).clamp(1, 168);
    let limit = limit.unwrap_or(200).clamp(1, 1000);
    let sync_shops = {
        let conn = open_connection(&app)?;
        load_order_sync_shops(&conn)?
    };
    let task_id = format!("guarantee-sync-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'aftersales.sync_guarantee_orders', 'running', 0, ?2, ?2)",
            params![task_id, created_at],
        )?;
    }

    if sync_shops.is_empty() {
        let conn = open_connection(&app)?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "没有可同步纠纷单的 active 店铺",
            None,
        )?;
        conn.execute(
            "UPDATE task_runs SET status = 'success', progress = 100, finished_at = ?1 WHERE id = ?2",
            params![now_shanghai(), task_id],
        )?;
        return Ok(GuaranteeSyncBatchResult {
            task_id,
            processed_shops: 0,
            synced_guarantees: 0,
            failed_shops: 0,
            failed_guarantees: 0,
        });
    }

    let client = WechatShopClient::default();
    let end_time = Utc::now().timestamp();
    let start_time = (Utc::now() - Duration::hours(lookback_hours)).timestamp();
    let mut processed_shops = 0i64;
    let mut synced_guarantees = 0i64;
    let mut failed_shops = 0i64;
    let mut failed_guarantees = 0i64;

    for shop in sync_shops {
        if synced_guarantees + failed_guarantees >= limit {
            break;
        }
        processed_shops += 1;
        let access_token = match ensure_access_token(&app, &shop.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                failed_shops += 1;
                insert_task_log_for_app(
                    &app,
                    &task_id,
                    Some(&shop.shop_id),
                    "error",
                    &format!("店铺 {} 获取 access_token 失败：{error}", shop.shop_name),
                    None,
                )?;
                continue;
            }
        };

        let mut offset = 0i64;
        let mut page_count = 0i64;
        loop {
            if synced_guarantees + failed_guarantees >= limit {
                break;
            }
            page_count += 1;
            let page_limit = (limit - synced_guarantees - failed_guarantees).min(50);
            let call = match client
                .search_guarantee_orders(&access_token, start_time, end_time, offset, page_limit)
                .await
            {
                Ok(call) => call,
                Err(error) => {
                    failed_shops += 1;
                    insert_task_log_for_app(
                        &app,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 获取纠纷单列表请求失败：{error}", shop.shop_name),
                        None,
                    )?;
                    break;
                }
            };

            let conn = open_connection(&app)?;
            match &call.result {
                WechatCallResult::Success(raw) => {
                    let guarantee_orders = raw
                        .raw_payload
                        .get("guarantee_order_list")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let total_num = json_value_to_i64(raw.raw_payload.get("total_num"));
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "success",
                        None,
                        None,
                        Some(&format!(
                            "guarantee list ok, count={}, offset={}, total={}",
                            guarantee_orders.len(),
                            offset,
                            total_num
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        )),
                    )?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "info",
                        &format!(
                            "店铺 {} 获取纠纷单 {} 个",
                            shop.shop_name,
                            guarantee_orders.len()
                        ),
                        Some(&serde_json::json!({
                            "lookback_hours": lookback_hours,
                            "page": page_count,
                            "offset": offset,
                            "limit": page_limit,
                            "total_num": total_num
                        })),
                    )?;
                    drop(conn);

                    for guarantee in &guarantee_orders {
                        if synced_guarantees + failed_guarantees >= limit {
                            break;
                        }
                        let guarantee_order_id =
                            json_value_to_string(guarantee.get("guarantee_order_id"));
                        let Some(guarantee_order_id) = guarantee_order_id else {
                            failed_guarantees += 1;
                            insert_task_log_for_app(
                                &app,
                                &task_id,
                                Some(&shop.shop_id),
                                "error",
                                "纠纷单列表项缺少 guarantee_order_id",
                                Some(&sanitize_aftersale_payload(guarantee)),
                            )?;
                            continue;
                        };

                        let detail_call = match client
                            .get_guarantee_order(&access_token, &guarantee_order_id)
                            .await
                        {
                            Ok(call) => call,
                            Err(error) => {
                                failed_guarantees += 1;
                                let conn = open_connection(&app)?;
                                save_failed_guarantee_order(
                                    &conn,
                                    &shop.shop_id,
                                    &guarantee_order_id,
                                    &format!("微信纠纷单详情请求失败：{error}"),
                                )?;
                                insert_task_log(
                                    &conn,
                                    &task_id,
                                    Some(&shop.shop_id),
                                    "error",
                                    &format!("纠纷单 {} 详情请求失败：{error}", guarantee_order_id),
                                    None,
                                )?;
                                continue;
                            }
                        };

                        let conn = open_connection(&app)?;
                        match &detail_call.result {
                            WechatCallResult::Success(raw_detail) => {
                                insert_api_call_log(
                                    &conn,
                                    Some(&shop.shop_id),
                                    detail_call.meta.endpoint,
                                    detail_call.meta.method,
                                    "success",
                                    None,
                                    None,
                                    Some(&format!("get guarantee ok, id={guarantee_order_id}")),
                                )?;
                                let guarantee_order = raw_detail
                                    .raw_payload
                                    .get("guarantee_order")
                                    .unwrap_or(guarantee);
                                save_synced_guarantee_order(&conn, &shop.shop_id, guarantee_order)?;
                                synced_guarantees += 1;
                            }
                            WechatCallResult::ApiError(error) => {
                                insert_api_call_log(
                                    &conn,
                                    Some(&shop.shop_id),
                                    detail_call.meta.endpoint,
                                    detail_call.meta.method,
                                    "api_error",
                                    Some(error.errcode),
                                    Some(&error.errmsg),
                                    Some("get guarantee api error"),
                                )?;
                                save_failed_guarantee_order(
                                    &conn,
                                    &shop.shop_id,
                                    &guarantee_order_id,
                                    &format!("微信纠纷单详情失败：{}", error.errmsg),
                                )?;
                                failed_guarantees += 1;
                                insert_task_log(
                                    &conn,
                                    &task_id,
                                    Some(&shop.shop_id),
                                    "error",
                                    &format!(
                                        "纠纷单 {} 详情同步失败：{}",
                                        guarantee_order_id, error.errmsg
                                    ),
                                    Some(&serde_json::json!({
                                        "errcode": error.errcode
                                    })),
                                )?;
                            }
                        }
                    }

                    if guarantee_orders.len() < page_limit as usize || page_count >= 5 {
                        break;
                    }
                    if let Some(total_num) = total_num {
                        if offset + page_limit >= total_num {
                            break;
                        }
                    }
                    offset += page_limit;
                }
                WechatCallResult::ApiError(error) => {
                    insert_api_call_log(
                        &conn,
                        Some(&shop.shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some("guarantee list api error"),
                    )?;
                    failed_shops += 1;
                    insert_task_log(
                        &conn,
                        &task_id,
                        Some(&shop.shop_id),
                        "error",
                        &format!("店铺 {} 纠纷单同步失败：{}", shop.shop_name, error.errmsg),
                        Some(&serde_json::json!({
                            "errcode": error.errcode
                        })),
                    )?;
                    break;
                }
            }
        }
    }

    let final_status = if failed_shops == 0 && failed_guarantees == 0 {
        "success"
    } else if processed_shops > 0 && failed_shops == processed_shops && synced_guarantees == 0 {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;

    Ok(GuaranteeSyncBatchResult {
        task_id,
        processed_shops,
        synced_guarantees,
        failed_shops,
        failed_guarantees,
    })
}
