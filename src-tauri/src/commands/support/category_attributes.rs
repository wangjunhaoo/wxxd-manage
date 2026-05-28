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
    let product_required = required_category_attrs(&raw_detail, "product_attr_list");
    let sale_required = required_category_attrs(&raw_detail, "sale_attr_list");
    let product_keys = payload_product_attr_keys(payload);
    let sale_keys = payload_sale_attr_keys(payload);

    Ok(CachedCategoryRequirementCheck {
        detail_found: true,
        missing_product_attrs: product_required
            .into_iter()
            .filter(|name| !product_keys.contains(name))
            .collect(),
        missing_sale_attrs: sale_required
            .into_iter()
            .filter(|name| !sale_keys.contains(name))
            .collect(),
    })
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
            .unwrap_or_else(|| CategoryRequiredAttr {
                key: attr_key.clone(),
                options: Vec::new(),
            });
        plan.suggestions
            .push(build_product_attr_suggestion(item, product, &spec));
    }
    for attr_key in &requirement_check.missing_sale_attrs {
        let spec = sale_specs
            .get(attr_key)
            .cloned()
            .unwrap_or_else(|| CategoryRequiredAttr {
                key: attr_key.clone(),
                options: Vec::new(),
            });
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
    specs
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
    options.into_iter().collect()
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

pub(in crate::commands) fn build_product_attr_suggestion(
    item: &PendingPublishItem,
    product: &ExternalProductInput,
    spec: &CategoryRequiredAttr,
) -> AttributeFillSuggestion {
    let prompt_json = build_attribute_prompt_json(item, product, "product", spec);
    if let Some(value) = metadata_attr_suggestion(product, &spec.key) {
        return AttributeFillSuggestion {
            attr_kind: "product",
            attr_key: spec.key.clone(),
            suggested_value: Some(normalize_suggested_value(&value, &spec.options)),
            sku_values: Vec::new(),
            confidence: 96,
            source: "metadata.ai_attr_suggestions".to_string(),
            applied: true,
            prompt_json,
        };
    }
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
    if let Some((value, confidence, source)) = infer_product_attr_value(product, &spec.key) {
        let normalized = normalize_suggested_value(&value, &spec.options);
        let applied =
            confidence >= 85 && (spec.options.is_empty() || spec.options.contains(&normalized));
        return AttributeFillSuggestion {
            attr_kind: "product",
            attr_key: spec.key.clone(),
            suggested_value: Some(normalized),
            sku_values: Vec::new(),
            confidence,
            source: source.to_string(),
            applied,
            prompt_json,
        };
    }
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
            source: "sku_attr_synonym".to_string(),
            applied: true,
            prompt_json,
        };
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
        return Ok(None);
    }
    let Some(inferred) = infer_wechat_category_from_cache(conn, &item.shop_id, product)? else {
        return Ok(None);
    };

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
        "UPDATE publish_products SET raw_payload = ?1 WHERE id = ?2",
        params![updated_raw, item.product_row_id.as_str()],
    )?;
    item.raw_payload = updated_raw.clone();
    *product = serde_json::from_str(&updated_raw).map_err(|error| {
        AppError::Validation(format!("微信类目推断后的商品数据无法解析：{error}"))
    })?;

    Ok(Some(format!(
        "已根据本地微信类目缓存推断类目：{}",
        inferred.category_path
    )))
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
    let child_context = has_children_category_context(product);
    let candidates = load_wechat_category_inference_candidates(conn, shop_id, product)?;
    let Some(best) = candidates.first() else {
        return Ok(None);
    };
    if best.score < 90 || (best.matched_terms < 2 && !child_context) {
        return Ok(None);
    }
    if let Some(second) = candidates.get(1) {
        if best.score - second.score < 18 && best.score < 180 {
            return Ok(None);
        }
    }
    Ok(Some(InferredWechatCategory {
        category_ids: best.category_ids.clone(),
        category_path: best.category_path.clone(),
        source: "local_category_cache_match",
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
) -> AppResult<Vec<CategoryInferenceCandidate>> {
    let terms = category_inference_terms(product);
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let child_context = has_children_category_context(product);
    let candidate_ids = load_category_inference_candidate_ids(conn, shop_id, &terms)?;
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

pub(in crate::commands) fn suggest_wechat_category_candidates_from_cache(
    conn: &Connection,
    shop_id: &str,
    product: &ExternalProductInput,
    limit: usize,
) -> AppResult<Vec<CollectionReviewCategoryCandidate>> {
    let candidates = load_wechat_category_inference_candidates(conn, shop_id, product)?;
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

fn load_category_inference_candidate_ids(
    conn: &Connection,
    shop_id: &str,
    terms: &[String],
) -> AppResult<Vec<i64>> {
    let mut candidate_ids = BTreeSet::new();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT c.cat_id
         FROM wechat_categories c
         LEFT JOIN wechat_categories child
           ON child.shop_id = c.shop_id AND child.parent_cat_id = c.cat_id
         WHERE c.shop_id = ?1
           AND child.cat_id IS NULL
           AND (
             c.name = ?2
             OR c.name LIKE '%' || ?2 || '%'
             OR ?2 LIKE '%' || c.name || '%'
           )
         ORDER BY COALESCE(c.level, 0) DESC, c.cat_id ASC
         LIMIT 80",
    )?;
    for term in terms.iter().take(16) {
        let ids = stmt
            .query_map(params![shop_id, term], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        candidate_ids.extend(ids);
    }
    Ok(candidate_ids.into_iter().collect())
}

fn category_inference_terms(product: &ExternalProductInput) -> Vec<String> {
    let mut terms = Vec::new();
    if let Some(category_hint) = product.category_hint.as_deref() {
        push_split_category_terms(&mut terms, category_hint);
    }
    if let Some((parent_name, leaf_name, _source)) = infer_category_terms(product) {
        push_category_term_with_derivatives(&mut terms, parent_name);
        push_category_term_with_derivatives(&mut terms, leaf_name);
    }
    let context = category_inference_context(product);
    for (needle, term) in [
        ("童装", "童装"),
        ("儿童", "童装"),
        ("女童", "童装"),
        ("男童", "童装"),
        ("宝宝", "童装"),
        ("婴儿", "童装"),
        ("婴幼儿", "童装"),
        ("亲子装", "童装"),
        ("T恤", "T恤"),
        ("t恤", "T恤"),
        ("短袖", "T恤"),
        ("半袖", "T恤"),
        ("连衣裙", "连衣裙"),
        ("裙", "连衣裙"),
        ("旗袍", "旗袍"),
        ("唐装", "唐装"),
        ("汉服", "汉服"),
        ("民族服装", "民族服装"),
        ("舞台装", "舞台装"),
    ] {
        if context.contains(needle) {
            push_category_term_with_derivatives(&mut terms, term);
        }
    }
    terms
}

fn category_inference_context(product: &ExternalProductInput) -> String {
    let mut context = format!(
        "{} {}",
        product.title,
        product.category_hint.as_deref().unwrap_or_default()
    );
    if let Some(metadata) = product.metadata.as_object() {
        if let Some(params) = metadata.get("taobao_item_params") {
            context.push(' ');
            context.push_str(&params.to_string());
        }
    }
    context
}

fn has_children_category_context(product: &ExternalProductInput) -> bool {
    contains_any(
        &category_inference_context(product),
        &[
            "童装",
            "儿童",
            "女童",
            "男童",
            "宝宝",
            "婴儿",
            "婴幼儿",
            "亲子装",
        ],
    )
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
    push_unique_category_term(terms, term.clone());
    for prefix in [
        "儿童",
        "童装",
        "女童",
        "男童",
        "宝宝",
        "婴儿",
        "婴幼儿",
        "亲子",
    ] {
        if let Some(stripped) = term.strip_prefix(prefix) {
            if stripped != term {
                if let Some(stripped) = normalize_category_match_term(stripped) {
                    push_unique_category_term(terms, stripped);
                }
            }
        }
    }
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
    if normalized.chars().count() < 2
        || matches!(
            normalized.as_str(),
            "其他" | "其它" | "服装" | "服饰" | "衣服" | "商品"
        )
    {
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

    let has_child_path = path_text.contains("童装")
        || path_text.contains("母婴")
        || path_text.contains("童鞋")
        || path_text.contains("婴童")
        || path_text.contains("儿童");
    if child_context && has_child_path {
        score += 45;
    } else if child_context {
        score -= 55;
    }
    if child_context
        && !has_child_path
        && (path_text.contains("女装") || path_text.contains("男装"))
    {
        score -= 35;
    }

    (score, matched_terms.len())
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

fn infer_category_terms(
    product: &ExternalProductInput,
) -> Option<(&'static str, &'static str, &'static str)> {
    let mut context = format!(
        "{} {}",
        product.title,
        product.category_hint.as_deref().unwrap_or_default()
    );
    if let Some(metadata) = product.metadata.as_object() {
        if let Some(params) = metadata.get("taobao_item_params") {
            context.push(' ');
            context.push_str(&params.to_string());
        }
    }
    let is_children = contains_any(
        &context,
        &["童装", "儿童", "女童", "男童", "宝宝", "婴儿", "婴幼儿"],
    );
    if is_children && contains_any(&context, &["T恤", "t恤", "短袖", "半袖"]) {
        return Some(("童装", "T恤", "local_children_tshirt_rule"));
    }
    if is_children && contains_any(&context, &["连衣裙", "裙"]) {
        return Some(("童装", "连衣裙", "local_children_dress_rule"));
    }
    None
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub(in crate::commands) fn infer_product_attr_value(
    product: &ExternalProductInput,
    attr_key: &str,
) -> Option<(String, i64, &'static str)> {
    let context = format!(
        "{} {} {}",
        product.title,
        product.category_hint.as_deref().unwrap_or_default(),
        product.brand_hint.as_deref().unwrap_or_default()
    );
    let lower = context.to_ascii_lowercase();
    if attr_key.contains("性别") || attr_key.contains("适用人群") {
        if context.contains('女') || lower.contains("women") || lower.contains("female") {
            return Some(("女".to_string(), 90, "title_gender_rule"));
        }
        if context.contains('男') || lower.contains("men") || lower.contains("male") {
            return Some(("男".to_string(), 90, "title_gender_rule"));
        }
        if context.contains("通用") || lower.contains("unisex") {
            return Some(("通用".to_string(), 88, "title_gender_rule"));
        }
    }
    if attr_key.contains("季节") {
        if context.contains('夏') || context.contains("防晒") || context.contains("速干") {
            return Some(("夏季".to_string(), 88, "title_season_rule"));
        }
        if context.contains('冬') || context.contains("加绒") || context.contains("保暖") {
            return Some(("冬季".to_string(), 88, "title_season_rule"));
        }
        if context.contains("春秋") {
            return Some(("春秋".to_string(), 88, "title_season_rule"));
        }
    }
    if attr_key.contains("材质") || attr_key.contains("面料") || attr_key.contains("成分") {
        if context.contains("聚酯纤维") || context.contains("涤纶") || context.contains("防晒")
        {
            return Some(("聚酯纤维".to_string(), 88, "title_material_rule"));
        }
        if context.contains("纯棉") || context.contains('棉') {
            return Some(("棉".to_string(), 88, "title_material_rule"));
        }
        if context.contains("真皮") || context.contains("皮革") {
            return Some(("皮革".to_string(), 88, "title_material_rule"));
        }
    }
    None
}

pub(in crate::commands) fn normalize_suggested_value(value: &str, options: &[String]) -> String {
    let trimmed = value.trim();
    if options.is_empty() {
        return trimmed.to_string();
    }
    options
        .iter()
        .find(|option| option.trim() == trimmed)
        .cloned()
        .or_else(|| {
            options
                .iter()
                .find(|option| option.contains(trimmed) || trimmed.contains(option.trim()))
                .cloned()
        })
        .unwrap_or_else(|| trimmed.to_string())
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
                if is_sale_attr_synonym(&key, required_key) {
                    json_value_to_string(object.get("attr_value"))
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

pub(in crate::commands) fn is_sale_attr_synonym(existing_key: &str, required_key: &str) -> bool {
    let existing = existing_key.trim();
    let required = required_key.trim();
    if existing == required {
        return false;
    }
    let color_required = required.contains("颜色") || required.contains('色');
    let color_existing = existing.contains("颜色") || existing.contains('色');
    if color_required && color_existing {
        return true;
    }
    let size_required =
        required.contains("尺码") || required.contains("尺寸") || required.contains("大小");
    let size_existing =
        existing.contains("尺码") || existing.contains("尺寸") || existing.contains("大小");
    if size_required && size_existing {
        return true;
    }
    required == "规格" && (size_existing || color_existing)
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
        "allowed_values": &spec.options,
        "external_product_id": &item.external_product_id,
        "title": &product.title,
        "category_hint": &product.category_hint,
        "brand_hint": &product.brand_hint,
        "skus": product.skus.iter().map(|sku| {
            serde_json::json!({
                "external_sku_id": &sku.external_sku_id,
                "specs": &sku.specs,
                "stock": sku.stock
            })
        }).collect::<Vec<_>>()
    })
}

pub(in crate::commands) fn apply_attribute_fill_plan_to_payload(
    payload: &mut Value,
    plan: &AttributeFillPlan,
) -> AppResult<()> {
    for suggestion in &plan.suggestions {
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
        "UPDATE publish_products SET raw_payload = ?1 WHERE id = ?2",
        params![raw_value.to_string(), item.product_row_id.as_str()],
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

pub(in crate::commands) fn parse_sku_attr_fill_values(
    raw: Option<&str>,
) -> AppResult<Vec<SkuAttrFill>> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };
    let values = serde_json::from_str::<Value>(raw)
        .map_err(|error| AppError::Validation(format!("SKU 属性建议 JSON 解析失败：{error}")))?;
    let items = values
        .as_array()
        .ok_or_else(|| AppError::Validation("SKU 属性建议必须是数组".to_string()))?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let sku_index = json_value_to_i64(object.get("sku_index"))?;
            let value = json_value_to_string(object.get("value"))?;
            if value.trim().is_empty() {
                return None;
            }
            Some(SkuAttrFill {
                sku_index: sku_index.max(0) as usize,
                value,
            })
        })
        .collect())
}

pub(in crate::commands) fn normalize_attribute_suggestion_status(
    value: Option<&str>,
) -> AppResult<&'static str> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("pending") => Ok("pending"),
        Some("applied") => Ok("applied"),
        Some("all") => Ok("all"),
        Some(other) => Err(AppError::Validation(format!(
            "未知属性建议状态筛选：{other}"
        ))),
    }
}

pub(in crate::commands) fn count_attribute_suggestions_by_status(
    conn: &Connection,
    status: &str,
    job_id: Option<&str>,
    item_id: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM publish_attribute_suggestions
         WHERE (?1 = 'all'
                OR (?1 = 'pending' AND applied = 0)
                OR (?1 = 'applied' AND applied = 1))
           AND (?2 IS NULL OR job_id = ?2)
           AND (?3 IS NULL OR item_id = ?3)",
        params![status, job_id, item_id],
        |row| row.get(0),
    )?)
}

pub(in crate::commands) fn load_publish_attribute_suggestion_views(
    conn: &Connection,
    status: &str,
    job_id: Option<&str>,
    item_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<PublishAttributeSuggestionView>> {
    let mut stmt = conn.prepare(
        "SELECT
           a.id,
           a.item_id,
           a.job_id,
           a.product_row_id,
           a.shop_id,
           COALESCE(s.name, a.shop_id),
           a.external_product_id,
           p.title,
           a.attr_kind,
           a.attr_key,
           a.suggested_value,
           a.sku_values_json,
           a.confidence,
           a.source,
           a.applied,
           a.prompt_json,
           a.updated_at
         FROM publish_attribute_suggestions a
         JOIN publish_products p ON p.id = a.product_row_id
         LEFT JOIN shops s ON s.id = a.shop_id
         WHERE (?1 = 'all'
                OR (?1 = 'pending' AND a.applied = 0)
                OR (?1 = 'applied' AND a.applied = 1))
           AND (?2 IS NULL OR a.job_id = ?2)
           AND (?3 IS NULL OR a.item_id = ?3)
         ORDER BY a.applied ASC, a.updated_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(params![status, job_id, item_id, limit], |row| {
            let prompt_json_raw: Option<String> = row.get(15)?;
            let prompt_json = prompt_json_raw
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .unwrap_or(Value::Null);
            let sku_values =
                parse_sku_attr_fill_values(row.get::<_, Option<String>>(11)?.as_deref())
                    .map(|values| {
                        values
                            .into_iter()
                            .map(|value| PublishAttributeSuggestionSkuValue {
                                sku_index: value.sku_index,
                                value: value.value,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
            Ok(PublishAttributeSuggestionView {
                id: row.get(0)?,
                item_id: row.get(1)?,
                job_id: row.get(2)?,
                product_row_id: row.get(3)?,
                shop_id: row.get(4)?,
                shop_name: row.get(5)?,
                external_product_id: row.get(6)?,
                title: row.get(7)?,
                attr_kind: row.get(8)?,
                attr_key: row.get(9)?,
                suggested_value: row.get(10)?,
                sku_values,
                confidence: row.get(12)?,
                source: row.get(13)?,
                applied: row.get::<_, i64>(14)? == 1,
                allowed_values: attribute_suggestion_allowed_values(&prompt_json),
                reason: attribute_suggestion_reason(&prompt_json),
                updated_at: row.get(16)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(in crate::commands) fn attribute_suggestion_allowed_values(prompt_json: &Value) -> Vec<String> {
    prompt_json
        .get("allowed_values")
        .or_else(|| prompt_json.pointer("/request/allowed_values"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| json_value_to_string(Some(item)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(in crate::commands) fn attribute_suggestion_reason(prompt_json: &Value) -> Option<String> {
    prompt_json
        .get("reason")
        .or_else(|| prompt_json.pointer("/response/reason"))
        .and_then(|value| json_value_to_string(Some(value)))
        .map(|value| truncate_for_summary(&value, 120))
}

pub(in crate::commands) fn load_attribute_suggestion_for_apply(
    conn: &Connection,
    suggestion_id: &str,
) -> AppResult<Option<AttributeSuggestionApplyRow>> {
    conn.query_row(
        "SELECT
           a.id,
           a.attr_kind,
           a.attr_key,
           a.suggested_value,
           a.sku_values_json,
           a.confidence,
           a.source,
           a.applied,
           a.prompt_json,
           i.id,
           i.job_id,
           i.product_row_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           p.external_product_id,
           p.raw_payload
         FROM publish_attribute_suggestions a
         JOIN publish_job_items i ON i.id = a.item_id
         JOIN publish_products p ON p.id = a.product_row_id
         JOIN shops s ON s.id = a.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         WHERE a.id = ?1",
        [suggestion_id],
        |row| {
            Ok(AttributeSuggestionApplyRow {
                id: row.get(0)?,
                attr_kind: row.get(1)?,
                attr_key: row.get(2)?,
                suggested_value: row.get(3)?,
                sku_values_json: row.get(4)?,
                confidence: row.get(5)?,
                source: row.get(6)?,
                applied: row.get::<_, i64>(7)? == 1,
                prompt_json: row.get(8)?,
                item: PendingPublishItem {
                    item_id: row.get(9)?,
                    job_id: row.get(10)?,
                    product_row_id: row.get(11)?,
                    shop_id: row.get(12)?,
                    shop_status: row.get(13)?,
                    shop_has_secret: row.get::<_, i64>(14)? == 1,
                    external_product_id: row.get(15)?,
                    raw_payload: row.get(16)?,
                },
            })
        },
    )
    .optional()
    .map_err(AppError::from)
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
    if let Some(value) = raw_payload
        .get("cats_v2")
        .or_else(|| raw_payload.get("cats"))
    {
        collect_wechat_categories(value, &mut categories);
    }
    categories.sort_by_key(|category| (category.level.unwrap_or(0), category.cat_id));
    categories.dedup_by_key(|category| category.cat_id);
    categories
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
    let cat_id = json_value_to_i64(object.get("cat_id"))?;
    let name = json_value_to_string(object.get("name")).unwrap_or_else(|| format!("{cat_id}"));
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
