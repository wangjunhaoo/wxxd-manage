use super::*;

#[tauri::command]
pub fn run_publish_tasks_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PublishTaskBatchResult> {
    let mut conn = open_connection(&app)?;
    let limit = limit.unwrap_or(20).clamp(1, 200);
    let pending_items = load_pending_publish_items(&conn, limit)?;
    if pending_items.is_empty() {
        return Ok(PublishTaskBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            ready_items: 0,
            failed_items: 0,
        });
    }

    let mut job_ids = BTreeSet::new();
    let mut ready_items = 0i64;
    let mut failed_items = 0i64;
    let tx = conn.transaction()?;

    for item in pending_items {
        job_ids.insert(item.job_id.clone());
        let started_at = now_shanghai();
        tx.execute(
            "UPDATE task_runs
             SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
             WHERE id = ?2",
            params![started_at, item.job_id],
        )?;
        tx.execute(
            "UPDATE publish_job_items SET status = 'prechecking' WHERE id = ?1",
            [item.item_id.as_str()],
        )?;
        insert_task_log(
            &tx,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始本地前置校验",
            None,
        )?;

        let product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                mark_publish_item_failed(
                    &tx,
                    &item,
                    "INVALID_PRODUCT_PAYLOAD",
                    &format!("商品原始数据无法解析：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        if let Some((code, summary)) = precheck_publish_item(&tx, &item, &product)? {
            mark_publish_item_failed(&tx, &item, code, &summary)?;
            failed_items += 1;
            continue;
        }

        let payload_prepare = match prepare_add_product_payload_for_publish(&tx, &item, &product)? {
            Ok(payload_prepare) => payload_prepare,
            Err((code, summary)) => {
                mark_publish_item_failed(&tx, &item, code, &summary)?;
                failed_items += 1;
                continue;
            }
        };
        let ready_summary = publish_payload_ready_summary(&payload_prepare);
        tx.execute(
            "UPDATE publish_job_items
             SET status = 'ready_to_publish',
                 error_code = NULL,
                 error_summary = ?1
             WHERE id = ?2",
            params![ready_summary, item.item_id.as_str()],
        )?;
        insert_task_log(
            &tx,
            &item.job_id,
            Some(&item.item_id),
            "info",
            &ready_summary,
            None,
        )?;
        ready_items += 1;
    }

    for job_id in &job_ids {
        recompute_publish_job(&tx, job_id)?;
    }

    tx.commit()?;

    Ok(PublishTaskBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items: ready_items + failed_items,
        ready_items,
        failed_items,
    })
}

#[tauri::command]
pub fn run_publish_attribute_fill_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PublishAttributeFillBatchResult> {
    let mut conn = open_connection(&app)?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let items = load_attribute_fill_items(&conn, limit)?;
    if items.is_empty() {
        return Ok(PublishAttributeFillBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            auto_filled_items: 0,
            suggestion_only_items: 0,
            failed_items: 0,
            generated_suggestions: 0,
        });
    }

    let tx = conn.transaction()?;
    let mut job_ids = BTreeSet::new();
    let mut auto_filled_items = 0i64;
    let mut suggestion_only_items = 0i64;
    let mut failed_items = 0i64;
    let mut generated_suggestions = 0i64;

    for item in items {
        job_ids.insert(item.job_id.clone());
        let product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                failed_items += 1;
                insert_task_log(
                    &tx,
                    &item.job_id,
                    Some(&item.item_id),
                    "error",
                    &format!("属性补齐失败：商品原始数据无法解析：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let mut draft = match resolve_add_product_base_payload(&product) {
            Ok(draft) => draft,
            Err(error) => {
                failed_items += 1;
                insert_task_log(
                    &tx,
                    &item.job_id,
                    Some(&item.item_id),
                    "error",
                    &format!("属性补齐失败：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let Some(cat_id) = extract_leaf_category_id_from_payload(&draft.payload) else {
            failed_items += 1;
            insert_task_log(
                &tx,
                &item.job_id,
                Some(&item.item_id),
                "error",
                "属性补齐失败：缺少微信叶子类目 ID",
                None,
            )?;
            continue;
        };
        let Some(raw_detail) = load_cached_category_detail_payload(&tx, &item.shop_id, cat_id)?
        else {
            failed_items += 1;
            let summary = "本地未缓存该店铺类目详情，无法生成必填属性补齐建议；请先同步类目规则";
            conn_update_publish_item_error(&tx, &item, "CATEGORY_DETAIL_CACHE_MISSING", summary)?;
            insert_task_log(
                &tx,
                &item.job_id,
                Some(&item.item_id),
                "warn",
                summary,
                Some(&serde_json::json!({
                    "shop_id": &item.shop_id,
                    "cat_id": cat_id,
                    "external_product_id": &item.external_product_id
                })),
            )?;
            continue;
        };

        let requirement_check =
            check_cached_category_requirements(&tx, &item.shop_id, cat_id, &draft.payload)?;
        if !requirement_check.has_missing_attrs() {
            set_publish_item_status_in_conn(
                &tx,
                &item,
                "ready_to_publish",
                None,
                Some("必填属性已完整，等待重新执行微信类目预检"),
            )?;
            auto_filled_items += 1;
            continue;
        }

        let plan = build_attribute_fill_plan(
            &item,
            &product,
            &draft.payload,
            &raw_detail,
            &requirement_check,
        );
        for suggestion in &plan.suggestions {
            upsert_publish_attribute_suggestion(&tx, &item, suggestion)?;
        }
        generated_suggestions += plan.suggestions.len() as i64;

        if plan.can_auto_apply() {
            apply_attribute_fill_plan_to_payload(&mut draft.payload, &plan)?;
            persist_filled_add_product_payload(&tx, &item, &draft.payload, &plan)?;
            let summary = format!(
                "已自动补齐 {} 个必填属性，等待重新执行微信类目预检",
                plan.suggestions.len()
            );
            set_publish_item_status_in_conn(&tx, &item, "ready_to_publish", None, Some(&summary))?;
            insert_task_log(
                &tx,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &summary,
                Some(&serde_json::json!({
                    "cat_id": cat_id,
                    "suggestions": attribute_suggestions_json(&plan.suggestions)
                })),
            )?;
            auto_filled_items += 1;
        } else {
            let summary = plan.suggestion_summary();
            conn_update_publish_item_error(&tx, &item, "CATEGORY_ATTRS_NEED_AI_FILL", &summary)?;
            upsert_notification(
                &tx,
                "warning",
                "publish_item",
                &item.item_id,
                Some(&item.shop_id),
                "铺货必填属性需要 AI/人工确认",
                &summary,
                Some(&serde_json::json!({
                    "job_id": &item.job_id,
                    "item_id": &item.item_id,
                    "external_product_id": &item.external_product_id,
                    "suggestions": attribute_suggestions_json(&plan.suggestions)
                })),
            )?;
            insert_task_log(
                &tx,
                &item.job_id,
                Some(&item.item_id),
                "warn",
                &summary,
                Some(&serde_json::json!({
                    "cat_id": cat_id,
                    "suggestions": attribute_suggestions_json(&plan.suggestions)
                })),
            )?;
            suggestion_only_items += 1;
        }
    }

    for job_id in &job_ids {
        recompute_publish_job(&tx, job_id)?;
    }
    tx.commit()?;

    Ok(PublishAttributeFillBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items: auto_filled_items + suggestion_only_items + failed_items,
        auto_filled_items,
        suggestion_only_items,
        failed_items,
        generated_suggestions,
    })
}

#[tauri::command]
pub async fn run_publish_ai_attribute_suggestions_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PublishAttributeFillBatchResult> {
    let Some(ai_config) = load_optional_ai_provider_config(&app)? else {
        return Ok(PublishAttributeFillBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            auto_filled_items: 0,
            suggestion_only_items: 0,
            failed_items: 0,
            generated_suggestions: 0,
        });
    };
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let items = {
        let conn = open_connection(&app)?;
        load_attribute_fill_items(&conn, limit)?
    };
    if items.is_empty() {
        return Ok(PublishAttributeFillBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            auto_filled_items: 0,
            suggestion_only_items: 0,
            failed_items: 0,
            generated_suggestions: 0,
        });
    }

    let mut job_ids = BTreeSet::new();
    let mut auto_filled_items = 0i64;
    let mut suggestion_only_items = 0i64;
    let mut failed_items = 0i64;
    let mut generated_suggestions = 0i64;

    for item in items {
        job_ids.insert(item.job_id.clone());
        let product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                failed_items += 1;
                insert_task_log_for_app(
                    &app,
                    &item.job_id,
                    Some(&item.item_id),
                    "error",
                    &format!("AI 属性补齐失败：商品原始数据无法解析：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let mut draft = match resolve_add_product_base_payload(&product) {
            Ok(draft) => draft,
            Err(error) => {
                failed_items += 1;
                insert_task_log_for_app(
                    &app,
                    &item.job_id,
                    Some(&item.item_id),
                    "error",
                    &format!("AI 属性补齐失败：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let Some(cat_id) = extract_leaf_category_id_from_payload(&draft.payload) else {
            failed_items += 1;
            insert_task_log_for_app(
                &app,
                &item.job_id,
                Some(&item.item_id),
                "error",
                "AI 属性补齐失败：缺少微信叶子类目 ID",
                None,
            )?;
            continue;
        };
        let raw_detail = {
            let conn = open_connection(&app)?;
            load_cached_category_detail_payload(&conn, &item.shop_id, cat_id)?
        };
        let Some(raw_detail) = raw_detail else {
            failed_items += 1;
            continue;
        };
        let requirement_check = {
            let conn = open_connection(&app)?;
            check_cached_category_requirements(&conn, &item.shop_id, cat_id, &draft.payload)?
        };
        if !requirement_check.has_missing_attrs() {
            let conn = open_connection(&app)?;
            set_publish_item_status_in_conn(
                &conn,
                &item,
                "ready_to_publish",
                None,
                Some("必填属性已完整，等待重新执行微信类目预检"),
            )?;
            recompute_publish_job(&conn, &item.job_id)?;
            auto_filled_items += 1;
            continue;
        }

        let mut plan = build_attribute_fill_plan(
            &item,
            &product,
            &draft.payload,
            &raw_detail,
            &requirement_check,
        );
        let ai_generated = match fill_attribute_plan_with_ai(&ai_config, &mut plan).await {
            Ok(count) => count,
            Err(error) => {
                failed_items += 1;
                insert_task_log_for_app(
                    &app,
                    &item.job_id,
                    Some(&item.item_id),
                    "error",
                    &format!("AI 属性补齐失败：{error}"),
                    None,
                )?;
                continue;
            }
        };
        generated_suggestions += ai_generated;

        let conn = open_connection(&app)?;
        for suggestion in &plan.suggestions {
            upsert_publish_attribute_suggestion(&conn, &item, suggestion)?;
        }
        if plan.can_auto_apply() {
            apply_attribute_fill_plan_to_payload(&mut draft.payload, &plan)?;
            persist_filled_add_product_payload(&conn, &item, &draft.payload, &plan)?;
            let summary = format!(
                "AI 已生成并自动补齐 {} 个必填属性，等待重新执行微信类目预检",
                ai_generated
            );
            set_publish_item_status_in_conn(
                &conn,
                &item,
                "ready_to_publish",
                None,
                Some(&summary),
            )?;
            insert_task_log(
                &conn,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &summary,
                Some(&serde_json::json!({
                    "cat_id": cat_id,
                    "suggestions": attribute_suggestions_json(&plan.suggestions)
                })),
            )?;
            auto_filled_items += 1;
        } else {
            let summary = plan.suggestion_summary();
            conn_update_publish_item_error(&conn, &item, "CATEGORY_ATTRS_NEED_AI_FILL", &summary)?;
            upsert_notification(
                &conn,
                "warning",
                "publish_item",
                &item.item_id,
                Some(&item.shop_id),
                "AI 已生成铺货属性建议，仍需人工确认",
                &summary,
                Some(&serde_json::json!({
                    "job_id": &item.job_id,
                    "item_id": &item.item_id,
                    "external_product_id": &item.external_product_id,
                    "suggestions": attribute_suggestions_json(&plan.suggestions)
                })),
            )?;
            suggestion_only_items += 1;
        }
        recompute_publish_job(&conn, &item.job_id)?;
    }

    Ok(PublishAttributeFillBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items: auto_filled_items + suggestion_only_items + failed_items,
        auto_filled_items,
        suggestion_only_items,
        failed_items,
        generated_suggestions,
    })
}

#[tauri::command]
pub fn list_publish_attribute_suggestions(
    app: AppHandle,
    status: Option<String>,
    limit: Option<i64>,
    job_id: Option<String>,
    item_id: Option<String>,
) -> AppResult<PublishAttributeSuggestionListResult> {
    let conn = open_connection(&app)?;
    let status = normalize_attribute_suggestion_status(status.as_deref())?;
    let limit = limit.unwrap_or(200).clamp(1, 500);
    let job_id = normalize_optional_filter(job_id);
    let item_id = normalize_optional_filter(item_id);
    let items = load_publish_attribute_suggestion_views(
        &conn,
        status,
        job_id.as_deref(),
        item_id.as_deref(),
        limit,
    )?;
    let total = count_attribute_suggestions_by_status(
        &conn,
        status,
        job_id.as_deref(),
        item_id.as_deref(),
    )?;
    let pending_count = count_attribute_suggestions_by_status(
        &conn,
        "pending",
        job_id.as_deref(),
        item_id.as_deref(),
    )?;
    let applied_count = count_attribute_suggestions_by_status(
        &conn,
        "applied",
        job_id.as_deref(),
        item_id.as_deref(),
    )?;
    Ok(PublishAttributeSuggestionListResult {
        items,
        total,
        pending_count,
        applied_count,
    })
}

#[tauri::command]
pub fn apply_publish_attribute_suggestions(
    app: AppHandle,
    request: PublishAttributeSuggestionApplyRequest,
) -> AppResult<PublishAttributeSuggestionApplyResult> {
    let mut suggestion_ids = request
        .suggestion_ids
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    suggestion_ids.sort();
    suggestion_ids.dedup();
    if suggestion_ids.is_empty() {
        return Err(AppError::Validation("请选择要采纳的属性建议".to_string()));
    }
    if suggestion_ids.len() > 100 {
        return Err(AppError::Validation(
            "一次最多采纳 100 条属性建议".to_string(),
        ));
    }

    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    let mut grouped = BTreeMap::<String, Vec<AttributeSuggestionApplyRow>>::new();
    let mut failed_suggestions = 0i64;
    for suggestion_id in &suggestion_ids {
        match load_attribute_suggestion_for_apply(&tx, suggestion_id)? {
            Some(row) if row.applied => {}
            Some(row) => {
                grouped
                    .entry(row.item.item_id.clone())
                    .or_default()
                    .push(row);
            }
            None => {
                failed_suggestions += 1;
            }
        }
    }

    let mut applied_suggestions = 0i64;
    let mut updated_items = 0i64;
    let mut job_ids = BTreeSet::new();

    for rows in grouped.values() {
        let first = rows
            .first()
            .ok_or_else(|| AppError::Validation("属性建议分组为空".to_string()))?;
        job_ids.insert(first.item.job_id.clone());
        let product = match serde_json::from_str::<ExternalProductInput>(&first.item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                failed_suggestions += rows.len() as i64;
                insert_task_log(
                    &tx,
                    &first.item.job_id,
                    Some(&first.item.item_id),
                    "error",
                    &format!("人工采纳属性建议失败：商品原始数据无法解析：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let mut draft = match resolve_add_product_base_payload(&product) {
            Ok(draft) => draft,
            Err(error) => {
                failed_suggestions += rows.len() as i64;
                insert_task_log(
                    &tx,
                    &first.item.job_id,
                    Some(&first.item.item_id),
                    "error",
                    &format!("人工采纳属性建议失败：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let suggestions = rows
            .iter()
            .filter_map(|row| match row.to_attribute_fill_suggestion() {
                Ok(suggestion) => Some(suggestion),
                Err(error) => {
                    let _ = insert_task_log(
                        &tx,
                        &row.item.job_id,
                        Some(&row.item.item_id),
                        "error",
                        &format!("人工采纳属性建议失败：{error}"),
                        None,
                    );
                    failed_suggestions += 1;
                    None
                }
            })
            .collect::<Vec<_>>();
        if suggestions.is_empty() {
            continue;
        }
        let plan = AttributeFillPlan { suggestions };
        apply_attribute_fill_plan_to_payload(&mut draft.payload, &plan)?;
        persist_filled_add_product_payload(&tx, &first.item, &draft.payload, &plan)?;

        for row in rows {
            tx.execute(
                "UPDATE publish_attribute_suggestions
                 SET applied = 1, updated_at = ?1
                 WHERE id = ?2",
                params![now_shanghai(), row.id.as_str()],
            )?;
        }
        applied_suggestions += rows.len() as i64;
        updated_items += 1;

        let summary = format!(
            "已人工采纳 {} 条属性建议，等待重新执行微信类目预检",
            rows.len()
        );
        let next_summary =
            if let Some(cat_id) = extract_leaf_category_id_from_payload(&draft.payload) {
                match check_cached_category_requirements(
                    &tx,
                    &first.item.shop_id,
                    cat_id,
                    &draft.payload,
                ) {
                    Ok(check) if check.has_missing_attrs() => Some(format!(
                        "已人工采纳 {} 条属性建议，仍{}",
                        rows.len(),
                        check.failure_summary()
                    )),
                    Ok(_) => None,
                    Err(error) => Some(format!(
                        "已人工采纳 {} 条属性建议，但重新校验类目属性失败：{}",
                        rows.len(),
                        error
                    )),
                }
            } else {
                Some(format!(
                    "已人工采纳 {} 条属性建议，但发品草稿缺少微信叶子类目 ID",
                    rows.len()
                ))
            };
        if let Some(error_summary) = next_summary {
            conn_update_publish_item_error(
                &tx,
                &first.item,
                "CATEGORY_ATTRS_NEED_AI_FILL",
                &error_summary,
            )?;
            upsert_notification(
                &tx,
                "warning",
                "publish_item",
                &first.item.item_id,
                Some(&first.item.shop_id),
                "铺货属性建议已部分采纳，仍需确认",
                &error_summary,
                Some(&serde_json::json!({
                    "job_id": &first.item.job_id,
                    "item_id": &first.item.item_id,
                    "external_product_id": &first.item.external_product_id
                })),
            )?;
        } else {
            set_publish_item_status_in_conn(
                &tx,
                &first.item,
                "ready_to_publish",
                None,
                Some(&summary),
            )?;
        }
        insert_task_log(
            &tx,
            &first.item.job_id,
            Some(&first.item.item_id),
            "info",
            &summary,
            Some(&serde_json::json!({
                "suggestion_ids": rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>()
            })),
        )?;
    }

    for job_id in &job_ids {
        recompute_publish_job(&tx, job_id)?;
    }
    tx.commit()?;

    Ok(PublishAttributeSuggestionApplyResult {
        processed_suggestions: suggestion_ids.len() as i64,
        processed_items: grouped.len() as i64,
        applied_suggestions,
        updated_items,
        failed_suggestions,
        message: format!(
            "已采纳 {} 条建议，更新 {} 个铺货项",
            applied_suggestions, updated_items
        ),
    })
}

#[tauri::command]
pub async fn run_publish_category_prechecks_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<PublishCategoryPrecheckBatchResult> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let ready_items = {
        let conn = open_connection(&app)?;
        load_category_precheck_items(&conn, limit)?
    };
    if ready_items.is_empty() {
        return Ok(PublishCategoryPrecheckBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            passed_items: 0,
            failed_items: 0,
            skipped_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = ready_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut passed_items = 0i64;
    let mut failed_items = 0i64;
    let skipped_items = 0i64;

    for item in ready_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
        }
        set_publish_item_status(
            &app,
            &item,
            "category_prechecking",
            None,
            Some("正在执行微信发品前类目预检"),
        )?;
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始微信发品前类目预检",
            None,
        )?;

        let product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "INVALID_PRODUCT_PAYLOAD",
                    &format!("商品原始数据无法解析：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };
        let draft = match resolve_add_product_base_payload(&product) {
            Ok(draft) => draft,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "WECHAT_PAYLOAD_NEEDS_AI_FILL",
                    &error,
                )?;
                failed_items += 1;
                continue;
            }
        };
        let Some(cat_id) = extract_leaf_category_id_from_payload(&draft.payload) else {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                "MISSING_WECHAT_LEAF_CATEGORY_ID",
                "微信发品参数缺少有效叶子类目 cat_id，无法调用 categoryprecheck",
            )?;
            failed_items += 1;
            continue;
        };

        {
            let conn = open_connection(&app)?;
            let requirement_check =
                check_cached_category_requirements(&conn, &item.shop_id, cat_id, &draft.payload)?;
            if !requirement_check.detail_found {
                insert_task_log(
                    &conn,
                    &item.job_id,
                    Some(&item.item_id),
                    "warn",
                    "本地未缓存该店铺类目详情，已跳过必填属性本地校验；建议先同步类目规则",
                    Some(&serde_json::json!({
                        "shop_id": item.shop_id,
                        "cat_id": cat_id,
                        "external_product_id": item.external_product_id
                    })),
                )?;
            } else if requirement_check.has_missing_attrs() {
                let summary = requirement_check.failure_summary();
                mark_publish_item_failed(&conn, &item, "CATEGORY_ATTRS_NEED_AI_FILL", &summary)?;
                failed_items += 1;
                continue;
            }
        }

        if item.shop_status != "active" {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                "SHOP_NOT_ACTIVE",
                &format!("店铺状态为 {}，不能执行微信发品前预检", item.shop_status),
            )?;
            failed_items += 1;
            continue;
        }
        if !item.shop_has_secret {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                "SHOP_SECRET_MISSING",
                "店铺未保存 app_secret，不能执行微信发品前预检",
            )?;
            failed_items += 1;
            continue;
        }

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let precheck_call = match client.category_precheck(&access_token, Some(cat_id)).await {
            Ok(call) => call,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "WECHAT_CATEGORY_PRECHECK_HTTP_FAILED",
                    &format!("微信 categoryprecheck 请求失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        match &precheck_call.result {
            WechatCallResult::Success(result) => {
                let conn = open_connection(&app)?;
                upsert_category_precheck_result(
                    &conn,
                    &item.shop_id,
                    cat_id,
                    result.all_pass,
                    &result.fail_reasons,
                    &result.raw_payload,
                )?;
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    precheck_call.meta.endpoint,
                    precheck_call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "categoryprecheck cat_id={}, all_pass={}",
                        cat_id, result.all_pass
                    )),
                )?;
                if result.all_pass {
                    set_publish_item_status(
                        &app,
                        &item,
                        "category_prechecked",
                        None,
                        Some("微信类目预检通过，等待素材上传"),
                    )?;
                    insert_task_log(
                        &conn,
                        &item.job_id,
                        Some(&item.item_id),
                        "info",
                        "微信类目预检通过，等待素材上传",
                        Some(&serde_json::json!({ "cat_id": cat_id })),
                    )?;
                    passed_items += 1;
                } else {
                    let summary = if result.fail_reasons.is_empty() {
                        "微信 categoryprecheck 返回未通过，但未给出具体原因".to_string()
                    } else {
                        format!("微信类目预检未通过：{}", result.fail_reasons.join("；"))
                    };
                    mark_publish_item_failed(
                        &conn,
                        &item,
                        "WECHAT_CATEGORY_PRECHECK_FAILED",
                        &summary,
                    )?;
                    failed_items += 1;
                }
            }
            WechatCallResult::ApiError(error) => {
                let conn = open_connection(&app)?;
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    precheck_call.meta.endpoint,
                    precheck_call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("categoryprecheck api error"),
                )?;
                drop(conn);
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    &format!("WECHAT_CATEGORY_PRECHECK_{}", error.errcode),
                    &format!("微信 categoryprecheck 失败：{}", error.errmsg),
                )?;
                failed_items += 1;
            }
        }
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_publish_job(&conn, job_id)?;
    }

    Ok(PublishCategoryPrecheckBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        passed_items,
        failed_items,
        skipped_items,
    })
}

#[tauri::command]
pub async fn run_publish_asset_uploads_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<AssetUploadBatchResult> {
    let limit = limit.unwrap_or(10).clamp(1, 50);
    let ready_items = {
        let conn = open_connection(&app)?;
        load_asset_upload_items(&conn, limit)?
    };
    if ready_items.is_empty() {
        return Ok(AssetUploadBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            uploaded_assets: 0,
            reused_assets: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = ready_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut uploaded_assets = 0i64;
    let mut reused_assets = 0i64;
    let mut failed_items = 0i64;

    for item in ready_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
        }
        set_publish_item_status(
            &app,
            &item,
            "asset_uploading",
            None,
            Some("正在上传微信商品素材"),
        )?;
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始上传微信商品素材",
            None,
        )?;

        let product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "INVALID_PRODUCT_PAYLOAD",
                    &format!("商品原始数据无法解析：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let assets = collect_product_assets(&product);
        if assets.is_empty() {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                "PRODUCT_ASSETS_EMPTY",
                "商品素材为空，无法进入微信发品",
            )?;
            failed_items += 1;
            continue;
        }

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let mut item_failed = false;
        for asset in assets {
            match upload_or_reuse_asset(&app, &client, &access_token, &item, &asset).await? {
                AssetUploadOutcome::Uploaded => uploaded_assets += 1,
                AssetUploadOutcome::Reused => reused_assets += 1,
                AssetUploadOutcome::Failed => {
                    failed_items += 1;
                    item_failed = true;
                    break;
                }
            }
        }

        if item_failed {
            continue;
        }

        set_publish_item_status(
            &app,
            &item,
            "assets_ready",
            None,
            Some("微信素材上传完成，等待 addproduct"),
        )?;
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "微信素材上传完成，等待 addproduct",
            None,
        )?;
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_publish_job(&conn, job_id)?;
    }

    Ok(AssetUploadBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        uploaded_assets,
        reused_assets,
        failed_items,
    })
}

#[tauri::command]
pub async fn run_publish_submits_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<ProductSubmitBatchResult> {
    let limit = limit.unwrap_or(10).clamp(1, 50);
    let submit_items = {
        let conn = open_connection(&app)?;
        load_product_submit_items(&conn, limit)?
    };
    if submit_items.is_empty() {
        return Ok(ProductSubmitBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            submitted_items: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = submit_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut submitted_items = 0i64;
    let mut failed_items = 0i64;

    for item in submit_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
        }
        set_publish_item_status(
            &app,
            &item,
            "publishing",
            None,
            Some("正在提交微信 addproduct"),
        )?;
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始提交微信 addproduct",
            None,
        )?;

        let product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "INVALID_PRODUCT_PAYLOAD",
                    &format!("商品原始数据无法解析：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let assets = load_prepared_assets(&app, &item.item_id)?;
        let payload = match build_add_product_payload(&product, &assets) {
            Ok(payload) => payload,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "MISSING_WECHAT_PRODUCT_PAYLOAD",
                    &error,
                )?;
                failed_items += 1;
                continue;
            }
        };

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let call = match client.add_product(&access_token, &payload).await {
            Ok(call) => call,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    "WECHAT_ADDPRODUCT_HTTP_FAILED",
                    &format!("微信 addproduct 请求失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(result) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!("addproduct ok, product_id={}", result.product_id)),
                )?;
                conn.execute(
                    "UPDATE publish_job_items
                     SET status = 'submitted',
                         error_code = NULL,
                         error_summary = '微信 addproduct 已提交，等待审核状态同步',
                         wechat_product_id = ?1
                     WHERE id = ?2",
                    params![result.product_id, item.item_id],
                )?;
                let source_url = conn
                    .query_row(
                        "SELECT source_url FROM publish_products WHERE id = ?1",
                        [item.product_row_id.as_str()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                conn.execute(
                    "INSERT INTO shop_products
                     (id, shop_id, external_product_id, source_url, wechat_product_id, status, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'submitted', ?6)
                     ON CONFLICT(shop_id, external_product_id) DO UPDATE SET
                       source_url = COALESCE(excluded.source_url, shop_products.source_url),
                       wechat_product_id = excluded.wechat_product_id,
                       status = 'submitted'",
                    params![
                        format!("shop-product-{}", Uuid::new_v4()),
                        item.shop_id,
                        item.external_product_id,
                        source_url,
                        result.product_id,
                        now_shanghai()
                    ],
                )?;
                insert_task_log(
                    &conn,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    "微信 addproduct 已提交，等待审核状态同步",
                    Some(&serde_json::json!({
                        "wechat_product_id": result.product_id
                    })),
                )?;
                submitted_items += 1;
            }
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("addproduct api error"),
                )?;
                drop(conn);
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    &format!("WECHAT_ADDPRODUCT_{}", error.errcode),
                    &format!("微信 addproduct 失败：{}", error.errmsg),
                )?;
                failed_items += 1;
            }
        }
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_publish_job(&conn, job_id)?;
    }

    Ok(ProductSubmitBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        submitted_items,
        failed_items,
    })
}

#[tauri::command]
pub async fn run_publish_status_sync_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<ProductStatusSyncBatchResult> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let sync_items = {
        let conn = open_connection(&app)?;
        load_product_status_sync_items(&conn, limit)?
    };
    if sync_items.is_empty() {
        return Ok(ProductStatusSyncBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            success_items: 0,
            pending_items: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = sync_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut success_items = 0i64;
    let mut pending_items = 0i64;
    let mut failed_items = 0i64;

    for item in sync_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
        }
        set_publish_status_sync_state(
            &app,
            &item,
            "audit_pending",
            None,
            "正在同步微信审核/商品状态",
            None,
            None,
        )?;
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始同步微信审核/商品状态",
            Some(&serde_json::json!({
                "wechat_product_id": item.wechat_product_id
            })),
        )?;

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                set_publish_status_sync_state(
                    &app,
                    &item,
                    "audit_pending",
                    Some("ACCESS_TOKEN_FAILED"),
                    &format!("获取 access_token 失败，稍后可重试状态同步：{error}"),
                    None,
                    None,
                )?;
                pending_items += 1;
                continue;
            }
        };

        let call = match client
            .get_product(&access_token, &item.wechat_product_id, 3)
            .await
        {
            Ok(call) => call,
            Err(error) => {
                set_publish_status_sync_state(
                    &app,
                    &item,
                    "audit_pending",
                    Some("WECHAT_GETPRODUCT_HTTP_FAILED"),
                    &format!("微信 getproduct 请求失败，稍后可重试：{error}"),
                    None,
                    None,
                )?;
                pending_items += 1;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(info) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "getproduct ok, product_id={}",
                        item.wechat_product_id
                    )),
                )?;
                drop(conn);

                let resolution = resolve_wechat_product_status(info);
                match resolution.status {
                    "success" | "audit_passed" => success_items += 1,
                    "failed" => failed_items += 1,
                    _ => pending_items += 1,
                }
                set_publish_status_sync_state(
                    &app,
                    &item,
                    resolution.status,
                    resolution.error_code.as_deref(),
                    &resolution.summary,
                    resolution.wechat_status,
                    resolution.wechat_edit_status,
                )?;
                insert_task_log_for_app(
                    &app,
                    &item.job_id,
                    Some(&item.item_id),
                    if resolution.status == "failed" {
                        "error"
                    } else {
                        "info"
                    },
                    &resolution.summary,
                    Some(&serde_json::json!({
                        "wechat_product_id": item.wechat_product_id,
                        "wechat_status": resolution.wechat_status,
                        "wechat_edit_status": resolution.wechat_edit_status
                    })),
                )?;
            }
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("getproduct api error"),
                )?;
                drop(conn);
                set_publish_status_sync_state(
                    &app,
                    &item,
                    "audit_pending",
                    Some(&format!("WECHAT_GETPRODUCT_{}", error.errcode)),
                    &format!("微信 getproduct 返回错误，稍后可重试：{}", error.errmsg),
                    None,
                    None,
                )?;
                pending_items += 1;
            }
        }
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_publish_job(&conn, job_id)?;
    }

    Ok(ProductStatusSyncBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        success_items,
        pending_items,
        failed_items,
    })
}

#[tauri::command]
pub async fn run_publish_listing_once(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<ProductListingBatchResult> {
    let limit = limit.unwrap_or(10).clamp(1, 50);
    let listing_items = {
        let conn = open_connection(&app)?;
        load_product_listing_items(&conn, limit)?
    };
    if listing_items.is_empty() {
        return Ok(ProductListingBatchResult {
            processed_jobs: 0,
            processed_items: 0,
            listing_submitted_items: 0,
            failed_items: 0,
        });
    }

    let client = WechatShopClient::default();
    let processed_items = listing_items.len() as i64;
    let mut job_ids = BTreeSet::new();
    let mut listing_submitted_items = 0i64;
    let mut failed_items = 0i64;

    for item in listing_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            conn.execute(
                "UPDATE task_runs
                 SET status = 'running', started_at = COALESCE(started_at, ?1), finished_at = NULL
                 WHERE id = ?2",
                params![now_shanghai(), item.job_id.as_str()],
            )?;
        }
        set_shop_product_item_state(
            &app,
            &item,
            "listing",
            None,
            "正在调用微信 listingproduct 上架商品",
        )?;
        insert_task_log_for_app(
            &app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "开始调用微信 listingproduct 上架商品",
            Some(&serde_json::json!({
                "wechat_product_id": item.wechat_product_id
            })),
        )?;

        let access_token = match ensure_access_token(&app, &item.shop_id, &client).await {
            Ok(access_token) => access_token,
            Err(error) => {
                mark_listing_item_failed(
                    &app,
                    &item,
                    "ACCESS_TOKEN_FAILED",
                    &format!("获取 access_token 失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let call = match client
            .listing_product(&access_token, &item.wechat_product_id)
            .await
        {
            Ok(call) => call,
            Err(error) => {
                mark_listing_item_failed(
                    &app,
                    &item,
                    "WECHAT_LISTINGPRODUCT_HTTP_FAILED",
                    &format!("微信 listingproduct 请求失败：{error}"),
                )?;
                failed_items += 1;
                continue;
            }
        };

        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(_) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "listingproduct ok, product_id={}",
                        item.wechat_product_id
                    )),
                )?;
                drop(conn);
                set_shop_product_item_state(
                    &app,
                    &item,
                    "audit_pending",
                    None,
                    "微信 listingproduct 已提交，等待 getproduct 确认已上架",
                )?;
                insert_task_log_for_app(
                    &app,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    "微信 listingproduct 已提交，等待 getproduct 确认已上架",
                    Some(&serde_json::json!({
                        "wechat_product_id": item.wechat_product_id
                    })),
                )?;
                listing_submitted_items += 1;
            }
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("listingproduct api error"),
                )?;
                drop(conn);
                mark_listing_item_failed(
                    &app,
                    &item,
                    &format!("WECHAT_LISTINGPRODUCT_{}", error.errcode),
                    &format!("微信 listingproduct 失败：{}", error.errmsg),
                )?;
                failed_items += 1;
            }
        }
    }

    for job_id in &job_ids {
        let conn = open_connection(&app)?;
        recompute_publish_job(&conn, job_id)?;
    }

    Ok(ProductListingBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        listing_submitted_items,
        failed_items,
    })
}
