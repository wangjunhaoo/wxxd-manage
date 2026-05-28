use super::*;

pub(in crate::commands) fn precheck_publish_item(
    conn: &Connection,
    item: &PendingPublishItem,
    product: &ExternalProductInput,
) -> AppResult<Option<(&'static str, String)>> {
    if item.shop_status != "active" {
        return Ok(Some((
            "SHOP_NOT_ACTIVE",
            "店铺未激活，请先验证凭证并同步店铺资料".to_string(),
        )));
    }
    if !item.shop_has_secret {
        return Ok(Some((
            "SHOP_SECRET_MISSING",
            "店铺缺少 app_secret，无法调用微信接口".to_string(),
        )));
    }
    let existing_product = conn
        .query_row(
            "SELECT id FROM shop_products WHERE shop_id = ?1 AND external_product_id = ?2",
            params![item.shop_id.as_str(), item.external_product_id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if existing_product.is_some() {
        return Ok(Some((
            "DUPLICATE_EXTERNAL_PRODUCT_IN_SHOP",
            "同一个 external_product_id 已经铺过该店铺".to_string(),
        )));
    }
    if product.images.len() < 3 {
        return Ok(Some((
            "INSUFFICIENT_HEAD_IMAGES",
            "商品主图少于 3 张，无法进入微信发品".to_string(),
        )));
    }
    if product.detail_images.is_empty() {
        return Ok(Some((
            "INSUFFICIENT_DETAIL_IMAGES",
            "商品详情图为空，无法进入微信发品".to_string(),
        )));
    }
    if product
        .category_hint
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
        && !product_has_wechat_category_metadata(product)?
    {
        return Ok(Some((
            "CATEGORY_NEEDS_AI_FILL",
            "缺少类目线索，需要先用 AI/规则补齐类目与属性".to_string(),
        )));
    }
    if product.skus.iter().all(|sku| sku.stock <= 0) {
        return Ok(Some((
            "SUPPLIER_STOCK_EMPTY",
            "所有 SKU 库存都为 0，暂不铺货".to_string(),
        )));
    }
    Ok(None)
}

pub(in crate::commands) fn prepare_add_product_payload_for_publish(
    conn: &Connection,
    item: &PendingPublishItem,
    product: &ExternalProductInput,
) -> AppResult<Result<AddProductPayloadPrepare, (&'static str, String)>> {
    let draft = match resolve_add_product_base_payload(product) {
        Ok(draft) => draft,
        Err(error) => return Ok(Err(("WECHAT_PAYLOAD_NEEDS_AI_FILL", error))),
    };

    if draft.source == "local_rules_v1" {
        persist_generated_add_product_payload(conn, item, &draft)?;
        insert_task_log(
            conn,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "已根据本地规则生成微信发品参数草稿",
            Some(&serde_json::json!({
                "source": draft.source,
                "product_row_id": item.product_row_id,
                "external_product_id": item.external_product_id,
                "warnings": &draft.warnings
            })),
        )?;
    } else {
        insert_task_log(
            conn,
            &item.job_id,
            Some(&item.item_id),
            "info",
            "已使用外部传入的微信发品参数",
            Some(&serde_json::json!({
                "source": draft.source,
                "product_row_id": item.product_row_id,
                "external_product_id": item.external_product_id
            })),
        )?;
    }

    if !draft.warnings.is_empty() {
        insert_task_log(
            conn,
            &item.job_id,
            Some(&item.item_id),
            "warn",
            "微信发品参数草稿存在需要关注的补齐项",
            Some(&serde_json::json!({
                "source": draft.source,
                "warnings": &draft.warnings
            })),
        )?;
    }

    Ok(Ok(AddProductPayloadPrepare {
        source: draft.source,
        warnings: draft.warnings,
    }))
}

pub(in crate::commands) fn publish_payload_ready_summary(
    prepare: &AddProductPayloadPrepare,
) -> String {
    let source_label = match prepare.source {
        "local_rules_v1" => "已生成微信发品参数草稿",
        "metadata.wechat_add_product_payload" => "已使用外部微信发品参数",
        "metadata.wechat_product_payload" => "已使用外部兼容微信发品参数",
        _ => "微信发品参数已就绪",
    };
    if prepare.warnings.is_empty() {
        format!("本地前置校验通过，{source_label}，等待素材上传与微信发品")
    } else {
        format!(
            "本地前置校验通过，{source_label}；仍需关注：{}",
            prepare.warnings.join("；")
        )
    }
}

pub(in crate::commands) fn persist_generated_add_product_payload(
    conn: &Connection,
    item: &PendingPublishItem,
    draft: &AddProductPayloadDraft,
) -> AppResult<()> {
    let mut raw_value = serde_json::from_str::<Value>(&item.raw_payload).map_err(|error| {
        AppError::Validation(format!(
            "商品原始数据无法解析，不能写入微信发品草稿：{error}"
        ))
    })?;
    let raw_object = raw_value.as_object_mut().ok_or_else(|| {
        AppError::Validation("商品原始数据不是对象，不能写入微信发品草稿".to_string())
    })?;
    let metadata_value = raw_object
        .entry("metadata".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if metadata_value.is_null() {
        *metadata_value = Value::Object(serde_json::Map::new());
    }
    let metadata = metadata_value.as_object_mut().ok_or_else(|| {
        AppError::Validation("metadata 必须是对象，不能写入微信发品草稿".to_string())
    })?;
    metadata.insert(
        "wechat_add_product_payload".to_string(),
        draft.payload.clone(),
    );
    metadata.insert(
        "wechat_payload_draft_source".to_string(),
        Value::String(draft.source.to_string()),
    );
    metadata.insert(
        "wechat_payload_draft_at".to_string(),
        Value::String(now_shanghai()),
    );
    metadata.insert(
        "wechat_payload_draft_warnings".to_string(),
        Value::Array(
            draft
                .warnings
                .iter()
                .map(|warning| Value::String(warning.clone()))
                .collect(),
        ),
    );

    let updated_raw = serde_json::to_string(&raw_value)
        .map_err(|error| AppError::Validation(format!("微信发品草稿无法序列化：{error}")))?;
    conn.execute(
        "UPDATE publish_products SET raw_payload = ?1 WHERE id = ?2",
        params![updated_raw, item.product_row_id.as_str()],
    )?;
    Ok(())
}

pub(in crate::commands) fn mark_publish_item_failed(
    conn: &Connection,
    item: &PendingPublishItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE publish_job_items
         SET status = 'failed', error_code = ?1, error_summary = ?2
         WHERE id = ?3",
        params![error_code, error_summary, item.item_id.as_str()],
    )?;
    insert_task_log(
        conn,
        &item.job_id,
        Some(&item.item_id),
        "error",
        error_summary,
        Some(&serde_json::json!({
            "error_code": error_code,
            "product_row_id": item.product_row_id,
            "shop_id": item.shop_id,
            "external_product_id": item.external_product_id
        })),
    )?;
    upsert_notification(
        conn,
        publish_failure_notification_severity(error_code),
        "publish_item",
        &item.item_id,
        Some(&item.shop_id),
        "铺货任务失败",
        &format!(
            "店铺 {} 铺货外部商品 {} 失败：{}（{}）",
            item.shop_id, item.external_product_id, error_summary, error_code
        ),
        Some(&serde_json::json!({
            "job_id": &item.job_id,
            "item_id": &item.item_id,
            "product_row_id": &item.product_row_id,
            "external_product_id": &item.external_product_id,
            "shop_id": &item.shop_id,
            "error_code": error_code
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn conn_update_publish_item_error(
    conn: &Connection,
    item: &PendingPublishItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE publish_job_items
         SET status = 'failed', error_code = ?1, error_summary = ?2
         WHERE id = ?3",
        params![error_code, error_summary, item.item_id.as_str()],
    )?;
    Ok(())
}

pub(in crate::commands) fn set_publish_item_status_in_conn(
    conn: &Connection,
    item: &PendingPublishItem,
    status: &str,
    error_code: Option<&str>,
    error_summary: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE publish_job_items
         SET status = ?1, error_code = ?2, error_summary = ?3
         WHERE id = ?4",
        params![status, error_code, error_summary, item.item_id.as_str()],
    )?;
    Ok(())
}

pub(in crate::commands) fn publish_failure_notification_severity(error_code: &str) -> &'static str {
    match error_code {
        "CATEGORY_NEEDS_AI_FILL"
        | "WECHAT_PAYLOAD_NEEDS_AI_FILL"
        | "CATEGORY_ATTRS_NEED_AI_FILL"
        | "SHOP_NOT_ACTIVE"
        | "SHOP_SECRET_MISSING"
        | "INSUFFICIENT_HEAD_IMAGES"
        | "INSUFFICIENT_DETAIL_IMAGES"
        | "SUPPLIER_STOCK_EMPTY" => "warning",
        _ => "critical",
    }
}

pub(in crate::commands) fn mark_publish_item_failed_for_app(
    app: &AppHandle,
    item: &PendingPublishItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    mark_publish_item_failed(&conn, item, error_code, error_summary)
}

pub(in crate::commands) fn set_publish_item_status(
    app: &AppHandle,
    item: &PendingPublishItem,
    status: &str,
    error_code: Option<&str>,
    error_summary: Option<&str>,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE publish_job_items
         SET status = ?1, error_code = ?2, error_summary = ?3
         WHERE id = ?4",
        params![status, error_code, error_summary, item.item_id.as_str()],
    )?;
    Ok(())
}

pub(in crate::commands) fn set_publish_status_sync_state(
    app: &AppHandle,
    item: &StatusSyncItem,
    status: &str,
    error_code: Option<&str>,
    summary: &str,
    wechat_status: Option<i64>,
    wechat_edit_status: Option<i64>,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "UPDATE publish_job_items
         SET status = ?1,
             error_code = ?2,
             error_summary = ?3,
             wechat_status = COALESCE(?4, wechat_status),
             wechat_edit_status = COALESCE(?5, wechat_edit_status),
             last_status_sync_at = ?6,
             audit_summary = ?3
         WHERE id = ?7",
        params![
            status,
            error_code,
            summary,
            wechat_status,
            wechat_edit_status,
            now,
            item.item_id.as_str()
        ],
    )?;
    conn.execute(
        "UPDATE shop_products
         SET status = ?1,
             wechat_status = COALESCE(?2, wechat_status),
             wechat_edit_status = COALESCE(?3, wechat_edit_status),
             last_status_sync_at = ?4,
             audit_summary = ?5
         WHERE shop_id = ?6 AND external_product_id = ?7",
        params![
            status,
            wechat_status,
            wechat_edit_status,
            now,
            summary,
            item.shop_id.as_str(),
            item.external_product_id.as_str()
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn set_shop_product_item_state(
    app: &AppHandle,
    item: &StatusSyncItem,
    status: &str,
    error_code: Option<&str>,
    summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE publish_job_items
         SET status = ?1, error_code = ?2, error_summary = ?3
         WHERE id = ?4",
        params![status, error_code, summary, item.item_id.as_str()],
    )?;
    conn.execute(
        "UPDATE shop_products
         SET status = ?1, audit_summary = ?2
         WHERE shop_id = ?3 AND external_product_id = ?4",
        params![
            status,
            summary,
            item.shop_id.as_str(),
            item.external_product_id.as_str()
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn mark_listing_item_failed(
    app: &AppHandle,
    item: &StatusSyncItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    set_shop_product_item_state(app, item, "failed", Some(error_code), error_summary)?;
    insert_task_log_for_app(
        app,
        &item.job_id,
        Some(&item.item_id),
        "error",
        error_summary,
        Some(&serde_json::json!({
            "error_code": error_code,
            "shop_id": item.shop_id,
            "external_product_id": item.external_product_id,
            "wechat_product_id": item.wechat_product_id
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn insert_task_log_for_app(
    app: &AppHandle,
    task_id: &str,
    item_id: Option<&str>,
    level: &str,
    message: &str,
    detail: Option<&serde_json::Value>,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    insert_task_log(&conn, task_id, item_id, level, message, detail)
}

pub(in crate::commands) fn mark_price_update_item_failed_for_app(
    app: &AppHandle,
    item: &PendingPriceUpdateItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE price_update_items
         SET status = 'failed', error_code = ?1, error_summary = ?2, updated_at = ?3
         WHERE id = ?4",
        params![error_code, error_summary, now_shanghai(), item.item_id],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "error",
        &format!(
            "商品 {} 改价失败：{error_summary}",
            item.external_product_id
        ),
        Some(&serde_json::json!({
            "error_code": error_code,
            "wechat_product_id": item.wechat_product_id,
            "target_price_cents": item.target_price_cents
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn set_price_update_item_pending_for_app(
    app: &AppHandle,
    item: &PendingPriceUpdateItem,
    error_code: Option<&str>,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE price_update_items
         SET status = 'audit_pending',
             error_code = ?1,
             error_summary = ?2,
             updated_at = ?3
         WHERE id = ?4",
        params![error_code, error_summary, now_shanghai(), item.item_id],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "info",
        &format!(
            "商品 {} 改价结果待确认：{error_summary}",
            item.external_product_id
        ),
        Some(&serde_json::json!({
            "error_code": error_code,
            "wechat_product_id": item.wechat_product_id,
            "target_price_cents": item.target_price_cents
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn set_price_update_item_confirmed_for_app(
    app: &AppHandle,
    item: &PendingPriceUpdateItem,
    summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "UPDATE price_update_items
         SET status = 'success',
             error_code = NULL,
             error_summary = ?1,
             updated_at = ?2
         WHERE id = ?3",
        params![summary, now, item.item_id],
    )?;
    conn.execute(
        "UPDATE shop_products
         SET current_price_cents = ?1,
             last_price_update_at = ?2
         WHERE shop_id = ?3 AND external_product_id = ?4",
        params![
            item.target_price_cents,
            now,
            item.shop_id,
            item.external_product_id
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn build_price_update_product_payload(
    info: &ProductGetInfo,
    product_id: &str,
    target_price_cents: i64,
) -> Result<Value, String> {
    let source = info
        .product
        .as_ref()
        .or(info.edit_product.as_ref())
        .ok_or_else(|| {
            "微信 getproduct 未返回 product 或 edit_product，无法构造 updateproduct 请求"
                .to_string()
        })?;
    let mut payload = serde_json::to_value(source)
        .map_err(|error| format!("微信商品快照无法序列化：{error}"))?
        .as_object()
        .cloned()
        .ok_or_else(|| "微信商品快照不是对象，无法构造 updateproduct 请求".to_string())?;

    sanitize_product_update_payload(&mut payload);
    payload.insert(
        "product_id".to_string(),
        Value::String(product_id.to_string()),
    );

    let skus = payload
        .get_mut("skus")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "微信商品缺少 skus，不能改价".to_string())?;
    if skus.is_empty() {
        return Err("微信商品 SKU 为空，不能改价".to_string());
    }
    for sku in skus {
        let sku = sku
            .as_object_mut()
            .ok_or_else(|| "微信商品 skus 中存在非对象项，不能改价".to_string())?;
        sku.insert(
            "sale_price".to_string(),
            Value::Number(serde_json::Number::from(target_price_cents)),
        );
    }

    require_price_update_field(&payload, "title")?;
    require_price_update_field(&payload, "head_imgs")?;
    require_price_update_field(&payload, "desc_info")?;
    require_price_update_field(&payload, "deliver_method")?;
    require_price_update_field(&payload, "extra_service")?;
    if !payload.contains_key("cats") && !payload.contains_key("cats_v2") {
        return Err("微信商品缺少 cats 或 cats_v2，不能构造 updateproduct 请求".to_string());
    }

    Ok(Value::Object(payload))
}

pub(in crate::commands) fn sanitize_product_update_payload(
    payload: &mut serde_json::Map<String, Value>,
) {
    for field in [
        "status",
        "edit_status",
        "min_price",
        "edit_time",
        "total_sold_num",
        "src_product_id",
        "sale_limit_info",
        "info_score",
        "cmp_price_info",
        "audit_info",
        "create_time",
        "update_time",
    ] {
        payload.remove(field);
    }
    if let Some(Value::Object(timing_onsale_info)) = payload.get_mut("timing_onsale_info") {
        timing_onsale_info.remove("task_id");
    }
}

pub(in crate::commands) fn require_price_update_field(
    payload: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match payload.get(field) {
        Some(Value::Null) | None => {
            Err(format!("微信商品缺少 {field}，不能构造 updateproduct 请求"))
        }
        _ => Ok(()),
    }
}

pub(in crate::commands) fn collect_product_assets(
    product: &ExternalProductInput,
) -> Vec<ProductAsset> {
    let mut assets = Vec::new();
    for (index, source_url) in product.images.iter().enumerate() {
        let source_url = source_url.trim();
        if !source_url.is_empty() {
            assets.push(ProductAsset {
                kind: "head_image",
                sort_order: index as i64,
                source_url: source_url.to_string(),
            });
        }
    }
    for (index, source_url) in product.detail_images.iter().enumerate() {
        let source_url = source_url.trim();
        if !source_url.is_empty() {
            assets.push(ProductAsset {
                kind: "detail_image",
                sort_order: index as i64,
                source_url: source_url.to_string(),
            });
        }
    }
    assets
}

pub(in crate::commands) fn load_prepared_assets(
    app: &AppHandle,
    item_id: &str,
) -> AppResult<Vec<PreparedAsset>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT asset_kind, sort_order, wechat_url
         FROM publish_assets
         WHERE item_id = ?1
           AND status IN ('success', 'reused')
           AND wechat_url IS NOT NULL
         ORDER BY asset_kind ASC, sort_order ASC",
    )?;
    let assets = stmt
        .query_map([item_id], |row| {
            Ok(PreparedAsset {
                kind: row.get(0)?,
                sort_order: row.get(1)?,
                wechat_url: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(assets)
}

pub(in crate::commands) fn resolve_add_product_base_payload(
    product: &ExternalProductInput,
) -> Result<AddProductPayloadDraft, String> {
    let metadata = product_metadata_object(product)?;
    if let Some(metadata) = metadata {
        for (key, source) in [
            (
                "wechat_add_product_payload",
                "metadata.wechat_add_product_payload",
            ),
            ("wechat_product_payload", "metadata.wechat_product_payload"),
        ] {
            if let Some(value) = metadata.get(key) {
                let mut payload = value
                    .as_object()
                    .cloned()
                    .ok_or_else(|| format!("{source} 必须是微信 addproduct 请求对象"))?;
                apply_add_product_defaults(&mut payload, product);
                validate_add_product_base_payload(&payload)?;
                return Ok(AddProductPayloadDraft {
                    payload: Value::Object(payload),
                    source,
                    warnings: Vec::new(),
                });
            }
        }
    }

    generate_add_product_payload_draft(product, metadata)
}

pub(in crate::commands) fn generate_add_product_payload_draft(
    product: &ExternalProductInput,
    metadata: Option<&serde_json::Map<String, Value>>,
) -> Result<AddProductPayloadDraft, String> {
    let mut payload = serde_json::Map::new();
    let mut warnings = Vec::new();

    payload.insert(
        "out_product_id".to_string(),
        Value::String(product.external_product_id.clone()),
    );
    payload.insert("title".to_string(), Value::String(product.title.clone()));
    payload.insert(
        "spu_code".to_string(),
        Value::String(product.external_product_id.clone()),
    );
    payload.insert(
        "listing".to_string(),
        Value::Number(serde_json::Number::from(0)),
    );
    payload.insert(
        "release_mode".to_string(),
        Value::Number(serde_json::Number::from(0)),
    );

    let deliver_method = metadata_i64(
        metadata,
        &["wechat_deliver_method", "deliver_method", "delivery_method"],
    )
    .unwrap_or(0);
    payload.insert(
        "deliver_method".to_string(),
        Value::Number(serde_json::Number::from(deliver_method)),
    );
    if deliver_method == 3 {
        let deliver_acct_type =
            metadata_array_clone(metadata, &["wechat_deliver_acct_type", "deliver_acct_type"])
                .unwrap_or_else(|| vec![Value::Number(serde_json::Number::from(3))]);
        payload.insert(
            "deliver_acct_type".to_string(),
            Value::Array(deliver_acct_type),
        );
    }

    insert_add_product_categories(&mut payload, metadata, product)?;
    payload.insert("extra_service".to_string(), resolve_extra_service(metadata));
    payload.insert(
        "skus".to_string(),
        Value::Array(build_add_product_skus(product, metadata, &mut warnings)?),
    );

    if let Some(attrs) = metadata_array_clone(metadata, &["wechat_attrs", "attrs", "product_attrs"])
    {
        if attrs.is_empty() {
            warnings
                .push("metadata.wechat_attrs 为空；如类目有必填商品参数，微信会拒绝".to_string());
        } else {
            payload.insert("attrs".to_string(), Value::Array(attrs));
        }
    } else {
        warnings.push(
            "未提供 metadata.wechat_attrs；如类目有必填商品参数，需要用 categorydetail + AI/规则补齐"
                .to_string(),
        );
    }

    if let Some(brand_id) = metadata_string(metadata, &["wechat_brand_id", "brand_id"]) {
        payload.insert("brand_id".to_string(), Value::String(brand_id));
    } else if product
        .brand_hint
        .as_deref()
        .map(|brand| brand.contains("无品牌") || brand.eq_ignore_ascii_case("none"))
        .unwrap_or(false)
    {
        payload.insert(
            "brand_id".to_string(),
            Value::String("2100000000".to_string()),
        );
    } else if product
        .brand_hint
        .as_deref()
        .map(str::trim)
        .is_some_and(|brand| !brand.is_empty())
    {
        warnings.push(
            "存在 brand_hint 但没有 metadata.wechat_brand_id；品牌类目可能需要先通过品牌接口映射"
                .to_string(),
        );
    }

    if let Some(desc) = metadata_string(metadata, &["wechat_desc", "desc", "description"]) {
        let mut desc_info = serde_json::Map::new();
        desc_info.insert("desc".to_string(), Value::String(desc));
        payload.insert("desc_info".to_string(), Value::Object(desc_info));
    }
    if let Some(express_info) = resolve_express_info(metadata, product, deliver_method) {
        payload.insert("express_info".to_string(), express_info);
    }
    if let Some(after_sale_address_id) = metadata_i64(
        metadata,
        &["after_sale_address_id", "wechat_after_sale_address_id"],
    ) {
        payload.insert(
            "after_sale_info".to_string(),
            serde_json::json!({ "after_sale_address_id": after_sale_address_id }),
        );
    }
    if let Some(supply_source) =
        metadata_object_clone(metadata, &["supply_source", "wechat_supply_source"])
    {
        payload.insert("supply_source".to_string(), Value::Object(supply_source));
    }

    validate_add_product_base_payload(&payload)?;
    Ok(AddProductPayloadDraft {
        payload: Value::Object(payload),
        source: "local_rules_v1",
        warnings,
    })
}

pub(in crate::commands) fn product_metadata_object(
    product: &ExternalProductInput,
) -> Result<Option<&serde_json::Map<String, Value>>, String> {
    match &product.metadata {
        Value::Object(metadata) => Ok(Some(metadata)),
        Value::Null => Ok(None),
        _ => Err("metadata 必须是对象；请传 {} 或包含微信发品参数的对象".to_string()),
    }
}

pub(in crate::commands) fn apply_add_product_defaults(
    payload: &mut serde_json::Map<String, Value>,
    product: &ExternalProductInput,
) {
    payload
        .entry("out_product_id".to_string())
        .or_insert_with(|| Value::String(product.external_product_id.clone()));
    payload
        .entry("title".to_string())
        .or_insert_with(|| Value::String(product.title.clone()));
    payload
        .entry("spu_code".to_string())
        .or_insert_with(|| Value::String(product.external_product_id.clone()));
}

pub(in crate::commands) fn insert_add_product_categories(
    payload: &mut serde_json::Map<String, Value>,
    metadata: Option<&serde_json::Map<String, Value>>,
    product: &ExternalProductInput,
) -> Result<(), String> {
    if let Some(value) = metadata_value(metadata, &["wechat_cats_v2", "cats_v2"]) {
        payload.insert(
            "cats_v2".to_string(),
            normalize_category_objects(value, "cats_v2")?,
        );
        return Ok(());
    }
    if let Some(value) = metadata_value(metadata, &["wechat_cats", "cats"]) {
        payload.insert(
            "cats".to_string(),
            normalize_category_objects(value, "cats")?,
        );
        return Ok(());
    }
    if let Some(value) = metadata_value(
        metadata,
        &["wechat_category_ids", "category_ids", "cat_ids"],
    ) {
        payload.insert(
            "cats_v2".to_string(),
            normalize_category_objects(value, "wechat_category_ids")?,
        );
        return Ok(());
    }

    let category_hint = product
        .category_hint
        .as_deref()
        .map(str::trim)
        .filter(|hint| !hint.is_empty())
        .unwrap_or("-");
    Err(format!(
        "缺少微信类目 ID。本地类目库未能高置信匹配 category_hint={category_hint}；请先同步类目规则或在商品 metadata.wechat_category_ids 填入 [一级cat_id, 二级cat_id, 三级或叶子cat_id]，也可以直接传 metadata.wechat_add_product_payload"
    ))
}

pub(in crate::commands) fn normalize_category_objects(
    value: &Value,
    field: &str,
) -> Result<Value, String> {
    let ids = category_ids_from_value(value);
    if ids.len() < 3 {
        return Err(format!("{field} 需至少包含 3 个有效 cat_id"));
    }
    Ok(Value::Array(
        ids.into_iter()
            .map(|cat_id| serde_json::json!({ "cat_id": cat_id }))
            .collect(),
    ))
}

pub(in crate::commands) fn category_ids_from_value(value: &Value) -> Vec<i64> {
    match value {
        Value::Array(items) => items.iter().filter_map(category_id_from_value).collect(),
        _ => Vec::new(),
    }
}

pub(in crate::commands) fn extract_leaf_category_id_from_payload(payload: &Value) -> Option<i64> {
    payload
        .get("cats_v2")
        .or_else(|| payload.get("cats"))
        .map(category_ids_from_value)
        .and_then(|ids| ids.into_iter().last())
}

pub(in crate::commands) fn category_id_from_value(value: &Value) -> Option<i64> {
    match value {
        Value::Object(object) => json_value_to_i64(object.get("cat_id")),
        _ => json_value_to_i64(Some(value)),
    }
    .filter(|cat_id| *cat_id > 0)
}

pub(in crate::commands) fn resolve_extra_service(
    metadata: Option<&serde_json::Map<String, Value>>,
) -> Value {
    let mut extra_service =
        metadata_object_clone(metadata, &["wechat_extra_service", "extra_service"])
            .unwrap_or_default();
    extra_service
        .entry("seven_day_return".to_string())
        .or_insert_with(|| Value::Number(serde_json::Number::from(1)));
    extra_service
        .entry("freight_insurance".to_string())
        .or_insert_with(|| Value::Number(serde_json::Number::from(0)));
    Value::Object(extra_service)
}

pub(in crate::commands) fn resolve_express_info(
    metadata: Option<&serde_json::Map<String, Value>>,
    product: &ExternalProductInput,
    deliver_method: i64,
) -> Option<Value> {
    if let Some(express_info) =
        metadata_object_clone(metadata, &["wechat_express_info", "express_info"])
    {
        return Some(Value::Object(express_info));
    }
    if deliver_method == 1 || deliver_method == 3 {
        return None;
    }

    let mut express_info = serde_json::Map::new();
    if let Some(template_id) = metadata_string(
        metadata,
        &["freight_template_id", "wechat_freight_template_id"],
    ) {
        express_info.insert("template_id".to_string(), Value::String(template_id));
    }
    if let Some(weight) = product
        .weight_gram
        .filter(|weight| *weight > 0)
        .or_else(|| metadata_i64(metadata, &["weight_gram", "weight"]))
    {
        express_info.insert(
            "weight".to_string(),
            Value::Number(serde_json::Number::from(weight)),
        );
    }
    if express_info.is_empty() {
        None
    } else {
        Some(Value::Object(express_info))
    }
}

pub(in crate::commands) fn build_add_product_skus(
    product: &ExternalProductInput,
    metadata: Option<&serde_json::Map<String, Value>>,
    warnings: &mut Vec<String>,
) -> Result<Vec<Value>, String> {
    let active_sku_count = product.skus.iter().filter(|sku| sku.stock > 0).count();
    let mut skipped_sku_count = 0usize;
    let mut skus = Vec::with_capacity(active_sku_count);
    for sku in &product.skus {
        if sku.stock <= 0 {
            skipped_sku_count += 1;
            continue;
        }

        let attrs = sku_attrs_from_specs(&sku.specs, &sku.external_sku_id)?;
        if active_sku_count > 1 && attrs.is_empty() {
            return Err(format!(
                "多 SKU 商品的 SKU {} 缺少规格；请补齐 skus[].specs，或直接传 metadata.wechat_add_product_payload.skus[].sku_attrs",
                sku.external_sku_id
            ));
        }

        let mut sku_payload = serde_json::Map::new();
        sku_payload.insert(
            "out_sku_id".to_string(),
            Value::String(sku.external_sku_id.clone()),
        );
        sku_payload.insert(
            "sku_code".to_string(),
            Value::String(sku.external_sku_id.clone()),
        );
        sku_payload.insert(
            "sale_price".to_string(),
            Value::Number(serde_json::Number::from(resolve_sku_sale_price_cents(
                sku, metadata,
            )?)),
        );
        sku_payload.insert(
            "stock_num".to_string(),
            Value::Number(serde_json::Number::from(sku.stock)),
        );
        if !attrs.is_empty() {
            sku_payload.insert("sku_attrs".to_string(), Value::Array(attrs));
        }
        skus.push(Value::Object(sku_payload));
    }

    if skipped_sku_count > 0 {
        warnings.push(format!("已跳过 {skipped_sku_count} 个库存为 0 的 SKU"));
    }
    if skus.is_empty() {
        return Err("没有可发布的有库存 SKU".to_string());
    }
    Ok(skus)
}

pub(in crate::commands) fn sku_attrs_from_specs(
    specs: &Value,
    external_sku_id: &str,
) -> Result<Vec<Value>, String> {
    match specs {
        Value::Object(specs) => {
            let mut attrs = Vec::new();
            for (key, value) in specs {
                let attr_key = key.trim();
                if attr_key.is_empty() {
                    continue;
                }
                let Some(attr_value) = sku_attr_value_to_string(value) else {
                    return Err(format!(
                        "SKU {external_sku_id} 的规格 {attr_key} 不是可转成文本的值"
                    ));
                };
                let attr_value = attr_value.trim();
                if attr_value.is_empty() {
                    continue;
                }
                validate_sku_attr_text(attr_key, attr_value, external_sku_id)?;
                attrs.push(serde_json::json!({
                    "attr_key": attr_key,
                    "attr_value": attr_value
                }));
            }
            Ok(attrs)
        }
        Value::Array(items) => {
            let mut attrs = Vec::with_capacity(items.len());
            for item in items {
                let object = item
                    .as_object()
                    .ok_or_else(|| format!("SKU {external_sku_id} 的 specs 数组项必须是对象"))?;
                let attr_key = json_value_to_string(object.get("attr_key"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let attr_value = json_value_to_string(object.get("attr_value"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if attr_key.is_empty() || attr_value.is_empty() {
                    continue;
                }
                validate_sku_attr_text(&attr_key, &attr_value, external_sku_id)?;
                attrs.push(serde_json::json!({
                    "attr_key": attr_key,
                    "attr_value": attr_value
                }));
            }
            Ok(attrs)
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(format!(
            "SKU {external_sku_id} 的 specs 必须是对象或 sku_attrs 数组"
        )),
    }
}

pub(in crate::commands) fn sku_attr_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(sku_attr_value_to_string)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(";"),
        ),
        _ => None,
    }
}

pub(in crate::commands) fn validate_sku_attr_text(
    attr_key: &str,
    attr_value: &str,
    external_sku_id: &str,
) -> Result<(), String> {
    if attr_key.chars().count() > 40 {
        return Err(format!(
            "SKU {external_sku_id} 的规格名 {attr_key} 超过 40 字符"
        ));
    }
    if attr_value.chars().count() > 40 {
        return Err(format!(
            "SKU {external_sku_id} 的规格值 {attr_value} 超过 40 字符"
        ));
    }
    Ok(())
}

pub(in crate::commands) fn resolve_sku_sale_price_cents(
    sku: &crate::models::ExternalSkuInput,
    metadata: Option<&serde_json::Map<String, Value>>,
) -> Result<i64, String> {
    for key in ["sku_sale_price_cents", "sku_prices_cents", "sku_prices"] {
        if let Some(Value::Object(price_map)) = metadata.and_then(|metadata| metadata.get(key)) {
            if let Some(price) = json_value_to_i64(price_map.get(&sku.external_sku_id)) {
                return validate_sale_price_cents(price, &sku.external_sku_id);
            }
        }
    }
    if let Some(price) = metadata_i64(
        metadata,
        &["sale_price_cents", "wechat_sale_price_cents", "price_cents"],
    ) {
        return validate_sale_price_cents(price, &sku.external_sku_id);
    }

    let markup_rate = metadata_f64(
        metadata,
        &["sale_price_markup_rate", "price_markup_rate", "markup_rate"],
    )
    .unwrap_or(1.6);
    if !markup_rate.is_finite() || markup_rate <= 0.0 {
        return Err("metadata.sale_price_markup_rate 必须大于 0".to_string());
    }
    let fixed_cents =
        metadata_i64(metadata, &["sale_price_fixed_cents", "price_fixed_cents"]).unwrap_or(0);
    let floor_cents = metadata_i64(
        metadata,
        &["sale_price_floor_cents", "min_sale_price_cents"],
    )
    .unwrap_or(100);
    let computed = (sku.cost_price * 100.0 * markup_rate).ceil() as i64 + fixed_cents;
    validate_sale_price_cents(computed.max(floor_cents), &sku.external_sku_id)
}

pub(in crate::commands) fn validate_sale_price_cents(
    price: i64,
    external_sku_id: &str,
) -> Result<i64, String> {
    if price <= 0 {
        return Err(format!("SKU {external_sku_id} 的 sale_price 必须大于 0"));
    }
    if price > 1_000_000_000 {
        return Err(format!("SKU {external_sku_id} 的 sale_price 超过微信上限"));
    }
    Ok(price)
}

pub(in crate::commands) fn validate_add_product_base_payload(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    require_add_product_field(payload, "deliver_method")?;
    require_add_product_field(payload, "extra_service")?;
    require_add_product_field(payload, "skus")?;
    if !payload.contains_key("cats") && !payload.contains_key("cats_v2") {
        return Err("缺少 cats 或 cats_v2，需先补齐微信类目".to_string());
    }
    validate_array_field(payload, "skus", 1, "微信发品 SKU 不能为空")?;
    validate_extra_service(payload)?;
    validate_add_product_skus(payload)?;
    Ok(())
}

pub(in crate::commands) fn validate_add_product_skus(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let skus = payload
        .get("skus")
        .and_then(Value::as_array)
        .ok_or_else(|| "微信发品 SKU 必须是数组".to_string())?;
    for (index, sku) in skus.iter().enumerate() {
        let sku = sku
            .as_object()
            .ok_or_else(|| format!("第 {} 个 SKU 不是对象", index + 1))?;
        let stock_num = json_value_to_i64(sku.get("stock_num"))
            .ok_or_else(|| format!("第 {} 个 SKU 缺少 stock_num", index + 1))?;
        if stock_num < 0 {
            return Err(format!("第 {} 个 SKU 库存不能为负数", index + 1));
        }
        if let Some(sale_price) = json_value_to_i64(sku.get("sale_price")) {
            validate_sale_price_cents(sale_price, &format!("第 {} 个 SKU", index + 1))?;
        }
    }
    Ok(())
}

pub(in crate::commands) fn metadata_value<'a>(
    metadata: Option<&'a serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<&'a Value> {
    let metadata = metadata?;
    keys.iter().find_map(|key| metadata.get(*key))
}

pub(in crate::commands) fn metadata_object_clone(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<serde_json::Map<String, Value>> {
    metadata_value(metadata, keys)
        .and_then(Value::as_object)
        .cloned()
}

pub(in crate::commands) fn metadata_array_clone(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<Vec<Value>> {
    metadata_value(metadata, keys)
        .and_then(Value::as_array)
        .cloned()
}

pub(in crate::commands) fn metadata_string(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    metadata_value(metadata, keys)
        .and_then(|value| json_value_to_string(Some(value)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(in crate::commands) fn metadata_i64(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<i64> {
    metadata_value(metadata, keys).and_then(|value| json_value_to_i64(Some(value)))
}

pub(in crate::commands) fn metadata_f64(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<f64> {
    metadata_value(metadata, keys).and_then(|value| json_value_to_f64(Some(value)))
}

pub(in crate::commands) fn build_add_product_payload(
    product: &ExternalProductInput,
    assets: &[PreparedAsset],
) -> Result<Value, String> {
    let base_payload = resolve_add_product_base_payload(product)?;
    let mut payload = match base_payload.payload {
        Value::Object(map) => map,
        _ => return Err("微信 addproduct 请求必须是对象".to_string()),
    };

    let head_imgs = prepared_asset_urls(assets, "head_image");
    let detail_imgs = prepared_asset_urls(assets, "detail_image");
    if head_imgs.len() < 3 {
        return Err("微信发品头图不足 3 张，请先完成素材上传".to_string());
    }
    if detail_imgs.is_empty() {
        return Err("微信发品详情图为空，请先完成素材上传".to_string());
    }
    payload.insert(
        "head_imgs".to_string(),
        Value::Array(head_imgs.into_iter().map(Value::String).collect()),
    );
    upsert_desc_images(&mut payload, detail_imgs);

    validate_add_product_base_payload(&payload)?;

    Ok(Value::Object(payload))
}

pub(in crate::commands) fn prepared_asset_urls(
    assets: &[PreparedAsset],
    kind: &str,
) -> Vec<String> {
    let mut values = assets
        .iter()
        .filter(|asset| asset.kind == kind)
        .map(|asset| (asset.sort_order, asset.wechat_url.clone()))
        .collect::<Vec<_>>();
    values.sort_by_key(|(sort_order, _)| *sort_order);
    values.into_iter().map(|(_, url)| url).collect()
}

pub(in crate::commands) fn upsert_desc_images(
    payload: &mut serde_json::Map<String, Value>,
    detail_imgs: Vec<String>,
) {
    let mut desc_info = payload
        .remove("desc_info")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    desc_info.insert(
        "imgs".to_string(),
        Value::Array(detail_imgs.into_iter().map(Value::String).collect()),
    );
    payload.insert("desc_info".to_string(), Value::Object(desc_info));
}

pub(in crate::commands) fn require_add_product_field(
    payload: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match payload.get(field) {
        Some(Value::Null) | None => Err(format!("缺少 {field}，需先补齐微信发品参数")),
        _ => Ok(()),
    }
}

pub(in crate::commands) fn validate_array_field(
    payload: &serde_json::Map<String, Value>,
    field: &str,
    min_len: usize,
    message: &str,
) -> Result<(), String> {
    match payload.get(field) {
        Some(Value::Array(items)) if items.len() >= min_len => Ok(()),
        _ => Err(message.to_string()),
    }
}

pub(in crate::commands) fn validate_extra_service(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let extra_service = payload
        .get("extra_service")
        .and_then(Value::as_object)
        .ok_or_else(|| "extra_service 必须是对象".to_string())?;
    if !extra_service.contains_key("seven_day_return") {
        return Err("extra_service 缺少 seven_day_return".to_string());
    }
    if !extra_service.contains_key("freight_insurance") {
        return Err("extra_service 缺少 freight_insurance".to_string());
    }
    Ok(())
}
