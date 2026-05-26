use super::*;

#[tauri::command]
pub fn get_ai_provider_settings(app: AppHandle) -> AppResult<AiProviderSettings> {
    let conn = open_connection(&app)?;
    load_ai_provider_settings(&conn)
}

#[tauri::command]
pub fn save_ai_provider_settings(
    app: AppHandle,
    request: AiProviderSettingsRequest,
) -> AppResult<AiProviderSettings> {
    let conn = open_connection(&app)?;
    let provider_type = normalize_ai_provider_type(&request.provider_type)?;
    let base_url = normalize_ai_base_url(&request.base_url)?;
    let model = request.model.trim().to_string();
    let temperature = request.temperature.unwrap_or(0.1).clamp(0.0, 1.0);
    if request.enabled {
        if base_url.is_empty() {
            return Err(AppError::Validation(
                "启用 AI 前必须填写 base_url".to_string(),
            ));
        }
        if model.is_empty() {
            return Err(AppError::Validation("启用 AI 前必须填写 model".to_string()));
        }
        let has_existing_key = ai_api_key_record(&conn)?.is_some();
        let has_new_key = request
            .api_key
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if !has_existing_key && !has_new_key {
            return Err(AppError::Validation(
                "启用 AI 前必须保存 API Key；API Key 会加密存储".to_string(),
            ));
        }
    }

    set_bool_setting(&conn, AI_PROVIDER_ENABLED_SETTING, request.enabled)?;
    set_string_setting(&conn, AI_PROVIDER_TYPE_SETTING, &provider_type)?;
    set_string_setting(&conn, AI_PROVIDER_BASE_URL_SETTING, &base_url)?;
    set_string_setting(&conn, AI_PROVIDER_MODEL_SETTING, &model)?;
    set_string_setting(
        &conn,
        AI_PROVIDER_TEMPERATURE_SETTING,
        &temperature.to_string(),
    )?;

    if request.clear_api_key {
        conn.execute(
            "DELETE FROM ai_provider_credentials WHERE id = ?1",
            [AI_PROVIDER_ID],
        )?;
    } else if let Some(api_key) = request
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        save_ai_api_key(&app, &conn, api_key)?;
    }

    load_ai_provider_settings(&conn)
}

#[tauri::command]
pub async fn test_ai_provider(app: AppHandle) -> AppResult<AiProviderTestResult> {
    let config = load_ai_provider_config(&app)?;
    let response = request_ai_chat_json(
        &config,
        "只返回 JSON 对象：{\"ok\":true}",
        &serde_json::json!({
            "task": "连通性测试",
            "expected": { "ok": true }
        }),
    )
    .await?;
    Ok(AiProviderTestResult {
        status: "success".to_string(),
        provider_type: config.provider_type,
        model: config.model,
        message: format!(
            "AI provider 连通成功：{}",
            response_summary_for_ai(&response)
        ),
    })
}
