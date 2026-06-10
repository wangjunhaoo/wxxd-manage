use super::*;

pub(in crate::commands) fn get_string_setting(
    conn: &Connection,
    key: &str,
) -> AppResult<Option<String>> {
    let value = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value
        .and_then(|raw| serde_json::from_str::<String>(&raw).ok())
        .filter(|value| !value.trim().is_empty()))
}

pub(in crate::commands) fn set_string_setting(
    conn: &Connection,
    key: &str,
    value: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = excluded.updated_at",
        params![key, serde_json::json!(value).to_string(), now_shanghai()],
    )?;
    Ok(())
}

pub(in crate::commands) fn get_bool_setting(
    conn: &Connection,
    key: &str,
    default_value: bool,
) -> AppResult<bool> {
    let value = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value
        .and_then(|raw| serde_json::from_str::<bool>(&raw).ok())
        .unwrap_or(default_value))
}

pub(in crate::commands) fn default_publish_pricing_strategy() -> PublishPricingStrategy {
    PublishPricingStrategy {
        sale_price_markup_rate: 1.6,
        sale_price_fixed_cents: 0,
        sale_price_floor_cents: 100,
    }
}

pub(in crate::commands) fn validate_publish_pricing_strategy(
    strategy: &PublishPricingStrategy,
) -> AppResult<()> {
    if !strategy.sale_price_markup_rate.is_finite()
        || strategy.sale_price_markup_rate <= 0.0
        || strategy.sale_price_markup_rate > 100.0
    {
        return Err(AppError::Validation(
            "售价加价倍率必须大于 0 且不超过 100".to_string(),
        ));
    }
    if strategy.sale_price_fixed_cents < 0 || strategy.sale_price_fixed_cents > 10_000_000 {
        return Err(AppError::Validation(
            "固定加价必须大于等于 0 且不超过 100000 元".to_string(),
        ));
    }
    if strategy.sale_price_floor_cents <= 0 || strategy.sale_price_floor_cents > 10_000_000 {
        return Err(AppError::Validation(
            "最低售价必须大于 0 且不超过 100000 元".to_string(),
        ));
    }
    Ok(())
}

pub(in crate::commands) fn load_publish_pricing_strategy(
    conn: &Connection,
) -> AppResult<PublishPricingStrategy> {
    let Some(raw) = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [PUBLISH_PRICING_STRATEGY_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Ok(default_publish_pricing_strategy());
    };
    let strategy = serde_json::from_str::<PublishPricingStrategy>(&raw)
        .unwrap_or_else(|_| default_publish_pricing_strategy());
    validate_publish_pricing_strategy(&strategy)?;
    Ok(strategy)
}

pub(in crate::commands) fn save_publish_pricing_strategy_to_db(
    conn: &Connection,
    strategy: &PublishPricingStrategy,
) -> AppResult<()> {
    validate_publish_pricing_strategy(strategy)?;
    let value_json = serde_json::to_string(strategy)
        .map_err(|error| AppError::Validation(format!("价格策略序列化失败：{error}")))?;
    conn.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = excluded.updated_at",
        params![PUBLISH_PRICING_STRATEGY_SETTING, value_json, now_shanghai()],
    )?;
    Ok(())
}

/// 读取所有店铺的默认运费模板映射 {shop_id: template_id}。无配置时返回空 map。
pub(in crate::commands) fn load_publish_default_freight_templates(
    conn: &Connection,
) -> AppResult<std::collections::HashMap<String, String>> {
    let Some(raw) = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [PUBLISH_DEFAULT_FREIGHT_TEMPLATES_SETTING],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Ok(std::collections::HashMap::new());
    };
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// 读取单个店铺指定的默认运费模板 ID（未指定返回 None）。
pub(in crate::commands) fn load_shop_default_freight_template(
    conn: &Connection,
    shop_id: &str,
) -> AppResult<Option<String>> {
    Ok(load_publish_default_freight_templates(conn)?.remove(shop_id))
}

/// 设置/清除单个店铺的默认运费模板（template_id 为 None 表示清除该店铺的指定）。
pub(in crate::commands) fn save_shop_default_freight_template_to_db(
    conn: &Connection,
    shop_id: &str,
    template_id: Option<&str>,
) -> AppResult<()> {
    let mut map = load_publish_default_freight_templates(conn)?;
    match template_id {
        Some(tid) => {
            map.insert(shop_id.to_string(), tid.to_string());
        }
        None => {
            map.remove(shop_id);
        }
    }
    let value_json = serde_json::to_string(&map)
        .map_err(|error| AppError::Validation(format!("默认运费模板序列化失败：{error}")))?;
    conn.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = excluded.updated_at",
        params![
            PUBLISH_DEFAULT_FREIGHT_TEMPLATES_SETTING,
            value_json,
            now_shanghai()
        ],
    )?;
    Ok(())
}

/// 校验某运费模板 ID 是否在指定店铺已同步的运费模板列表中。
pub(in crate::commands) fn cached_freight_template_exists(
    conn: &Connection,
    shop_id: &str,
    template_id: &str,
) -> AppResult<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM wechat_freight_templates WHERE shop_id = ?1 AND template_id = ?2 LIMIT 1",
            params![shop_id, template_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

pub(in crate::commands) fn apply_publish_pricing_strategy(
    product: &mut ExternalProductInput,
    strategy: &PublishPricingStrategy,
) {
    if !product.metadata.is_object() {
        product.metadata = Value::Object(serde_json::Map::new());
    }
    if let Some(metadata) = product.metadata.as_object_mut() {
        metadata.insert(
            "sale_price_markup_rate".to_string(),
            Value::from(strategy.sale_price_markup_rate),
        );
        metadata.insert(
            "sale_price_fixed_cents".to_string(),
            Value::from(strategy.sale_price_fixed_cents),
        );
        metadata.insert(
            "sale_price_floor_cents".to_string(),
            Value::from(strategy.sale_price_floor_cents),
        );
    }
}

pub(in crate::commands) fn default_automation_settings() -> OperationalAutomationSettings {
    OperationalAutomationSettings {
        order_sync_enabled: true,
        order_detail_sync_enabled: true,
        aftersale_sync_enabled: true,
        purchase_task_enabled: true,
        delivery_submission_enabled: true,
        publish_enabled: true,
        price_confirm_enabled: true,
    }
}

pub(in crate::commands) fn load_automation_settings(
    conn: &Connection,
) -> AppResult<OperationalAutomationSettings> {
    let defaults = default_automation_settings();
    Ok(OperationalAutomationSettings {
        order_sync_enabled: get_bool_setting(
            conn,
            AUTOMATION_ORDER_SYNC_SETTING,
            defaults.order_sync_enabled,
        )?,
        order_detail_sync_enabled: get_bool_setting(
            conn,
            AUTOMATION_ORDER_DETAIL_SYNC_SETTING,
            defaults.order_detail_sync_enabled,
        )?,
        aftersale_sync_enabled: get_bool_setting(
            conn,
            AUTOMATION_AFTERSALE_SYNC_SETTING,
            defaults.aftersale_sync_enabled,
        )?,
        purchase_task_enabled: get_bool_setting(
            conn,
            AUTOMATION_PURCHASE_TASK_SETTING,
            defaults.purchase_task_enabled,
        )?,
        delivery_submission_enabled: get_bool_setting(
            conn,
            AUTOMATION_DELIVERY_SUBMISSION_SETTING,
            defaults.delivery_submission_enabled,
        )?,
        publish_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_ENABLED_SETTING,
            defaults.publish_enabled,
        )?,
        price_confirm_enabled: get_bool_setting(
            conn,
            AUTOMATION_PRICE_CONFIRM_SETTING,
            defaults.price_confirm_enabled,
        )?,
    })
}

pub(in crate::commands) fn save_automation_settings(
    conn: &Connection,
    settings: &OperationalAutomationSettings,
) -> AppResult<()> {
    set_bool_setting(
        conn,
        AUTOMATION_ORDER_SYNC_SETTING,
        settings.order_sync_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_ORDER_DETAIL_SYNC_SETTING,
        settings.order_detail_sync_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_AFTERSALE_SYNC_SETTING,
        settings.aftersale_sync_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PURCHASE_TASK_SETTING,
        settings.purchase_task_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_DELIVERY_SUBMISSION_SETTING,
        settings.delivery_submission_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_ENABLED_SETTING,
        settings.publish_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PRICE_CONFIRM_SETTING,
        settings.price_confirm_enabled,
    )?;
    Ok(())
}

pub(in crate::commands) fn push_automation_error(
    result: &mut OperationalAutomationRunResult,
    step: &str,
    error: AppError,
) {
    result.errors.push(AutomationStepError {
        step: step.to_string(),
        error: error.to_string(),
    });
}

pub(in crate::commands) fn merge_attribute_fill_results(
    left: PublishAttributeFillBatchResult,
    right: PublishAttributeFillBatchResult,
) -> PublishAttributeFillBatchResult {
    PublishAttributeFillBatchResult {
        processed_jobs: (left.processed_jobs + right.processed_jobs).min(i64::MAX),
        processed_items: left.processed_items + right.processed_items,
        auto_filled_items: left.auto_filled_items + right.auto_filled_items,
        suggestion_only_items: left.suggestion_only_items + right.suggestion_only_items,
        failed_items: left.failed_items + right.failed_items,
        generated_suggestions: left.generated_suggestions + right.generated_suggestions,
    }
}
