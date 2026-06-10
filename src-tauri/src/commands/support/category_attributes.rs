use super::*;

pub(in crate::commands) fn check_cached_category_requirements(
    conn: &Connection,
    shop_id: &str,
    cat_id: i64,
    payload: &Value,
) -> AppResult<CachedCategoryRequirementCheck> {
    let Some(raw_detail) = load_cached_category_detail_payload(conn, shop_id, cat_id)? else {
        return Ok(CachedCategoryRequirementCheck::default());
    };
    Ok(check_category_requirements_from_detail(
        &raw_detail,
        payload,
    ))
}

pub(in crate::commands) fn check_category_requirements_from_detail(
    raw_detail: &Value,
    payload: &Value,
) -> CachedCategoryRequirementCheck {
    let product_required = required_category_attrs(&raw_detail, "product_attr_list");
    let sale_required = required_category_attrs(&raw_detail, "sale_attr_list");
    let product_keys = payload_product_attr_keys(payload);
    let sale_keys = payload_sale_attr_keys(payload);

    CachedCategoryRequirementCheck {
        missing_product_attrs: product_required
            .into_iter()
            .filter(|name| !product_keys.contains(name))
            .collect(),
        missing_sale_attrs: sale_required
            .into_iter()
            .filter(|name| !sale_keys.contains(name))
            .collect(),
    }
}

#[derive(Debug, Default, Clone)]
pub(in crate::commands) struct CategoryAttrSanitizeReport {
    changed_attrs: Vec<String>,
    removed_attrs: Vec<String>,
}

impl CategoryAttrSanitizeReport {
    pub(in crate::commands) fn has_changes(&self) -> bool {
        !self.changed_attrs.is_empty() || !self.removed_attrs.is_empty()
    }

    pub(in crate::commands) fn has_removed(&self) -> bool {
        !self.removed_attrs.is_empty()
    }

    pub(in crate::commands) fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.changed_attrs.is_empty() {
            parts.push(format!(
                "已修正 {} 个类目属性值：{}",
                self.changed_attrs.len(),
                self.changed_attrs
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("、")
            ));
        }
        if !self.removed_attrs.is_empty() {
            parts.push(format!(
                "已移除 {} 个无法匹配微信枚举的属性：{}",
                self.removed_attrs.len(),
                self.removed_attrs
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("、")
            ));
        }
        parts.join("；")
    }
}

pub(in crate::commands) fn sanitize_payload_attrs_with_category_detail(
    product: &ExternalProductInput,
    payload: &mut Value,
    raw_detail: &Value,
) -> AppResult<CategoryAttrSanitizeReport> {
    let product_specs = category_attr_specs_by_key(raw_detail, "product_attr_list");
    let sale_specs = category_attr_specs_by_key(raw_detail, "sale_attr_list");
    // 必填名单：sanitize 删除非法/空值前，只对「必填」属性兜底合法值(非必填仍按原逻辑删除)，
    // 避免删空必填项后被 check_category_requirements 判缺 → CATEGORY_ATTRS_NEED_AI_FILL 卡 submit。
    let product_required = required_category_attrs(raw_detail, "product_attr_list");
    let sale_required = required_category_attrs(raw_detail, "sale_attr_list");
    let mut report = CategoryAttrSanitizeReport::default();

    if let Some(attrs) = payload.get_mut("attrs").and_then(Value::as_array_mut) {
        sanitize_payload_attr_array(
            product,
            attrs,
            &product_specs,
            &product_required,
            "商品",
            &mut report,
        )?;
    }

    if let Some(skus) = payload.get_mut("skus").and_then(Value::as_array_mut) {
        for (index, sku) in skus.iter_mut().enumerate() {
            if let Some(attrs) = sku.get_mut("sku_attrs").and_then(Value::as_array_mut) {
                sanitize_payload_attr_array(
                    product,
                    attrs,
                    &sale_specs,
                    &sale_required,
                    &format!("第{}个 SKU", index + 1),
                    &mut report,
                )?;
            }
        }
    }

    Ok(report)
}

pub(in crate::commands) fn normalize_attr_value_for_category_spec(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
    value: &str,
) -> Option<String> {
    normalize_payload_attr_value_for_spec(product, spec, value)
}

pub(in crate::commands) fn load_cached_category_detail_payload(
    conn: &Connection,
    shop_id: &str,
    cat_id: i64,
) -> AppResult<Option<Value>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT raw_payload FROM wechat_category_details WHERE shop_id = ?1 AND cat_id = ?2",
            params![shop_id, cat_id],
            |row| row.get(0),
        )
        .optional()?;
    raw.map(|raw| {
        serde_json::from_str::<Value>(&raw).map_err(|error| {
            AppError::Validation(format!(
                "类目详情缓存 JSON 解析失败，cat_id={cat_id}：{error}"
            ))
        })
    })
    .transpose()
}

pub(in crate::commands) fn required_category_attrs(
    raw_payload: &Value,
    list_key: &str,
) -> BTreeSet<String> {
    let mut attrs = BTreeSet::new();
    collect_required_category_attrs(raw_payload, list_key, &mut attrs);
    attrs
}

pub(in crate::commands) fn collect_required_category_attrs(
    value: &Value,
    list_key: &str,
    attrs: &mut BTreeSet<String>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_required_category_attrs(item, list_key, attrs);
            }
        }
        Value::Object(object) => {
            if let Some(items) = object.get(list_key).and_then(Value::as_array) {
                for item in items {
                    if let Some(attr) = item.as_object() {
                        if is_required_category_attr(attr) {
                            if let Some(name) = category_attr_name(attr) {
                                attrs.insert(name);
                            }
                        }
                    }
                }
            }
            for child in object.values() {
                collect_required_category_attrs(child, list_key, attrs);
            }
        }
        _ => {}
    }
}

pub(in crate::commands) fn is_required_category_attr(
    object: &serde_json::Map<String, Value>,
) -> bool {
    ["is_required", "required", "mandatory", "is_mandatory"]
        .iter()
        .any(|key| json_value_to_bool(object.get(*key)).unwrap_or(false))
        || object
            .get("required_rule")
            .and_then(Value::as_object)
            .and_then(|rule| json_value_to_i64(rule.get("rule_type")))
            == Some(1)
}

pub(in crate::commands) fn category_attr_name(
    object: &serde_json::Map<String, Value>,
) -> Option<String> {
    ["name", "attr_key", "attr_name", "key"]
        .iter()
        .find_map(|key| json_value_to_string(object.get(*key)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(in crate::commands) fn payload_product_attr_keys(payload: &Value) -> BTreeSet<String> {
    payload
        .get("attrs")
        .and_then(Value::as_array)
        .map(|items| payload_attr_keys_from_array(items))
        .unwrap_or_default()
}

pub(in crate::commands) fn payload_sale_attr_keys(payload: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(skus) = payload.get("skus").and_then(Value::as_array) {
        for sku in skus {
            if let Some(attrs) = sku.get("sku_attrs").and_then(Value::as_array) {
                keys.extend(payload_attr_keys_from_array(attrs));
            }
        }
    }
    keys
}

pub(in crate::commands) fn payload_attr_keys_from_array(items: &[Value]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|object| {
            ["attr_key", "name", "attr_name", "key"]
                .iter()
                .find_map(|key| json_value_to_string(object.get(*key)))
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(in crate::commands) fn build_attribute_fill_plan(
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    payload: &Value,
    raw_detail: &Value,
    requirement_check: &CachedCategoryRequirementCheck,
) -> AttributeFillPlan {
    let product_specs = required_category_attr_specs(raw_detail, "product_attr_list");
    let sale_specs = required_category_attr_specs(raw_detail, "sale_attr_list");
    let mut plan = AttributeFillPlan::default();
    for attr_key in &requirement_check.missing_product_attrs {
        let spec = product_specs
            .get(attr_key)
            .cloned()
            .unwrap_or_else(|| category_required_attr_fallback(attr_key));
        plan.suggestions
            .push(build_product_attr_suggestion(item, product, &spec));
    }
    for attr_key in &requirement_check.missing_sale_attrs {
        let spec = sale_specs
            .get(attr_key)
            .cloned()
            .unwrap_or_else(|| category_required_attr_fallback(attr_key));
        plan.suggestions
            .push(build_sale_attr_suggestion(item, product, payload, &spec));
    }
    plan
}

pub(in crate::commands) fn required_category_attr_specs(
    raw_payload: &Value,
    list_key: &str,
) -> BTreeMap<String, CategoryRequiredAttr> {
    let mut specs = BTreeMap::new();
    collect_required_category_attr_specs(raw_payload, list_key, &mut specs);
    enrich_required_attr_specs_with_related_options(raw_payload, list_key, &mut specs);
    specs
}

fn category_attr_specs_by_key(
    raw_payload: &Value,
    list_key: &str,
) -> BTreeMap<String, CategoryRequiredAttr> {
    let mut items = Vec::new();
    collect_category_attr_specs(raw_payload, list_key, &mut items);
    let mut specs = BTreeMap::new();
    for spec in items {
        specs.entry(spec.key.clone()).or_insert(spec);
    }
    specs
}

fn category_required_attr_fallback(attr_key: &str) -> CategoryRequiredAttr {
    CategoryRequiredAttr {
        key: attr_key.to_string(),
        options: Vec::new(),
        attr_type: None,
        append_allowed: true,
        related_options: Vec::new(),
    }
}

/// 兜底默认值：metadata 预设 / 唯一选项 / 本地规则都无法确定属性值时，从类目可选值里挑一个
/// 合法兜底值，让必填属性也能 applied 自动上架，不再卡 need_confirm 等人工。
/// 选值优先级：含「其他/其它/通用」等通配语义的选项 > 第一个可选值 > 允许自定义时填「其他」。
/// 设计取舍：用户明确选择「自动填充优先上架，填错可事后在商品里改」。填的是类目合法值，
/// 能过 addproduct 校验；至于微信内容审核是否认可，由审核阶段反馈，不在补齐阶段阻断。
fn category_fallback_attr_value(spec: &CategoryRequiredAttr) -> Option<String> {
    if let Some(generic) = spec.options.iter().find(|option| {
        ["其他", "其它", "通用", "其余", "均码", "其他材质"]
            .iter()
            .any(|word| option.contains(word))
    }) {
        return Some(generic.clone());
    }
    if let Some(first) = spec.options.first() {
        return Some(first.clone());
    }
    // 空 options 的自由文本必填属性(颜色/面料材质等 type=string 且 append_allowed=false)：
    // 下游 apply/sanitize 的 spec_allows_free_text 都放行任意值，唯独此生成函数过去太保守只在
    // append_allowed=true 时兜底、否则返回 None → 走 needs_ai 被 block。与下游对齐：只要允许自由文本
    // 就兜底「其他」(类目合法值)，不再卡 need_confirm。
    if spec_allows_free_text(spec) {
        return Some("其他".to_string());
    }
    None
}

pub(in crate::commands) fn collect_required_category_attr_specs(
    value: &Value,
    list_key: &str,
    specs: &mut BTreeMap<String, CategoryRequiredAttr>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_required_category_attr_specs(item, list_key, specs);
            }
        }
        Value::Object(object) => {
            if let Some(items) = object.get(list_key).and_then(Value::as_array) {
                for item in items {
                    if let Some(attr) = item.as_object() {
                        if is_required_category_attr(attr) {
                            if let Some(key) = category_attr_name(attr) {
                                specs
                                    .entry(key.clone())
                                    .or_insert_with(|| CategoryRequiredAttr {
                                        key,
                                        options: category_attr_options(attr),
                                        attr_type: category_attr_type(attr),
                                        append_allowed: category_attr_append_allowed(attr),
                                        related_options: Vec::new(),
                                    });
                            }
                        }
                    }
                }
            }
            for child in object.values() {
                collect_required_category_attr_specs(child, list_key, specs);
            }
        }
        _ => {}
    }
}

fn enrich_required_attr_specs_with_related_options(
    raw_payload: &Value,
    list_key: &str,
    specs: &mut BTreeMap<String, CategoryRequiredAttr>,
) {
    let mut all_specs = Vec::new();
    collect_category_attr_specs(raw_payload, list_key, &mut all_specs);
    for spec in specs.values_mut() {
        if looks_like_percent_attr(&spec.key) {
            continue;
        }
        let mut related_options = BTreeSet::new();
        for candidate in &all_specs {
            if candidate.key == spec.key || candidate.options.is_empty() {
                continue;
            }
            if is_related_attr_key(&candidate.key, &spec.key) {
                related_options.extend(candidate.options.iter().cloned());
            }
        }
        spec.related_options = related_options.into_iter().collect();
    }
}

fn collect_category_attr_specs(
    value: &Value,
    list_key: &str,
    specs: &mut Vec<CategoryRequiredAttr>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_category_attr_specs(item, list_key, specs);
            }
        }
        Value::Object(object) => {
            if let Some(items) = object.get(list_key).and_then(Value::as_array) {
                for item in items {
                    if let Some(attr) = item.as_object() {
                        if let Some(key) = category_attr_name(attr) {
                            specs.push(CategoryRequiredAttr {
                                key,
                                options: category_attr_options(attr),
                                attr_type: category_attr_type(attr),
                                append_allowed: category_attr_append_allowed(attr),
                                related_options: Vec::new(),
                            });
                        }
                    }
                }
            }
            for child in object.values() {
                collect_category_attr_specs(child, list_key, specs);
            }
        }
        _ => {}
    }
}

pub(in crate::commands) fn category_attr_options(
    object: &serde_json::Map<String, Value>,
) -> Vec<String> {
    let mut options = BTreeSet::new();
    for key in [
        "value_list",
        "values",
        "attr_values",
        "option_list",
        "options",
        "enum_values",
    ] {
        if let Some(items) = object.get(key).and_then(Value::as_array) {
            for item in items {
                if let Some(value) = category_option_value(item) {
                    options.insert(value);
                }
            }
        }
    }
    for key in ["value", "value_text", "enum_value"] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            for option in split_category_option_values(value) {
                options.insert(option);
            }
        }
    }
    options.into_iter().collect()
}

fn category_attr_type(object: &serde_json::Map<String, Value>) -> Option<String> {
    ["type_v2", "type"]
        .iter()
        .find_map(|key| json_value_to_string(object.get(*key)))
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn category_attr_append_allowed(object: &serde_json::Map<String, Value>) -> bool {
    ["append_allowed", "allow_append", "custom_allowed"]
        .iter()
        .any(|key| json_value_to_bool(object.get(*key)).unwrap_or(false))
}

fn split_category_option_values(value: &str) -> Vec<String> {
    value
        .split(|ch| matches!(ch, ';' | '；'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(in crate::commands) fn category_option_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Object(object) => [
            "attr_value",
            "value",
            "name",
            "value_name",
            "word",
            "option",
        ]
        .iter()
        .find_map(|key| json_value_to_string(object.get(*key)))
        .map(|value| value.trim().to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn sanitize_payload_attr_array(
    product: &ExternalProductInput,
    attrs: &mut Vec<Value>,
    specs: &BTreeMap<String, CategoryRequiredAttr>,
    required_keys: &BTreeSet<String>,
    label: &str,
    report: &mut CategoryAttrSanitizeReport,
) -> AppResult<()> {
    let mut sanitized = Vec::with_capacity(attrs.len());
    for mut attr in std::mem::take(attrs) {
        let Some(object) = attr.as_object_mut() else {
            report.removed_attrs.push(format!("{label}属性结构无效"));
            continue;
        };
        let Some(attr_key) = ["attr_key", "name", "attr_name", "key"]
            .iter()
            .find_map(|key| json_value_to_string(object.get(*key)))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            sanitized.push(attr);
            continue;
        };
        let Some(spec) = specs.get(&attr_key) else {
            sanitized.push(attr);
            continue;
        };
        let Some(raw_value) = json_value_to_string(object.get("attr_value"))
            .or_else(|| json_value_to_string(object.get("value")))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            // 必填项空值：用类目合法值兜底而非删空，避免删后被判缺卡 CATEGORY_ATTRS_NEED_AI_FILL。
            if required_keys.contains(&attr_key) {
                if let Some(fallback) = category_fallback_attr_value(spec) {
                    object.insert("attr_value".to_string(), Value::String(fallback.clone()));
                    report
                        .changed_attrs
                        .push(format!("{attr_key} 空值兜底->{fallback}"));
                    sanitized.push(attr);
                    continue;
                }
            }
            report.removed_attrs.push(format!("{label}属性 {attr_key}"));
            continue;
        };
        let Some(normalized) = normalize_payload_attr_value_for_spec(product, spec, &raw_value)
        else {
            // 必填项的值不匹配微信枚举(如 面料材质=聚酯纤维100% 不在 cat6236 枚举)：兜底替换为合法值
            // 而非删空，否则 submit 阶段 check 判缺 → CATEGORY_ATTRS_NEED_AI_FILL。非必填仍按原逻辑删除。
            if required_keys.contains(&attr_key) {
                if let Some(fallback) = category_fallback_attr_value(spec) {
                    object.insert("attr_value".to_string(), Value::String(fallback.clone()));
                    report
                        .changed_attrs
                        .push(format!("{attr_key} {raw_value}->{fallback}(兜底)"));
                    sanitized.push(attr);
                    continue;
                }
            }
            report
                .removed_attrs
                .push(format!("{label}属性 {attr_key}={raw_value}"));
            continue;
        };
        if normalized.trim() != raw_value.trim() {
            object.insert("attr_value".to_string(), Value::String(normalized.clone()));
            report
                .changed_attrs
                .push(format!("{attr_key} {raw_value}->{normalized}"));
        }
        sanitized.push(attr);
    }
    *attrs = sanitized;
    Ok(())
}

fn normalize_payload_attr_value_for_spec(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
    value: &str,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if spec.options.is_empty() {
        return spec_allows_free_text(spec)
            .then(|| trimmed.to_string())
            .filter(|value| is_trusted_product_attr_value(&spec.key, value));
    }
    let normalized = normalize_suggested_value(trimmed, &spec.options);
    if suggestion_value_allowed_for_spec(&normalized, spec) {
        return Some(normalized);
    }
    if let Some(alias) = normalize_known_category_attr_alias(product, spec, trimmed) {
        let alias = normalize_suggested_value(&alias, &spec.options);
        if suggestion_value_allowed_for_spec(&alias, spec) {
            return Some(alias);
        }
    }
    if let Some((inferred, _, _)) = infer_product_attr_value_from_spec(product, spec) {
        let inferred = normalize_suggested_value(&inferred, &spec.options);
        if suggestion_value_allowed_for_spec(&inferred, spec) {
            return Some(inferred);
        }
    }
    None
}

fn normalize_known_category_attr_alias(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
    value: &str,
) -> Option<String> {
    if is_applicable_age_attr(&spec.key) {
        return normalize_age_alias_value(product, spec, value);
    }
    if is_style_attr(&spec.key) {
        return normalize_style_alias_value(product, spec, value);
    }
    None
}

pub(in crate::commands) fn build_product_attr_suggestion(
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> AttributeFillSuggestion {
    let prompt_json = build_attribute_prompt_json(item, product, "product", spec);
    // 优先使用采集审查阶段预设的属性建议（确定性高）
    if let Some(value) = metadata_attr_suggestion(product, &spec.key) {
        if let Some(normalized) = normalize_payload_attr_value_for_spec(product, spec, &value) {
            return AttributeFillSuggestion {
                attr_kind: "product",
                attr_key: spec.key.clone(),
                suggested_value: Some(normalized),
                sku_values: Vec::new(),
                confidence: 96,
                source: "metadata.ai_attr_suggestions".to_string(),
                applied: true,
                prompt_json,
            };
        }
    }
    // 仅一个选项时直接采用（无需 AI）
    if spec.options.len() == 1 {
        return AttributeFillSuggestion {
            attr_kind: "product",
            attr_key: spec.key.clone(),
            suggested_value: Some(spec.options[0].clone()),
            sku_values: Vec::new(),
            confidence: 90,
            source: "category_single_option".to_string(),
            applied: true,
            prompt_json,
        };
    }
    // 本地规则推断：从商品标题、SKU 规格、外部字段中匹配微信官方选项
    if let Some((value, confidence, source)) = infer_product_attr_value_from_spec(product, spec) {
        return AttributeFillSuggestion {
            attr_kind: "product",
            attr_key: spec.key.clone(),
            suggested_value: Some(value),
            sku_values: Vec::new(),
            confidence,
            source: source.to_string(),
            applied: true,
            prompt_json,
        };
    }
    // 兜底：metadata 预设、唯一选项、本地规则都补不出时，用类目可选值兜底填充，
    // 避免卡在 need_confirm 人工（用户选择「自动填充优先上架，填错可事后修正」）。
    if let Some(value) = category_fallback_attr_value(spec) {
        return AttributeFillSuggestion {
            attr_kind: "product",
            attr_key: spec.key.clone(),
            suggested_value: Some(value),
            sku_values: Vec::new(),
            // 65 = can_auto_apply 阈值：兜底值是类目合法值，放行自动上架(用户偏好「自动填错再改」)；
            // 仅 category_fallback_default 这一来源提到 65，needs_ai 来源仍 applied=false 走人工。
            confidence: 65,
            source: "category_fallback_default".to_string(),
            applied: true,
            prompt_json,
        };
    }
    // 连可选值都没有、又不允许自定义：极少见，仍交 AI
    AttributeFillSuggestion {
        attr_kind: "product",
        attr_key: spec.key.clone(),
        suggested_value: None,
        sku_values: Vec::new(),
        confidence: 0,
        source: "needs_ai".to_string(),
        applied: false,
        prompt_json,
    }
}

pub(in crate::commands) fn build_sale_attr_suggestion(
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    payload: &Value,
    spec: &CategoryRequiredAttr,
) -> AttributeFillSuggestion {
    let prompt_json = build_attribute_prompt_json(item, product, "sale", spec);
    if let Some(values) = infer_sale_attr_values_from_payload(payload, &spec.key) {
        let sku_values = values
            .into_iter()
            .enumerate()
            .map(|(sku_index, value)| SkuAttrFill {
                sku_index,
                value: normalize_suggested_value(&value, &spec.options),
            })
            .collect::<Vec<_>>();
        let applied = sku_values
            .iter()
            .all(|value| suggestion_value_allowed_for_spec(&value.value, spec));
        if applied {
            let summary = sku_values
                .iter()
                .map(|value| value.value.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join("/");
            return AttributeFillSuggestion {
                attr_kind: "sale",
                attr_key: spec.key.clone(),
                suggested_value: Some(format!("按 SKU 映射：{summary}")),
                sku_values,
                confidence: 92,
                source: "sku_attr_exact".to_string(),
                applied: true,
                prompt_json,
            };
        }
        // 推断出的 SKU 值里有不在类目枚举内的：不在此 return(否则 applied=false 会让销售属性永远
        // block)，继续往下走单选 / 类目兜底，保证销售属性也能兜底合法值自动上架。
    }
    if spec.options.len() == 1 {
        return AttributeFillSuggestion {
            attr_kind: "sale",
            attr_key: spec.key.clone(),
            suggested_value: Some(spec.options[0].clone()),
            sku_values: Vec::new(),
            confidence: 90,
            source: "category_single_option".to_string(),
            applied: true,
            prompt_json,
        };
    }
    // 兜底：本地规则也补不出销售属性时，用类目可选值兜底——所有 SKU 统一填该值
    // （apply 阶段 suggested_value 走 ensure_payload_sku_attr_for_all 写入每个 SKU）。
    if let Some(value) = category_fallback_attr_value(spec) {
        return AttributeFillSuggestion {
            attr_kind: "sale",
            attr_key: spec.key.clone(),
            suggested_value: Some(value),
            sku_values: Vec::new(),
            // 65 = can_auto_apply 阈值：兜底值是类目合法值，放行自动上架(用户偏好「自动填错再改」)；
            // 仅 category_fallback_default 这一来源提到 65，needs_ai 来源仍 applied=false 走人工。
            confidence: 65,
            source: "category_fallback_default".to_string(),
            applied: true,
            prompt_json,
        };
    }
    AttributeFillSuggestion {
        attr_kind: "sale",
        attr_key: spec.key.clone(),
        suggested_value: None,
        sku_values: Vec::new(),
        confidence: 0,
        source: "needs_ai".to_string(),
        applied: false,
        prompt_json,
    }
}

pub(in crate::commands) fn metadata_attr_suggestion(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<String> {
    let metadata = product.metadata.as_object()?;
    for key in [
        "ai_attr_suggestions",
        "wechat_attr_suggestions",
        "wechat_attr_defaults",
    ] {
        if let Some(Value::Object(attrs)) = metadata.get(key) {
            if let Some(value) = json_value_to_string(attrs.get(attr_key)) {
                if !value.trim().is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

pub(in crate::commands) fn product_has_wechat_category_metadata(
    product: &ExternalProductInput,
) -> AppResult<bool> {
    let Some(metadata) = product_metadata_object(product).map_err(AppError::Validation)? else {
        return Ok(false);
    };
    Ok(
        metadata_value(Some(metadata), &["wechat_add_product_payload"]).is_some()
            || metadata_value(Some(metadata), &["wechat_cats_v2", "cats_v2"]).is_some()
            || metadata_value(Some(metadata), &["wechat_cats", "cats"]).is_some()
            || metadata_value(
                Some(metadata),
                &["wechat_category_ids", "category_ids", "cat_ids"],
            )
            .is_some(),
    )
}

pub(in crate::commands) fn ensure_wechat_category_metadata(
    conn: &Connection,
    item: &mut PendingPublishItem,
    product: &mut ExternalProductInput,
) -> AppResult<Option<String>> {
    if product_has_wechat_category_metadata(product)? {
        return maybe_repair_low_confidence_review_category(conn, item, product);
    }
    let Some(inferred) = infer_wechat_category_from_cache(conn, &item.shop_id, product)? else {
        return Ok(None);
    };
    write_wechat_category_metadata(conn, item, product, &inferred).map(|_| {
        Some(format!(
            "已根据本地微信类目缓存推断类目：{}",
            inferred.category_path
        ))
    })
}

fn maybe_repair_low_confidence_review_category(
    conn: &Connection,
    item: &mut PendingPublishItem,
    product: &mut ExternalProductInput,
) -> AppResult<Option<String>> {
    let Some(metadata) = product.metadata.as_object() else {
        return Ok(None);
    };
    let source = metadata
        .get("wechat_category_infer_source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let category_applied = metadata
        .get("collection_review")
        .and_then(Value::as_object)
        .and_then(|review| json_value_to_bool(review.get("category_applied")))
        .unwrap_or(true);
    if source != "manual_review_confirm" || category_applied {
        return Ok(None);
    }
    let current_ids = metadata_value(
        Some(metadata),
        &["wechat_category_ids", "category_ids", "cat_ids"],
    )
    .and_then(Value::as_array)
    .map(|items| {
        items
            .iter()
            .filter_map(|item| json_value_to_i64(Some(item)))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    let Some(inferred) = infer_wechat_category_from_title_cache(conn, &item.shop_id, product)?
    else {
        return Ok(None);
    };
    if current_ids == inferred.category_ids {
        return Ok(None);
    }
    write_wechat_category_metadata(conn, item, product, &inferred).map(|_| {
        Some(format!(
            "已根据商品标题校正低置信微信类目：{}",
            inferred.category_path
        ))
    })
}

fn write_wechat_category_metadata(
    conn: &Connection,
    item: &mut PendingPublishItem,
    product: &mut ExternalProductInput,
    inferred: &InferredWechatCategory,
) -> AppResult<()> {
    let mut raw_value = serde_json::from_str::<Value>(&item.raw_payload).map_err(|error| {
        AppError::Validation(format!(
            "商品原始数据无法解析，不能写入微信类目推断结果：{error}"
        ))
    })?;
    let raw_object = raw_value.as_object_mut().ok_or_else(|| {
        AppError::Validation("商品原始数据不是对象，不能写入微信类目推断结果".to_string())
    })?;
    raw_object.insert(
        "category_hint".to_string(),
        Value::String(inferred.category_path.clone()),
    );
    let metadata_value = raw_object
        .entry("metadata".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if metadata_value.is_null() {
        *metadata_value = Value::Object(serde_json::Map::new());
    }
    let metadata = metadata_value.as_object_mut().ok_or_else(|| {
        AppError::Validation("metadata 必须是对象，不能写入微信类目推断结果".to_string())
    })?;
    metadata.insert(
        "wechat_category_ids".to_string(),
        Value::Array(
            inferred
                .category_ids
                .iter()
                .map(|cat_id| Value::Number(serde_json::Number::from(*cat_id)))
                .collect(),
        ),
    );
    metadata.insert(
        "wechat_category_infer_source".to_string(),
        Value::String(inferred.source.to_string()),
    );
    metadata.insert(
        "wechat_category_infer_at".to_string(),
        Value::String(now_shanghai()),
    );

    let updated_raw = serde_json::to_string(&raw_value)
        .map_err(|error| AppError::Validation(format!("微信类目推断结果无法序列化：{error}")))?;
    conn.execute(
        "UPDATE pipeline_shop_targets SET raw_payload = ?1 WHERE id = ?2",
        params![updated_raw, item.item_id.as_str()],
    )?;
    item.raw_payload = updated_raw.clone();
    *product = serde_json::from_str(&updated_raw).map_err(|error| {
        AppError::Validation(format!("微信类目推断后的商品数据无法解析：{error}"))
    })?;
    Ok(())
}

#[derive(Debug)]
pub(in crate::commands) struct InferredWechatCategory {
    pub(in crate::commands) category_ids: Vec<i64>,
    pub(in crate::commands) category_path: String,
    pub(in crate::commands) source: &'static str,
}

pub(in crate::commands) fn infer_wechat_category_from_cache(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
) -> AppResult<Option<InferredWechatCategory>> {
    infer_wechat_category_with_options(conn, shop_id, product, true, "local_category_cache_match")
}

fn infer_wechat_category_from_title_cache(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
) -> AppResult<Option<InferredWechatCategory>> {
    infer_wechat_category_with_options(conn, shop_id, product, false, "local_category_title_repair")
}

fn infer_wechat_category_with_options(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
    include_category_hint: bool,
    source: &'static str,
) -> AppResult<Option<InferredWechatCategory>> {
    let child_context = has_children_category_context(product);
    let candidates = load_wechat_category_inference_candidates(
        conn,
        shop_id,
        product,
        include_category_hint,
        child_context,
    )?;
    let Some(best) = candidates.first() else {
        return Ok(None);
    };
    if best.score < 90 || best.matched_terms == 0 {
        return Ok(None);
    }
    if let Some(second) = candidates.get(1) {
        if best.score - second.score < 18 && best.score < 180 {
            return Ok(None);
        }
    }
    if should_skip_single_term_category_inference(product, best, child_context) {
        return Ok(None);
    }
    Ok(Some(InferredWechatCategory {
        category_ids: best.category_ids.clone(),
        category_path: best.category_path.clone(),
        source,
    }))
}

#[derive(Debug)]
struct CategoryInferenceCandidate {
    score: i64,
    matched_terms: usize,
    category_ids: Vec<i64>,
    category_path: String,
}

fn load_wechat_category_inference_candidates(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
    include_category_hint: bool,
    child_context: bool,
) -> AppResult<Vec<CategoryInferenceCandidate>> {
    let mut terms = if include_category_hint {
        category_inference_terms(product)
    } else {
        Vec::new()
    };
    extend_terms_with_cached_leaf_names_in_text(conn, shop_id, &mut terms, &product.title)?;
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    // 按词条逐个查询候选叶子类目并集（最多取 16 个词条，每词条限 80 条），BTreeSet 去重并保持升序
    let mut candidate_ids = BTreeSet::new();
    for term in terms.iter().take(16) {
        candidate_ids.extend(load_active_leaf_category_ids(conn, shop_id, Some(term))?);
    }
    let mut candidates = Vec::new();
    for cat_id in candidate_ids {
        let path = load_wechat_category_path(conn, shop_id, cat_id)?;
        if path.len() < 3 {
            continue;
        }
        let (score, matched_terms) =
            score_category_inference_candidate(&path, &terms, child_context);
        if score < 45 {
            continue;
        }
        candidates.push(CategoryInferenceCandidate {
            score,
            matched_terms,
            category_ids: path.iter().map(|(id, _)| *id).collect(),
            category_path: path
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(" > "),
        });
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.matched_terms.cmp(&left.matched_terms))
            .then_with(|| left.category_path.cmp(&right.category_path))
    });
    Ok(candidates)
}

fn should_skip_single_term_category_inference(
    product: &ExternalProductInput,
    candidate: &CategoryInferenceCandidate,
    child_context: bool,
) -> bool {
    if candidate.matched_terms != 1 {
        return false;
    }
    if child_context && path_text_has_children_context(&candidate.category_path) {
        return false;
    }
    category_inference_terms(product).len() == 1 && candidate.score < 180
}

pub(in crate::commands) fn suggest_wechat_category_candidates_from_cache(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
    limit: usize,
) -> AppResult<Vec<CollectionReviewCategoryCandidate>> {
    let candidates = load_wechat_category_inference_candidates(
        conn,
        shop_id,
        product,
        true,
        has_children_category_context(product),
    )?;
    Ok(candidates
        .into_iter()
        .take(limit)
        .map(|candidate| CollectionReviewCategoryCandidate {
            category_ids: candidate.category_ids,
            category_path: candidate.category_path,
            score: candidate.score,
            source: "local_category_cache_match".to_string(),
        })
        .collect())
}

pub(in crate::commands) fn suggest_wechat_category_broad_candidates_from_cache(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
    limit: usize,
) -> AppResult<Vec<CollectionReviewCategoryCandidate>> {
    let leaf_ids = load_active_leaf_category_ids(conn, shop_id, None)?;
    let child_context = has_children_category_context(product);
    let mut candidates = Vec::new();
    for cat_id in leaf_ids {
        let path = load_wechat_category_path(conn, shop_id, cat_id)?;
        if path.len() < 3 {
            continue;
        }
        let mut score = if path.len() >= 3 { 8 } else { 0 };
        if child_context && path_has_children_context(&path) {
            score += 40;
        }
        candidates.push(CollectionReviewCategoryCandidate {
            category_ids: path.iter().map(|(id, _)| *id).collect(),
            category_path: path
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(" > "),
            score,
            source: "local_category_cache_broad".to_string(),
        });
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.category_path.cmp(&right.category_path))
    });
    Ok(candidates.into_iter().take(limit).collect())
}

/// 加载已开通（relation.status=1）的叶子类目 ID（无子类目的节点）。
/// name_term 为 None 时返回全部叶子类目；为 Some 时按类目名与词条互相模糊匹配过滤并限 80 条。
fn load_active_leaf_category_ids(
    conn: &Connection,
    shop_id: &str,
    name_term: Option<&str>,
) -> AppResult<Vec<i64>> {
    let mut sql = String::from(
        "SELECT DISTINCT c.cat_id
         FROM wechat_categories c
         JOIN wechat_category_relations relation
           ON relation.shop_id = c.shop_id
          AND relation.cat_id = c.cat_id
          AND relation.status = 1
         LEFT JOIN wechat_categories child
           ON child.shop_id = c.shop_id AND child.parent_cat_id = c.cat_id
         WHERE c.shop_id = ?1
           AND child.cat_id IS NULL",
    );
    if name_term.is_some() {
        sql.push_str(
            "
           AND (
             c.name = ?2
             OR c.name LIKE '%' || ?2 || '%'
             OR ?2 LIKE '%' || c.name || '%'
           )",
        );
    }
    sql.push_str(
        "
         ORDER BY COALESCE(c.level, 0) DESC, c.cat_id ASC",
    );
    if name_term.is_some() {
        sql.push_str(
            "
         LIMIT 80",
        );
    }
    let mut stmt = conn.prepare(&sql)?;
    let ids = match name_term {
        Some(term) => stmt
            .query_map(params![shop_id, term], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?,
        None => stmt
            .query_map([shop_id], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(ids)
}

fn category_inference_terms(product: &ExternalProductInput) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(category_hint) = product.category_hint.as_deref() {
        push_split_category_terms(&mut terms, category_hint);
    }
    terms
}

fn extend_terms_with_cached_leaf_names_in_text(
    conn: &Connection,
    shop_id: &str,
    terms: &mut Vec<String>,
    text: &str,
) -> AppResult<()> {
    let Some(normalized_text) = normalize_category_match_term(text) else {
        return Ok(());
    };
    let mut stmt = conn.prepare(
        "SELECT DISTINCT c.name
         FROM wechat_categories c
         JOIN wechat_category_relations relation
           ON relation.shop_id = c.shop_id
          AND relation.cat_id = c.cat_id
          AND relation.status = 1
         LEFT JOIN wechat_categories child
           ON child.shop_id = c.shop_id AND child.parent_cat_id = c.cat_id
         WHERE c.shop_id = ?1
           AND child.cat_id IS NULL
         ORDER BY COALESCE(c.level, 0) DESC, c.cat_id ASC",
    )?;
    let rows = stmt.query_map([shop_id], |row| row.get::<_, String>(0))?;
    for name in rows {
        for part in category_name_match_parts(&name?) {
            if part.chars().count() >= 2 && normalized_text.contains(&part) {
                push_unique_category_term(terms, part);
            }
        }
    }
    Ok(())
}

fn push_split_category_terms(terms: &mut Vec<String>, value: &str) {
    for part in value.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '/' | '／' | '>' | '＞' | '|' | '｜' | ',' | '，' | '、' | ';' | '；'
            )
    }) {
        push_category_term_with_derivatives(terms, part);
    }
}

fn push_category_term_with_derivatives(terms: &mut Vec<String>, value: &str) {
    let Some(term) = normalize_category_match_term(value) else {
        return;
    };
    push_unique_category_term(terms, term);
}

fn push_unique_category_term(terms: &mut Vec<String>, term: String) {
    if !terms.iter().any(|existing| existing == &term) {
        terms.push(term);
    }
}

fn normalize_category_match_term(value: &str) -> Option<String> {
    let normalized = value
        .trim_matches(|ch: char| {
            ch.is_ascii_punctuation()
                || matches!(
                    ch,
                    '，' | '。' | '、' | '；' | '：' | '！' | '？' | '（' | '）'
                )
        })
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized.chars().count() < 2 {
        None
    } else {
        Some(normalized)
    }
}

fn score_category_inference_candidate(
    path: &[(i64, String)],
    terms: &[String],
    child_context: bool,
) -> (i64, usize) {
    let path_parts = category_path_match_parts(path);
    let leaf_parts = path
        .last()
        .map(|(_, name)| category_name_match_parts(name))
        .unwrap_or_default();
    let path_text = path_parts.join("/");
    let mut score = if path.len() >= 3 { 8 } else { 0 };
    let mut matched_terms = BTreeSet::new();

    for term in terms {
        let mut term_score = 0;
        for leaf_part in &leaf_parts {
            term_score = term_score.max(match_category_term(term, leaf_part, 92, 48));
        }
        for path_part in &path_parts {
            term_score = term_score.max(match_category_term(term, path_part, 28, 16));
        }
        if path_text.contains(term) {
            term_score = term_score.max(14);
        }
        if term_score > 0 {
            matched_terms.insert(term.clone());
            score += term_score;
        }
    }

    if child_context && path_has_children_context(path) {
        score += 40;
    }

    (score, matched_terms.len())
}

fn has_children_category_context(product: &ExternalProductInput) -> bool {
    let mut text = product.title.clone();
    if let Some(category_hint) = product.category_hint.as_deref() {
        text.push_str(category_hint);
    }
    [
        "儿童",
        "童装",
        "婴儿",
        "幼儿",
        "宝宝",
        "男童",
        "女童",
        "中大童",
    ]
    .iter()
    .any(|term| text.contains(term))
}

fn path_has_children_context(path: &[(i64, String)]) -> bool {
    path.iter()
        .any(|(_, name)| name.contains("母婴") || name.contains('童') || name.contains("婴"))
}

fn path_text_has_children_context(path: &str) -> bool {
    path.contains("母婴") || path.contains('童') || path.contains("婴")
}

fn category_path_match_parts(path: &[(i64, String)]) -> Vec<String> {
    let mut parts = Vec::new();
    for (_, name) in path {
        for part in category_name_match_parts(name) {
            push_unique_category_term(&mut parts, part);
        }
    }
    parts
}

fn category_name_match_parts(name: &str) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(normalized) = normalize_category_match_term(name) {
        push_unique_category_term(&mut parts, normalized);
    }
    push_split_category_terms(&mut parts, name);
    parts
}

fn match_category_term(term: &str, target: &str, exact_score: i64, contains_score: i64) -> i64 {
    if term == target {
        return exact_score;
    }
    let term_len = term.chars().count();
    let target_len = target.chars().count();
    if term_len >= 2 && target.contains(term) {
        return contains_score;
    }
    if target_len >= 2 && term.contains(target) {
        return contains_score.saturating_sub(8);
    }
    0
}

fn load_wechat_category_path(
    conn: &Connection,
    shop_id: &str,
    leaf_cat_id: i64,
) -> AppResult<Vec<(i64, String)>> {
    let mut path = Vec::new();
    let mut current = Some(leaf_cat_id);
    for _ in 0..8 {
        let Some(cat_id) = current else {
            break;
        };
        let row = conn
            .query_row(
                "SELECT cat_id, name, parent_cat_id
                 FROM wechat_categories
                 WHERE shop_id = ?1 AND cat_id = ?2",
                params![shop_id, cat_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, name, parent_id)) = row else {
            break;
        };
        path.push((id, name));
        current = parent_id.filter(|value| *value > 0);
    }
    path.reverse();
    Ok(path)
}

#[cfg(test)]
pub(in crate::commands) fn infer_product_attr_value(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<(String, i64, &'static str)> {
    infer_product_attr_value_from_spec(product, &category_required_attr_fallback(attr_key))
}

pub(in crate::commands) fn infer_product_attr_value_from_spec(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<(String, i64, &'static str)> {
    if let Some(value) = infer_product_attr_from_external_fields(product, &spec.key) {
        if is_trusted_product_attr_value(&spec.key, &value) {
            let normalized = normalize_alias_product_attr_value(&spec.key, &value).unwrap_or(value);
            return Some((normalized, 96, "external_attr_exact"));
        }
    }
    if let Some(value) = infer_product_attr_from_alias_fields(product, &spec.key) {
        return Some((value, 90, "external_attr_alias"));
    }
    if let Some(value) = infer_child_clothing_common_attr(product, spec) {
        return Some(value);
    }
    if let Some(value) = infer_product_attr_from_allowed_options(product, spec) {
        return Some((value, 92, "official_option_text_match"));
    }
    if let Some(value) = infer_product_attr_from_related_options(product, spec) {
        return Some((value, 88, "related_category_option_match"));
    }
    if let Some(value) = infer_percent_attr_from_evidence(product, &spec.key) {
        return Some((value, 88, "percent_text_match"));
    }
    if let Some(value) = infer_percent_attr_from_fabric_material(product, &spec.key) {
        return Some((value, 85, "fabric_material_infer"));
    }
    if let Some(value) = infer_product_attr_from_sku_specs(product, &spec.key) {
        return Some((value, 88, "sku_specs_alias"));
    }
    None
}

fn infer_child_clothing_common_attr(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<(String, i64, &'static str)> {
    if !has_children_category_context(product) {
        return None;
    }
    if is_safety_level_attr(&spec.key) {
        return infer_children_safety_level(product, spec)
            .map(|value| (value, 84, "children_safety_rule"));
    }
    if is_applicable_age_attr(&spec.key) {
        return infer_children_applicable_age(product, spec)
            .map(|value| (value, 86, "children_age_rule"));
    }
    if is_style_attr(&spec.key) {
        return infer_children_style(product, spec).map(|value| (value, 88, "children_style_rule"));
    }
    if looks_like_percent_attr(&spec.key) {
        return infer_children_fabric_composition(product, spec)
            .map(|value| (value, 84, "children_fabric_composition_rule"));
    }
    if is_fabric_material_attr(&spec.key) {
        return infer_children_fabric_material(product, spec)
            .map(|value| (value, 86, "children_fabric_rule"));
    }
    None
}

fn is_safety_level_attr(attr_key: &str) -> bool {
    let normalized = normalize_attr_key_for_exact_match(attr_key);
    normalized.contains("安全等级") || normalized.contains("安全类别")
}

fn is_applicable_age_attr(attr_key: &str) -> bool {
    let normalized = normalize_attr_key_for_exact_match(attr_key);
    normalized.contains("适用年龄") || normalized.contains("年龄段")
}

fn is_style_attr(attr_key: &str) -> bool {
    normalize_attr_key_for_exact_match(attr_key).contains("风格")
}

fn is_fabric_material_attr(attr_key: &str) -> bool {
    if looks_like_percent_attr(attr_key) {
        return false;
    }
    let normalized = normalize_attr_key_for_exact_match(attr_key);
    normalized.contains("面料材质") || normalized == "面料" || normalized == "材质"
}

fn infer_children_safety_level(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    let text = product_context_text(product);
    if text.contains("婴儿") || text.contains("婴幼儿") || text.contains("0-3") {
        return option_exact(&spec.options, "A类");
    }
    if text.contains("宝宝")
        && (text.contains("连体") || text.contains("爬服") || text.contains("哈衣"))
    {
        return option_exact(&spec.options, "A类");
    }
    option_exact(&spec.options, "B类").or_else(|| option_exact(&spec.options, "A类"))
}

fn infer_children_applicable_age(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    let mut selected = BTreeSet::<String>::new();
    let text = product_context_text(product);
    let heights = collect_children_height_values(product);
    let min_height = heights.iter().min().copied();
    let max_height = heights.iter().max().copied();

    if text.contains("婴儿")
        || text.contains("婴幼儿")
        || max_height.is_some_and(|value| value <= 100)
    {
        insert_option_exact(&mut selected, &spec.options, "3周岁以下");
    }
    if text.contains("小童")
        || text.contains("幼儿园")
        || min_height.is_some_and(|value| value <= 120)
        || max_height.is_some_and(|value| value >= 100)
    {
        insert_option_exact(&mut selected, &spec.options, "3周岁以上");
    }
    if text.contains("儿童")
        || text.contains("童装")
        || text.contains("男童")
        || text.contains("女童")
        || max_height.is_some_and(|value| value >= 120)
    {
        insert_option_exact(&mut selected, &spec.options, "6周岁以上");
    }
    if text.contains("中大童")
        || text.contains("大童")
        || max_height.is_some_and(|value| value >= 140)
    {
        insert_option_exact(&mut selected, &spec.options, "8周岁以上");
    }
    if text.contains("青少年") || max_height.is_some_and(|value| value >= 165) {
        insert_option_exact(&mut selected, &spec.options, "14周岁以上");
    }
    if selected.is_empty() {
        insert_option_exact(&mut selected, &spec.options, "3周岁以上");
    }
    if selected.is_empty() {
        return option_exact(&spec.options, "通用");
    }
    let ordered = spec
        .options
        .iter()
        .filter(|option| selected.contains(option.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        None
    } else if is_select_many_attr(spec) {
        Some(ordered.join(";"))
    } else {
        ordered.last().cloned()
    }
}

fn infer_children_style(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    let text = product_context_text(product);
    let candidates: &[(&[&str], &[&str])] = &[
        (
            &["篮球", "球衣", "运动", "瑜伽", "健身", "速干", "队服"],
            &["运动风"],
        ),
        (
            &["汉服", "唐装", "旗袍", "古装", "国学", "国风", "民族服"],
            &["汉风", "新中式风", "民族风", "国潮"],
        ),
        (&["公主", "蓬蓬裙"], &["公主风", "甜美风"]),
        (&["卡通", "可爱", "小兔", "恐龙", "草莓"], &["可爱风"]),
        (&["复古"], &["复古风"]),
    ];
    for (keywords, options) in candidates {
        if keywords.iter().any(|keyword| text.contains(keyword)) {
            if let Some(option) = first_available_option(&spec.options, options) {
                return Some(option);
            }
        }
    }
    None
}

fn normalize_age_alias_value(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
    value: &str,
) -> Option<String> {
    let parts = split_age_alias_parts(value);
    let mut selected = BTreeSet::new();
    for part in parts {
        let normalized = normalize_attr_key_for_exact_match(&part);
        if normalized.contains("通用") {
            insert_option_exact(&mut selected, &spec.options, "通用");
        }
        if normalized.contains("婴幼儿")
            || normalized.contains("03岁")
            || normalized.contains("0至3岁")
            || normalized.contains("0到3岁")
            || normalized.contains("3岁以下")
            || normalized.contains("3周岁以下")
        {
            insert_option_exact(&mut selected, &spec.options, "3周岁以下");
        }
        if normalized.contains("小童")
            || normalized.contains("幼儿")
            || normalized.contains("36岁")
            || normalized.contains("3至6岁")
            || normalized.contains("3到6岁")
            || normalized.contains("3周岁以上")
        {
            insert_option_exact(&mut selected, &spec.options, "3周岁以上");
        }
        if normalized.contains("中童")
            || normalized.contains("儿童")
            || normalized.contains("614岁")
            || normalized.contains("6至14岁")
            || normalized.contains("6到14岁")
            || normalized.contains("6周岁以上")
        {
            insert_option_exact(&mut selected, &spec.options, "6周岁以上");
        }
        if normalized.contains("中大童")
            || normalized.contains("大童")
            || normalized.contains("8周岁以上")
        {
            insert_option_exact(&mut selected, &spec.options, "8周岁以上");
        }
        if normalized.contains("青少年")
            || normalized.contains("14周岁以上")
            || normalized.contains("14岁以上")
        {
            insert_option_exact(&mut selected, &spec.options, "14周岁以上");
        }
    }
    if selected.is_empty() {
        return infer_children_applicable_age(product, spec);
    }
    let ordered = spec
        .options
        .iter()
        .filter(|option| selected.contains(option.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        None
    } else if is_select_many_attr(spec) {
        Some(ordered.join(";"))
    } else {
        ordered.last().cloned()
    }
}

fn split_age_alias_parts(value: &str) -> Vec<String> {
    let parts = value
        .split(|ch| matches!(ch, ';' | '；' | ',' | '，' | '、'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        vec![value.trim().to_string()]
    } else {
        parts
    }
}

fn normalize_style_alias_value(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
    value: &str,
) -> Option<String> {
    infer_children_style(product, spec).or_else(|| {
        let normalized = normalize_attr_key_for_exact_match(value);
        if [
            "休闲", "日常", "简约", "基础", "百搭", "时尚", "洋气", "潮流",
        ]
        .iter()
        .any(|keyword| normalized.contains(keyword))
        {
            option_exact(&spec.options, "其他")
        } else {
            None
        }
    })
}

fn infer_children_fabric_material(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    let text = product_context_text(product);
    let value = if text.contains("纯棉") {
        "纯棉"
    } else if text.contains("棉") {
        "棉"
    } else if text.contains("冰丝") {
        "锦纶"
    } else if text.contains("牛仔") {
        "棉"
    } else if text.contains("篮球")
        || text.contains("球衣")
        || text.contains("速干")
        || text.contains("运动")
        || text.contains("泳衣")
        || text.contains("防晒")
    {
        "聚酯纤维"
    } else {
        return None;
    };
    normalize_common_attr_value_for_spec(value, spec)
}

fn infer_children_fabric_composition(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    let material = find_known_fabric_material(product)
        .or_else(|| infer_children_fabric_material(product, spec))?;
    infer_composition_from_material(&material)
}

fn product_context_text(product: &ExternalProductInput) -> String {
    let mut text = product.title.clone();
    if let Some(value) = product.category_hint.as_deref() {
        text.push_str(value);
    }
    if let Some(value) = product.brand_hint.as_deref() {
        text.push_str(value);
    }
    if let Some(metadata) = product.metadata.as_object() {
        for key in [
            "taobao_sku_options",
            "ai_attr_suggestions",
            "wechat_attr_suggestions",
        ] {
            if let Some(value) = metadata.get(key) {
                text.push_str(&value.to_string());
            }
        }
    }
    text
}

fn collect_children_height_values(product: &ExternalProductInput) -> Vec<i64> {
    let mut values = Vec::new();
    for sku in &product.skus {
        collect_height_values_from_json(&sku.specs, &mut values);
    }
    if let Some(metadata) = product.metadata.as_object() {
        if let Some(value) = metadata.get("taobao_sku_options") {
            collect_height_values_from_json(value, &mut values);
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

fn collect_height_values_from_json(value: &Value, values: &mut Vec<i64>) {
    match value {
        Value::String(value) => collect_height_values_from_text(value, values),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                if (50..=190).contains(&value) {
                    values.push(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_height_values_from_json(item, values);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                if is_related_attr_key(key, "身高") || is_related_attr_key(key, "尺码") {
                    collect_height_values_from_json(value, values);
                }
            }
        }
        _ => {}
    }
}

fn collect_height_values_from_text(text: &str, values: &mut Vec<i64>) {
    let mut current = String::new();
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(value) = current.parse::<i64>() {
                if (50..=190).contains(&value) {
                    values.push(value);
                }
            }
            current.clear();
        }
    }
}

fn option_exact(options: &[String], expected: &str) -> Option<String> {
    options
        .iter()
        .find(|option| option.trim() == expected)
        .cloned()
}

fn insert_option_exact(selected: &mut BTreeSet<String>, options: &[String], expected: &str) {
    if let Some(option) = option_exact(options, expected) {
        selected.insert(option);
    }
}

fn first_available_option(options: &[String], candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find_map(|candidate| option_exact(options, candidate))
}

fn normalize_common_attr_value_for_spec(
    value: &str,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    if spec.options.is_empty() {
        return Some(value.to_string());
    }
    option_exact(&spec.options, value).or_else(|| {
        spec.options
            .iter()
            .find(|option| option.contains(value) || value.contains(option.as_str()))
            .cloned()
    })
}

fn infer_product_attr_from_external_fields(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<String> {
    let params = taobao_item_params(product)?;
    let wanted = normalize_attr_key_for_exact_match(attr_key);
    params.iter().find_map(|(key, value)| {
        if normalize_attr_key_for_exact_match(key) != wanted {
            return None;
        }
        json_value_to_string(Some(value))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn taobao_item_params(product: &ExternalProductInput) -> Option<&serde_json::Map<String, Value>> {
    let metadata = product.metadata.as_object()?;
    if metadata
        .get("taobao_item_params_quality")
        .and_then(Value::as_object)
        .and_then(|quality| json_value_to_bool(quality.get("trusted")))
        != Some(true)
    {
        return None;
    }
    metadata.get("taobao_item_params")?.as_object()
}

fn infer_product_attr_from_alias_fields(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<String> {
    let params = taobao_item_params(product)?;
    let normalized = normalize_attr_key_for_exact_match(attr_key);
    params.iter().find_map(|(key, value)| {
        let key = normalize_attr_key_for_exact_match(key);
        if !is_related_attr_key(&key, &normalized) {
            return None;
        }
        json_value_to_string(Some(value))
            .map(|value| value.trim().to_string())
            .filter(|value| is_trusted_product_attr_value(attr_key, value))
    })
}

fn is_related_attr_key(left: &str, right: &str) -> bool {
    let left_exact = normalize_attr_key_for_exact_match(left);
    let right_exact = normalize_attr_key_for_exact_match(right);
    if left_exact == right_exact {
        return true;
    }
    let left_variants = attr_key_match_variants(left);
    let right_variants = attr_key_match_variants(right);
    !left_variants.is_disjoint(&right_variants)
        || (left_exact.chars().count() >= 4 && right_exact.contains(&left_exact))
        || (right_exact.chars().count() >= 4 && left_exact.contains(&right_exact))
}

fn normalize_alias_product_attr_value(attr_key: &str, value: &str) -> Option<String> {
    let _ = attr_key;
    Some(value.trim().to_string())
}

fn is_trusted_product_attr_value(attr_key: &str, value: &str) -> bool {
    let _ = attr_key;
    let value = value.trim();
    !value.is_empty() && value.chars().count() <= 120
}

fn infer_product_attr_from_sku_specs(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<String> {
    let values = collect_sku_spec_values_for_attr(product, attr_key);
    let values = cleaned_unique_sku_values(&values, attr_key);
    summarize_product_attr_values(&values)
}

fn collect_sku_spec_values_for_attr(product: &ExternalProductInput, attr_key: &str) -> Vec<String> {
    let mut values = Vec::new();
    for sku in &product.skus {
        match &sku.specs {
            Value::Object(specs) => {
                for (key, value) in specs {
                    if is_related_attr_key(key, attr_key) {
                        if let Some(value) = json_value_to_string(Some(value)) {
                            values.push(value);
                        }
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    let Some(object) = item.as_object() else {
                        continue;
                    };
                    let Some(key) = json_value_to_string(object.get("attr_key")) else {
                        continue;
                    };
                    if is_related_attr_key(&key, attr_key) {
                        if let Some(value) = json_value_to_string(object.get("attr_value")) {
                            values.push(value);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    values
}

fn infer_product_attr_from_allowed_options(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    if spec.options.is_empty() {
        return None;
    }
    match_options_in_product_evidence(product, spec, &spec.options)
}

fn infer_product_attr_from_related_options(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> Option<String> {
    if spec.related_options.is_empty() || !spec_allows_free_text(spec) {
        return None;
    }
    match_options_in_product_evidence(product, spec, &spec.related_options)
}

fn match_options_in_product_evidence(
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
    options: &[String],
) -> Option<String> {
    let evidence = product_evidence_texts(product);
    let mut matches = BTreeSet::new();
    for option in options {
        if is_low_information_option(option) {
            continue;
        }
        let Some(normalized_option) = normalize_evidence_text(option) else {
            continue;
        };
        if evidence
            .iter()
            .any(|text| text.contains(&normalized_option))
        {
            matches.insert(option.trim().to_string());
        }
    }
    if matches.is_empty() {
        return None;
    }
    if is_select_many_attr(spec) {
        return Some(matches.into_iter().collect::<Vec<_>>().join(";"));
    }
    matches
        .into_iter()
        .max_by_key(|value| value.chars().count())
}

fn product_evidence_texts(product: &ExternalProductInput) -> Vec<String> {
    let mut values = Vec::new();
    push_evidence_text(&mut values, &product.title);
    if let Some(value) = product.category_hint.as_deref() {
        push_evidence_text(&mut values, value);
    }
    if let Some(value) = product.brand_hint.as_deref() {
        push_evidence_text(&mut values, value);
    }
    for sku in &product.skus {
        collect_json_evidence_texts(&sku.specs, &mut values);
    }
    if let Some(metadata) = product.metadata.as_object() {
        for key in [
            "taobao_item_params",
            "taobao_sku_options",
            "collection_review",
            "external_attrs",
            "source_attrs",
        ] {
            if let Some(value) = metadata.get(key) {
                collect_json_evidence_texts(value, &mut values);
            }
        }
    }
    values
}

fn collect_json_evidence_texts(value: &Value, values: &mut Vec<String>) {
    match value {
        Value::String(value) => push_evidence_text(values, value),
        Value::Number(value) => push_evidence_text(values, &value.to_string()),
        Value::Array(items) => {
            for item in items {
                collect_json_evidence_texts(item, values);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                push_evidence_text(values, key);
                collect_json_evidence_texts(value, values);
            }
        }
        _ => {}
    }
}

fn push_evidence_text(values: &mut Vec<String>, value: &str) {
    if let Some(value) = normalize_evidence_text(value) {
        if !values.iter().any(|existing| existing == &value) {
            values.push(value);
        }
    }
}

fn normalize_evidence_text(value: &str) -> Option<String> {
    let normalized = value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || matches!(ch, '%' | '％'))
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized.chars().count() < 2 {
        None
    } else {
        Some(normalized)
    }
}

fn is_low_information_option(value: &str) -> bool {
    matches!(value.trim(), "其他" | "其它" | "通用")
}

fn is_select_many_attr(spec: &CategoryRequiredAttr) -> bool {
    spec.attr_type.as_deref() == Some("select_many")
}

fn spec_allows_free_text(spec: &CategoryRequiredAttr) -> bool {
    spec.options.is_empty()
        || spec
            .attr_type
            .as_deref()
            .is_some_and(|value| value == "string")
        || spec.append_allowed
}

fn attr_key_match_variants(value: &str) -> BTreeSet<String> {
    let mut variants = BTreeSet::new();
    let normalized = normalize_attr_key_for_exact_match(value);
    if normalized.is_empty() {
        return variants;
    }
    variants.insert(normalized.clone());
    for suffix in [
        "分类", "规格", "选项", "属性", "款式", "型号", "号码", "号型",
    ] {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            if stripped.chars().count() >= 2 {
                variants.insert(stripped.to_string());
            }
        }
    }
    if normalized.contains("颜色") {
        variants.insert("颜色".to_string());
    }
    if normalized.contains("尺码")
        || normalized.contains("尺寸")
        || normalized.contains("码数")
        || normalized.contains("身高")
        || normalized.contains("身长")
        || normalized.contains("大小")
    {
        variants.insert("尺码".to_string());
    }
    if normalized.contains("面料") || normalized.contains("材质") {
        variants.insert("面料材质".to_string());
    }
    variants
}

fn cleaned_unique_sku_values(values: &[String], attr_key: &str) -> Vec<String> {
    let mut cleaned = Vec::new();
    for value in values {
        let Some(value) = clean_sku_attr_value_for_key(value, attr_key) else {
            continue;
        };
        if !cleaned.iter().any(|existing| existing == &value) {
            cleaned.push(value);
        }
    }
    cleaned
}

fn summarize_product_attr_values(values: &[String]) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let mut selected = Vec::new();
    let mut total_chars = 0usize;
    for value in values {
        let separator_chars = if selected.is_empty() { 0 } else { 1 };
        let next_chars = value.chars().count();
        if !selected.is_empty() && total_chars + separator_chars + next_chars > 120 {
            break;
        }
        total_chars += separator_chars + next_chars;
        selected.push(value.clone());
        if selected.len() >= 20 {
            break;
        }
    }
    (!selected.is_empty()).then(|| selected.join(";"))
}

fn infer_percent_attr_from_evidence(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<String> {
    if !looks_like_percent_attr(attr_key) {
        return None;
    }
    for text in product_evidence_texts(product) {
        if let Some(value) = extract_percent_fragment(&text) {
            return Some(value);
        }
    }
    None
}

/// 根据已知面料材质推断成分含量百分比
/// 例如：面料材质="纯棉" → 成分含量="棉100%"
fn infer_percent_attr_from_fabric_material(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<String> {
    if !looks_like_percent_attr(attr_key) {
        return None;
    }
    let material = find_known_fabric_material(product)?;
    infer_composition_from_material(&material)
}

/// 从商品数据中查找已知的面料材质
fn find_known_fabric_material(product: &ExternalProductInput) -> Option<String> {
    // 优先从 metadata.ai_attr_suggestions 中查找
    if let Some(metadata) = product.metadata.as_object() {
        for key in [
            "ai_attr_suggestions",
            "wechat_attr_suggestions",
            "wechat_attr_defaults",
        ] {
            if let Some(Value::Object(attrs)) = metadata.get(key) {
                for material_key in &["面料材质", "面料", "材质"] {
                    if let Some(value) = attrs.get(*material_key).and_then(|v| {
                        if let Value::String(s) = v {
                            Some(s.clone())
                        } else {
                            Some(v.to_string())
                        }
                    }) {
                        let trimmed = value.trim().to_string();
                        if !trimmed.is_empty() {
                            return Some(trimmed);
                        }
                    }
                }
            }
        }
    }
    // 从 taobao_item_params 中查找
    if let Some(params) = product
        .metadata
        .as_object()
        .and_then(|m| m.get("taobao_item_params"))
        .and_then(|v| v.as_object())
    {
        for (key, value) in params {
            let normalized = normalize_attr_key_for_exact_match(key);
            if normalized.contains("面料")
                || normalized.contains("材质")
                || normalized.contains("成分")
            {
                if let Some(s) = json_value_to_string(Some(value)) {
                    let trimmed = s.trim().to_string();
                    if !trimmed.is_empty() && trimmed.chars().count() <= 20 {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    None
}

/// 根据面料材质推断典型成分含量
fn infer_composition_from_material(material: &str) -> Option<String> {
    let normalized = material.trim().to_lowercase();
    let result = if normalized.contains("纯棉") || normalized == "棉" {
        "棉100%"
    } else if normalized.contains("涤棉") || normalized.contains("棉涤") {
        "涤纶65%,棉35%"
    } else if normalized.contains("纯涤") || normalized == "涤纶" || normalized.contains("聚酯纤维")
    {
        if normalized.contains("聚酯纤维") {
            "聚酯纤维100%"
        } else {
            "涤纶100%"
        }
    } else if normalized.contains("冰丝") {
        "锦纶88%,氨纶12%"
    } else if normalized.contains("真丝") || normalized.contains("桑蚕丝") {
        "桑蚕丝100%"
    } else if normalized.contains("亚麻") {
        "亚麻100%"
    } else if normalized.contains("羊毛") {
        "羊毛100%"
    } else if normalized.contains("雪纺") {
        "涤纶100%"
    } else if normalized.contains("牛仔") {
        "棉100%"
    } else if normalized.contains("莱卡") || normalized.contains("氨纶") {
        "氨纶100%"
    } else {
        return None;
    };
    Some(result.to_string())
}

fn looks_like_percent_attr(attr_key: &str) -> bool {
    attr_key.contains('%') || attr_key.contains('％') || attr_key.contains("含量")
}

fn extract_percent_fragment(text: &str) -> Option<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let percent_index = chars.iter().position(|ch| matches!(ch, '%' | '％'))?;
    let mut start = percent_index;
    while start > 0 && chars[start - 1].is_ascii_digit() {
        start -= 1;
    }
    if start == percent_index {
        return None;
    }
    let mut end = percent_index + 1;
    while end < chars.len() && chars[end].is_alphanumeric() {
        end += 1;
    }
    Some(chars[start..end].iter().collect::<String>())
}

fn normalize_attr_key_for_exact_match(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

pub(in crate::commands) fn normalize_suggested_value(value: &str, options: &[String]) -> String {
    let trimmed = value.trim();
    if options.is_empty() {
        return trimmed.to_string();
    }
    if options.iter().any(|option| option.trim() == trimmed) {
        return trimmed.to_string();
    }
    let parts = split_category_option_values(trimmed);
    if parts.len() > 1 {
        let mut normalized = Vec::new();
        for part in parts {
            let value = normalize_single_suggested_value(&part, options);
            if !normalized.iter().any(|existing| existing == &value) {
                normalized.push(value);
            }
        }
        return normalized.join(";");
    }
    normalize_single_suggested_value(trimmed, options)
}

fn normalize_single_suggested_value(value: &str, options: &[String]) -> String {
    let trimmed = value.trim();
    // 1. 完全匹配
    if options.iter().any(|option| option.trim() == trimmed) {
        return trimmed.to_string();
    }
    // 2. 包含匹配（option 包含 value 或 value 包含 option）
    if let Some(option) = options
        .iter()
        .find(|option| option.contains(trimmed) || trimmed.contains(option.trim()))
    {
        return option.clone();
    }
    // 3. 匹配不到，保留原值（后续由 AI 侧强制从 allowed_values 选择来保证正确性）
    trimmed.to_string()
}

fn suggestion_value_allowed(value: &str, options: &[String]) -> bool {
    if options.is_empty() {
        return false;
    }
    if options.iter().any(|option| option.trim() == value.trim()) {
        return true;
    }
    // 分号分隔的多值（select_many 场景）：每个部分都需要存在于 options 中
    split_category_option_values(value)
        .into_iter()
        .all(|part| options.iter().any(|option| option.trim() == part))
}

fn suggestion_value_allowed_for_spec(value: &str, spec: &CategoryRequiredAttr) -> bool {
    if spec.options.is_empty() {
        return spec_allows_free_text(spec) && is_trusted_product_attr_value(&spec.key, value);
    }
    suggestion_value_allowed(value, &spec.options)
}

pub(in crate::commands) fn infer_sale_attr_values_from_payload(
    payload: &Value,
    required_key: &str,
) -> Option<Vec<String>> {
    let skus = payload.get("skus").and_then(Value::as_array)?;
    let mut values = Vec::with_capacity(skus.len());
    for sku in skus {
        let attrs = sku.get("sku_attrs").and_then(Value::as_array)?;
        let value = attrs
            .iter()
            .filter_map(Value::as_object)
            .find_map(|object| {
                let key = json_value_to_string(object.get("attr_key"))?;
                if is_same_sale_attr_key(&key, required_key) {
                    json_value_to_string(object.get("attr_value"))
                        .and_then(|value| clean_sku_attr_value_for_key(&value, required_key))
                } else {
                    None
                }
            })?;
        values.push(value);
    }
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

pub(in crate::commands) fn is_same_sale_attr_key(existing_key: &str, required_key: &str) -> bool {
    is_related_attr_key(existing_key, required_key)
}

fn clean_sku_attr_value_for_key(value: &str, required_key: &str) -> Option<String> {
    let _ = required_key;
    let mut value = value.trim().to_string();
    if value.is_empty() {
        return None;
    }

    value = strip_trailing_bracket_note(&value);
    value = value
        .trim_matches(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '-' | '_'
                        | '+'
                        | '/'
                        | '，'
                        | ','
                        | '、'
                        | '；'
                        | ';'
                        | '（'
                        | '）'
                        | '('
                        | ')'
                        | '【'
                        | '】'
                )
        })
        .trim()
        .to_string();
    value = strip_trailing_bracket_note(&value);

    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn strip_trailing_bracket_note(value: &str) -> String {
    let trimmed = value.trim();
    for (left, right) in [('（', '）'), ('(', ')'), ('【', '】'), ('[', ']')] {
        if !trimmed.ends_with(right) {
            continue;
        }
        if let Some(index) = trimmed.rfind(left) {
            let prefix = trimmed[..index].trim();
            if !prefix.is_empty() {
                return prefix.to_string();
            }
        }
    }
    trimmed.to_string()
}

pub(in crate::commands) fn build_attribute_prompt_json(
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    attr_kind: &str,
    spec: &CategoryRequiredAttr,
) -> Value {
    serde_json::json!({
        "task": "补齐微信小店发品必填属性，仅从商品标题、类目线索、SKU规格、外部系统已有字段和图片理解中判断；不确定时返回 null",
        "attr_kind": attr_kind,
        "attr_key": &spec.key,
        "attr_type": &spec.attr_type,
        "allowed_values": &spec.options,
        "related_allowed_values": &spec.related_options,
        "append_allowed": spec.append_allowed,
        "external_product_id": &item.external_product_id,
        "title": &product.title,
        "category_hint": &product.category_hint,
        "brand_hint": &product.brand_hint,
        "external_context": attribute_prompt_external_context(product),
        "skus": product.skus.iter().map(|sku| {
            serde_json::json!({
                "external_sku_id": &sku.external_sku_id,
                "specs": &sku.specs,
                "stock": sku.stock
            })
        }).collect::<Vec<_>>()
    })
}

fn attribute_prompt_external_context(product: &ExternalProductInput) -> Value {
    let Some(metadata) = product.metadata.as_object() else {
        return Value::Null;
    };
    let mut context = serde_json::Map::new();
    for key in [
        "collection_source",
        "taobao_item_params",
        "taobao_item_params_quality",
        "taobao_sku_options",
    ] {
        if let Some(value) = metadata.get(key) {
            context.insert(key.to_string(), value.clone());
        }
    }
    // 将已有的属性建议作为上下文传给 AI，便于关联推断
    // 例如：面料材质=涤棉 → 可推断面料材质成分含量=涤纶65%,棉35%
    for key in [
        "ai_attr_suggestions",
        "wechat_attr_suggestions",
        "wechat_attr_defaults",
    ] {
        if let Some(value) = metadata.get(key) {
            if value.is_object() && !value.as_object().unwrap().is_empty() {
                context.insert("known_attrs".to_string(), value.clone());
                break;
            }
        }
    }
    if context.is_empty() {
        Value::Null
    } else {
        context.insert(
            "usage_note".to_string(),
            Value::String(
                "外部采集字段可能存在错位或噪声，只能作为 AI 判断参考；最终值必须匹配微信官方 allowed_values，无法确认时返回 null。known_attrs 是已确定的同商品其他属性值，可用于关联推断（如面料材质→成分含量）"
                    .to_string(),
            ),
        );
        Value::Object(context)
    }
}

pub(in crate::commands) fn apply_attribute_fill_plan_to_payload(
    payload: &mut Value,
    plan: &AttributeFillPlan,
) -> AppResult<()> {
    for suggestion in &plan.suggestions {
        if !suggestion.applied {
            continue;
        }
        if suggestion.attr_kind == "product" {
            if let Some(value) = &suggestion.suggested_value {
                ensure_payload_product_attr(payload, &suggestion.attr_key, value)?;
            }
        } else if !suggestion.sku_values.is_empty() {
            ensure_payload_sku_attrs(payload, &suggestion.attr_key, &suggestion.sku_values)?;
        } else if let Some(value) = &suggestion.suggested_value {
            ensure_payload_sku_attr_for_all(payload, &suggestion.attr_key, value)?;
        }
    }
    Ok(())
}

pub(in crate::commands) fn ensure_payload_product_attr(
    payload: &mut Value,
    attr_key: &str,
    attr_value: &str,
) -> AppResult<()> {
    let object = payload
        .as_object_mut()
        .ok_or_else(|| AppError::Validation("微信发品 payload 必须是对象".to_string()))?;
    let attrs = object
        .entry("attrs".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !attrs.is_array() {
        *attrs = Value::Array(Vec::new());
    }
    let attrs = attrs
        .as_array_mut()
        .ok_or_else(|| AppError::Validation("微信发品 payload.attrs 必须是数组".to_string()))?;
    if !attrs.iter().any(|attr| {
        attr.as_object()
            .and_then(|object| json_value_to_string(object.get("attr_key")))
            .as_deref()
            == Some(attr_key)
    }) {
        attrs.push(serde_json::json!({
            "attr_key": attr_key,
            "attr_value": attr_value
        }));
    }
    Ok(())
}

pub(in crate::commands) fn ensure_payload_sku_attrs(
    payload: &mut Value,
    attr_key: &str,
    sku_values: &[SkuAttrFill],
) -> AppResult<()> {
    let skus = payload
        .get_mut("skus")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| AppError::Validation("微信发品 payload.skus 必须是数组".to_string()))?;
    for sku_value in sku_values {
        if let Some(sku) = skus.get_mut(sku_value.sku_index) {
            ensure_single_sku_attr(sku, attr_key, &sku_value.value)?;
        }
    }
    Ok(())
}

pub(in crate::commands) fn ensure_payload_sku_attr_for_all(
    payload: &mut Value,
    attr_key: &str,
    attr_value: &str,
) -> AppResult<()> {
    let skus = payload
        .get_mut("skus")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| AppError::Validation("微信发品 payload.skus 必须是数组".to_string()))?;
    for sku in skus {
        ensure_single_sku_attr(sku, attr_key, attr_value)?;
    }
    Ok(())
}

pub(in crate::commands) fn ensure_single_sku_attr(
    sku: &mut Value,
    attr_key: &str,
    attr_value: &str,
) -> AppResult<()> {
    let object = sku
        .as_object_mut()
        .ok_or_else(|| AppError::Validation("微信发品 payload.skus[] 必须是对象".to_string()))?;
    let attrs = object
        .entry("sku_attrs".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !attrs.is_array() {
        *attrs = Value::Array(Vec::new());
    }
    let attrs = attrs.as_array_mut().ok_or_else(|| {
        AppError::Validation("微信发品 payload.skus[].sku_attrs 必须是数组".to_string())
    })?;
    if !attrs.iter().any(|attr| {
        attr.as_object()
            .and_then(|object| json_value_to_string(object.get("attr_key")))
            .as_deref()
            == Some(attr_key)
    }) {
        attrs.push(serde_json::json!({
            "attr_key": attr_key,
            "attr_value": attr_value
        }));
    }
    Ok(())
}

pub(in crate::commands) fn persist_filled_add_product_payload(
    conn: &Connection,
    item: &PendingPublishItem,
    payload: &Value,
    plan: &AttributeFillPlan,
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
        Value::String("local_ai_rules_v1".to_string()),
    );
    metadata.insert(
        "wechat_attr_fill_at".to_string(),
        Value::String(now_shanghai()),
    );
    metadata.insert(
        "wechat_attr_fill_suggestions".to_string(),
        attribute_suggestions_json(&plan.suggestions),
    );
    conn.execute(
        "UPDATE pipeline_shop_targets SET raw_payload = ?1 WHERE id = ?2",
        params![raw_value.to_string(), item.item_id.as_str()],
    )?;
    Ok(())
}

pub(in crate::commands) fn upsert_publish_attribute_suggestion(
    conn: &Connection,
    item: &PendingPublishItem,
    suggestion: &AttributeFillSuggestion,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO publish_attribute_suggestions
         (id, item_id, job_id, product_row_id, shop_id, external_product_id, attr_kind,
          attr_key, suggested_value, sku_values_json, confidence, source, applied, prompt_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
         ON CONFLICT(item_id, attr_kind, attr_key) DO UPDATE SET
           suggested_value = excluded.suggested_value,
           sku_values_json = excluded.sku_values_json,
           confidence = excluded.confidence,
           source = excluded.source,
           applied = excluded.applied,
           prompt_json = excluded.prompt_json,
           updated_at = excluded.updated_at",
        params![
            format!("attr-suggestion-{}", Uuid::new_v4()),
            item.item_id.as_str(),
            item.job_id.as_str(),
            item.product_row_id.as_str(),
            item.shop_id.as_str(),
            item.external_product_id.as_str(),
            suggestion.attr_kind,
            suggestion.attr_key.as_str(),
            suggestion.suggested_value.as_deref(),
            sku_values_json(&suggestion.sku_values),
            suggestion.confidence,
            suggestion.source.as_str(),
            if suggestion.applied { 1 } else { 0 },
            suggestion.prompt_json.to_string(),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn attribute_suggestions_json(
    suggestions: &[AttributeFillSuggestion],
) -> Value {
    Value::Array(
        suggestions
            .iter()
            .map(|suggestion| {
                serde_json::json!({
                    "attr_kind": suggestion.attr_kind,
                    "attr_key": &suggestion.attr_key,
                    "suggested_value": &suggestion.suggested_value,
                    "sku_values": suggestion.sku_values.iter().map(|sku_value| {
                        serde_json::json!({
                            "sku_index": sku_value.sku_index,
                            "value": &sku_value.value
                        })
                    }).collect::<Vec<_>>(),
                    "confidence": suggestion.confidence,
                    "source": &suggestion.source,
                    "applied": suggestion.applied
                })
            })
            .collect(),
    )
}

pub(in crate::commands) fn sku_values_json(values: &[SkuAttrFill]) -> String {
    serde_json::to_string(
        &values
            .iter()
            .map(|value| {
                serde_json::json!({
                    "sku_index": value.sku_index,
                    "value": &value.value
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string())
}

pub(in crate::commands) fn insert_success_api_and_task_log(
    conn: &Connection,
    task_id: &str,
    shop_id: &str,
    endpoint: &'static str,
    method: &'static str,
    message: &str,
) -> AppResult<()> {
    insert_api_call_log(
        conn,
        Some(shop_id),
        endpoint,
        method,
        "success",
        None,
        None,
        Some(message),
    )?;
    insert_task_log(conn, task_id, None, "info", message, None)?;
    Ok(())
}

pub(in crate::commands) fn insert_api_error_and_task_log(
    app: &AppHandle,
    task_id: &str,
    shop_id: &str,
    meta: &WechatCallMeta,
    error: &WechatApiError,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    insert_api_call_log(
        &conn,
        Some(shop_id),
        meta.endpoint,
        meta.method,
        "api_error",
        Some(error.errcode),
        Some(&error.errmsg),
        Some("category rule api error"),
    )?;
    insert_task_log(
        &conn,
        task_id,
        None,
        "error",
        &error.errmsg,
        Some(&serde_json::json!({ "errcode": error.errcode, "endpoint": meta.endpoint })),
    )?;
    Ok(())
}

pub(in crate::commands) fn extract_wechat_categories(
    raw_payload: &Value,
) -> Vec<CachedWechatCategory> {
    let mut categories = Vec::new();
    let category_tree = raw_payload
        .get("cats_v2")
        .or_else(|| raw_payload.get("cats"))
        .or_else(|| {
            raw_payload
                .get("audit_info")
                .and_then(|value| value.get("cats_v2"))
        })
        .or_else(|| {
            raw_payload
                .get("audit_info")
                .and_then(|value| value.get("cats"))
        });
    if let Some(value) = category_tree {
        collect_wechat_categories(value, &mut categories);
    }
    categories.sort_by_key(|category| (category.level.unwrap_or(0), category.cat_id));
    categories.dedup_by_key(|category| category.cat_id);
    categories
}

pub(in crate::commands) fn extract_wechat_category_detail_info(
    raw_payload: &Value,
    fallback_cat_id: i64,
) -> Option<CachedWechatCategory> {
    let object = raw_payload
        .get("info")
        .and_then(Value::as_object)
        .or_else(|| raw_payload.as_object())?;
    let cat_id = json_value_to_i64(object.get("cat_id"))
        .or_else(|| json_value_to_i64(object.get("category_id")))
        .or_else(|| json_value_to_i64(object.get("id")))
        .unwrap_or(fallback_cat_id);
    if cat_id <= 0 {
        return None;
    }
    let name = json_value_to_string(object.get("name"))
        .or_else(|| json_value_to_string(object.get("cat_name")))
        .or_else(|| json_value_to_string(object.get("category_name")))
        .unwrap_or_else(|| format!("{cat_id}"));
    Some(CachedWechatCategory {
        cat_id,
        parent_cat_id: json_value_to_i64(object.get("f_cat_id"))
            .or_else(|| json_value_to_i64(object.get("parent_cat_id"))),
        level: json_value_to_i64(object.get("level"))
            .or_else(|| json_value_to_i64(object.get("cat_level"))),
        name,
        raw_payload: Value::Object(object.clone()),
    })
}

pub(in crate::commands) fn extract_wechat_category_relations(
    raw_payload: &Value,
) -> Vec<CachedWechatCategoryRelation> {
    let mut relations = raw_payload
        .get("list")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    Some(CachedWechatCategoryRelation {
                        cat_id: json_value_to_i64(object.get("id"))?,
                        status: json_value_to_i64(object.get("status")).unwrap_or(0),
                        uneffective_reason: json_value_to_string(object.get("uneffective_reason")),
                        effective_time: json_value_to_i64(object.get("effective_time")),
                        uneffective_time: json_value_to_i64(object.get("uneffective_time")),
                        qua_id: json_value_to_i64(object.get("qua_id")),
                        raw_payload: Value::Object(object.clone()),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    relations.sort_by_key(|relation| relation.cat_id);
    relations.dedup_by_key(|relation| relation.cat_id);
    relations
}

pub(in crate::commands) fn collect_wechat_categories(
    value: &Value,
    categories: &mut Vec<CachedWechatCategory>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_wechat_categories(item, categories);
            }
        }
        Value::Object(object) => {
            if let Some(cat) = object.get("cat").and_then(Value::as_object) {
                if let Some(category) = category_from_object(cat) {
                    categories.push(category);
                }
            } else if let Some(category) = category_from_object(object) {
                categories.push(category);
            }
            for child in object.values() {
                collect_wechat_categories(child, categories);
            }
        }
        _ => {}
    }
}

pub(in crate::commands) fn category_from_object(
    object: &serde_json::Map<String, Value>,
) -> Option<CachedWechatCategory> {
    let cat_id = json_value_to_i64(object.get("cat_id"))
        .or_else(|| json_value_to_i64(object.get("category_id")))
        .or_else(|| json_value_to_i64(object.get("id")))?;
    let name = json_value_to_string(object.get("name"))
        .or_else(|| json_value_to_string(object.get("cat_name")))
        .or_else(|| json_value_to_string(object.get("category_name")))
        .unwrap_or_else(|| format!("{cat_id}"));
    Some(CachedWechatCategory {
        cat_id,
        parent_cat_id: json_value_to_i64(object.get("f_cat_id"))
            .or_else(|| json_value_to_i64(object.get("parent_cat_id"))),
        level: json_value_to_i64(object.get("level")),
        name,
        raw_payload: Value::Object(object.clone()),
    })
}

pub(in crate::commands) fn extract_freight_template_ids(raw_payload: &Value) -> Vec<String> {
    raw_payload
        .get("template_id_list")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_value_to_string(Some(item)))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// 从运费模板详情响应（getfreighttemplatedetail）中取出 freight_template 对象，整体存入缓存。
pub(in crate::commands) fn extract_freight_template_detail(raw_payload: &Value) -> Option<Value> {
    raw_payload
        .get("freight_template")
        .filter(|value| value.is_object())
        .cloned()
}

/// 从已缓存的运费模板 raw_payload 中解析模板名称（freight_template.name，缺失或空串返回 None）。
pub(in crate::commands) fn freight_template_name_from_payload(
    raw_payload: &Value,
) -> Option<String> {
    raw_payload
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

pub(in crate::commands) fn category_detail_counts(raw_payload: &Value) -> CategoryDetailCounts {
    CategoryDetailCounts {
        product_attr_count: count_named_arrays(raw_payload, "product_attr_list"),
        sale_attr_count: count_named_arrays(raw_payload, "sale_attr_list"),
        product_qua_count: count_named_arrays(raw_payload, "product_qua_list"),
    }
}

pub(in crate::commands) fn count_named_arrays(value: &Value, key: &str) -> i64 {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| count_named_arrays(item, key))
            .sum::<i64>(),
        Value::Object(object) => {
            let current = object
                .get(key)
                .and_then(Value::as_array)
                .map(|items| items.len() as i64)
                .unwrap_or(0);
            current
                + object
                    .values()
                    .map(|child| count_named_arrays(child, key))
                    .sum::<i64>()
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_freight_template_detail_picks_object_then_name_round_trips() {
        // getfreighttemplatedetail 成功响应：errcode/errmsg 平铺 + freight_template 对象
        let response = serde_json::json!({
            "errcode": 0,
            "errmsg": "ok",
            "freight_template": {
                "template_id": "1012494298004",
                "name": "标准快递（满 99 包邮）",
                "shipping_method": "CONDITION_FREE"
            }
        });
        // 同步时取出的 freight_template 对象即为存入 raw_payload 的内容
        let stored =
            extract_freight_template_detail(&response).expect("应取出 freight_template 对象");
        assert_eq!(
            stored.get("template_id").and_then(Value::as_str),
            Some("1012494298004")
        );
        // load 时再从存储对象的顶层 name 解析出模板名称（两跳契约）
        assert_eq!(
            freight_template_name_from_payload(&stored).as_deref(),
            Some("标准快递（满 99 包邮）")
        );
    }

    #[test]
    fn freight_template_name_falls_back_to_none_when_missing_or_blank() {
        // 详情查询失败的降级对象只有 template_id，无 name → None（前端兜底显示 ID）
        let degraded = serde_json::json!({ "template_id": "1012494298004" });
        assert_eq!(freight_template_name_from_payload(&degraded), None);
        // 纯空白名称同样视为无名称
        let blank = serde_json::json!({ "name": "   " });
        assert_eq!(freight_template_name_from_payload(&blank), None);
        // 缺少 freight_template 字段的响应取不到对象
        let empty = serde_json::json!({ "errcode": 0, "errmsg": "ok" });
        assert!(extract_freight_template_detail(&empty).is_none());
    }

    #[test]
    fn fallback_fills_empty_options_free_text_required_attr() {
        // 颜色/面料材质：空 options + type=string + append_allowed=false 的自由文本必填属性
        let spec = CategoryRequiredAttr {
            key: "颜色".to_string(),
            options: Vec::new(),
            attr_type: Some("string".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };
        assert_eq!(category_fallback_attr_value(&spec).as_deref(), Some("其他"));
    }

    #[test]
    fn fallback_prefers_generic_option_when_enum_present() {
        let spec = CategoryRequiredAttr {
            key: "面料材质".to_string(),
            options: vec!["棉".to_string(), "聚酯纤维".to_string(), "其他".to_string()],
            attr_type: Some("select_one".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };
        assert_eq!(category_fallback_attr_value(&spec).as_deref(), Some("其他"));
    }

    #[test]
    fn sanitize_fills_invalid_required_attr_instead_of_removing() {
        // cat6236 风格：面料材质 是带枚举(含「其他」)的必填项；payload 里是无法归一到任一枚举的非法值，
        // 走兜底路径——既不能删空，也匹配不上枚举，应填兜底「其他」。
        let raw_detail = serde_json::json!({
            "data": { "attr": {
                "product_attr_list": [
                    { "name": "面料材质", "is_required": true, "type": "select_one",
                      "value": "棉;聚酯纤维;其他" }
                ],
                "sale_attr_list": []
            }}
        });
        let mut payload = serde_json::json!({
            "attrs": [ { "attr_key": "面料材质", "attr_value": "天丝莱赛尔混纺" } ],
            "skus": []
        });
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "x", "title": "t", "source_url": "u", "skus": []
        }))
        .unwrap();
        let report =
            sanitize_payload_attrs_with_category_detail(&product, &mut payload, &raw_detail)
                .unwrap();
        let attrs = payload["attrs"].as_array().unwrap();
        // 必填项不应被删空，而是被兜底为合法值「其他」
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0]["attr_value"].as_str(), Some("其他"));
        assert!(report.removed_attrs.is_empty(), "必填项不应进 removed");
    }

    #[test]
    fn requirement_check_from_detail_reports_missing_product_and_sale_attrs() {
        let raw_detail = serde_json::json!({
            "data": {
                "attr": {
                    "product_attr_list": [
                        { "name": "商品属性一", "is_required": true },
                        { "name": "商品属性二", "is_required": true }
                    ],
                    "sale_attr_list": [
                        { "name": "销售属性一", "is_required": true },
                        { "name": "销售属性二", "is_required": true }
                    ]
                }
            }
        });
        let payload = serde_json::json!({
            "attrs": [
                { "attr_key": "商品属性一", "attr_value": "已填商品值" }
            ],
            "skus": [
                {
                    "sku_attrs": [
                        { "attr_key": "销售属性一", "attr_value": "已填销售值" }
                    ]
                }
            ]
        });

        let check = check_category_requirements_from_detail(&raw_detail, &payload);

        assert_eq!(check.missing_product_attrs, vec!["商品属性二".to_string()]);
        assert_eq!(check.missing_sale_attrs, vec!["销售属性二".to_string()]);
    }

    #[test]
    fn category_options_support_wechat_semicolon_value_string() {
        let mut object = serde_json::Map::new();
        object.insert(
            "value".to_string(),
            Value::String("选项一;选项二;选项三".to_string()),
        );

        let options = category_attr_options(&object);

        assert_eq!(options, vec!["选项一", "选项三", "选项二"]);
    }

    #[test]
    fn category_options_keep_slash_inside_single_wechat_option() {
        let mut object = serde_json::Map::new();
        object.insert(
            "value".to_string(),
            Value::String("田园/小清新风;运动风".to_string()),
        );

        let options = category_attr_options(&object);

        assert!(options.contains(&"田园/小清新风".to_string()));
        assert!(options.contains(&"运动风".to_string()));
        assert!(!options.contains(&"田园".to_string()));
    }

    #[test]
    fn sanitize_payload_attrs_maps_review_aliases_to_wechat_options() {
        let product = ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: "儿童短袖纯棉T恤男童女童夏季婴幼儿宝宝A类上衣".to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            main_video: None,
            skus: Vec::new(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: Some("童装/T恤".to_string()),
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({}),
        };
        let raw_detail = serde_json::json!({
            "attr": {
                "product_attr_list": [
                    {
                        "name": "风格",
                        "is_required": true,
                        "type_v2": "select_one",
                        "value": "运动风;可爱风;其他"
                    },
                    {
                        "name": "适用年龄",
                        "is_required": true,
                        "type_v2": "select_many",
                        "value": "通用;3周岁以下;3周岁以上;6周岁以上;8周岁以上;14周岁以上"
                    }
                ]
            }
        });
        let mut payload = serde_json::json!({
            "attrs": [
                { "attr_key": "风格", "attr_value": "休闲" },
                { "attr_key": "适用年龄", "attr_value": "婴幼儿(0-3岁);小童(3-6岁)" }
            ],
            "skus": []
        });

        let report =
            sanitize_payload_attrs_with_category_detail(&product, &mut payload, &raw_detail)
                .expect("属性清洗应成功");

        assert!(report.has_changes());
        assert_eq!(
            payload
                .pointer("/attrs/0/attr_value")
                .and_then(Value::as_str),
            Some("其他")
        );
        let age_value = payload
            .pointer("/attrs/1/attr_value")
            .and_then(Value::as_str)
            .expect("应写入适用年龄");
        assert!(age_value.contains("3周岁以下"));
        assert!(age_value.contains("3周岁以上"));
        assert!(!age_value.contains("婴幼儿"));
        assert!(!age_value.contains("小童"));
    }

    #[test]
    fn product_attr_inference_uses_exact_external_field_only() {
        let product = ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: "测试商品".to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            main_video: None,
            skus: Vec::new(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: None,
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({
                "taobao_item_params_quality": {
                    "trusted": true
                },
                "taobao_item_params": {
                    "官方属性": "外部已确认值",
                    "近似属性": "不能自动采纳"
                }
            }),
        };

        assert_eq!(
            infer_product_attr_value(&product, "官方属性").map(|value| value.0),
            Some("外部已确认值".to_string())
        );
        assert!(infer_product_attr_value(&product, "属性").is_none());
    }

    #[test]
    fn product_attr_inference_rejects_untrusted_external_params() {
        let product = ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: "测试商品".to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            main_video: None,
            skus: Vec::new(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: None,
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({
                "taobao_item_params_quality": {
                    "trusted": false
                },
                "taobao_item_params": {
                    "官方属性": "外部值"
                }
            }),
        };

        assert!(infer_product_attr_value(&product, "官方属性").is_none());
    }

    #[test]
    fn sale_attr_match_requires_exact_normalized_key() {
        assert!(is_same_sale_attr_key("销售属性一", "销售属性一"));
        assert!(is_same_sale_attr_key("颜色分类", "颜色"));
        assert!(is_same_sale_attr_key("身高", "尺码"));
        assert!(!is_same_sale_attr_key("销售属性一", "属性一"));
    }

    #[test]
    fn product_attr_inference_matches_allowed_value_from_title() {
        let product = test_product(
            "2026新款儿童短袖纯棉T恤男童女童夏季婴幼儿宝宝a类上衣",
            serde_json::json!({}),
            serde_json::json!({}),
        );
        let spec = CategoryRequiredAttr {
            key: "安全等级".to_string(),
            options: vec!["A类".to_string(), "B类".to_string(), "C类".to_string()],
            attr_type: Some("select_one".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };

        assert_eq!(
            infer_product_attr_value_from_spec(&product, &spec).map(|value| value.0),
            Some("A类".to_string())
        );
    }

    #[test]
    fn product_attr_inference_uses_related_category_options_for_string_material() {
        let raw_detail = serde_json::json!({
            "attr": {
                "product_attr_list": [
                    {
                        "name": "面料",
                        "is_required": false,
                        "type_v2": "select_one",
                        "value": "纯棉;棉麻"
                    },
                    {
                        "name": "面料材质",
                        "is_required": true,
                        "type_v2": "string",
                        "value": ""
                    }
                ]
            }
        });
        let specs = required_category_attr_specs(&raw_detail, "product_attr_list");
        let spec = specs.get("面料材质").expect("应提取面料材质属性");
        let product = test_product(
            "儿童短袖纯棉T恤男童女童夏季上衣",
            serde_json::json!({}),
            serde_json::json!({}),
        );

        assert_eq!(
            infer_product_attr_value_from_spec(&product, spec).map(|value| value.0),
            Some("纯棉".to_string())
        );
    }

    #[test]
    fn product_attr_inference_fills_children_sportswear_common_attrs() {
        let product = ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: "儿童篮球服23号詹姆斯球衣男童中大童速干两件运动套装短袖队服".to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            main_video: None,
            skus: [110, 130, 150]
                .into_iter()
                .map(|height| crate::models::ExternalSkuInput {
                    external_sku_id: format!("sku-{height}"),
                    specs: serde_json::json!({
                        "身高": height.to_string(),
                        "颜色分类": "湖蓝"
                    }),
                    cost_price: 10.0,
                    stock: 10,
                    sku_image: None,
                })
                .collect(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: Some("母婴 > 童装 > 套装".to_string()),
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({}),
        };
        let safety = CategoryRequiredAttr {
            key: "安全等级".to_string(),
            options: vec!["A类".to_string(), "B类".to_string(), "C类".to_string()],
            attr_type: Some("select_one".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };
        let age = CategoryRequiredAttr {
            key: "适用年龄".to_string(),
            options: vec![
                "通用".to_string(),
                "3周岁以下".to_string(),
                "3周岁以上".to_string(),
                "6周岁以上".to_string(),
                "8周岁以上".to_string(),
                "14周岁以上".to_string(),
            ],
            attr_type: Some("select_many".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };
        let material = CategoryRequiredAttr {
            key: "面料材质".to_string(),
            options: Vec::new(),
            attr_type: Some("string".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };
        let composition = CategoryRequiredAttr {
            key: "面料材质成分含量（%）".to_string(),
            options: Vec::new(),
            attr_type: Some("string".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };
        let style = CategoryRequiredAttr {
            key: "风格".to_string(),
            options: vec![
                "运动风".to_string(),
                "汉风".to_string(),
                "可爱风".to_string(),
            ],
            attr_type: Some("select_one".to_string()),
            append_allowed: false,
            related_options: Vec::new(),
        };

        assert_eq!(
            infer_product_attr_value_from_spec(&product, &safety).map(|value| value.0),
            Some("B类".to_string())
        );
        assert_eq!(
            infer_product_attr_value_from_spec(&product, &age).map(|value| value.0),
            Some("3周岁以上;6周岁以上;8周岁以上".to_string())
        );
        assert_eq!(
            infer_product_attr_value_from_spec(&product, &material).map(|value| value.0),
            Some("聚酯纤维".to_string())
        );
        assert_eq!(
            infer_product_attr_value_from_spec(&product, &composition).map(|value| value.0),
            Some("聚酯纤维100%".to_string())
        );
        assert_eq!(
            infer_product_attr_value_from_spec(&product, &style).map(|value| value.0),
            Some("运动风".to_string())
        );
    }

    #[test]
    fn product_attr_inference_summarizes_related_sku_values() {
        let product = test_product(
            "儿童裤子",
            serde_json::json!({}),
            serde_json::json!({
                "身高": "90cm （建议80cm）",
                "颜色分类": "蓝色（买一送五）"
            }),
        );

        assert_eq!(
            infer_product_attr_value(&product, "尺码").map(|value| value.0),
            Some("90cm".to_string())
        );
        assert_eq!(
            infer_product_attr_value(&product, "颜色").map(|value| value.0),
            Some("蓝色".to_string())
        );
    }

    #[test]
    fn product_attr_inference_keeps_long_sku_summary_prefix() {
        let product = ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: "女童洋气冰丝防蚊裤子".to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            main_video: None,
            skus: (80..130)
                .map(|size| crate::models::ExternalSkuInput {
                    external_sku_id: format!("sku-{size}"),
                    specs: serde_json::json!({
                        "身高": format!("{size}cm")
                    }),
                    cost_price: 10.0,
                    stock: 10,
                    sku_image: None,
                })
                .collect(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: Some("童装/裤子".to_string()),
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({}),
        };

        let inferred = infer_product_attr_value(&product, "尺码")
            .map(|value| value.0)
            .expect("应从 SKU 身高汇总出尺码");

        assert!(inferred.contains("80cm"));
        assert!(inferred.chars().count() <= 120);
    }

    #[test]
    fn sale_attr_inference_trims_wrapping_symbols() {
        let payload = serde_json::json!({
            "skus": [
                { "sku_attrs": [{ "attr_key": "颜色", "attr_value": "（红色）" }] },
                { "sku_attrs": [{ "attr_key": "颜色", "attr_value": "【蓝色】" }] }
            ]
        });

        assert_eq!(
            infer_sale_attr_values_from_payload(&payload, "颜色"),
            Some(vec!["红色".to_string(), "蓝色".to_string()])
        );
    }

    #[test]
    fn attribute_fill_plan_only_applies_high_confidence_suggestions() {
        let mut payload = serde_json::json!({});
        let plan = AttributeFillPlan {
            suggestions: vec![
                AttributeFillSuggestion {
                    attr_kind: "product",
                    attr_key: "安全等级".to_string(),
                    suggested_value: Some("短款".to_string()),
                    sku_values: Vec::new(),
                    confidence: 20,
                    source: "needs_ai".to_string(),
                    applied: false,
                    prompt_json: serde_json::json!({}),
                },
                AttributeFillSuggestion {
                    attr_kind: "product",
                    attr_key: "面料材质".to_string(),
                    suggested_value: Some("棉".to_string()),
                    sku_values: Vec::new(),
                    confidence: 90,
                    source: "external_attr_alias".to_string(),
                    applied: true,
                    prompt_json: serde_json::json!({}),
                },
            ],
        };

        apply_attribute_fill_plan_to_payload(&mut payload, &plan).expect("应用补齐计划应成功");

        assert_eq!(
            payload.get("attrs"),
            Some(&serde_json::json!([{ "attr_key": "面料材质", "attr_value": "棉" }]))
        );
    }

    fn test_product(title: &str, metadata: Value, specs: Value) -> ExternalProductInput {
        ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: title.to_string(),
            source_url: "https://example.com/item".to_string(),
            images: Vec::new(),
            detail_images: Vec::new(),
            main_video: None,
            skus: vec![crate::models::ExternalSkuInput {
                external_sku_id: "sku-1".to_string(),
                specs,
                cost_price: 1.0,
                stock: 10,
                sku_image: None,
            }],
            supplier_name: None,
            supplier_product_id: None,
            category_hint: None,
            brand_hint: None,
            weight_gram: None,
            metadata,
        }
    }
}
