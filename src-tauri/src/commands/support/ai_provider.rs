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
        "image_urls": image_urls.iter().take(12).cloned().collect::<Vec<_>>(),
        "agent_api_base_url": "http://127.0.0.1:17890"
    });
    let request_json = serde_json::to_vec(&request_body)
        .map_err(|error| AppError::Validation(format!("AI agent 请求无法序列化：{error}")))?;

    // 限流冷却：命中过 429 后短时间内直接快速失败，不再 spawn 子进程去捅 provider，
    // 避免 driver 每 8s tick 反复打爆配额(配合审查侧「限流保持 collecting」形成温和退避)。
    if ai_provider_in_cooldown() {
        return Err(AppError::Validation(format!(
            "pi-coding-agent {} 跳过：AI provider 限流冷却中，稍后自动重试",
            skill_name
        )));
    }

    let node_binary = resolve_agent_node_binary(app);
    let timeout = ai_agent_timeout_duration();
    let max_attempts = ai_provider_max_attempts();
    let mut attempt = 0u32;
    loop {
        let mut command = tokio::process::Command::new(&node_binary);
        command
            .arg(&script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| AppError::Validation(format!("启动 pi-coding-agent 失败：{error}")))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Validation("pi-coding-agent stdin 不可用".to_string()))?;
        let request_body_bytes = request_json.clone();
        tokio::spawn(async move {
            let _ = stdin.write_all(&request_body_bytes).await;
            let _ = stdin.shutdown().await;
        });
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| AppError::Validation(format!("pi-coding-agent {} 执行超时", skill_name)))?
            .map_err(|error| AppError::Validation(format!("等待 pi-coding-agent 失败：{error}")))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !output.status.success() {
            let detail = summarize_agent_stderr(&stderr);
            // 仅对「限流类快速失败」做有限退避重试(429 通常子进程快速非 0 退出，
            // 重试总耗时远小于 tick 超时)；其它失败立即返回，不浪费时间。
            if is_ai_provider_rate_limited(&detail) {
                attempt += 1;
                if attempt < max_attempts {
                    let backoff_ms = 1000u64 * u64::from(attempt); // 1s, 2s
                    tokio::time::sleep(StdDuration::from_millis(backoff_ms)).await;
                    continue;
                }
                // 重试用尽仍限流：进入冷却期，让后续调用快速失败、温和退避不再硬扛。
                set_ai_provider_cooldown();
            }
            return Err(AppError::Validation(format!(
                "pi-coding-agent {} 调用失败：{}",
                skill_name, detail
            )));
        }
        if stdout.is_empty() {
            return Err(AppError::Validation(format!(
                "pi-coding-agent {} 未返回输出",
                skill_name
            )));
        }
        return parse_agent_stdout_json(&stdout).map_err(|error| {
            AppError::Validation(format!(
                "pi-coding-agent {} 返回非 JSON：{}；{}",
                skill_name,
                error,
                truncate_for_summary(&stdout, 240)
            ))
        });
    }
}

fn ai_provider_max_attempts() -> u32 {
    std::env::var("WX_XD_AI_RETRY_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(2)
        .clamp(1, 3)
}

/// 是否命中 AI provider 限流(429/Too many requests/限流)。与审查侧 is_ai_rate_limit_error 同义，
/// 这里独立一份纯函数，避免跨模块耦合。
fn is_ai_provider_rate_limited(summary: &str) -> bool {
    summary.contains("429")
        || summary.contains("Too many requests")
        || summary.contains("too many requests")
        || summary.contains("limitation")
        || summary.contains("限流")
}

fn ai_provider_cooldown_cell() -> &'static std::sync::Mutex<Option<std::time::Instant>> {
    static CELL: std::sync::OnceLock<std::sync::Mutex<Option<std::time::Instant>>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| std::sync::Mutex::new(None))
}

fn ai_provider_in_cooldown() -> bool {
    ai_provider_cooldown_cell()
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .is_some_and(|until| std::time::Instant::now() < until)
}

fn set_ai_provider_cooldown() {
    let seconds = std::env::var("WX_XD_AI_COOLDOWN_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(60)
        .clamp(5, 600);
    if let Ok(mut guard) = ai_provider_cooldown_cell().lock() {
        *guard = Some(std::time::Instant::now() + StdDuration::from_secs(seconds));
    }
}

fn ai_agent_timeout_duration() -> StdDuration {
    let seconds = std::env::var("WX_XD_AI_AGENT_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(180)
        .clamp(30, 600);
    StdDuration::from_secs(seconds)
}

fn parse_agent_stdout_json(stdout: &str) -> Result<Value, serde_json::Error> {
    let trimmed = stdout.trim();
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => return Ok(value),
        Err(error) => {
            if let Some(candidate) = extract_json_candidate(trimmed) {
                return serde_json::from_str::<Value>(&candidate);
            }
            Err(error)
        }
    }
}

fn extract_json_candidate(value: &str) -> Option<String> {
    if let Some(start) = value.find("```") {
        let after_start = &value[start + 3..];
        let content = after_start.strip_prefix("json").unwrap_or(after_start);
        if let Some(end) = content.find("```") {
            let candidate = content[..end].trim();
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }
    let object = json_slice_between(value, '{', '}');
    let array = json_slice_between(value, '[', ']');
    match (object, array) {
        (Some(object), Some(array)) => {
            if object.0 <= array.0 {
                Some(object.1)
            } else {
                Some(array.1)
            }
        }
        (Some(object), None) => Some(object.1),
        (None, Some(array)) => Some(array.1),
        (None, None) => None,
    }
}

fn json_slice_between(value: &str, left: char, right: char) -> Option<(usize, String)> {
    let start = value.find(left)?;
    let chars: Vec<char> = value[start..].chars().collect();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut end_idx = 0usize;
    for (i, ch) in chars.iter().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if *ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if *ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if *ch == left {
            depth += 1;
        }
        if *ch == right {
            depth -= 1;
            if depth == 0 {
                end_idx = i;
                break;
            }
        }
    }
    if depth != 0 {
        return None;
    }
    let end_byte = start + chars[..=end_idx].iter().collect::<String>().len();
    Some((start, value[start..end_byte].to_string()))
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
    relative
}

pub(in crate::commands) fn resolve_agent_node_binary(app: &AppHandle) -> PathBuf {
    if let Ok(path) = std::env::var("WX_XD_AGENT_NODE") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return candidate;
        }
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join("bin").join("node");
        if candidate.exists() {
            return candidate;
        }
    }
    let local_candidate = PathBuf::from("runtime").join("bin").join("node");
    if local_candidate.exists() {
        return local_candidate;
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
    let pending = plan
        .suggestions
        .iter()
        .enumerate()
        .filter_map(|(index, suggestion)| {
            (!suggestion.applied && suggestion.source == "needs_ai").then_some(index)
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(0);
    }
    match request_ai_attribute_suggestions_batch(app, config, plan, &pending).await {
        Ok(generated) => Ok(generated),
        Err(error) => {
            let summary = truncate_for_summary(&error.to_string(), 240);
            for index in pending {
                let request = plan.suggestions[index].prompt_json.clone();
                plan.suggestions[index].prompt_json = serde_json::json!({
                    "request": request,
                    "ai_error": &summary
                });
            }
            // 静默降级：AI 调用失败时不中断循环，让其他商品继续处理
            Ok(0)
        }
    }
}

pub(in crate::commands) async fn request_ai_attribute_suggestion(
    app: &AppHandle,
    config: &AiProviderConfig,
    base: &AttributeFillSuggestion,
) -> AppResult<Option<AttributeFillSuggestion>> {
    let instruction = "请补齐单个微信小店发品必填属性。返回格式：{\"value\":\"属性值或null\",\"sku_values\":[{\"sku_index\":0,\"value\":\"值\"}],\"confidence\":0-100,\"reason\":\"一句话原因\"}。\n\
        【核心规则】allowed_values 非空时，value 必须从中**原样复制**一个值，不允许缩写、近义词或自己造值。例如 allowed_values=[\"纯棉\",\"涤纶\"]，必须返回\"纯棉\"，不能返回\"棉\"。\n\
        基于标题、类目、SKU、外部元数据和 allowed_values 做语义推断。external_context.known_attrs 是已确定的同商品其他属性，请利用它们做关联推断。只有所有候选都明显不匹配时才返回 null。销售属性每个 SKU 不同时优先返回 sku_values。";
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
    Ok(Some(attribute_suggestion_from_ai_response(
        config, base, &response,
    )))
}

async fn request_ai_attribute_suggestions_batch(
    app: &AppHandle,
    config: &AiProviderConfig,
    plan: &mut AttributeFillPlan,
    pending: &[usize],
) -> AppResult<i64> {
    // 从第一个 suggestion 的 prompt_json 中提取商品上下文
    let first_prompt = pending
        .first()
        .and_then(|i| plan.suggestions.get(*i))
        .map(|s| &s.prompt_json);
    let Some(first_prompt) = first_prompt else {
        return Ok(0);
    };

    // 构建缺失属性列表（含类型和允许值）
    let missing_attrs: Vec<Value> = pending
        .iter()
        .filter_map(|index| {
            plan.suggestions.get(*index).map(|suggestion| {
                let allowed_values = response_allowed_values(&suggestion.prompt_json);
                let attr_type = suggestion
                    .prompt_json
                    .get("attr_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("string");
                serde_json::json!({
                    "index": index,
                    "attr_kind": suggestion.attr_kind,
                    "attr_key": &suggestion.attr_key,
                    "attr_type": attr_type,
                    "allowed_values": allowed_values,
                })
            })
        })
        .collect();

    // 构建完整上下文：商品数据 + 缺失属性定义
    let context = serde_json::json!({
        "product": {
            "title": first_prompt.get("title"),
            "category_hint": first_prompt.get("category_hint"),
            "brand_hint": first_prompt.get("brand_hint"),
            "external_product_id": first_prompt.get("external_product_id"),
            "skus": first_prompt.get("skus"),
            "external_context": first_prompt.get("external_context"),
        },
        "missing_attrs": missing_attrs,
    });

    let instruction = "你是微信小店发品属性补齐助手。根据商品数据，一次性补齐所有缺失的必填属性。\n\n\
        【核心规则 — 必须严格遵守】\n\
        1. select_one / select_many 类型：value 必须从 allowed_values 数组中**原样复制**一个值，一个字都不能改。不允许缩写、不允许近义词、不允许自己造值。\n\
        2. string 类型：根据商品信息填写合理的文本值。\n\
        3. 销售属性(attr_kind=sale)：为每个 SKU 在 sku_values 中分别填写值，每个 value 也必须从 allowed_values 中原样选取。\n\
        4. 基于标题、类目、SKU 规格、external_context 做语义推断。\n\
        5. 各属性之间关联推断（如面料材质→成分含量、标题→适用年龄/风格）。\n\
        6. 如果 allowed_values 中的所有选项都不匹配商品信息，才返回 null。\n\
        7. confidence：确定能从 allowed_values 中找到匹配 → 85-95；有依据但略有不确定 → 65-84；完全无法判断 → 0。\n\n\
        【错误示例】allowed_values 是 [\"纯棉\",\"涤纶\",\"锦纶\"]，你返回 \"棉\" → ❌ 错误！必须返回 \"纯棉\"。\n\
        【正确示例】allowed_values 是 [\"纯棉\",\"涤纶\",\"锦纶\"]，你返回 \"纯棉\" → ✅ 正确！\n\n\
        返回 JSON：{\"suggestions\":[{\"index\":0,\"attr_kind\":\"product或sale\",\"attr_key\":\"属性名\",\"value\":\"从allowed_values原样复制的值或null\",\"sku_values\":[{\"sku_index\":0,\"value\":\"从allowed_values原样复制的值\"}],\"confidence\":0-100,\"reason\":\"依据\"}]}";

    let output_schema = r#"{
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "suggestions": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
              "index": { "type": "integer", "minimum": 0 },
              "attr_kind": { "type": "string" },
              "attr_key": { "type": "string" },
              "value": { "type": ["string", "null"] },
              "sku_values": {
                "type": "array",
                "items": {
                  "type": "object",
                  "additionalProperties": false,
                  "properties": {
                    "sku_index": { "type": "integer", "minimum": 0 },
                    "value": { "type": "string" }
                  },
                  "required": ["sku_index", "value"]
                }
              },
              "confidence": { "type": "integer", "minimum": 0, "maximum": 100 },
              "reason": { "type": "string" }
            },
            "required": ["index", "attr_kind", "attr_key", "value", "sku_values", "confidence", "reason"]
          }
        }
      },
      "required": ["suggestions"]
    }"#;
    let response = request_pi_agent_json(
        app,
        config,
        ATTRIBUTE_SUGGESTION_SKILL.name,
        ATTRIBUTE_SUGGESTION_SKILL.version,
        instruction,
        output_schema,
        &context,
        &[],
    )
    .await?;
    let Some(items) = response.get("suggestions").and_then(Value::as_array) else {
        return Ok(0);
    };
    let mut generated = 0i64;
    for (fallback_order, item) in items.iter().enumerate() {
        let Some(index) = match_ai_batch_response_index(item, pending, fallback_order) else {
            continue;
        };
        let Some(base) = plan.suggestions.get(index).cloned() else {
            continue;
        };
        if !ai_batch_response_matches_base(item, &base) {
            continue;
        }
        let next = attribute_suggestion_from_ai_response(config, &base, item);
        if next.suggested_value.is_some() || !next.sku_values.is_empty() {
            plan.suggestions[index] = next;
            generated += 1;
        }
    }
    Ok(generated)
}

fn match_ai_batch_response_index(
    item: &Value,
    pending: &[usize],
    _fallback_order: usize,
) -> Option<usize> {
    json_value_to_i64(item.get("index"))
        .and_then(|value| usize::try_from(value).ok())
        .filter(|index| pending.contains(index))
}

fn ai_batch_response_matches_base(item: &Value, base: &AttributeFillSuggestion) -> bool {
    let attr_kind = json_value_to_string(item.get("attr_kind"));
    let attr_key = json_value_to_string(item.get("attr_key"));
    attr_kind
        .as_deref()
        .is_none_or(|value| value == base.attr_kind)
        && attr_key
            .as_deref()
            .is_none_or(|value| value == base.attr_key)
}

fn attribute_suggestion_from_ai_response(
    config: &AiProviderConfig,
    base: &AttributeFillSuggestion,
    response: &Value,
) -> AttributeFillSuggestion {
    let mut confidence = json_value_to_i64(response.get("confidence"))
        .unwrap_or(0)
        .clamp(0, 100);
    let allowed_values = response_allowed_values(&base.prompt_json);
    let mut next = base.clone();
    next.source = format!("ai_provider:{}", config.model);
    next.prompt_json = serde_json::json!({
        "request": &base.prompt_json,
        "response": response
    });

    // 提取 AI 返回的原始值
    let raw_value = json_value_to_string(response.get("value"))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && v != "null");

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
                            value: force_match_allowed_value(&value, &allowed_values),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if next.sku_values.is_empty() {
            if let Some(value) = raw_value {
                let matched = force_match_allowed_value(&value, &allowed_values);
                next.suggested_value = Some(matched);
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
    } else if let Some(value) = raw_value {
        let matched = force_match_allowed_value(&value, &allowed_values);
        // 如果 forced match 返回的值和 AI 原始值不同，说明 AI 没有严格从列表中选，降低置信度
        if matched != value && !allowed_values.is_empty() {
            confidence = (confidence - 10).max(60);
        }
        next.suggested_value = Some(matched);
    }

    next.confidence = confidence;
    next.applied = confidence >= 65
        && (next.suggested_value.is_some() || !next.sku_values.is_empty())
        && suggestion_values_allowed(&next, &allowed_values);
    next
}

/// 将 AI 返回值强制匹配到 allowed_values 中：先尝试 normalize_suggested_value，
/// 如果仍不匹配则返回原值（由 can_auto_apply 决定是否采纳）
fn force_match_allowed_value(raw: &str, allowed_values: &[String]) -> String {
    if allowed_values.is_empty() {
        return raw.trim().to_string();
    }
    let normalized = normalize_suggested_value(raw, allowed_values);
    // 归一化后的值是否在允许列表中（完全匹配或包含匹配）
    if allowed_values
        .iter()
        .any(|opt| opt.trim() == normalized.trim())
    {
        return normalized;
    }
    // 仍不匹配：保留归一化值，由后续 suggestion_values_allowed 判断
    normalized
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_stdout_json_accepts_fenced_json() {
        let raw = "```json\n{\"value\":\"A类\",\"sku_values\":[],\"confidence\":90,\"reason\":\"标题包含\"}\n```";

        let parsed = parse_agent_stdout_json(raw).expect("应能提取代码块 JSON");

        assert_eq!(parsed.get("value").and_then(Value::as_str), Some("A类"));
    }

    #[test]
    fn parse_agent_stdout_json_accepts_json_with_leading_text() {
        let raw = "结果如下：{\"value\":\"纯棉\",\"sku_values\":[],\"confidence\":88,\"reason\":\"标题包含纯棉\"}";

        let parsed = parse_agent_stdout_json(raw).expect("应能提取文本中的 JSON 对象");

        assert_eq!(parsed.get("value").and_then(Value::as_str), Some("纯棉"));
    }
}
