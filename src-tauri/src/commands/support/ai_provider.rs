use super::*;

pub(in crate::commands) fn normalize_ai_provider_type(value: &str) -> AppResult<String> {
    let provider_type = value.trim();
    let normalized = match provider_type {
        "" | AI_PROVIDER_LEGACY_PI => AI_PROVIDER_CUSTOM,
        value => value,
    };
    if is_ai_provider_slug(normalized) {
        Ok(normalized.to_string())
    } else {
        Err(AppError::Validation(
            "AI provider id 只能包含小写字母、数字、点、横线和下划线".to_string(),
        ))
    }
}

pub(in crate::commands) fn normalize_ai_custom_provider_id(value: &str) -> AppResult<String> {
    let provider_id = value.trim();
    let normalized = if provider_id.is_empty() {
        AI_PROVIDER_DEFAULT_CUSTOM_ID
    } else {
        provider_id
    };
    if is_ai_provider_slug(normalized) {
        Ok(normalized.to_string())
    } else {
        Err(AppError::Validation(
            "Custom provider id 只能包含小写字母、数字、点、横线和下划线".to_string(),
        ))
    }
}

pub(in crate::commands) fn normalize_ai_provider_api(value: Option<&str>) -> AppResult<String> {
    let api = value.unwrap_or("").trim();
    let normalized = if api.is_empty() {
        AI_PROVIDER_DEFAULT_API
    } else {
        api
    };
    match normalized {
        "openai-completions"
        | "openai-responses"
        | "anthropic-messages"
        | "google-generative-ai" => Ok(normalized.to_string()),
        _ => Err(AppError::Validation(
            "Custom API 只支持 openai-completions、openai-responses、anthropic-messages 或 google-generative-ai"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn normalize_ai_provider_limit(
    value: Option<i64>,
    default_value: i64,
    label: &str,
) -> AppResult<i64> {
    let normalized = value.unwrap_or(default_value);
    if normalized <= 0 || normalized > 4_000_000 {
        return Err(AppError::Validation(format!(
            "{label} 必须大于 0 且不超过 4000000"
        )));
    }
    Ok(normalized)
}

fn is_ai_provider_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '-' | '_')
        })
}

fn is_custom_ai_provider(provider_type: &str) -> bool {
    provider_type == AI_PROVIDER_CUSTOM
}

pub(in crate::commands) fn normalize_ai_base_url_for_provider(
    _provider_type: &str,
    value: &str,
) -> AppResult<String> {
    let base_url = normalize_openai_api_base_url(value);
    if base_url.is_empty() {
        return Ok(base_url);
    }
    Url::parse(&base_url)
        .map_err(|error| AppError::Validation(format!("AI base_url 不是有效 URL：{error}")))?;
    Ok(base_url)
}

pub(in crate::commands) fn ai_provider_credential_id(
    provider_type: &str,
    custom_provider_id: &str,
) -> String {
    if is_custom_ai_provider(provider_type) {
        format!("ai_provider:{}:{}", provider_type, custom_provider_id)
    } else {
        format!("ai_provider:{provider_type}")
    }
}

pub(in crate::commands) fn ai_provider_runtime_label(provider_type: &str) -> String {
    if is_custom_ai_provider(provider_type) {
        "pi-coding-agent / Custom".to_string()
    } else {
        format!("pi-coding-agent / {provider_type}")
    }
}

pub(in crate::commands) fn load_ai_provider_settings(
    conn: &Connection,
) -> AppResult<AiProviderSettings> {
    let provider_type = normalize_ai_provider_type(
        get_string_setting(conn, AI_PROVIDER_TYPE_SETTING)?
            .as_deref()
            .unwrap_or(AI_PROVIDER_CUSTOM),
    )?;
    let custom_provider_id = normalize_ai_custom_provider_id(
        get_string_setting(conn, AI_PROVIDER_CUSTOM_ID_SETTING)?
            .as_deref()
            .unwrap_or(AI_PROVIDER_DEFAULT_CUSTOM_ID),
    )?;
    let key = ai_api_key_record_for_provider(conn, &provider_type, &custom_provider_id)?;
    let updated_at = max_ai_setting_updated_at(conn)?;
    Ok(AiProviderSettings {
        enabled: get_bool_setting(conn, AI_PROVIDER_ENABLED_SETTING, false)?,
        provider_type,
        custom_provider_id,
        api: normalize_ai_provider_api(
            get_string_setting(conn, AI_PROVIDER_API_SETTING)?.as_deref(),
        )?,
        base_url: get_string_setting(conn, AI_PROVIDER_BASE_URL_SETTING)?.unwrap_or_default(),
        model: get_string_setting(conn, AI_PROVIDER_MODEL_SETTING)?.unwrap_or_default(),
        temperature: get_string_setting(conn, AI_PROVIDER_TEMPERATURE_SETTING)?
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.1),
        context_window: get_string_setting(conn, AI_PROVIDER_CONTEXT_WINDOW_SETTING)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(AI_PROVIDER_DEFAULT_CONTEXT_WINDOW),
        max_tokens: get_string_setting(conn, AI_PROVIDER_MAX_TOKENS_SETTING)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(AI_PROVIDER_DEFAULT_MAX_TOKENS),
        has_api_key: key.is_some(),
        api_key_hint: key.map(|record| {
            let fingerprint = record.api_key_fingerprint;
            format!("指纹 {}", fingerprint.chars().take(8).collect::<String>())
        }),
        updated_at,
    })
}

pub(in crate::commands) fn max_ai_setting_updated_at(
    conn: &Connection,
) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT MAX(updated_at)
             FROM (
               SELECT updated_at FROM app_settings WHERE key LIKE 'ai_provider.%'
               UNION ALL
               SELECT updated_at FROM ai_provider_credentials
             )",
            [],
            |row| row.get::<_, Option<String>>(0),
        )?
        .filter(|value| !value.trim().is_empty()))
}

pub(in crate::commands) fn ai_api_key_record_for_provider(
    conn: &Connection,
    provider_type: &str,
    custom_provider_id: &str,
) -> AppResult<Option<AiApiKeyRecord>> {
    let credential_id = ai_provider_credential_id(provider_type, custom_provider_id);
    conn.query_row(
        "SELECT encrypted_api_key, api_key_nonce, api_key_fingerprint
         FROM ai_provider_credentials WHERE id = ?1",
        [credential_id],
        |row| {
            Ok(AiApiKeyRecord {
                encrypted_api_key: row.get(0)?,
                api_key_nonce: row.get(1)?,
                api_key_fingerprint: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(AppError::from)
}

pub(in crate::commands) fn save_ai_api_key(
    app: &AppHandle,
    conn: &Connection,
    provider_type: &str,
    custom_provider_id: &str,
    api_key: &str,
) -> AppResult<()> {
    let encrypted = encrypt_secret(app, api_key)?;
    let credential_id = ai_provider_credential_id(provider_type, custom_provider_id);
    conn.execute(
        "INSERT INTO ai_provider_credentials
         (id, encrypted_api_key, api_key_nonce, key_version, api_key_fingerprint, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           encrypted_api_key = excluded.encrypted_api_key,
           api_key_nonce = excluded.api_key_nonce,
           key_version = excluded.key_version,
           api_key_fingerprint = excluded.api_key_fingerprint,
           updated_at = excluded.updated_at",
        params![
            credential_id,
            encrypted.ciphertext,
            encrypted.nonce,
            encrypted.key_version,
            secret_fingerprint(api_key),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn load_optional_ai_provider_config(
    app: &AppHandle,
) -> AppResult<Option<AiProviderConfig>> {
    let conn = open_connection(app)?;
    if !get_bool_setting(&conn, AI_PROVIDER_ENABLED_SETTING, false)? {
        return Ok(None);
    }
    let provider_type = normalize_ai_provider_type(
        get_string_setting(&conn, AI_PROVIDER_TYPE_SETTING)?
            .as_deref()
            .unwrap_or(AI_PROVIDER_CUSTOM),
    )?;
    let custom_provider_id = normalize_ai_custom_provider_id(
        get_string_setting(&conn, AI_PROVIDER_CUSTOM_ID_SETTING)?
            .as_deref()
            .unwrap_or(AI_PROVIDER_DEFAULT_CUSTOM_ID),
    )?;
    let record = ai_api_key_record_for_provider(&conn, &provider_type, &custom_provider_id)?;
    let base_url = get_string_setting(&conn, AI_PROVIDER_BASE_URL_SETTING)?.unwrap_or_default();
    let model = get_string_setting(&conn, AI_PROVIDER_MODEL_SETTING)?.unwrap_or_default();
    if model.trim().is_empty()
        || (is_custom_ai_provider(&provider_type) && base_url.trim().is_empty())
    {
        return Ok(None);
    }
    Ok(Some(AiProviderConfig {
        provider_type,
        custom_provider_id,
        api: normalize_ai_provider_api(
            get_string_setting(&conn, AI_PROVIDER_API_SETTING)?.as_deref(),
        )?,
        base_url,
        model,
        temperature: get_string_setting(&conn, AI_PROVIDER_TEMPERATURE_SETTING)?
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.1),
        context_window: get_string_setting(&conn, AI_PROVIDER_CONTEXT_WINDOW_SETTING)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(AI_PROVIDER_DEFAULT_CONTEXT_WINDOW),
        max_tokens: get_string_setting(&conn, AI_PROVIDER_MAX_TOKENS_SETTING)?
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(AI_PROVIDER_DEFAULT_MAX_TOKENS),
        api_key: record
            .map(|record| decrypt_secret(app, &record.encrypted_api_key, &record.api_key_nonce))
            .transpose()?
            .unwrap_or_default(),
    }))
}

pub(in crate::commands) fn load_ai_provider_config(app: &AppHandle) -> AppResult<AiProviderConfig> {
    load_optional_ai_provider_config(app)?.ok_or_else(|| {
        AppError::Validation(
            "AI Agent 未启用或配置不完整。Custom provider 需要 base_url 和 model；内置 Pi provider 需要 provider 和 model"
                .to_string(),
        )
    })
}

pub(in crate::commands) async fn request_ai_collection_review_json(
    app: &AppHandle,
    config: &AiProviderConfig,
    instruction: &str,
    input: &Value,
    image_urls: &[String],
) -> AppResult<Value> {
    let skill_input = serde_json::json!({
        "task": instruction,
        "input": input,
    });
    request_ai_skill_json(app, config, &PRODUCT_REVIEW_SKILL, &skill_input, image_urls).await
}

pub(in crate::commands) async fn request_ai_skill_json(
    app: &AppHandle,
    config: &AiProviderConfig,
    skill: &AiSkillDefinition,
    input: &Value,
    image_urls: &[String],
) -> AppResult<Value> {
    request_pi_agent_json(
        app,
        config,
        skill.name,
        skill.version,
        skill.instructions,
        skill.output_schema,
        input,
        image_urls,
    )
    .await
}

pub(in crate::commands) async fn request_pi_agent_json(
    app: &AppHandle,
    config: &AiProviderConfig,
    skill_name: &str,
    skill_version: &str,
    instructions: &str,
    output_schema: &str,
    input: &Value,
    image_urls: &[String],
) -> AppResult<Value> {
    let script_path = resolve_pi_agent_script_path(app);
    if !script_path.exists() {
        return Err(AppError::Validation(format!(
            "pi-coding-agent 运行时脚本不存在：{}",
            script_path.display()
        )));
    }
    let request_body = serde_json::json!({
        "provider_type": config.provider_type,
        "custom_provider_id": config.custom_provider_id,
        "api": config.api,
        "base_url": config.base_url,
        "api_key": config.api_key,
        "model": config.model,
        "temperature": config.temperature,
        "context_window": config.context_window,
        "max_tokens": config.max_tokens,
        "skill_name": skill_name,
        "skill_version": skill_version,
        "instructions": instructions,
        "output_schema": output_schema,
        "input": input,
        "image_urls": image_urls.iter().take(12).cloned().collect::<Vec<_>>()
    });
    let request_json = serde_json::to_vec(&request_body)
        .map_err(|error| AppError::Validation(format!("AI agent 请求无法序列化：{error}")))?;
    let mut command = tokio::process::Command::new(resolve_agent_node_binary());
    command
        .arg(&script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| AppError::Validation(format!("启动 pi-coding-agent 失败：{error}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Validation("pi-coding-agent stdin 不可用".to_string()))?;
    tokio::spawn(async move {
        let _ = stdin.write_all(&request_json).await;
        let _ = stdin.shutdown().await;
    });
    let output = tokio::time::timeout(StdDuration::from_secs(75), child.wait_with_output())
        .await
        .map_err(|_| AppError::Validation(format!("pi-coding-agent {} 执行超时", skill_name)))?
        .map_err(|error| AppError::Validation(format!("等待 pi-coding-agent 失败：{error}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(AppError::Validation(format!(
            "pi-coding-agent {} 调用失败：{}",
            skill_name,
            summarize_agent_stderr(&stderr)
        )));
    }
    if stdout.is_empty() {
        return Err(AppError::Validation(format!(
            "pi-coding-agent {} 未返回输出",
            skill_name
        )));
    }
    serde_json::from_str::<Value>(&stdout).map_err(|error| {
        AppError::Validation(format!(
            "pi-coding-agent {} 返回非 JSON：{}；{}",
            skill_name,
            error,
            truncate_for_summary(&stdout, 240)
        ))
    })
}

pub(in crate::commands) fn resolve_pi_agent_script_path(app: &AppHandle) -> PathBuf {
    let relative = PathBuf::from("scripts")
        .join("pi_agents")
        .join("json_skill_agent.mjs");
    if relative.exists() {
        return relative;
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join(&relative);
        if candidate.exists() {
            return candidate;
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        let candidate = current_dir.join(&relative);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("/Users/wangjunhao/Code/project/wx-xd").join(relative)
}

pub(in crate::commands) fn resolve_agent_node_binary() -> PathBuf {
    if let Ok(path) = std::env::var("WX_XD_AGENT_NODE") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return candidate;
        }
    }
    for path in [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ] {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("node")
}

fn normalize_openai_api_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    let without_endpoint = trimmed
        .strip_suffix("/chat/completions")
        .or_else(|| trimmed.strip_suffix("/responses"))
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if without_endpoint == "https://api.openai.com" {
        "https://api.openai.com/v1".to_string()
    } else {
        without_endpoint.to_string()
    }
}

pub(in crate::commands) fn summarize_agent_stderr(stderr: &str) -> String {
    if stderr.trim().is_empty() {
        return "无 stderr 输出".to_string();
    }
    let raw_error = if let Ok(value) = serde_json::from_str::<Value>(stderr) {
        value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or(stderr)
            .to_string()
    } else {
        stderr.to_string()
    };
    if is_openai_responses_api_404(&raw_error) {
        return "当前 AI Base URL 返回 404；Custom provider 默认按 OpenAI-compatible /chat/completions 调用，请确认 Base URL 是包含 /v1 的兼容网关根路径，或切换到 Pi 内置 provider。".to_string();
    }
    let lower = raw_error.to_ascii_lowercase();
    if lower.contains("cannot find package") || lower.contains("module_not_found") {
        return "pi-coding-agent 运行时无法启动。请确认已执行 npm install 并安装 @mariozechner/pi-coding-agent。".to_string();
    }
    if lower.contains("authentication")
        || lower.contains("api key")
        || lower.contains("unauthorized")
        || lower.contains("401")
    {
        return "pi-coding-agent 模型请求未通过认证。请在设置页保存当前 provider 需要的 API Key，或确认本机 Pi 环境变量/认证文件可用。".to_string();
    }
    truncate_for_summary(&raw_error, 240)
}

fn is_openai_responses_api_404(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let has_404 = lower.contains("404");
    let looks_like_not_found = lower.contains("not found")
        || lower.contains("notfounderror")
        || lower.contains("openresty")
        || lower.contains("error getting response");
    let looks_like_openai_call = lower.contains("response") || lower.contains("openai");
    has_404 && looks_like_not_found && looks_like_openai_call
}

pub(in crate::commands) fn response_summary_for_ai(value: &Value) -> String {
    truncate_for_summary(&value.to_string(), 120)
}

pub(in crate::commands) fn truncate_for_summary(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let summary = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

pub(in crate::commands) async fn fill_attribute_plan_with_ai(
    app: &AppHandle,
    config: &AiProviderConfig,
    plan: &mut AttributeFillPlan,
) -> AppResult<i64> {
    let mut generated = 0i64;
    for suggestion in &mut plan.suggestions {
        if suggestion.applied || suggestion.source != "needs_ai" {
            continue;
        }
        if let Some(ai_suggestion) =
            request_ai_attribute_suggestion(app, config, suggestion).await?
        {
            *suggestion = ai_suggestion;
            generated += 1;
        }
    }
    Ok(generated)
}

pub(in crate::commands) async fn request_ai_attribute_suggestion(
    app: &AppHandle,
    config: &AiProviderConfig,
    base: &AttributeFillSuggestion,
) -> AppResult<Option<AttributeFillSuggestion>> {
    let instruction = "请补齐单个微信小店发品必填属性。返回格式：{\"value\":\"属性值或null\",\"sku_values\":[{\"sku_index\":0,\"value\":\"值\"}],\"confidence\":0-100,\"reason\":\"一句话原因\"}。销售属性如果每个 SKU 不同，优先返回 sku_values。";
    let response = request_pi_agent_json(
        app,
        config,
        ATTRIBUTE_SUGGESTION_SKILL.name,
        ATTRIBUTE_SUGGESTION_SKILL.version,
        instruction,
        ATTRIBUTE_SUGGESTION_SKILL.output_schema,
        &base.prompt_json,
        &[],
    )
    .await?;
    let confidence = json_value_to_i64(response.get("confidence"))
        .unwrap_or(0)
        .clamp(0, 100);
    let allowed_values = response_allowed_values(&base.prompt_json);
    let mut next = base.clone();
    next.confidence = confidence;
    next.source = format!("ai_provider:{}", config.model);
    next.prompt_json = serde_json::json!({
        "request": &base.prompt_json,
        "response": response
    });
    if base.attr_kind == "sale" {
        next.sku_values = response
            .get("sku_values")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let object = item.as_object()?;
                        let sku_index = json_value_to_i64(object.get("sku_index"))?;
                        let value = json_value_to_string(object.get("value"))?;
                        Some(SkuAttrFill {
                            sku_index: sku_index.max(0) as usize,
                            value: normalize_suggested_value(&value, &allowed_values),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if next.sku_values.is_empty() {
            if let Some(value) = json_value_to_string(response.get("value")) {
                let value = normalize_suggested_value(&value, &allowed_values);
                if !value.is_empty() && value != "null" {
                    next.suggested_value = Some(value);
                }
            }
        } else {
            let summary = next
                .sku_values
                .iter()
                .map(|value| value.value.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("/");
            next.suggested_value = Some(format!("按 SKU 映射：{summary}"));
        }
    } else if let Some(value) = json_value_to_string(response.get("value")) {
        let value = normalize_suggested_value(&value, &allowed_values);
        if !value.is_empty() && value != "null" {
            next.suggested_value = Some(value);
        }
    }
    next.applied = confidence >= 85
        && (next.suggested_value.is_some() || !next.sku_values.is_empty())
        && suggestion_values_allowed(&next, &allowed_values);
    Ok(Some(next))
}

pub(in crate::commands) fn response_allowed_values(prompt_json: &Value) -> Vec<String> {
    prompt_json
        .get("allowed_values")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_value_to_string(Some(item)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(in crate::commands) fn suggestion_values_allowed(
    suggestion: &AttributeFillSuggestion,
    allowed_values: &[String],
) -> bool {
    if allowed_values.is_empty() {
        return true;
    }
    if !suggestion.sku_values.is_empty() {
        return suggestion
            .sku_values
            .iter()
            .all(|value| allowed_values.contains(&value.value));
    }
    suggestion
        .suggested_value
        .as_ref()
        .is_some_and(|value| allowed_values.contains(value))
}
