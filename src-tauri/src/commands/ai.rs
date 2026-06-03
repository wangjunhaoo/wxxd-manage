use super::*;

#[tauri::command]
pub fn get_ai_provider_settings(app: AppHandle) -> AppResult<AiProviderSettings> {
    let conn = open_connection(&app)?;
    load_ai_provider_settings(&conn)
}

#[tauri::command]
pub fn list_agent_skills(app: AppHandle) -> AppResult<Vec<AgentSkillView>> {
    let conn = open_connection(&app)?;
    let settings = load_ai_provider_settings(&conn)?;
    let runtime_status = agent_runtime_status(&app, &settings.provider_type);
    builtin_agent_skills()
        .into_iter()
        .map(|skill| agent_skill_view(&conn, skill, &runtime_status))
        .collect()
}

#[tauri::command]
pub fn list_agent_runs(
    app: AppHandle,
    scene: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<AgentRunView>> {
    let conn = open_connection(&app)?;
    let scene = normalize_optional_filter(scene);
    let status = normalize_optional_filter(status);
    let limit = limit.unwrap_or(80).clamp(1, 300);
    let mut stmt = conn.prepare(
        "SELECT id, skill_name, skill_version, scene, source_type, source_id, shop_id,
                status, provider_type, model, temperature, input_summary, decision,
                error_code, error_summary, started_at, finished_at, duration_ms, created_at
           FROM agent_runs
          WHERE (?1 IS NULL OR scene = ?1)
            AND (?2 IS NULL OR status = ?2)
          ORDER BY created_at DESC
          LIMIT ?3",
    )?;
    let runs = stmt
        .query_map(params![scene, status, limit], |row| {
            Ok(AgentRunView {
                id: row.get(0)?,
                skill_name: row.get(1)?,
                skill_version: row.get(2)?,
                scene: row.get(3)?,
                source_type: row.get(4)?,
                source_id: row.get(5)?,
                shop_id: row.get(6)?,
                status: row.get(7)?,
                provider_type: row.get(8)?,
                model: row.get(9)?,
                temperature: row.get(10)?,
                input_summary: row.get(11)?,
                decision: row.get(12)?,
                error_code: row.get(13)?,
                error_summary: row.get(14)?,
                started_at: row.get(15)?,
                finished_at: row.get(16)?,
                duration_ms: row.get(17)?,
                created_at: row.get(18)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(runs)
}

#[tauri::command]
pub fn list_agent_run_events(app: AppHandle, run_id: String) -> AppResult<Vec<AgentRunEventView>> {
    let conn = open_connection(&app)?;
    let mut stmt = conn.prepare(
        "SELECT id, run_id, event_type, level, message, data_json, created_at
           FROM agent_run_events
          WHERE run_id = ?1
          ORDER BY created_at ASC",
    )?;
    let events = stmt
        .query_map([run_id], |row| {
            Ok(AgentRunEventView {
                id: row.get(0)?,
                run_id: row.get(1)?,
                event_type: row.get(2)?,
                level: row.get(3)?,
                message: row.get(4)?,
                data_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

#[tauri::command]
pub fn save_agent_skill_settings(
    app: AppHandle,
    request: AgentSkillSettingsRequest,
) -> AppResult<AgentSkillView> {
    let skill = find_agent_skill(&request.name)?;
    let conn = open_connection(&app)?;
    let model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let temperature = request.temperature.map(|value| value.clamp(0.0, 1.0));
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO agent_skill_settings
         (name, enabled, model, temperature, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(name) DO UPDATE SET
           enabled = excluded.enabled,
           model = excluded.model,
           temperature = excluded.temperature,
           updated_at = excluded.updated_at",
        params![
            skill.name,
            if request.enabled { 1 } else { 0 },
            model,
            temperature,
            now
        ],
    )?;
    let settings = load_ai_provider_settings(&conn)?;
    let runtime_status = agent_runtime_status(&app, &settings.provider_type);
    agent_skill_view(&conn, skill, &runtime_status)
}

#[tauri::command]
pub async fn test_agent_skill(app: AppHandle, name: String) -> AppResult<AgentSkillTestResult> {
    let skill = find_agent_skill(&name)?;
    let config = load_ai_provider_config(&app)?;
    let response_result = if skill.name == ATTRIBUTE_SUGGESTION_SKILL.name {
        let base = AttributeFillSuggestion {
            attr_kind: "product",
            attr_key: "适用季节".to_string(),
            suggested_value: None,
            sku_values: Vec::new(),
            confidence: 0,
            source: "needs_ai".to_string(),
            applied: false,
            prompt_json: serde_json::json!({
                "product": {
                    "title": "儿童夏季短袖上衣",
                    "category_hint": "童装 / 短袖",
                    "sku_count": 1
                },
                "attr_kind": "product",
                "attr_key": "适用季节",
                "allowed_values": ["夏季", "四季通用"],
                "review_rules": ["这是桌面端技能试跑，低置信时返回 null"]
            }),
        };
        request_ai_attribute_suggestion(&app, &config, &base)
            .await
            .map(|suggestion| {
                suggestion
                    .map(|value| attribute_suggestions_json(&[value]))
                    .unwrap_or_else(|| serde_json::json!({ "suggestions": [] }))
            })
    } else {
        request_ai_skill_json(
            &app,
            &config,
            skill,
            &serde_json::json!({
                "task": "技能试跑",
                "input": {
                    "product": {
                        "title": "淘宝 2026 新款儿童短袖上衣",
                        "original_title": "淘宝 2026 新款儿童短袖上衣",
                        "supplier_name": "示例供应商",
                        "brand_hint": null,
                        "category_hint": "童装 / 短袖",
                        "source_url": "https://example.com/item",
                        "main_images": [],
                        "detail_images": [],
                        "sku_count": 1
                    },
                    "category_candidates": [],
                    "review_rules": [
                        "这是桌面端技能试跑，不包含真实商品图片",
                        "返回合法 JSON，并在缺少图片时给出人工确认建议"
                    ]
                }
            }),
            &[],
        )
        .await
    };
    let response = match response_result {
        Ok(response) => response,
        Err(error) => {
            let now = now_shanghai();
            let summary = truncate_for_summary(&error.to_string(), 240);
            let conn = open_connection(&app)?;
            conn.execute(
                "INSERT INTO agent_skill_settings
                 (name, enabled, last_test_status, last_test_summary, last_test_at, updated_at)
                 VALUES (?1, 1, 'failed', ?2, ?3, ?3)
                 ON CONFLICT(name) DO UPDATE SET
                   last_test_status = excluded.last_test_status,
                   last_test_summary = excluded.last_test_summary,
                   last_test_at = excluded.last_test_at,
                   updated_at = excluded.updated_at",
                params![skill.name, summary, now],
            )?;
            return Err(error);
        }
    };
    let now = now_shanghai();
    let summary = response_summary_for_ai(&response);
    let conn = open_connection(&app)?;
    conn.execute(
        "INSERT INTO agent_skill_settings
         (name, enabled, last_test_status, last_test_summary, last_test_at, updated_at)
         VALUES (?1, 1, 'success', ?2, ?3, ?3)
         ON CONFLICT(name) DO UPDATE SET
           last_test_status = excluded.last_test_status,
           last_test_summary = excluded.last_test_summary,
           last_test_at = excluded.last_test_at,
           updated_at = excluded.updated_at",
        params![skill.name, summary, now],
    )?;
    Ok(AgentSkillTestResult {
        name: skill.name.to_string(),
        status: "success".to_string(),
        summary,
        checked_at: now,
    })
}

#[tauri::command]
pub fn save_ai_provider_settings(
    app: AppHandle,
    request: AiProviderSettingsRequest,
) -> AppResult<AiProviderSettings> {
    let conn = open_connection(&app)?;
    let provider_type = normalize_ai_provider_type(&request.provider_type)?;
    let custom_provider_id =
        normalize_ai_custom_provider_id(request.custom_provider_id.as_deref().unwrap_or(""))?;
    let api = normalize_ai_provider_api(request.api.as_deref())?;
    let base_url = normalize_ai_base_url_for_provider(&provider_type, &request.base_url)?;
    let model = request.model.trim().to_string();
    let temperature = request.temperature.unwrap_or(0.1).clamp(0.0, 1.0);
    let context_window = normalize_ai_provider_limit(
        request.context_window,
        AI_PROVIDER_DEFAULT_CONTEXT_WINDOW,
        "context_window",
    )?;
    let max_tokens = normalize_ai_provider_limit(
        request.max_tokens,
        AI_PROVIDER_DEFAULT_MAX_TOKENS,
        "max_tokens",
    )?;
    if request.enabled {
        if provider_type == AI_PROVIDER_CUSTOM && base_url.is_empty() {
            return Err(AppError::Validation(format!(
                "启用 {} 前必须填写 base_url",
                ai_provider_runtime_label(&provider_type)
            )));
        }
        if model.is_empty() {
            return Err(AppError::Validation("启用 AI 前必须填写 model".to_string()));
        }
    }

    set_bool_setting(&conn, AI_PROVIDER_ENABLED_SETTING, request.enabled)?;
    set_string_setting(&conn, AI_PROVIDER_TYPE_SETTING, &provider_type)?;
    set_string_setting(&conn, AI_PROVIDER_CUSTOM_ID_SETTING, &custom_provider_id)?;
    set_string_setting(&conn, AI_PROVIDER_API_SETTING, &api)?;
    set_string_setting(&conn, AI_PROVIDER_BASE_URL_SETTING, &base_url)?;
    set_string_setting(&conn, AI_PROVIDER_MODEL_SETTING, &model)?;
    set_string_setting(
        &conn,
        AI_PROVIDER_TEMPERATURE_SETTING,
        &temperature.to_string(),
    )?;
    set_string_setting(
        &conn,
        AI_PROVIDER_CONTEXT_WINDOW_SETTING,
        &context_window.to_string(),
    )?;
    set_string_setting(
        &conn,
        AI_PROVIDER_MAX_TOKENS_SETTING,
        &max_tokens.to_string(),
    )?;

    if request.clear_api_key {
        let credential_id = ai_provider_credential_id(&provider_type, &custom_provider_id);
        conn.execute(
            "DELETE FROM ai_provider_credentials WHERE id = ?1",
            [credential_id],
        )?;
    } else if let Some(api_key) = request
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        save_ai_api_key(&app, &conn, &provider_type, &custom_provider_id, api_key)?;
    }

    load_ai_provider_settings(&conn)
}

#[tauri::command]
pub async fn test_ai_provider(app: AppHandle) -> AppResult<AiProviderTestResult> {
    let config = load_ai_provider_config(&app)?;
    let response = request_ai_skill_json(
        &app,
        &config,
        &PRODUCT_REVIEW_SKILL,
        &serde_json::json!({
            "task": "连通性测试",
            "input": {
                "product": {
                    "title": "儿童短袖上衣",
                    "source_url": "https://example.com/item",
                    "main_images": [],
                    "detail_images": [],
                    "sku_count": 1
                },
                "category_candidates": [],
                "review_rules": ["这是 AI Agent 连通性测试，返回合法审查 JSON"]
            }
        }),
        &[],
    )
    .await?;
    let provider_label = ai_provider_runtime_label(&config.provider_type);
    Ok(AiProviderTestResult {
        status: "success".to_string(),
        provider_type: config.provider_type,
        model: config.model,
        message: format!(
            "{} 连通成功：{}",
            provider_label,
            response_summary_for_ai(&response)
        ),
    })
}

fn find_agent_skill(name: &str) -> AppResult<&'static AiSkillDefinition> {
    let trimmed = name.trim();
    builtin_agent_skills()
        .into_iter()
        .find(|skill| skill.name == trimmed)
        .ok_or_else(|| AppError::Validation(format!("未知 Agent 技能：{trimmed}")))
}

fn agent_skill_view(
    conn: &Connection,
    skill: &'static AiSkillDefinition,
    runtime_status: &str,
) -> AppResult<AgentSkillView> {
    let settings = conn
        .query_row(
            "SELECT enabled, model, temperature, last_test_status, last_test_summary, last_test_at, updated_at
             FROM agent_skill_settings
             WHERE name = ?1",
            [skill.name],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?;
    let (
        enabled,
        model,
        temperature,
        last_test_status,
        last_test_summary,
        last_test_at,
        updated_at,
    ) = settings.unwrap_or((1, None, None, None, None, None, None));
    Ok(AgentSkillView {
        name: skill.name.to_string(),
        version: skill.version.to_string(),
        description: skill.description.to_string(),
        enabled: enabled != 0,
        runtime: skill.runtime.to_string(),
        model,
        temperature,
        skill_path: skill.skill_path.to_string(),
        schema_path: skill.schema_path.to_string(),
        checksum: agent_skill_checksum(skill),
        file_status: agent_skill_file_status(skill),
        runtime_status: runtime_status.to_string(),
        last_test_status,
        last_test_summary,
        last_test_at,
        updated_at,
    })
}

fn agent_skill_checksum(skill: &AiSkillDefinition) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill.instructions.as_bytes());
    hasher.update(skill.output_schema.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn agent_skill_file_status(skill: &AiSkillDefinition) -> String {
    if serde_json::from_str::<Value>(skill.output_schema).is_err() {
        return "schema_invalid".to_string();
    }
    if !skill.instructions.contains("name:") || !skill.instructions.contains("description:") {
        return "skill_metadata_missing".to_string();
    }
    "ready".to_string()
}

fn agent_runtime_status(app: &AppHandle, provider_type: &str) -> String {
    let _ = provider_type;
    let script_path = resolve_pi_agent_script_path(app);
    if !script_path.exists() {
        return "script_missing".to_string();
    }
    match std::process::Command::new(resolve_agent_node_binary(app))
        .arg(&script_path)
        .arg("--check")
        .output()
    {
        Ok(output) if output.status.success() => "ready".to_string(),
        Ok(_) => "sdk_missing".to_string(),
        Err(_) => "node_missing".to_string(),
    }
}
