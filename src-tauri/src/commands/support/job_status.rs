use super::*;

pub(in crate::commands) fn recompute_publish_job(conn: &Connection, job_id: &str) -> AppResult<()> {
    let mut product_stmt = conn.prepare("SELECT id FROM publish_products WHERE job_id = ?1")?;
    let product_ids = product_stmt
        .query_map([job_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    for product_id in product_ids {
        let counts = load_publish_status_counts(conn, "product_row_id = ?1", &product_id)?;
        let status = resolve_publish_status(&counts);
        let error_summary = if counts.total > 0 && counts.failed == counts.total {
            Some("全部目标店铺均失败".to_string())
        } else if counts.failed > 0 {
            Some(format!("{} 个店铺失败，请查看店铺维度原因", counts.failed))
        } else {
            None
        };
        conn.execute(
            "UPDATE publish_products SET status = ?1, error_summary = ?2 WHERE id = ?3",
            params![status, error_summary, product_id],
        )?;
    }

    let counts = load_publish_status_counts(conn, "job_id = ?1", job_id)?;
    let status = resolve_publish_status(&counts);
    let progress = compute_publish_progress(&counts);
    let finished_at =
        if counts.pending + counts.ready + counts.submitted + counts.audit_pending == 0 {
            Some(now_shanghai())
        } else {
            None
        };
    conn.execute(
        "UPDATE publish_jobs SET status = ?1 WHERE id = ?2",
        params![status, job_id],
    )?;
    conn.execute(
        "UPDATE task_runs
         SET status = ?1, progress = ?2, finished_at = ?3
         WHERE id = ?4",
        params![status, progress, finished_at, job_id],
    )?;
    Ok(())
}

pub(in crate::commands) fn recompute_price_update_job(
    conn: &Connection,
    job_id: &str,
) -> AppResult<()> {
    let counts = conn.query_row(
	        "SELECT
	           COUNT(*) AS total,
	           COALESCE(SUM(CASE WHEN status IN ('pending', 'prechecking', 'submitting') THEN 1 ELSE 0 END), 0) AS pending_count,
	           COALESCE(SUM(CASE WHEN status = 'ready_to_update' THEN 1 ELSE 0 END), 0) AS ready_count,
	           COALESCE(SUM(CASE WHEN status = 'submitted' THEN 1 ELSE 0 END), 0) AS submitted_count,
	           COALESCE(SUM(CASE WHEN status = 'audit_pending' THEN 1 ELSE 0 END), 0) AS audit_pending_count,
	           COALESCE(SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END), 0) AS success_count,
	           COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count
	         FROM price_update_items
	         WHERE job_id = ?1",
	        [job_id],
        |row| {
            Ok((
	                row.get::<_, i64>(0)?,
	                row.get::<_, i64>(1)?,
	                row.get::<_, i64>(2)?,
	                row.get::<_, i64>(3)?,
	                row.get::<_, i64>(4)?,
	                row.get::<_, i64>(5)?,
	                row.get::<_, i64>(6)?,
	            ))
	        },
	    )?;
    let (total, pending, ready, submitted, audit_pending, success, failed) = counts;
    let status = if total == 0 {
        "failed"
    } else if failed == total {
        "failed"
    } else if success == total {
        "success"
    } else if pending > 0 {
        "running"
    } else if ready > 0 && submitted + audit_pending + success + failed == 0 {
        "ready_to_update"
    } else if submitted + audit_pending == total {
        if audit_pending > 0 {
            "audit_pending"
        } else {
            "submitted"
        }
    } else if ready > 0 || submitted > 0 || audit_pending > 0 || success > 0 {
        "partial_success"
    } else {
        "queued"
    };
    let progress = if total == 0 {
        0
    } else {
        ((ready * 50 + submitted * 75 + audit_pending * 80 + success * 100 + failed * 100) / total)
            .clamp(0, 100)
    };
    let finished_at = if pending + ready + submitted + audit_pending == 0 {
        Some(now_shanghai())
    } else {
        None
    };
    conn.execute(
        "UPDATE price_update_jobs SET status = ?1 WHERE id = ?2",
        params![status, job_id],
    )?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = ?2, finished_at = ?3 WHERE id = ?4",
        params![status, progress, finished_at, job_id],
    )?;
    Ok(())
}

#[derive(Debug)]
pub(in crate::commands) struct PublishStatusCounts {
    total: i64,
    pending: i64,
    ready: i64,
    assets_ready: i64,
    submitted: i64,
    audit_pending: i64,
    audit_passed: i64,
    success: i64,
    failed: i64,
}

pub(in crate::commands) fn load_publish_status_counts(
    conn: &Connection,
    where_clause: &str,
    value: &str,
) -> AppResult<PublishStatusCounts> {
    let sql = format!(
        "SELECT
           COUNT(*) AS total,
           COALESCE(SUM(CASE WHEN status IN ('pending', 'prechecking', 'category_prechecking', 'asset_uploading', 'publishing', 'listing', 'running') THEN 1 ELSE 0 END), 0) AS pending_count,
           COALESCE(SUM(CASE WHEN status IN ('ready_to_publish', 'category_prechecked', 'assets_ready') THEN 1 ELSE 0 END), 0) AS ready_count,
           COALESCE(SUM(CASE WHEN status = 'assets_ready' THEN 1 ELSE 0 END), 0) AS assets_ready_count,
           COALESCE(SUM(CASE WHEN status = 'submitted' THEN 1 ELSE 0 END), 0) AS submitted_count,
           COALESCE(SUM(CASE WHEN status = 'audit_pending' THEN 1 ELSE 0 END), 0) AS audit_pending_count,
           COALESCE(SUM(CASE WHEN status = 'audit_passed' THEN 1 ELSE 0 END), 0) AS audit_passed_count,
           COALESCE(SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END), 0) AS success_count,
           COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count
         FROM publish_job_items
         WHERE {where_clause}"
    );
    let counts = conn.query_row(&sql, [value], |row| {
        Ok(PublishStatusCounts {
            total: row.get(0)?,
            pending: row.get(1)?,
            ready: row.get(2)?,
            assets_ready: row.get(3)?,
            submitted: row.get(4)?,
            audit_pending: row.get(5)?,
            audit_passed: row.get(6)?,
            success: row.get(7)?,
            failed: row.get(8)?,
        })
    })?;
    Ok(counts)
}

pub(in crate::commands) fn resolve_publish_status(counts: &PublishStatusCounts) -> &'static str {
    let waiting_for_audit = counts.submitted + counts.audit_pending;
    let passed_or_success = counts.audit_passed + counts.success;
    if counts.total == 0 {
        "failed"
    } else if counts.failed == counts.total {
        "failed"
    } else if counts.pending > 0 {
        "running"
    } else if counts.success == counts.total {
        "success"
    } else if passed_or_success == counts.total {
        "audit_passed"
    } else if waiting_for_audit == counts.total {
        if counts.audit_pending > 0 {
            "audit_pending"
        } else {
            "submitted"
        }
    } else if waiting_for_audit > 0 {
        "running"
    } else if counts.submitted == counts.total {
        "submitted"
    } else if counts.assets_ready == counts.total {
        "assets_ready"
    } else if counts.ready > 0 && counts.failed == 0 {
        "ready_to_publish"
    } else if counts.ready > 0 || counts.submitted > 0 || passed_or_success > 0 {
        "partial_success"
    } else {
        "queued"
    }
}

pub(in crate::commands) fn compute_publish_progress(counts: &PublishStatusCounts) -> i64 {
    if counts.total == 0 {
        return 0;
    }
    let ready_before_assets = (counts.ready - counts.assets_ready).max(0);
    let weighted = ready_before_assets * 25
        + counts.assets_ready * 50
        + counts.submitted * 70
        + counts.audit_pending * 80
        + counts.audit_passed * 90
        + counts.success * 100
        + counts.failed * 100;
    (weighted / counts.total).clamp(0, 100)
}

pub(in crate::commands) fn insert_task_log(
    conn: &Connection,
    task_id: &str,
    item_id: Option<&str>,
    level: &str,
    message: &str,
    detail: Option<&serde_json::Value>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO task_logs
         (id, task_id, item_id, level, message, detail_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            format!("log-{}", Uuid::new_v4()),
            task_id,
            item_id,
            level,
            message,
            detail.map(|value| value.to_string()),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(in crate::commands) struct AgentRunCreate<'a> {
    pub skill_name: &'a str,
    pub skill_version: &'a str,
    pub scene: &'a str,
    pub source_type: &'a str,
    pub source_id: &'a str,
    pub shop_id: Option<&'a str>,
    pub provider_type: Option<&'a str>,
    pub model: Option<&'a str>,
    pub temperature: Option<f64>,
    pub input_summary: &'a str,
    pub input_snapshot: Option<&'a Value>,
}

pub(in crate::commands) struct AgentRunFinish<'a> {
    pub status: &'a str,
    pub output: Option<&'a Value>,
    pub validated_output: Option<&'a Value>,
    pub tool_calls: Option<&'a Value>,
    pub decision: Option<&'a str>,
    pub error_code: Option<&'a str>,
    pub error_summary: Option<&'a str>,
    pub duration_ms: Option<i64>,
}

pub(in crate::commands) fn create_agent_run(
    conn: &Connection,
    create: AgentRunCreate<'_>,
) -> AppResult<String> {
    let run_id = format!("agent-run-{}", Uuid::new_v4());
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO agent_runs
         (id, skill_name, skill_version, scene, source_type, source_id, shop_id,
          status, provider_type, model, temperature, input_summary, input_snapshot_json,
          started_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 'running', ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            run_id,
            create.skill_name,
            create.skill_version,
            create.scene,
            create.source_type,
            create.source_id,
            create.shop_id,
            create.provider_type,
            create.model,
            create.temperature,
            truncate_for_summary(create.input_summary, 500),
            create.input_snapshot.map(|value| value.to_string()),
            now
        ],
    )?;
    insert_agent_run_event(
        conn,
        &run_id,
        "input_prepared",
        "info",
        create.input_summary,
        create.input_snapshot,
    )?;
    Ok(run_id)
}

pub(in crate::commands) fn create_agent_run_for_skill(
    conn: &Connection,
    skill: &AiSkillDefinition,
    scene: &str,
    source_type: &str,
    source_id: &str,
    shop_id: Option<&str>,
    config: Option<&AiProviderConfig>,
    input_summary: &str,
    input_snapshot: Option<&Value>,
) -> AppResult<String> {
    create_agent_run(
        conn,
        AgentRunCreate {
            skill_name: skill.name,
            skill_version: skill.version,
            scene,
            source_type,
            source_id,
            shop_id,
            provider_type: config.map(|value| value.provider_type.as_str()),
            model: config.map(|value| value.model.as_str()),
            temperature: config.map(|value| value.temperature),
            input_summary,
            input_snapshot,
        },
    )
}

pub(in crate::commands) fn finish_agent_run(
    conn: &Connection,
    run_id: &str,
    finish: AgentRunFinish<'_>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE agent_runs
         SET status = ?1,
             output_json = ?2,
             validated_output_json = ?3,
             tool_calls_json = ?4,
             decision = ?5,
             error_code = ?6,
             error_summary = ?7,
             finished_at = ?8,
             duration_ms = ?9
         WHERE id = ?10",
        params![
            finish.status,
            finish.output.map(|value| value.to_string()),
            finish.validated_output.map(|value| value.to_string()),
            finish.tool_calls.map(|value| value.to_string()),
            finish.decision,
            finish.error_code,
            finish
                .error_summary
                .map(|value| truncate_for_summary(value, 500)),
            now_shanghai(),
            finish.duration_ms,
            run_id
        ],
    )?;
    let event_type = if finish.status == "failed" {
        "failed"
    } else if finish.status == "blocked" {
        "blocked"
    } else if finish.decision == Some("needs_review") {
        "human_gate_required"
    } else {
        "finished"
    };
    let message = finish
        .error_summary
        .unwrap_or_else(|| match finish.decision {
            Some("passed") | Some("ready_to_publish") | Some("success") => {
                "Agent 输出已自动应用或进入下一状态"
            }
            Some("needs_review") => "Agent 输出需要人工确认",
            Some("blocked") => "Agent 输出被业务规则拦截",
            Some(value) => value,
            None => "Agent 运行完成",
        });
    insert_agent_run_event(
        conn,
        run_id,
        event_type,
        if finish.status == "failed" {
            "error"
        } else {
            "info"
        },
        message,
        finish.validated_output.or(finish.output),
    )
}

pub(in crate::commands) fn insert_agent_run_event(
    conn: &Connection,
    run_id: &str,
    event_type: &str,
    level: &str,
    message: &str,
    data: Option<&Value>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO agent_run_events
         (id, run_id, event_type, level, message, data_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            format!("agent-event-{}", Uuid::new_v4()),
            run_id,
            event_type,
            level,
            truncate_for_summary(message, 500),
            data.map(|value| value.to_string()),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn agent_error_code(error: &AppError) -> &'static str {
    match error {
        AppError::Validation(message) if message.contains("未启用") => "PROVIDER_NOT_CONFIGURED",
        AppError::Validation(message)
            if message.contains("缺少") && message.contains("API Key") =>
        {
            "PROVIDER_NOT_CONFIGURED"
        }
        AppError::Validation(message) if message.contains("超时") => "RUNTIME_TIMEOUT",
        AppError::Validation(message) if message.contains("返回非 JSON") => {
            "MODEL_NON_JSON_OUTPUT"
        }
        AppError::Validation(message) if message.contains("未返回") => "MODEL_EMPTY_OUTPUT",
        AppError::Validation(message)
            if message.contains("HTTP 401") || message.contains("HTTP 403") =>
        {
            "PROVIDER_AUTH_FAILED"
        }
        AppError::Validation(message) if message.contains("HTTP") => "PROVIDER_HTTP_FAILED",
        AppError::Validation(_) => "BUSINESS_VALIDATION_FAILED",
        AppError::Security(_) => "SENSITIVE_DATA_BLOCKED",
        AppError::Http(_) => "PROVIDER_HTTP_FAILED",
        _ => "UNKNOWN_AGENT_ERROR",
    }
}

pub(in crate::commands) fn agent_status_for_business_decision(decision: &str) -> &'static str {
    match decision {
        "passed" | "ready_to_publish" | "success" => "succeeded",
        "needs_review" => "needs_review",
        "blocked" => "blocked",
        _ => "succeeded",
    }
}

pub(in crate::commands) fn supplier_bridge_skill_name() -> &'static str {
    SUPPLIER_BRIDGE_SKILL_NAME
}

pub(in crate::commands) fn supplier_bridge_skill_version() -> &'static str {
    SUPPLIER_BRIDGE_SKILL_VERSION
}

pub(in crate::commands) fn count_by_sql(conn: &Connection, sql: &str) -> AppResult<i64> {
    Ok(conn.query_row(sql, [], |row| row.get(0))?)
}
