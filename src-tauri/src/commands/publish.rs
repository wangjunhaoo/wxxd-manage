use super::*;
use crate::wechat::{ProductAddCall, WechatApiError};

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

    for mut item in pending_items {
        job_ids.insert(item.job_id.clone());
        // 每个 item 独立事务做毒丸隔离：单个商品的类目/payload 错误只回滚自己，
        // 不会像之前共用一个 tx 那样让整批 target 全部回滚、永远卡在 precheck。
        match run_precheck_one_item(&mut conn, &mut item) {
            Ok(true) => ready_items += 1,
            Ok(false) => failed_items += 1,
            Err(error) => {
                failed_items += 1;
                // 处理中发生未预期错误：单独标记该 item 失败（含错误详情便于定位），不波及其他 item
                if let Err(mark_err) = block_target(
                    &conn,
                    &item.item_id,
                    "PRECHECK_UNEXPECTED_ERROR",
                    &format!("前置校验异常：{error}"),
                ) {
                    eprintln!("标记 precheck 失败 item 出错：{mark_err}");
                }
            }
        }
    }

    // 统一重算受影响商品的聚合状态
    for job_id in &job_ids {
        if let Err(error) = recompute_pipeline_product(&conn, job_id) {
            eprintln!("precheck 后重算商品聚合状态出错：{error}");
        }
    }

    Ok(PublishTaskBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items: ready_items + failed_items,
        ready_items,
        failed_items,
    })
}

/// 单个 item 的本地前置校验，使用独立事务：成功返回 Ok(true)；业务校验失败返回 Ok(false)
/// 并已在事务内标记失败；处理过程中的未预期错误以 Err 返回（事务回滚），由调用方单独隔离标记。
fn run_precheck_one_item(conn: &mut Connection, item: &mut PendingPublishItem) -> AppResult<bool> {
    let tx = conn.transaction()?;
    mark_target_running(&tx, &item.item_id)?;
    insert_task_log(
        &tx,
        &item.job_id,
        Some(&item.item_id),
        "info",
        "开始本地前置校验",
        None,
    )?;

    let mut product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
        Ok(product) => product,
        Err(error) => {
            mark_publish_item_failed(
                &tx,
                item,
                "INVALID_PRODUCT_PAYLOAD",
                &format!("商品原始数据无法解析：{error}"),
            )?;
            tx.commit()?;
            return Ok(false);
        }
    };

    if let Some(summary) = ensure_wechat_category_metadata(&tx, item, &mut product)? {
        insert_task_log(&tx, &item.job_id, Some(&item.item_id), "info", &summary, None)?;
    }

    if let Some((code, summary)) = precheck_publish_item(&tx, item, &product)? {
        mark_publish_item_failed(&tx, item, code, &summary)?;
        tx.commit()?;
        return Ok(false);
    }

    let payload_prepare = match prepare_add_product_payload_for_publish(&tx, item, &product)? {
        Ok(payload_prepare) => payload_prepare,
        Err((code, summary)) => {
            mark_publish_item_failed(&tx, item, code, &summary)?;
            tx.commit()?;
            return Ok(false);
        }
    };
    let ready_summary = publish_payload_ready_summary(&payload_prepare);
    advance_target(&tx, &item.item_id, target_stage::ATTR_FILL)?;
    insert_task_log(
        &tx,
        &item.job_id,
        Some(&item.item_id),
        "info",
        &ready_summary,
        None,
    )?;
    recompute_pipeline_product(&tx, &item.job_id)?;
    tx.commit()?;
    Ok(true)
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

    for mut item in items {
        job_ids.insert(item.job_id.clone());
        mark_target_running(&tx, &item.item_id)?;
        let mut product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                failed_items += 1;
                block_target(
                    &tx,
                    &item.item_id,
                    "INVALID_PRODUCT_PAYLOAD",
                    &format!("属性补齐失败：商品原始数据无法解析：{error}"),
                )?;
                continue;
            }
        };
        let inferred_category = ensure_wechat_category_metadata(&tx, &mut item, &mut product)?;
        if let Some(summary) = &inferred_category {
            insert_task_log(
                &tx,
                &item.job_id,
                Some(&item.item_id),
                "info",
                summary,
                None,
            )?;
        }
        let mut draft = match resolve_add_product_base_payload_relaxed_after_sale(&product) {
            Ok(draft) => draft,
            Err(error) => {
                failed_items += 1;
                conn_update_publish_item_error(
                    &tx,
                    &item,
                    add_product_payload_error_code(&error),
                    &error,
                )?;
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
            block_target(
                &tx,
                &item.item_id,
                "MISSING_WECHAT_LEAF_CATEGORY_ID",
                "属性补齐失败：缺少微信叶子类目 ID",
            )?;
            continue;
        };
        let Some(raw_detail) = load_cached_category_detail_payload(&tx, &item.shop_id, cat_id)?
        else {
            if inferred_category.is_some() {
                persist_generated_add_product_payload(&tx, &item, &draft)?;
                let summary = "已补齐微信类目，等待执行微信类目预检";
                advance_target(&tx, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
                insert_task_log(
                    &tx,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    summary,
                    Some(&serde_json::json!({ "cat_id": cat_id })),
                )?;
                auto_filled_items += 1;
                continue;
            }
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
            advance_target(&tx, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
            auto_filled_items += 1;
            continue;
        }

        // 优先使用审查阶段产出的 ai_attr_suggestions（零 AI 调用）
        let review_filled = apply_review_ai_attr_suggestions(
            &tx,
            &item,
            &product,
            &mut draft.payload,
            &raw_detail,
            &requirement_check,
        )?;
        // 重新检查：apply 可能已补齐部分属性
        let remaining_check =
            check_cached_category_requirements(&tx, &item.shop_id, cat_id, &draft.payload)?;
        if !remaining_check.has_missing_attrs() {
            let filled_count = review_filled.unwrap_or(0);
            let summary = if filled_count > 0 {
                format!("已从审查阶段 AI 建议中补齐 {filled_count} 个必填属性，等待重新执行微信类目预检")
            } else {
                "必填属性已完整，等待重新执行微信类目预检".to_string()
            };
            advance_target(&tx, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
            insert_task_log(
                &tx,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &summary,
                Some(&serde_json::json!({
                    "cat_id": cat_id,
                    "source": "review_ai_attr_suggestions"
                })),
            )?;
            auto_filled_items += 1;
            continue;
        }

        // 本地推断 + 审查数据兜底：只自动应用高置信且完整的建议。
        let plan = build_attribute_fill_plan(
            &item,
            &product,
            &draft.payload,
            &raw_detail,
            &remaining_check,
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
            advance_target(&tx, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
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
        recompute_pipeline_product(&tx, job_id)?;
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
    let ai_config = load_optional_ai_provider_config(&app)?;
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

    for mut item in items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            mark_target_running(&conn, &item.item_id)?;
        }
        let mut product = match serde_json::from_str::<ExternalProductInput>(&item.raw_payload) {
            Ok(product) => product,
            Err(error) => {
                failed_items += 1;
                {
                    let conn = open_connection(&app)?;
                    block_target(
                        &conn,
                        &item.item_id,
                        "INVALID_PRODUCT_PAYLOAD",
                        &format!("类目/属性补齐失败：商品原始数据无法解析：{error}"),
                    )?;
                }
                continue;
            }
        };
        let inferred_category = {
            let conn = open_connection(&app)?;
            ensure_wechat_category_metadata(&conn, &mut item, &mut product)?
        };
        if let Some(summary) = &inferred_category {
            insert_task_log_for_app(
                &app,
                &item.job_id,
                Some(&item.item_id),
                "info",
                summary,
                None,
            )?;
        }
        let mut draft = match resolve_add_product_base_payload_relaxed_after_sale(&product) {
            Ok(draft) => draft,
            Err(error) => {
                failed_items += 1;
                let conn = open_connection(&app)?;
                conn_update_publish_item_error(
                    &conn,
                    &item,
                    add_product_payload_error_code(&error),
                    &error,
                )?;
                insert_task_log_for_app(
                    &app,
                    &item.job_id,
                    Some(&item.item_id),
                    "error",
                    &format!("类目/属性补齐失败：{error}"),
                    None,
                )?;
                continue;
            }
        };
        let Some(cat_id) = extract_leaf_category_id_from_payload(&draft.payload) else {
            failed_items += 1;
            {
                let conn = open_connection(&app)?;
                block_target(
                    &conn,
                    &item.item_id,
                    "MISSING_WECHAT_LEAF_CATEGORY_ID",
                    "类目/属性补齐失败：缺少微信叶子类目 ID",
                )?;
            }
            continue;
        };
        let raw_detail = {
            let conn = open_connection(&app)?;
            load_cached_category_detail_payload(&conn, &item.shop_id, cat_id)?
        };
        let Some(raw_detail) = raw_detail else {
            if inferred_category.is_some() {
                let conn = open_connection(&app)?;
                persist_generated_add_product_payload(&conn, &item, &draft)?;
                let summary = "已补齐微信类目，等待执行微信类目预检";
                advance_target(&conn, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
                insert_task_log(
                    &conn,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    summary,
                    Some(&serde_json::json!({ "cat_id": cat_id })),
                )?;
                recompute_pipeline_product(&conn, &item.job_id)?;
                auto_filled_items += 1;
                continue;
            }
            // 类目详情未缓存且无新推断类目：标记阻塞并退避，避免 target 停留在 running 空转重试
            failed_items += 1;
            {
                let conn = open_connection(&app)?;
                block_target(
                    &conn,
                    &item.item_id,
                    "MISSING_WECHAT_LEAF_CATEGORY_ID",
                    "类目/属性补齐失败：缺少微信类目详情缓存，请先同步该店类目后重试",
                )?;
            }
            continue;
        };
        let requirement_check = {
            let conn = open_connection(&app)?;
            check_cached_category_requirements(&conn, &item.shop_id, cat_id, &draft.payload)?
        };
        if !requirement_check.has_missing_attrs() {
            let conn = open_connection(&app)?;
            advance_target(&conn, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
            recompute_pipeline_product(&conn, &item.job_id)?;
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
        let requires_ai = plan
            .suggestions
            .iter()
            .any(|suggestion| !suggestion.applied && suggestion.source == "needs_ai");
        let mut agent_run_id = None::<String>;
        let mut agent_started = None::<std::time::Instant>;
        if requires_ai {
            let input_snapshot = serde_json::json!({
                "job_id": &item.job_id,
                "item_id": &item.item_id,
                "shop_id": &item.shop_id,
                "external_product_id": &item.external_product_id,
                "cat_id": cat_id,
                "missing_product_attrs": &requirement_check.missing_product_attrs,
                "missing_sale_attrs": &requirement_check.missing_sale_attrs,
                "suggestions": attribute_suggestions_json(&plan.suggestions)
            });
            let input_summary = format!(
                "属性建议 item_id={} 商品属性缺失={} 销售属性缺失={}",
                item.item_id,
                requirement_check.missing_product_attrs.len(),
                requirement_check.missing_sale_attrs.len()
            );
            let conn = open_connection(&app)?;
            agent_run_id = Some(create_agent_run_for_skill(
                &conn,
                &ATTRIBUTE_SUGGESTION_SKILL,
                "publish_attribute",
                "publish_item",
                &item.item_id,
                Some(&item.shop_id),
                ai_config.as_ref(),
                &input_summary,
                Some(&input_snapshot),
            )?);
            agent_started = Some(std::time::Instant::now());
        }
        let ai_generated = if let Some(ai_config) = ai_config.as_ref() {
            // AI 属性补齐调用加超时：同审查，provider 偶发 hang 不该挂死整轮 tick；
            // 120s 内未返回按失败处理，走下方 Err 分支(标失败+退避)。
            let ai_call_result = match tokio::time::timeout(
                std::time::Duration::from_secs(120),
                fill_attribute_plan_with_ai(&app, ai_config, &mut plan),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(AppError::Validation(
                    "AI 属性补齐调用超时(120s)".to_string(),
                )),
            };
            match ai_call_result {
                Ok(count) => {
                    if let Some(run_id) = agent_run_id.as_deref() {
                        let conn = open_connection(&app)?;
                        let output = attribute_suggestions_json(&plan.suggestions);
                        finish_agent_run(
                            &conn,
                            run_id,
                            AgentRunFinish {
                                status: if plan.can_auto_apply() {
                                    "succeeded"
                                } else {
                                    "needs_review"
                                },
                                output: None,
                                validated_output: Some(&output),
                                tool_calls: None,
                                decision: Some(if plan.can_auto_apply() {
                                    "ready_to_publish"
                                } else {
                                    "needs_review"
                                }),
                                error_code: None,
                                error_summary: None,
                                duration_ms: agent_started.map(|started| {
                                    started.elapsed().as_millis().min(i64::MAX as u128) as i64
                                }),
                            },
                        )?;
                    }
                    count
                }
                Err(error) => {
                    if let Some(run_id) = agent_run_id.as_deref() {
                        let conn = open_connection(&app)?;
                        let error_summary = error.to_string();
                        finish_agent_run(
                            &conn,
                            run_id,
                            AgentRunFinish {
                                status: "failed",
                                output: None,
                                validated_output: Some(&attribute_suggestions_json(
                                    &plan.suggestions,
                                )),
                                tool_calls: None,
                                decision: Some("failed"),
                                error_code: Some(agent_error_code(&error)),
                                error_summary: Some(&error_summary),
                                duration_ms: agent_started.map(|started| {
                                    started.elapsed().as_millis().min(i64::MAX as u128) as i64
                                }),
                            },
                        )?;
                    }
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
            }
        } else {
            if let Some(run_id) = agent_run_id.as_deref() {
                let conn = open_connection(&app)?;
                finish_agent_run(
                    &conn,
                    run_id,
                    AgentRunFinish {
                        status: "blocked",
                        output: None,
                        validated_output: Some(&attribute_suggestions_json(&plan.suggestions)),
                        tool_calls: None,
                        decision: Some("needs_review"),
                        error_code: Some("PROVIDER_NOT_CONFIGURED"),
                        error_summary: Some("未启用 AI provider，属性建议仅保留为人工确认项"),
                        duration_ms: agent_started.map(|started| {
                            started.elapsed().as_millis().min(i64::MAX as u128) as i64
                        }),
                    },
                )?;
            }
            0
        };
        generated_suggestions += ai_generated;

        let conn = open_connection(&app)?;
        for suggestion in &plan.suggestions {
            upsert_publish_attribute_suggestion(&conn, &item, suggestion)?;
        }
        if plan.can_auto_apply() {
            apply_attribute_fill_plan_to_payload(&mut draft.payload, &plan)?;
            persist_filled_add_product_payload(&conn, &item, &draft.payload, &plan)?;
            let summary = if ai_generated > 0 {
                format!(
                    "AI 已生成并自动补齐 {} 个必填属性，等待重新执行微信类目预检",
                    ai_generated
                )
            } else {
                "已按本地规则补齐必填属性，等待重新执行微信类目预检".to_string()
            };
            advance_target(&conn, &item.item_id, target_stage::CATEGORY_PRECHECK)?;
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
                "铺货属性建议仍需人工确认",
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
        recompute_pipeline_product(&conn, &item.job_id)?;
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

async fn ensure_category_detail_payload_for_publish(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
    cat_id: i64,
) -> AppResult<Result<Value, (String, String)>> {
    {
        let conn = open_connection(app)?;
        if let Some(raw_detail) = load_cached_category_detail_payload(&conn, &item.shop_id, cat_id)?
        {
            return Ok(Ok(raw_detail));
        }
        insert_task_log(
            &conn,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "本地未缓存该店铺类目详情，开始同步微信类目详情",
            Some(&serde_json::json!({
                "shop_id": item.shop_id,
                "cat_id": cat_id,
                "external_product_id": item.external_product_id
            })),
        )?;
    }

    let detail_call = match client.get_category_detail(access_token, cat_id).await {
        Ok(call) => call,
        Err(error) => {
            return Ok(Err((
                "CATEGORY_DETAIL_SYNC_HTTP_FAILED".to_string(),
                format!("同步微信类目详情失败：{error}"),
            )));
        }
    };

    match detail_call.result {
        WechatCallResult::Success(raw) => {
            let counts = category_detail_counts(&raw.raw_payload);
            let conn = open_connection(app)?;
            upsert_category_detail(&conn, &item.shop_id, cat_id, &raw.raw_payload, &counts)?;
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                detail_call.meta.endpoint,
                detail_call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "publish synced category detail cat_id={cat_id}, product_attrs={}, sale_attrs={}",
                    counts.product_attr_count, counts.sale_attr_count
                )),
            )?;
            insert_task_log(
                &conn,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &format!(
                    "微信类目详情同步完成：商品属性 {}，销售属性 {}，资质 {}",
                    counts.product_attr_count, counts.sale_attr_count, counts.product_qua_count
                ),
                Some(&serde_json::json!({ "cat_id": cat_id })),
            )?;
            Ok(Ok(raw.raw_payload))
        }
        WechatCallResult::ApiError(error) => {
            let conn = open_connection(app)?;
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                detail_call.meta.endpoint,
                detail_call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("publish category detail api error"),
            )?;
            Ok(Err((
                format!("WECHAT_CATEGORY_DETAIL_{}", error.errcode),
                format!("同步微信类目详情失败：{}", error.errmsg),
            )))
        }
    }
}

#[derive(Debug, Clone)]
struct AutoAfterSaleAddressSelection {
    address_id: i64,
    source: &'static str,
}

#[derive(Debug, Clone)]
struct AutoFreightTemplateSelection {
    template_id: String,
    source: &'static str,
}

async fn ensure_after_sale_address_for_publish(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
    payload: &mut Value,
    cache: &mut BTreeMap<String, AutoAfterSaleAddressSelection>,
) -> AppResult<Result<Option<AutoAfterSaleAddressSelection>, (String, String)>> {
    if payload_deliver_method(payload) != 0 || payload_after_sale_address_id(payload).is_some() {
        return Ok(Ok(None));
    }

    let selection = if let Some(selection) = cache.get(&item.shop_id) {
        selection.clone()
    } else {
        let selection =
            match resolve_default_after_sale_address_for_publish(app, client, access_token, item)
                .await?
            {
                Ok(selection) => selection,
                Err(error) => return Ok(Err(error)),
            };
        cache.insert(item.shop_id.clone(), selection.clone());
        selection
    };
    set_payload_after_sale_address_id(payload, selection.address_id)?;
    insert_task_log_for_app(
        app,
        &item.job_id,
        Some(&item.item_id),
        "info",
        "已使用微信默认售后/退货地址 ID 补齐发品参数",
        Some(&serde_json::json!({
            "address_id": selection.address_id,
            "source": selection.source
        })),
    )?;
    Ok(Ok(Some(selection)))
}

async fn ensure_freight_template_for_publish(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
    payload: &mut Value,
    cache: &mut BTreeMap<String, AutoFreightTemplateSelection>,
) -> AppResult<Result<Option<AutoFreightTemplateSelection>, (String, String)>> {
    if payload_deliver_method(payload) != 0 || payload_freight_template_id(payload).is_some() {
        return Ok(Ok(None));
    }

    let selection = if let Some(selection) = cache.get(&item.shop_id) {
        selection.clone()
    } else {
        let selection =
            match resolve_default_freight_template_for_publish(app, client, access_token, item)
                .await?
            {
                Ok(selection) => selection,
                Err(error) => return Ok(Err(error)),
            };
        cache.insert(item.shop_id.clone(), selection.clone());
        selection
    };
    set_payload_freight_template_id(payload, &selection.template_id)?;
    insert_task_log_for_app(
        app,
        &item.job_id,
        Some(&item.item_id),
        "info",
        "已使用店铺缓存运费模板 ID 补齐发品参数",
        Some(&serde_json::json!({
            "template_id": selection.template_id,
            "source": selection.source
        })),
    )?;
    Ok(Ok(Some(selection)))
}

async fn retry_add_product_with_alternative_freight_templates(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
    payload: &mut Value,
) -> AppResult<Option<ProductAddCall>> {
    let current_template_id = payload_freight_template_id(payload);
    let template_ids = {
        let conn = open_connection(app)?;
        list_cached_freight_template_ids(&conn, &item.shop_id)?
    };
    let alternatives = template_ids
        .into_iter()
        .filter(|template_id| current_template_id.as_deref() != Some(template_id.as_str()))
        .take(5)
        .collect::<Vec<_>>();
    if alternatives.is_empty() {
        return Ok(None);
    }

    let mut last_template_error = None;
    for template_id in alternatives {
        set_payload_freight_template_id(payload, &template_id)?;
        insert_task_log_for_app(
            app,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "当前运费模板被微信拒绝，自动切换同店铺其他运费模板重试 addproduct",
            Some(&serde_json::json!({
                "template_id": template_id
            })),
        )?;
        let call = match client.add_product(access_token, payload).await {
            Ok(call) => call,
            Err(error) => {
                return Err(error);
            }
        };
        if matches!(
            &call.result,
            WechatCallResult::ApiError(error) if is_freight_template_id_error(error)
        ) {
            last_template_error = Some(call);
            continue;
        }
        return Ok(Some(call));
    }

    Ok(last_template_error)
}

fn is_freight_template_id_error(error: &WechatApiError) -> bool {
    error.errmsg.contains("查询模板ID失败")
        || (error.errmsg.contains("模板") && error.errmsg.contains("ID"))
}

async fn resolve_default_after_sale_address_for_publish(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
) -> AppResult<Result<AutoAfterSaleAddressSelection, (String, String)>> {
    let list_call = match client.list_merchant_addresses(access_token, 0, 100).await {
        Ok(call) => call,
        Err(error) => {
            return Ok(Err((
                "WECHAT_ADDRESS_LIST_HTTP_FAILED".to_string(),
                format!("读取微信店铺地址列表失败：{error}"),
            )));
        }
    };

    let address_ids = match list_call.result {
        WechatCallResult::Success(result) => {
            let conn = open_connection(app)?;
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                list_call.meta.endpoint,
                list_call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "merchant address list count={}",
                    result.address_ids.len()
                )),
            )?;
            result.address_ids
        }
        WechatCallResult::ApiError(error) => {
            let conn = open_connection(app)?;
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                list_call.meta.endpoint,
                list_call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("merchant address list api error"),
            )?;
            return Ok(Err((
                format!("WECHAT_ADDRESS_LIST_{}", error.errcode),
                format!("读取微信店铺地址列表失败：{}", error.errmsg),
            )));
        }
    };

    if address_ids.is_empty() {
        return Ok(Err((
            "MISSING_AFTER_SALE_ADDRESS".to_string(),
            "快递发货必须提供售后/退货地址 ID；微信店铺地址列表为空，请先在微信小店配置退货地址，或在商品 metadata.wechat_after_sale_address_id 填入地址 ID"
                .to_string(),
        )));
    }

    let mut details = Vec::new();
    for address_id in address_ids {
        let detail_call = match client.get_merchant_address(access_token, address_id).await {
            Ok(call) => call,
            Err(error) => {
                return Ok(Err((
                    "WECHAT_ADDRESS_DETAIL_HTTP_FAILED".to_string(),
                    format!("读取微信店铺地址详情失败：{error}"),
                )));
            }
        };
        match detail_call.result {
            WechatCallResult::Success(detail) => {
                let conn = open_connection(app)?;
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    detail_call.meta.endpoint,
                    detail_call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "merchant address detail address_id={}, recv_addr={}, default_recv={}, send_addr={}, default_send={}",
                        detail.address_id,
                        detail.recv_addr,
                        detail.default_recv,
                        detail.send_addr,
                        detail.default_send
                    )),
                )?;
                details.push(detail);
            }
            WechatCallResult::ApiError(error) => {
                let conn = open_connection(app)?;
                insert_api_call_log(
                    &conn,
                    Some(&item.shop_id),
                    detail_call.meta.endpoint,
                    detail_call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("merchant address detail api error"),
                )?;
                return Ok(Err((
                    format!("WECHAT_ADDRESS_DETAIL_{}", error.errcode),
                    format!("读取微信店铺地址详情失败：{}", error.errmsg),
                )));
            }
        }
    }

    Ok(select_after_sale_address(&details).map_err(|summary| {
        let code = if summary.contains("没有可用") {
            "MISSING_AFTER_SALE_ADDRESS"
        } else {
            "AMBIGUOUS_AFTER_SALE_ADDRESS"
        };
        (
            code.to_string(),
            format!("{summary}；请在商品 metadata.wechat_after_sale_address_id 填入明确的微信售后/退货地址 ID"),
        )
    }))
}

async fn resolve_default_freight_template_for_publish(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
) -> AppResult<Result<AutoFreightTemplateSelection, (String, String)>> {
    {
        let conn = open_connection(app)?;
        if let Some(template_id) = select_cached_freight_template_id(&conn, &item.shop_id)? {
            return Ok(Ok(AutoFreightTemplateSelection {
                template_id,
                source: "cached_first",
            }));
        }
    }

    if let Err(error) =
        sync_freight_template_ids(app, client, access_token, &item.shop_id, &item.job_id).await
    {
        return Ok(Err((
            "WECHAT_FREIGHT_TEMPLATE_SYNC_FAILED".to_string(),
            format!("同步微信运费模板失败：{error}"),
        )));
    }

    let conn = open_connection(app)?;
    if let Some(template_id) = select_cached_freight_template_id(&conn, &item.shop_id)? {
        return Ok(Ok(AutoFreightTemplateSelection {
            template_id,
            source: "synced_first",
        }));
    }

    Ok(Err((
        "MISSING_FREIGHT_TEMPLATE".to_string(),
        "快递发货必须提供运费模板 ID；微信店铺没有同步到可用运费模板，请先在微信小店创建运费模板，或在商品 metadata.wechat_freight_template_id 填入模板 ID"
            .to_string(),
    )))
}

fn select_cached_freight_template_id(
    conn: &Connection,
    shop_id: &str,
) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT template_id
         FROM wechat_freight_templates
         WHERE shop_id = ?1
         ORDER BY synced_at DESC, template_id ASC
         LIMIT 1",
        [shop_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(AppError::from)
}

fn list_cached_freight_template_ids(conn: &Connection, shop_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT template_id
         FROM wechat_freight_templates
         WHERE shop_id = ?1
         ORDER BY synced_at DESC, template_id ASC",
    )?;
    let template_ids = stmt
        .query_map([shop_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(template_ids)
}

/// 从 ai_attr_suggestions 补齐当前缺失属性，并持久化发品 payload。
fn apply_review_ai_attr_suggestions_to_payload(
    conn: &Connection,
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    payload: &mut Value,
    raw_detail: &Value,
    check: &CachedCategoryRequirementCheck,
) -> AppResult<i64> {
    let suggestions = product
        .metadata
        .as_object()
        .and_then(|m| m.get("ai_attr_suggestions"))
        .and_then(Value::as_object);
    let Some(suggestions) = suggestions else {
        return Ok(0);
    };

    let mut filled = 0i64;
    let product_specs = required_category_attr_specs(raw_detail, "product_attr_list");
    let sale_specs = required_category_attr_specs(raw_detail, "sale_attr_list");
    for attr_key in &check.missing_product_attrs {
        if let Some(value) = suggestions
            .get(attr_key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            if let Some(spec) = product_specs.get(attr_key) {
                if let Some(value) = normalize_attr_value_for_category_spec(product, spec, &value) {
                    ensure_payload_product_attr(payload, attr_key, &value)?;
                    filled += 1;
                }
            }
        }
    }
    for attr_key in &check.missing_sale_attrs {
        if let Some(value) = suggestions
            .get(attr_key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            if let Some(spec) = sale_specs.get(attr_key) {
                if let Some(value) = normalize_attr_value_for_category_spec(product, spec, &value) {
                    ensure_payload_sku_attr_for_all(payload, attr_key, &value)?;
                    filled += 1;
                }
            }
        }
    }
    if filled > 0 {
        persist_filled_add_product_payload_from_suggestions(conn, item, payload)?;
    }
    Ok(filled)
}

/// 从审查阶段产出的 ai_attr_suggestions 中补齐缺失属性。
/// 返回 Some(count) 表示补齐成功（所有缺失属性均已补齐），None 表示仍有缺失。
fn apply_review_ai_attr_suggestions(
    conn: &Connection,
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    payload: &mut Value,
    raw_detail: &Value,
    check: &CachedCategoryRequirementCheck,
) -> AppResult<Option<i64>> {
    let suggestions = product
        .metadata
        .as_object()
        .and_then(|m| m.get("ai_attr_suggestions"))
        .and_then(Value::as_object);
    let Some(suggestions) = suggestions else {
        return Ok(None);
    };
    if suggestions.is_empty() {
        return Ok(None);
    }

    let mut filled = 0i64;
    let product_specs = required_category_attr_specs(raw_detail, "product_attr_list");
    let sale_specs = required_category_attr_specs(raw_detail, "sale_attr_list");
    // 补齐商品属性
    for attr_key in &check.missing_product_attrs {
        if let Some(value) = suggestions
            .get(attr_key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            if let Some(spec) = product_specs.get(attr_key) {
                if let Some(value) = normalize_attr_value_for_category_spec(product, spec, &value) {
                    ensure_payload_product_attr(payload, attr_key, &value)?;
                    filled += 1;
                }
            }
        }
    }
    // 补齐销售属性
    for attr_key in &check.missing_sale_attrs {
        if let Some(value) = suggestions
            .get(attr_key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            if let Some(spec) = sale_specs.get(attr_key) {
                if let Some(value) = normalize_attr_value_for_category_spec(product, spec, &value) {
                    ensure_payload_sku_attr_for_all(payload, attr_key, &value)?;
                    filled += 1;
                }
            }
        }
    }

    if filled == 0 {
        return Ok(None);
    }

    // 重新检查是否还有缺失
    let Some(cat_id) = extract_leaf_category_id_from_payload(payload) else {
        persist_filled_add_product_payload_from_suggestions(conn, item, payload)?;
        return Ok(None);
    };
    let recheck = match check_cached_category_requirements(conn, &item.shop_id, cat_id, payload)? {
        recheck if !recheck.has_missing_attrs() => {
            // 全覆盖！写入补齐记录
            persist_filled_add_product_payload_from_suggestions(conn, item, payload)?;
            Ok(Some(filled))
        }
        _ => {
            // 仍有缺失，但已补齐的部分写入 payload
            persist_filled_add_product_payload_from_suggestions(conn, item, payload)?;
            Ok(None)
        }
    };
    recheck
}

/// 将 ai_attr_suggestions 的补齐结果持久化到 raw_payload
fn persist_filled_add_product_payload_from_suggestions(
    conn: &Connection,
    item: &PendingPublishItem,
    payload: &Value,
) -> AppResult<()> {
    let mut raw_value = serde_json::from_str::<Value>(&item.raw_payload).map_err(|error| {
        AppError::Validation(format!(
            "商品原始数据无法解析，不能写入属性补齐结果：{error}"
        ))
    })?;
    let raw_object = raw_value.as_object_mut().ok_or_else(|| {
        AppError::Validation("商品原始数据不是对象，不能写入属性补齐结果".to_string())
    })?;
    let metadata_value = raw_object
        .entry("metadata".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if metadata_value.is_null() {
        *metadata_value = Value::Object(serde_json::Map::new());
    }
    let metadata = metadata_value.as_object_mut().ok_or_else(|| {
        AppError::Validation("metadata 必须是对象，不能写入属性补齐结果".to_string())
    })?;
    metadata.insert("wechat_add_product_payload".to_string(), payload.clone());
    metadata.insert(
        "wechat_attr_fill_source".to_string(),
        Value::String("review_ai_attr_suggestions".to_string()),
    );
    metadata.insert(
        "wechat_attr_fill_at".to_string(),
        Value::String(now_shanghai()),
    );
    conn.execute(
        "UPDATE pipeline_shop_targets SET raw_payload = ?1 WHERE id = ?2",
        params![raw_value.to_string(), item.item_id.as_str()],
    )?;
    Ok(())
}

fn select_after_sale_address(
    details: &[MerchantAddressDetailSummary],
) -> Result<AutoAfterSaleAddressSelection, String> {
    let default_recv = details
        .iter()
        .filter(|detail| detail.default_recv)
        .collect::<Vec<_>>();
    if default_recv.len() == 1 {
        return Ok(AutoAfterSaleAddressSelection {
            address_id: default_recv[0].address_id,
            source: "default_recv",
        });
    }
    if default_recv.len() > 1 {
        return Err("微信返回多个默认退货地址，无法自动选择售后地址".to_string());
    }

    let recv = details
        .iter()
        .filter(|detail| detail.recv_addr)
        .collect::<Vec<_>>();
    if recv.len() == 1 {
        return Ok(AutoAfterSaleAddressSelection {
            address_id: recv[0].address_id,
            source: "only_recv",
        });
    }
    if recv.is_empty() {
        return Err("微信地址列表中没有可用退货/售后地址".to_string());
    }
    Err("微信返回多个退货/售后地址，无法自动选择售后地址".to_string())
}

fn payload_deliver_method(payload: &Value) -> i64 {
    payload
        .get("deliver_method")
        .and_then(|value| json_value_to_i64(Some(value)))
        .unwrap_or(0)
}

fn payload_after_sale_address_id(payload: &Value) -> Option<i64> {
    payload
        .get("after_sale_info")
        .and_then(Value::as_object)
        .and_then(|after_sale_info| after_sale_info.get("after_sale_address_id"))
        .and_then(|value| json_value_to_i64(Some(value)))
        .filter(|address_id| *address_id > 0)
}

fn payload_freight_template_id(payload: &Value) -> Option<String> {
    payload
        .get("express_info")
        .and_then(Value::as_object)
        .and_then(|express_info| express_info.get("template_id"))
        .and_then(|value| json_value_to_string(Some(value)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn set_payload_after_sale_address_id(payload: &mut Value, address_id: i64) -> AppResult<()> {
    let object = payload.as_object_mut().ok_or_else(|| {
        AppError::Validation("微信 addproduct 请求必须是对象，不能补齐售后地址".to_string())
    })?;
    object.insert(
        "after_sale_info".to_string(),
        serde_json::json!({ "after_sale_address_id": address_id }),
    );
    Ok(())
}

fn set_payload_freight_template_id(payload: &mut Value, template_id: &str) -> AppResult<()> {
    let object = payload.as_object_mut().ok_or_else(|| {
        AppError::Validation("微信 addproduct 请求必须是对象，不能补齐运费模板".to_string())
    })?;
    let express_info = object
        .entry("express_info".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if express_info.is_null() {
        *express_info = Value::Object(serde_json::Map::new());
    }
    let express_info = express_info.as_object_mut().ok_or_else(|| {
        AppError::Validation("express_info 必须是对象，不能补齐运费模板".to_string())
    })?;
    express_info.insert(
        "template_id".to_string(),
        Value::String(template_id.trim().to_string()),
    );
    Ok(())
}

fn validate_publish_payload_value(payload: &Value) -> Result<(), String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "微信 addproduct 请求必须是对象".to_string())?;
    validate_add_product_base_payload(object)
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
    let mut after_sale_address_cache = BTreeMap::new();
    let mut freight_template_cache = BTreeMap::new();

    for item in ready_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            mark_target_running(&conn, &item.item_id)?;
        }
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
        let mut draft = match resolve_add_product_base_payload_relaxed_after_sale(&product) {
            Ok(draft) => draft,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    add_product_payload_error_code(&error),
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

        if let Err((code, summary)) = ensure_after_sale_address_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            &mut draft.payload,
            &mut after_sale_address_cache,
        )
        .await?
        {
            mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
            failed_items += 1;
            continue;
        }
        if let Err((code, summary)) = ensure_freight_template_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            &mut draft.payload,
            &mut freight_template_cache,
        )
        .await?
        {
            mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
            failed_items += 1;
            continue;
        }
        if let Err(error) = validate_publish_payload_value(&draft.payload) {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                add_product_payload_error_code(&error),
                &error,
            )?;
            failed_items += 1;
            continue;
        }

        let raw_detail = match ensure_category_detail_payload_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            cat_id,
        )
        .await?
        {
            Ok(raw_detail) => raw_detail,
            Err((code, summary)) => {
                mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
                failed_items += 1;
                continue;
            }
        };
        // 类目预检前先用审查阶段 AI 产出补齐缺失属性，并写回发品 payload。
        let mut requirement_check =
            check_category_requirements_from_detail(&raw_detail, &draft.payload);
        if requirement_check.has_missing_attrs() {
            let conn = open_connection(&app)?;
            let review_filled = match apply_review_ai_attr_suggestions_to_payload(
                &conn,
                &item,
                &product,
                &mut draft.payload,
                &raw_detail,
                &requirement_check,
            ) {
                Ok(filled) => filled,
                Err(error) => {
                    mark_publish_item_failed_for_app(
                        &app,
                        &item,
                        "CATEGORY_ATTR_FILL_FAILED",
                        &format!("审查阶段属性建议写入发品参数失败：{error}"),
                    )?;
                    failed_items += 1;
                    continue;
                }
            };
            if review_filled > 0 {
                let summary =
                    format!("已从审查阶段 AI 建议中补齐 {review_filled} 个必填属性并写入发品参数");
                insert_task_log(
                    &conn,
                    &item.job_id,
                    Some(&item.item_id),
                    "info",
                    &summary,
                    Some(&serde_json::json!({
                        "source": "review_ai_attr_suggestions",
                        "filled_count": review_filled
                    })),
                )?;
                requirement_check =
                    check_category_requirements_from_detail(&raw_detail, &draft.payload);
            }
        }
        if requirement_check.has_missing_attrs() {
            let summary = requirement_check.failure_summary();
            mark_publish_item_failed_for_app(&app, &item, "CATEGORY_ATTRS_NEED_AI_FILL", &summary)?;
            failed_items += 1;
            continue;
        }

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
                    advance_target(&conn, &item.item_id, target_stage::ASSET_UPLOAD)?;
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
        recompute_pipeline_product(&conn, job_id)?;
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
    let mut after_sale_address_cache = BTreeMap::new();
    let mut freight_template_cache = BTreeMap::new();

    for item in ready_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            mark_target_running(&conn, &item.item_id)?;
        }
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

        let mut draft = match resolve_add_product_base_payload_relaxed_after_sale(&product) {
            Ok(draft) => draft,
            Err(error) => {
                mark_publish_item_failed_for_app(
                    &app,
                    &item,
                    add_product_payload_error_code(&error),
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
                "微信发品参数缺少有效叶子类目 cat_id，不能上传素材",
            )?;
            failed_items += 1;
            continue;
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

        if let Err((code, summary)) = ensure_after_sale_address_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            &mut draft.payload,
            &mut after_sale_address_cache,
        )
        .await?
        {
            mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
            failed_items += 1;
            continue;
        }
        if let Err((code, summary)) = ensure_freight_template_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            &mut draft.payload,
            &mut freight_template_cache,
        )
        .await?
        {
            mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
            failed_items += 1;
            continue;
        }
        if let Err(error) = validate_publish_payload_value(&draft.payload) {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                add_product_payload_error_code(&error),
                &error,
            )?;
            failed_items += 1;
            continue;
        }

        let raw_detail = match ensure_category_detail_payload_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            cat_id,
        )
        .await?
        {
            Ok(raw_detail) => raw_detail,
            Err((code, summary)) => {
                mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
                failed_items += 1;
                continue;
            }
        };
        let requirement_check =
            check_category_requirements_from_detail(&raw_detail, &draft.payload);
        if requirement_check.has_missing_attrs() {
            let summary = requirement_check.failure_summary();
            mark_publish_item_failed_for_app(&app, &item, "CATEGORY_ATTRS_NEED_AI_FILL", &summary)?;
            failed_items += 1;
            continue;
        }

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

        {
            let conn = open_connection(&app)?;
            advance_target(&conn, &item.item_id, target_stage::SUBMIT)?;
        }
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
        recompute_pipeline_product(&conn, job_id)?;
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
    let mut after_sale_address_cache = BTreeMap::new();
    let mut freight_template_cache = BTreeMap::new();

    for item in submit_items {
        job_ids.insert(item.job_id.clone());
        {
            let conn = open_connection(&app)?;
            mark_target_running(&conn, &item.item_id)?;
        }
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
        let mut payload = match build_add_product_payload_relaxed_after_sale(&product, &assets) {
            Ok(payload) => payload,
            Err(error) => {
                let error_code = if error.contains("after_sale_info.after_sale_address_id") {
                    "MISSING_AFTER_SALE_ADDRESS"
                } else {
                    "MISSING_WECHAT_PRODUCT_PAYLOAD"
                };
                mark_publish_item_failed_for_app(&app, &item, error_code, &error)?;
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

        if let Err((code, summary)) = ensure_after_sale_address_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            &mut payload,
            &mut after_sale_address_cache,
        )
        .await?
        {
            mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
            failed_items += 1;
            continue;
        }
        if let Err((code, summary)) = ensure_freight_template_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            &mut payload,
            &mut freight_template_cache,
        )
        .await?
        {
            mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
            failed_items += 1;
            continue;
        }
        if let Err(error) = validate_publish_payload_value(&payload) {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                add_product_payload_error_code(&error),
                &error,
            )?;
            failed_items += 1;
            continue;
        }

        let Some(cat_id) = extract_leaf_category_id_from_payload(&payload) else {
            mark_publish_item_failed_for_app(
                &app,
                &item,
                "MISSING_WECHAT_LEAF_CATEGORY_ID",
                "微信发品参数缺少有效叶子类目 cat_id，不能提交 addproduct",
            )?;
            failed_items += 1;
            continue;
        };
        let raw_detail = match ensure_category_detail_payload_for_publish(
            &app,
            &client,
            &access_token,
            &item,
            cat_id,
        )
        .await?
        {
            Ok(raw_detail) => raw_detail,
            Err((code, summary)) => {
                mark_publish_item_failed_for_app(&app, &item, &code, &summary)?;
                failed_items += 1;
                continue;
            }
        };
        let sanitize_report =
            sanitize_payload_attrs_with_category_detail(&product, &mut payload, &raw_detail)?;
        if sanitize_report.has_changes() {
            insert_task_log_for_app(
                &app,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &format!(
                    "已按微信类目选项清洗发品属性：{}",
                    sanitize_report.summary()
                ),
                None,
            )?;
        }
        let requirement_check = check_category_requirements_from_detail(&raw_detail, &payload);
        if requirement_check.has_missing_attrs() {
            let summary = if sanitize_report.has_removed() {
                format!(
                    "{}；{}",
                    requirement_check.failure_summary(),
                    sanitize_report.summary()
                )
            } else {
                requirement_check.failure_summary()
            };
            mark_publish_item_failed_for_app(&app, &item, "CATEGORY_ATTRS_NEED_AI_FILL", &summary)?;
            failed_items += 1;
            continue;
        }

        let mut call = match client.add_product(&access_token, &payload).await {
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
        if matches!(
            &call.result,
            WechatCallResult::ApiError(error) if is_freight_template_id_error(error)
        ) {
            if let Some(retry_call) = retry_add_product_with_alternative_freight_templates(
                &app,
                &client,
                &access_token,
                &item,
                &mut payload,
            )
            .await?
            {
                call = retry_call;
            }
        }

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
                    "UPDATE pipeline_shop_targets
                     SET stage = ?1,
                         status = ?2,
                         error_code = NULL,
                         error_summary = NULL,
                         wechat_product_id = ?3,
                         updated_at = ?4
                     WHERE id = ?5",
                    params![
                        target_stage::AUDIT,
                        target_status::PENDING,
                        result.product_id,
                        now_shanghai(),
                        item.item_id
                    ],
                )?;
                let source_url = conn
                    .query_row(
                        "SELECT source_url FROM pipeline_products WHERE id = ?1",
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
        recompute_pipeline_product(&conn, job_id)?;
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
                    &resolution.summary,
                    resolution.wechat_status,
                    resolution.wechat_edit_status,
                )?;
                {
                    // 方案Y：审核结果决定 target 推进——已上架→完成；审核通过→进上架阶段；
                    // 拒绝→阻塞等人工；审核中→保持 audit 阶段待下轮 tick 重查（不推进）。
                    let conn = open_connection(&app)?;
                    match resolution.status {
                        "success" => finish_target(&conn, &item.item_id)?,
                        "audit_passed" => {
                            advance_target(&conn, &item.item_id, target_stage::LISTING)?
                        }
                        "failed" => block_target(
                            &conn,
                            &item.item_id,
                            resolution.error_code.as_deref().unwrap_or("WECHAT_API_ERROR"),
                            &resolution.summary,
                        )?,
                        _ => {}
                    }
                }
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
        recompute_pipeline_product(&conn, job_id)?;
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
                {
                    // 方案Y：listingproduct 成功即上架完成，target 收尾到 done。
                    let conn = open_connection(&app)?;
                    finish_target(&conn, &item.item_id)?;
                }
                set_shop_product_item_state(
                    &app,
                    &item,
                    "listed",
                    "微信 listingproduct 成功，商品已上架",
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
        recompute_pipeline_product(&conn, job_id)?;
    }

    Ok(ProductListingBatchResult {
        processed_jobs: job_ids.len() as i64,
        processed_items,
        listing_submitted_items,
        failed_items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address_detail(
        address_id: i64,
        recv_addr: bool,
        default_recv: bool,
    ) -> MerchantAddressDetailSummary {
        MerchantAddressDetailSummary {
            address_id,
            send_addr: true,
            default_send: false,
            recv_addr,
            default_recv,
        }
    }

    #[test]
    fn select_after_sale_address_prefers_default_recv() {
        let selection = select_after_sale_address(&[
            address_detail(10, true, false),
            address_detail(20, false, true),
        ])
        .expect("应选择默认退货地址");

        assert_eq!(selection.address_id, 20);
        assert_eq!(selection.source, "default_recv");
    }

    #[test]
    fn select_after_sale_address_uses_only_recv_candidate() {
        let selection = select_after_sale_address(&[
            address_detail(10, false, false),
            address_detail(20, true, false),
        ])
        .expect("唯一退货地址可自动选择");

        assert_eq!(selection.address_id, 20);
        assert_eq!(selection.source, "only_recv");
    }

    #[test]
    fn select_after_sale_address_rejects_multiple_recv_candidates() {
        let error = select_after_sale_address(&[
            address_detail(10, true, false),
            address_detail(20, true, false),
        ])
        .expect_err("多个退货地址不能猜测");

        assert!(error.contains("多个退货/售后地址"));
    }

    #[test]
    fn select_after_sale_address_rejects_multiple_default_recv_candidates() {
        let error = select_after_sale_address(&[
            address_detail(10, false, true),
            address_detail(20, false, true),
        ])
        .expect_err("多个默认退货地址不能猜测");

        assert!(error.contains("多个默认退货地址"));
    }

    #[test]
    fn set_payload_freight_template_id_preserves_express_info() {
        let mut payload = serde_json::json!({
            "deliver_method": 0,
            "express_info": {
                "weight": 500
            }
        });

        set_payload_freight_template_id(&mut payload, "tpl-1").expect("应能补齐运费模板");

        assert_eq!(
            payload
                .pointer("/express_info/template_id")
                .and_then(Value::as_str),
            Some("tpl-1")
        );
        assert_eq!(
            payload
                .pointer("/express_info/weight")
                .and_then(Value::as_i64),
            Some(500)
        );
    }

    #[test]
    fn payload_freight_template_id_reads_non_empty_template() {
        let payload = serde_json::json!({
            "express_info": {
                "template_id": " tpl-2 "
            }
        });

        assert_eq!(
            payload_freight_template_id(&payload).as_deref(),
            Some("tpl-2")
        );
    }

    #[test]
    fn review_ai_attr_suggestions_persist_filled_payload() {
        let conn = Connection::open_in_memory().expect("应能创建内存数据库");
        conn.execute(
            "CREATE TABLE pipeline_shop_targets (id TEXT PRIMARY KEY, raw_payload TEXT)",
            [],
        )
        .expect("应能创建店级 target 表");

        let raw_payload = serde_json::json!({
            "external_product_id": "external-1",
            "metadata": {
                "ai_attr_suggestions": {
                    "颜色": "红色",
                    "尺码": "均码"
                }
            }
        })
        .to_string();
        // persist 按 item.item_id 写回店级 target 的 raw_payload（方案Y）
        conn.execute(
            "INSERT INTO pipeline_shop_targets (id, raw_payload) VALUES (?1, ?2)",
            params!["item-1", raw_payload.as_str()],
        )
        .expect("应能插入店级 target");

        let item = PendingPublishItem {
            item_id: "item-1".to_string(),
            job_id: "job-1".to_string(),
            product_row_id: "product-1".to_string(),
            shop_id: "shop-1".to_string(),
            shop_status: "connected".to_string(),
            shop_has_secret: true,
            external_product_id: "external-1".to_string(),
            raw_payload,
        };
        let product = ExternalProductInput {
            external_product_id: "external-1".to_string(),
            title: "测试商品".to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            skus: Vec::new(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: None,
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({
                "ai_attr_suggestions": {
                    "颜色": "红色",
                    "尺码": "均码"
                }
            }),
        };
        let mut payload = serde_json::json!({
            "attrs": [],
            "skus": [{}]
        });
        let check = CachedCategoryRequirementCheck {
            missing_product_attrs: vec!["颜色".to_string()],
            missing_sale_attrs: vec!["尺码".to_string()],
        };
        let raw_detail = serde_json::json!({
            "attr": {
                "product_attr_list": [
                    {
                        "name": "颜色",
                        "is_required": true,
                        "type_v2": "string",
                        "value": ""
                    }
                ],
                "sale_attr_list": [
                    {
                        "name": "尺码",
                        "is_required": true,
                        "type_v2": "string",
                        "value": ""
                    }
                ]
            }
        });

        let filled = apply_review_ai_attr_suggestions_to_payload(
            &conn,
            &item,
            &product,
            &mut payload,
            &raw_detail,
            &check,
        )
        .expect("应能应用审查阶段属性建议");

        assert_eq!(filled, 2);
        let saved: String = conn
            .query_row(
                "SELECT raw_payload FROM pipeline_shop_targets WHERE id = ?1",
                ["item-1"],
                |row| row.get(0),
            )
            .expect("应能读取更新后的店级 target");
        let saved_value = serde_json::from_str::<Value>(&saved).expect("raw_payload 应为 JSON");
        let saved_payload = saved_value
            .pointer("/metadata/wechat_add_product_payload")
            .expect("应持久化微信发品 payload");

        assert_eq!(
            saved_payload
                .pointer("/attrs/0/attr_key")
                .and_then(Value::as_str),
            Some("颜色")
        );
        assert_eq!(
            saved_payload
                .pointer("/skus/0/sku_attrs/0/attr_key")
                .and_then(Value::as_str),
            Some("尺码")
        );
        assert_eq!(
            saved_value
                .pointer("/metadata/wechat_attr_fill_source")
                .and_then(Value::as_str),
            Some("review_ai_attr_suggestions")
        );
    }
}
