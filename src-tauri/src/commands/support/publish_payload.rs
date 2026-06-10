use super::*;

const ADD_PRODUCT_MIN_HEAD_IMAGES: usize = 3;
const ADD_PRODUCT_MAX_HEAD_IMAGES: usize = 9;
const ADD_PRODUCT_MIN_DETAIL_IMAGES: usize = 1;
// 微信详情图：addproduct 接口允许最多 50 张，但草稿箱「发布」上架时实测只放行 20 张
// （接口层与发布层限制不一致）。按可发布上限取 20，提交时即截断，避免后续发布被卡。
const ADD_PRODUCT_MAX_DETAIL_IMAGES: usize = 20;
const ADD_PRODUCT_MAX_SKUS: usize = 500;

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
    // external_product_id 为空时跳过查重：空值不是有效去重键，
    // 否则同店多个无外部 id 的商品会互相误判重复（历史空串撞库 bug）。
    if !item.external_product_id.trim().is_empty() {
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
    }
    if product.images.len() < ADD_PRODUCT_MIN_HEAD_IMAGES {
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
    let draft = match resolve_add_product_base_payload_relaxed_after_sale(product) {
        Ok(draft) => draft,
        Err(error) => return Ok(Err((add_product_payload_error_code(&error), error))),
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

pub(in crate::commands) fn add_product_payload_error_code(error: &str) -> &'static str {
    if error.contains("after_sale_info.after_sale_address_id") {
        "MISSING_AFTER_SALE_ADDRESS"
    } else if error.contains("超过 40 字符") {
        // SKU 规格值超 40 字符通常是采集错位(多个候选值挤一格)，AI 补不了，专用码避免误导成「可 AI 补齐」。
        "SKU_SPEC_VALUE_MALFORMED"
    } else {
        "WECHAT_PAYLOAD_NEEDS_AI_FILL"
    }
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
        "UPDATE pipeline_shop_targets SET raw_payload = ?1 WHERE id = ?2",
        params![updated_raw, item.item_id.as_str()],
    )?;
    Ok(())
}

pub(in crate::commands) fn mark_publish_item_failed(
    conn: &Connection,
    item: &PendingPublishItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    block_target(conn, &item.item_id, error_code, error_summary)?;
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
    block_target(conn, &item.item_id, error_code, error_summary)
}

pub(in crate::commands) fn publish_failure_notification_severity(error_code: &str) -> &'static str {
    match error_code {
        "CATEGORY_NEEDS_AI_FILL"
        | "WECHAT_PAYLOAD_NEEDS_AI_FILL"
        | "SKU_SPEC_VALUE_MALFORMED"
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

/// 审核轮询：只回写 target 的微信审核元数据（不碰 stage/status 推进——
/// 推进由 audit runner 用 pipeline 原语 finish_target/advance_target/block_target/requeue_target 决定），
/// 同时把运营态写到 shop_products（运营展示用，独立于流水线推进态）。
pub(in crate::commands) fn set_publish_status_sync_state(
    app: &AppHandle,
    item: &StatusSyncItem,
    shop_status: &str,
    summary: &str,
    wechat_status: Option<i64>,
    wechat_edit_status: Option<i64>,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET wechat_status = COALESCE(?1, wechat_status),
             wechat_edit_status = COALESCE(?2, wechat_edit_status),
             last_status_sync_at = ?3,
             audit_summary = ?4
         WHERE id = ?5",
        params![
            wechat_status,
            wechat_edit_status,
            now,
            summary,
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
            shop_status,
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

/// 上架阶段：只回写 shop_products 运营态（target 推进由 listing runner 用 pipeline 原语决定）。
pub(in crate::commands) fn set_shop_product_item_state(
    app: &AppHandle,
    item: &StatusSyncItem,
    shop_status: &str,
    summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE shop_products
         SET status = ?1, audit_summary = ?2
         WHERE shop_id = ?3 AND external_product_id = ?4",
        params![
            shop_status,
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
    {
        let conn = open_connection(app)?;
        block_target(&conn, &item.item_id, error_code, error_summary)?;
    }
    set_shop_product_item_state(app, item, "failed", error_summary)?;
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
    for (index, source_url) in product
        .images
        .iter()
        .take(ADD_PRODUCT_MAX_HEAD_IMAGES)
        .enumerate()
    {
        let source_url = source_url.trim();
        if !source_url.is_empty() {
            assets.push(ProductAsset {
                kind: "head_image",
                sort_order: index as i64,
                source_url: source_url.to_string(),
            });
        }
    }
    for (index, source_url) in product
        .detail_images
        .iter()
        .take(ADD_PRODUCT_MAX_DETAIL_IMAGES)
        .enumerate()
    {
        let source_url = source_url.trim();
        if !source_url.is_empty() {
            assets.push(ProductAsset {
                kind: "detail_image",
                sort_order: index as i64,
                source_url: source_url.to_string(),
            });
        }
    }
    // SKU 颜色图：同色多尺码 SKU 共用一张，按 source_url 去重后只上传一次；submit 时按
    // source_url 映射回各 SKU 填 thumb_img，实现「切换 SKU 换主图」。
    let mut seen_sku_images = std::collections::HashSet::new();
    for sku in &product.skus {
        if let Some(sku_image) = sku.sku_image.as_deref() {
            let source_url = sku_image.trim();
            if !source_url.is_empty() && seen_sku_images.insert(source_url.to_string()) {
                assets.push(ProductAsset {
                    kind: "sku_image",
                    sort_order: (seen_sku_images.len() - 1) as i64,
                    source_url: source_url.to_string(),
                });
            }
        }
    }
    // 主图视频（可选增强）：每商品最多 1 个，铺货时下载淘宝视频→微信 4 步上传→填 head_videos（数组 [{video_url}]）。
    if let Some(main_video) = product.main_video.as_deref() {
        let source_url = main_video.trim();
        if !source_url.is_empty() {
            assets.push(ProductAsset {
                kind: "head_video",
                sort_order: 0,
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
        "SELECT asset_kind, sort_order, wechat_url, source_url
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
                source_url: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(assets)
}

pub(in crate::commands) fn resolve_add_product_base_payload_relaxed_after_sale(
    product: &ExternalProductInput,
) -> Result<AddProductPayloadDraft, String> {
    resolve_add_product_base_payload_with_options(product, false)
}

fn resolve_add_product_base_payload_with_options(
    product: &ExternalProductInput,
    require_after_sale: bool,
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
                let mut warnings = Vec::new();
                apply_add_product_defaults(&mut payload, product, &mut warnings);
                validate_add_product_base_payload_with_options(&payload, require_after_sale)?;
                return Ok(AddProductPayloadDraft {
                    payload: Value::Object(payload),
                    source,
                    warnings,
                });
            }
        }
    }

    generate_add_product_payload_draft(product, metadata, require_after_sale)
}

pub(in crate::commands) fn generate_add_product_payload_draft(
    product: &ExternalProductInput,
    metadata: Option<&serde_json::Map<String, Value>>,
    require_after_sale: bool,
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
    if deliver_method == 0 && !payload_has_freight_template_id(&payload) {
        warnings.push(
            "快递发货未提供 metadata.wechat_freight_template_id；建议按店铺配置明确运费模板，避免微信侧默认规则不可控"
                .to_string(),
        );
    }
    if let Some(after_sale_address_id) = metadata_i64(
        metadata,
        &["after_sale_address_id", "wechat_after_sale_address_id"],
    ) {
        payload.insert(
            "after_sale_info".to_string(),
            serde_json::json!({ "after_sale_address_id": after_sale_address_id }),
        );
    } else if deliver_method == 0 {
        warnings.push(
            "未提供 metadata.wechat_after_sale_address_id；后续将尝试读取微信默认售后/退货地址，无法唯一确认时需人工配置"
                .to_string(),
        );
    }
    if let Some(supply_source) =
        metadata_object_clone(metadata, &["supply_source", "wechat_supply_source"])
    {
        payload.insert("supply_source".to_string(), Value::Object(supply_source));
    }
    if metadata_value(metadata, &["stock_source", "inventory_source"]).is_none()
        && product.skus.iter().any(|sku| sku.stock >= 100)
    {
        warnings.push(
            "SKU 库存缺少 metadata.stock_source，且存在默认化高库存；发布前建议确认真实供应商库存"
                .to_string(),
        );
    }

    // SKU 约束收敛单点：动态生成（路径A）同样经 apply_add_product_defaults →
    // normalize_skus_for_wechat 完成截断；out_product_id/title/spu_code 上方已填，
    // entry().or_insert 对已填字段为 no-op，不会覆盖。
    apply_add_product_defaults(&mut payload, product, &mut warnings);

    validate_add_product_base_payload_with_options(&payload, require_after_sale)?;
    Ok(AddProductPayloadDraft {
        payload: Value::Object(payload),
        source: "local_rules_v1",
        warnings,
    })
}

fn payload_has_freight_template_id(payload: &serde_json::Map<String, Value>) -> bool {
    payload
        .get("express_info")
        .and_then(Value::as_object)
        .and_then(|express_info| express_info.get("template_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
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

/// 微信 add_product 对 SKU 编码的字段上限(官方文档 + 真机错误码 6600096 实测)：
/// - `sku_code`：≤100 UTF8 字节，且小店后台不做唯一性约束、本地链路不消费——超长可安全截断；
/// - `out_sku_id`：≤128 字符，是「微信订单原样回传 → order_items → 采购寻源」的映射键，须尽量保持完整。
const WECHAT_SKU_CODE_MAX_BYTES: usize = 100;
const WECHAT_OUT_SKU_ID_MAX_CHARS: usize = 128;

/// 把字符串按 UTF8 字节安全截断到 ≤ max_bytes（不切断半个多字节字符）。
/// 超长时保留前缀（尺码/颜色等语义在前、营销文案在后，正好砍掉无用尾巴）再拼 8 位哈希后缀，
/// 既限长又保证「仅尾部不同的多 SKU」截断后仍互不相同。
fn truncate_sku_code_bytes(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_string();
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut hasher);
    let suffix = format!("{:08x}", (hasher.finish() & 0xffff_ffff) as u32);
    let suffix_budget = suffix.len() + 1; // '-' + 8 位 hex
    let head_budget = max_bytes.saturating_sub(suffix_budget);
    let mut cut = head_budget.min(raw.len());
    while cut > 0 && !raw.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}-{}", &raw[..cut], suffix)
}

/// out_sku_id 限 128 字符（非字节）。本批最长约 41 字符不会触发，仅作极端长度防御，
/// 触发时同样保留前缀 + 哈希后缀保唯一。
fn truncate_out_sku_id(raw: &str) -> String {
    if raw.chars().count() <= WECHAT_OUT_SKU_ID_MAX_CHARS {
        return raw.to_string();
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut hasher);
    let suffix = format!("{:08x}", (hasher.finish() & 0xffff_ffff) as u32);
    let head: String = raw
        .chars()
        .take(WECHAT_OUT_SKU_ID_MAX_CHARS - (suffix.len() + 1))
        .collect();
    format!("{}-{}", head, suffix)
}

/// 微信 addproduct SKU 统一规范化单点：sku_code ≤100 UTF8 字节、out_sku_id ≤128 字符、
/// 单商品 SKU 上限 500（超出截断保留前 500 并记 warning）。
/// 新建（动态生成）与固化 payload 两条路径都必须且只经此处理一次。
fn normalize_skus_for_wechat(skus: &mut Vec<Value>, warnings: &mut Vec<String>) {
    // 超长 sku_code（淘宝规格串带营销文案 → UTF8 >100 字节）会触发 6600096 整批 SKU 被拒，
    // 统一截断到 ≤100 字节；out_sku_id 是采购对账映射键（微信原样回传），限 128 字符内保持完整。
    for sku in skus.iter_mut() {
        if let Some(obj) = sku.as_object_mut() {
            if let Some(code) = obj.get("sku_code").and_then(Value::as_str) {
                if code.len() > WECHAT_SKU_CODE_MAX_BYTES {
                    let fixed = truncate_sku_code_bytes(code, WECHAT_SKU_CODE_MAX_BYTES);
                    obj.insert("sku_code".to_string(), Value::String(fixed));
                }
            }
            if let Some(out_id) = obj.get("out_sku_id").and_then(Value::as_str) {
                if out_id.chars().count() > WECHAT_OUT_SKU_ID_MAX_CHARS {
                    let fixed = truncate_out_sku_id(out_id);
                    obj.insert("out_sku_id".to_string(), Value::String(fixed));
                }
            }
        }
    }

    // 微信 addproduct 单商品 SKU 上限 500，超出整商品被拒。截断保留前 500 个
    // （有损：丢弃多余规格组合），记 warning 进 task log 留痕，避免整商品发布失败。
    if skus.len() > ADD_PRODUCT_MAX_SKUS {
        let dropped = skus.len() - ADD_PRODUCT_MAX_SKUS;
        warnings.push(format!(
            "SKU 数超过微信上限 {ADD_PRODUCT_MAX_SKUS}，已截断保留前 {ADD_PRODUCT_MAX_SKUS} 个，丢弃 {dropped} 个规格组合"
        ));
        skus.truncate(ADD_PRODUCT_MAX_SKUS);
    }
}

pub(in crate::commands) fn apply_add_product_defaults(
    payload: &mut serde_json::Map<String, Value>,
    product: &ExternalProductInput,
    warnings: &mut Vec<String>,
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

    // SKU 规范化单点：此处是「新建（动态生成）/ 固化」两条路径进 submit 前的统一必经点，
    // 固化的 metadata.wechat_add_product_payload 含历史超长 sku_code / 超量 SKU 时，存量重跑即纠正。
    if let Some(Value::Array(skus)) = payload.get_mut("skus") {
        normalize_skus_for_wechat(skus, warnings);
    }
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
        // out_sku_id / sku_code 此处插原值；长度截断与 500 上限统一由 normalize_skus_for_wechat
        // 单点处理（generate_add_product_payload_draft 末尾经 apply_add_product_defaults 必经）。
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
    validate_add_product_base_payload_with_options(payload, true)
}

pub(in crate::commands) fn validate_add_product_base_payload_without_after_sale(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    validate_add_product_base_payload_with_options(payload, false)
}

pub(in crate::commands) fn validate_add_product_base_payload_with_options(
    payload: &serde_json::Map<String, Value>,
    require_after_sale: bool,
) -> Result<(), String> {
    require_add_product_field(payload, "deliver_method")?;
    require_add_product_field(payload, "extra_service")?;
    require_add_product_field(payload, "skus")?;
    if !payload.contains_key("cats") && !payload.contains_key("cats_v2") {
        return Err("缺少 cats 或 cats_v2，需先补齐微信类目".to_string());
    }
    validate_array_field(payload, "skus", 1, "微信发品 SKU 不能为空")?;
    validate_array_field_max(
        payload,
        "skus",
        ADD_PRODUCT_MAX_SKUS,
        "微信发品 SKU 不能超过 500 个",
    )?;
    validate_optional_array_field_range(
        payload,
        "head_imgs",
        ADD_PRODUCT_MIN_HEAD_IMAGES,
        ADD_PRODUCT_MAX_HEAD_IMAGES,
        "微信发品头图需为 3 到 9 张",
    )?;
    validate_desc_images_range(payload)?;
    validate_extra_service(payload)?;
    if require_after_sale {
        validate_after_sale_info(payload)?;
    }
    validate_add_product_skus(payload)?;
    Ok(())
}

pub(in crate::commands) fn validate_after_sale_info(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let deliver_method = json_value_to_i64(payload.get("deliver_method")).unwrap_or(0);
    if deliver_method != 0 {
        return Ok(());
    }

    let address_id = payload
        .get("after_sale_info")
        .and_then(Value::as_object)
        .and_then(|after_sale_info| json_value_to_i64(after_sale_info.get("after_sale_address_id")))
        .unwrap_or(0);
    if address_id <= 0 {
        return Err(
            "快递发货必须提供 after_sale_info.after_sale_address_id；请先在商品 metadata.wechat_after_sale_address_id 或 metadata.after_sale_address_id 填入微信售后/退货地址 ID"
                .to_string(),
        );
    }
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

pub(in crate::commands) fn build_add_product_payload_relaxed_after_sale(
    product: &ExternalProductInput,
    assets: &[PreparedAsset],
) -> Result<Value, String> {
    let base_payload = resolve_add_product_base_payload_relaxed_after_sale(product)?;
    let mut payload = match base_payload.payload {
        Value::Object(map) => map,
        _ => return Err("微信 addproduct 请求必须是对象".to_string()),
    };

    // 按商品原始图片 URL 反查已上传素材的微信图，不依赖 publish_assets.asset_kind。
    // 同一张淘宝图既作详情图又作 SKU 颜色图时，record_asset_success 的
    // ON CONFLICT(item_id, source_url) 会把先写入的 detail 记录 asset_kind 覆盖成 sku_image，
    // 按 kind 过滤会漏掉详情图（误报「详情图为空」）；头图与详情图同图同理。改按 source_url 匹配根治。
    let mut head_imgs = resolve_asset_urls_by_source(&product.images, assets);
    let mut detail_imgs = resolve_asset_urls_by_source(&product.detail_images, assets);
    if head_imgs.len() < ADD_PRODUCT_MIN_HEAD_IMAGES {
        return Err("微信发品头图不足 3 张，请先完成素材上传".to_string());
    }
    if detail_imgs.is_empty() {
        return Err("微信发品详情图为空，请先完成素材上传".to_string());
    }
    head_imgs.truncate(ADD_PRODUCT_MAX_HEAD_IMAGES);
    detail_imgs.truncate(ADD_PRODUCT_MAX_DETAIL_IMAGES);
    payload.insert(
        "head_imgs".to_string(),
        Value::Array(head_imgs.into_iter().map(Value::String).collect()),
    );
    upsert_desc_images(&mut payload, detail_imgs);
    apply_sku_thumb_images(&mut payload, product, assets);
    apply_head_video(&mut payload, assets);

    validate_add_product_base_payload_without_after_sale(&payload)?;

    Ok(Value::Object(payload))
}

/// 给 add_product payload 的 skus 填颜色图 thumb_img，实现「切换 SKU 换主图」。
///
/// 微信要求 thumb_img 为 mmecimage.cn/p 前缀的微信 URL（错误码 10020035），故必须用上传后的
/// wechat_url，不能直接用淘宝 URL。映射链：payload.sku.out_sku_id == truncate_out_sku_id(external_sku_id)
/// → product.sku.sku_image(淘宝 source_url) → assets[kind=sku_image].wechat_url。
/// 找不到对应已上传微信图的 SKU 不填 thumb_img（宁缺勿错，避免 10020035）。
fn apply_sku_thumb_images(
    payload: &mut serde_json::Map<String, Value>,
    product: &ExternalProductInput,
    assets: &[PreparedAsset],
) {
    let source_to_wechat: std::collections::HashMap<&str, &str> = assets
        .iter()
        .filter(|asset| asset.kind == "sku_image")
        .map(|asset| (asset.source_url.as_str(), asset.wechat_url.as_str()))
        .collect();
    if source_to_wechat.is_empty() {
        return;
    }
    let mut out_sku_to_thumb: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for sku in &product.skus {
        if let Some(source_url) = sku.sku_image.as_deref() {
            if let Some(wechat_url) = source_to_wechat.get(source_url.trim()) {
                out_sku_to_thumb.insert(
                    truncate_out_sku_id(&sku.external_sku_id),
                    (*wechat_url).to_string(),
                );
            }
        }
    }
    if out_sku_to_thumb.is_empty() {
        return;
    }
    if let Some(Value::Array(skus)) = payload.get_mut("skus") {
        for sku in skus.iter_mut() {
            let Some(obj) = sku.as_object_mut() else {
                continue;
            };
            let out_sku_id = obj
                .get("out_sku_id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string());
            if let Some(out_sku_id) = out_sku_id {
                if let Some(thumb) = out_sku_to_thumb.get(&out_sku_id) {
                    obj.insert("thumb_img".to_string(), Value::String(thumb.clone()));
                }
            }
        }
    }
}

/// 给 add_product payload 填主图视频 head_videos（搬运淘宝主图视频到微信小店）。
///
/// 取已上传的 head_video 素材（微信临时播放 URL）；无则不填（视频为可选增强，缺失不影响上架）。
/// 格式：head_videos 须为数组，元素为 { video_url: string }，即 `[{ "video_url": "..." }]`。
/// ⚠️ 微信官方文档把 head_videos 标为 object，但 addproduct 服务端实测返回
/// "data format error, expecting an array for field: head_videos"（错误码 47001）——
/// 文档滞后/不准，以实测为准用数组。video_url 自身为 string（微信 2026-05-18 由 array 改 string）。
fn apply_head_video(payload: &mut serde_json::Map<String, Value>, assets: &[PreparedAsset]) {
    if let Some(video_url) = prepared_asset_urls(assets, "head_video").into_iter().next() {
        payload.insert(
            "head_videos".to_string(),
            serde_json::json!([{ "video_url": video_url }]),
        );
    }
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

/// 按商品原始图片 URL（淘宝侧）顺序反查已上传素材的微信图 URL，保持商品图片列表的原始顺序。
///
/// 不依赖 publish_assets.asset_kind：同一张图既作详情图又作 SKU 颜色图时，record_asset_success
/// 的 ON CONFLICT(item_id, source_url) 会用后写入的 kind 覆盖先写入记录的 asset_kind，按 kind
/// 过滤会漏图。改按 source_url 匹配可彻底规避，并天然复用同图记录（同 source_url 只上传一次微信图）。
/// publish_assets.source_url 是 normalize 后的，故对商品原始 URL 同样 normalize 再匹配；
/// 同一张微信图在列表内去重，避免重复图片进入 payload。
fn resolve_asset_urls_by_source(source_urls: &[String], assets: &[PreparedAsset]) -> Vec<String> {
    let source_to_wechat: std::collections::HashMap<&str, &str> = assets
        .iter()
        .map(|asset| (asset.source_url.as_str(), asset.wechat_url.as_str()))
        .collect();
    let mut resolved = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for source_url in source_urls {
        let Ok(normalized) = normalize_image_source_url(source_url) else {
            continue;
        };
        if let Some(wechat_url) = source_to_wechat.get(normalized.as_str()) {
            if seen.insert(*wechat_url) {
                resolved.push((*wechat_url).to_string());
            }
        }
    }
    resolved
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

pub(in crate::commands) fn validate_array_field_max(
    payload: &serde_json::Map<String, Value>,
    field: &str,
    max_len: usize,
    message: &str,
) -> Result<(), String> {
    match payload.get(field) {
        Some(Value::Array(items)) if items.len() > max_len => Err(message.to_string()),
        _ => Ok(()),
    }
}

pub(in crate::commands) fn validate_optional_array_field_range(
    payload: &serde_json::Map<String, Value>,
    field: &str,
    min_len: usize,
    max_len: usize,
    message: &str,
) -> Result<(), String> {
    match payload.get(field) {
        Some(Value::Array(items)) if items.len() >= min_len && items.len() <= max_len => Ok(()),
        Some(Value::Array(_)) => Err(message.to_string()),
        Some(_) => Err(format!("{field} 必须是数组")),
        None => Ok(()),
    }
}

pub(in crate::commands) fn validate_desc_images_range(
    payload: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let Some(desc_info) = payload.get("desc_info") else {
        return Ok(());
    };
    let Some(desc_info) = desc_info.as_object() else {
        return Err("desc_info 必须是对象".to_string());
    };
    match desc_info.get("imgs") {
        Some(Value::Array(items))
            if items.len() >= ADD_PRODUCT_MIN_DETAIL_IMAGES
                && items.len() <= ADD_PRODUCT_MAX_DETAIL_IMAGES =>
        {
            Ok(())
        }
        Some(Value::Array(_)) => Err("微信发品详情图需为 1 到 20 张".to_string()),
        Some(_) => Err("desc_info.imgs 必须是数组".to_string()),
        None => Ok(()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_payload(deliver_method: i64) -> serde_json::Map<String, Value> {
        serde_json::json!({
            "deliver_method": deliver_method,
            "extra_service": {
                "seven_day_return": 1,
                "freight_insurance": 0
            },
            "cats_v2": [
                { "cat_id": 1 },
                { "cat_id": 2 },
                { "cat_id": 3 }
            ],
            "skus": [
                {
                    "out_sku_id": "sku-1",
                    "sale_price": 100,
                    "stock_num": 1
                }
            ]
        })
        .as_object()
        .cloned()
        .expect("测试 payload 必须是对象")
    }

    #[test]
    fn truncate_sku_code_keeps_short_value() {
        let short = "尺码=L;颜色分类=绿色"; // 远小于 100 字节
        assert_eq!(truncate_sku_code_bytes(short, WECHAT_SKU_CODE_MAX_BYTES), short);
    }

    #[test]
    fn truncate_sku_code_limits_oversize_to_100_bytes_on_boundary() {
        // 真机超长样本：颜色含营销文案，UTF8 = 108 字节 > 100
        let long = "尺码=00:05/00:30;身高=120cm;颜色分类=绿色（升级款）【面料更舒适，90%用户选择】";
        assert!(long.len() > WECHAT_SKU_CODE_MAX_BYTES);
        let fixed = truncate_sku_code_bytes(long, WECHAT_SKU_CODE_MAX_BYTES);
        assert!(fixed.len() <= WECHAT_SKU_CODE_MAX_BYTES, "截断后={}字节", fixed.len());
        // 不得切断半个 UTF8 字符
        assert!(std::str::from_utf8(fixed.as_bytes()).is_ok());
        // 保留了可读前缀语义
        assert!(fixed.starts_with("尺码="));
    }

    #[test]
    fn truncate_sku_code_keeps_tail_different_skus_unique() {
        let a = "尺码=00:05;身高=120cm;颜色分类=绿色（升级款）【面料更舒适，90%用户选择，A 款专属编号】";
        let b = "尺码=00:05;身高=120cm;颜色分类=绿色（升级款）【面料更舒适，90%用户选择，B 款专属编号】";
        assert!(a.len() > WECHAT_SKU_CODE_MAX_BYTES && b.len() > WECHAT_SKU_CODE_MAX_BYTES);
        let fa = truncate_sku_code_bytes(a, WECHAT_SKU_CODE_MAX_BYTES);
        let fb = truncate_sku_code_bytes(b, WECHAT_SKU_CODE_MAX_BYTES);
        assert_ne!(fa, fb, "仅尾部不同的 SKU 截断后必须仍唯一");
    }

    #[test]
    fn apply_defaults_fixes_persisted_oversize_sku_code() {
        let long = "尺码=00:05/00:30;身高=120cm;颜色分类=绿色（升级款）【面料更舒适，90%用户选择】";
        let mut payload = serde_json::json!({
            "skus": [ { "out_sku_id": long, "sku_code": long, "sale_price": 100, "stock_num": 1 } ]
        })
        .as_object()
        .cloned()
        .unwrap();
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "https://item.taobao.com/item.htm?id=1",
            "title": "测试",
            "source_url": "https://item.taobao.com/item.htm?id=1",
            "skus": []
        }))
        .unwrap();
        apply_add_product_defaults(&mut payload, &product, &mut Vec::new());
        let sku = payload["skus"][0].as_object().unwrap();
        let code = sku["sku_code"].as_str().unwrap();
        assert!(code.len() <= WECHAT_SKU_CODE_MAX_BYTES);
        // out_sku_id 仅 ~约 41 字符 < 128，应保持完整不动(采购映射键)
        assert_eq!(sku["out_sku_id"].as_str().unwrap(), long);
    }

    #[test]
    fn generate_draft_truncates_skus_over_wechat_limit() {
        // 微信 addproduct 单商品 SKU 上限 500（路径A 动态生成）。构造 501 个有库存 SKU，
        // 经 generate_add_product_payload_draft 断言最终 payload 截断到 500 且 warning 留痕——
        // 截断由 normalize_skus_for_wechat 单点完成（draft 末尾经 apply_add_product_defaults 必经）；
        // 截断后恰好 500 个，能通过 validate（>500 才拦截），不会让整商品发布失败。
        let skus: Vec<Value> = (0..501)
            .map(|i| {
                serde_json::json!({
                    "external_sku_id": format!("sku-{i}"),
                    "specs": { "编号": format!("{i}") },
                    "cost_price": 10.0,
                    "stock": 1
                })
            })
            .collect();
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "https://item.taobao.com/item.htm?id=1",
            "title": "测试超量 SKU",
            "source_url": "https://item.taobao.com/item.htm?id=1",
            "skus": skus,
            "metadata": { "wechat_category_ids": [1, 2, 3] }
        }))
        .expect("构造测试商品");

        let draft =
            generate_add_product_payload_draft(&product, product.metadata.as_object(), false)
                .expect("应截断后通过校验生成草稿");
        assert_eq!(
            draft
                .payload
                .get("skus")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(ADD_PRODUCT_MAX_SKUS),
            "最终 payload 的 SKU 应被截断到微信上限"
        );
        assert!(
            draft
                .warnings
                .iter()
                .any(|w| w.contains("截断") && w.contains("丢弃 1 个")),
            "应记录截断 warning，实际={:?}",
            draft.warnings
        );
    }

    #[test]
    fn apply_defaults_truncates_skus_over_wechat_limit() {
        // 固化 payload（路径B）同样需对超 500 SKU 截断，否则 addproduct 整商品被拒。
        let skus: Vec<Value> = (0..501)
            .map(|i| {
                serde_json::json!({
                    "out_sku_id": format!("sku-{i}"),
                    "sku_code": format!("sku-{i}"),
                    "sale_price": 100,
                    "stock_num": 1
                })
            })
            .collect();
        let mut payload = serde_json::json!({ "skus": skus })
            .as_object()
            .cloned()
            .unwrap();
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "https://item.taobao.com/item.htm?id=1",
            "title": "测试",
            "source_url": "https://item.taobao.com/item.htm?id=1",
            "skus": []
        }))
        .unwrap();

        let mut warnings = Vec::new();
        apply_add_product_defaults(&mut payload, &product, &mut warnings);
        assert_eq!(
            payload["skus"].as_array().map(Vec::len),
            Some(ADD_PRODUCT_MAX_SKUS),
            "固化 payload 的 SKU 应被截断到微信上限"
        );
        assert!(
            warnings.iter().any(|w| w.contains("截断")),
            "应记录截断 warning，实际={warnings:?}"
        );
    }

    #[test]
    fn express_payload_requires_after_sale_address_id() {
        let payload = base_payload(0);

        let error = validate_add_product_base_payload(&payload).expect_err("应拦截缺失售后地址");

        assert!(error.contains("after_sale_info.after_sale_address_id"));
        assert_eq!(
            add_product_payload_error_code(&error),
            "MISSING_AFTER_SALE_ADDRESS"
        );
    }

    #[test]
    fn relaxed_validation_allows_missing_after_sale_address_id() {
        let payload = base_payload(0);

        validate_add_product_base_payload_without_after_sale(&payload)
            .expect("预检阶段允许后续自动补齐售后地址");
    }

    #[test]
    fn non_express_payload_does_not_require_after_sale_address_id() {
        let payload = base_payload(1);

        validate_add_product_base_payload(&payload).expect("无需快递发货不应要求售后地址");
    }

    #[test]
    fn express_payload_accepts_after_sale_address_id() {
        let mut payload = base_payload(0);
        payload.insert(
            "after_sale_info".to_string(),
            serde_json::json!({ "after_sale_address_id": 123 }),
        );

        validate_add_product_base_payload(&payload).expect("有效售后地址应通过校验");
    }

    #[test]
    fn add_product_payload_rejects_image_and_sku_over_limits() {
        let mut payload = base_payload(0);
        payload.insert(
            "head_imgs".to_string(),
            Value::Array(
                (0..10)
                    .map(|index| Value::String(format!("h{index}")))
                    .collect(),
            ),
        );

        let error = validate_add_product_base_payload_without_after_sale(&payload)
            .expect_err("头图超过 9 张应被拦截");
        assert!(error.contains("头图"));

        let mut payload = base_payload(0);
        payload.insert(
            "desc_info".to_string(),
            serde_json::json!({ "imgs": (0..21).map(|index| format!("d{index}")).collect::<Vec<_>>() }),
        );
        let error = validate_add_product_base_payload_without_after_sale(&payload)
            .expect_err("详情图超过 20 张应被拦截");
        assert!(error.contains("详情图"));

        let mut payload = base_payload(0);
        payload.insert(
            "skus".to_string(),
            Value::Array(
                (0..501)
                    .map(|_| serde_json::json!({ "stock_num": 1 }))
                    .collect(),
            ),
        );
        let error = validate_add_product_base_payload_without_after_sale(&payload)
            .expect_err("SKU 超过 500 个应被拦截");
        assert!(error.contains("SKU"));
    }

    #[test]
    fn add_product_payload_truncates_prepared_images_to_wechat_limits() {
        let product = ExternalProductInput {
            external_product_id: "item-1".to_string(),
            title: "测试商品标题".to_string(),
            source_url: "https://example.com/item".to_string(),
            // 头图/详情图原始 URL 须与下方 assets 的 source_url 对应，submit 端按 source_url 反查微信图
            images: (0..12)
                .map(|i| format!("https://img.alicdn.com/h{i}.jpg"))
                .collect(),
            detail_images: (0..55)
                .map(|i| format!("https://img.alicdn.com/d{i}.jpg"))
                .collect(),
            main_video: None,
            skus: Vec::new(),
            supplier_name: None,
            supplier_product_id: None,
            category_hint: None,
            brand_hint: None,
            weight_gram: None,
            metadata: serde_json::json!({
                "wechat_add_product_payload": {
                    "deliver_method": 0,
                    "extra_service": {
                        "seven_day_return": 1,
                        "freight_insurance": 0
                    },
                    "cats_v2": [{ "cat_id": 1 }, { "cat_id": 2 }, { "cat_id": 3 }],
                    "skus": [{ "out_sku_id": "sku-1", "sale_price": 100, "stock_num": 1 }]
                }
            }),
        };
        let assets = (0..12)
            .map(|index| PreparedAsset {
                kind: "head_image".to_string(),
                sort_order: index,
                wechat_url: format!("https://mmecimage.cn/p/h{index}.jpg"),
                source_url: format!("https://img.alicdn.com/h{index}.jpg"),
            })
            .chain((0..55).map(|index| PreparedAsset {
                kind: "detail_image".to_string(),
                sort_order: index,
                wechat_url: format!("https://mmecimage.cn/p/d{index}.jpg"),
                source_url: format!("https://img.alicdn.com/d{index}.jpg"),
            }))
            .collect::<Vec<_>>();

        let payload = build_add_product_payload_relaxed_after_sale(&product, &assets)
            .expect("超量图片应被裁剪后通过");

        assert_eq!(
            payload
                .get("head_imgs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(ADD_PRODUCT_MAX_HEAD_IMAGES)
        );
        assert_eq!(
            payload
                .pointer("/desc_info/imgs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(ADD_PRODUCT_MAX_DETAIL_IMAGES)
        );
    }

    #[test]
    fn apply_sku_thumb_images_fills_per_color_thumb_and_degrades_gracefully() {
        // 锁死「切换 SKU 换主图」铺货端契约。模拟真实主路径：AI 属性补齐后
        // metadata.wechat_add_product_payload 已固化，其 skus[].out_sku_id == external_sku_id
        // （≤128 字符时 normalize_skus_for_wechat 保持原值，out_sku_id 是采购对账映射键）。
        // 断言：①同色多尺码共享一张颜色图 → 各 SKU 都填同一 thumb_img；
        //       ②颜色图上传失败 / 无颜色图的 SKU 不填 thumb_img（宁缺勿错，避免微信 10020035）。
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "https://item.taobao.com/item.htm?id=1",
            "title": "测试连衣裙",
            "source_url": "https://item.taobao.com/item.htm?id=1",
            "images": [
                "https://img.alicdn.com/h0.jpg",
                "https://img.alicdn.com/h1.jpg",
                "https://img.alicdn.com/h2.jpg"
            ],
            "detail_images": ["https://img.alicdn.com/d0.jpg"],
            "skus": [
                {"external_sku_id": "颜色;红;尺码;S", "cost_price": 10.0, "stock": 5,
                 "sku_image": "https://img.alicdn.com/red.jpg"},
                {"external_sku_id": "颜色;红;尺码;M", "cost_price": 10.0, "stock": 5,
                 "sku_image": "https://img.alicdn.com/red.jpg"},
                {"external_sku_id": "颜色;蓝;尺码;S", "cost_price": 10.0, "stock": 5,
                 "sku_image": "https://img.alicdn.com/blue.jpg"},
                {"external_sku_id": "颜色;绿;尺码;S", "cost_price": 10.0, "stock": 5}
            ],
            "metadata": {
                "wechat_add_product_payload": {
                    "deliver_method": 0,
                    "extra_service": { "seven_day_return": 1, "freight_insurance": 0 },
                    "cats_v2": [{ "cat_id": 1 }, { "cat_id": 2 }, { "cat_id": 3 }],
                    "skus": [
                        { "out_sku_id": "颜色;红;尺码;S", "sale_price": 100, "stock_num": 5 },
                        { "out_sku_id": "颜色;红;尺码;M", "sale_price": 100, "stock_num": 5 },
                        { "out_sku_id": "颜色;蓝;尺码;S", "sale_price": 100, "stock_num": 5 },
                        { "out_sku_id": "颜色;绿;尺码;S", "sale_price": 100, "stock_num": 5 }
                    ]
                }
            }
        }))
        .expect("构造测试商品");

        let mut assets: Vec<PreparedAsset> = (0..3)
            .map(|index| PreparedAsset {
                kind: "head_image".to_string(),
                sort_order: index,
                wechat_url: format!("https://mmecimage.cn/p/h{index}.jpg"),
                source_url: format!("https://img.alicdn.com/h{index}.jpg"),
            })
            .collect();
        assets.push(PreparedAsset {
            kind: "detail_image".to_string(),
            sort_order: 0,
            wechat_url: "https://mmecimage.cn/p/d0.jpg".to_string(),
            source_url: "https://img.alicdn.com/d0.jpg".to_string(),
        });
        // 仅红色颜色图上传成功；蓝色缺失，模拟其颜色图上传失败被降级跳过。
        assets.push(PreparedAsset {
            kind: "sku_image".to_string(),
            sort_order: 0,
            wechat_url: "https://mmecimage.cn/p/red.jpg".to_string(),
            source_url: "https://img.alicdn.com/red.jpg".to_string(),
        });

        let payload = build_add_product_payload_relaxed_after_sale(&product, &assets)
            .expect("应构建 add_product payload 成功");
        let skus = payload
            .get("skus")
            .and_then(Value::as_array)
            .expect("payload 应含 skus 数组");
        let thumb_of = |out_sku_id: &str| -> Option<String> {
            skus.iter()
                .find(|sku| sku.get("out_sku_id").and_then(Value::as_str) == Some(out_sku_id))
                .and_then(|sku| sku.get("thumb_img"))
                .and_then(Value::as_str)
                .map(str::to_string)
        };

        // 同色多尺码共享同一张已上传微信颜色图
        assert_eq!(
            thumb_of("颜色;红;尺码;S").as_deref(),
            Some("https://mmecimage.cn/p/red.jpg"),
            "红色 S 应填入颜色图 thumb_img"
        );
        assert_eq!(
            thumb_of("颜色;红;尺码;M").as_deref(),
            Some("https://mmecimage.cn/p/red.jpg"),
            "红色 M 应与红色 S 共享同一颜色图"
        );
        // 颜色图上传失败 / 无颜色图 → 不填 thumb_img（宁缺勿错）
        assert_eq!(
            thumb_of("颜色;蓝;尺码;S"),
            None,
            "蓝色颜色图上传失败时不应填 thumb_img"
        );
        assert_eq!(
            thumb_of("颜色;绿;尺码;S"),
            None,
            "无颜色图的 SKU 不应填 thumb_img"
        );
    }

    #[test]
    fn collect_product_assets_includes_main_video() {
        // 主图视频应被收集为 kind="head_video" 素材（每商品最多 1 个）；无 main_video 时不产生该素材。
        let video_url =
            "https://cloud.video.taobao.com/play/u/1/p/2/e/6/t/1/457890291899.mp4?appKey=38829";
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "id-1",
            "title": "测试睡衣",
            "source_url": "https://item.taobao.com/item.htm?id=1",
            "images": ["https://img.alicdn.com/h0.jpg"],
            "main_video": video_url,
            "skus": [{"external_sku_id": "sku-1", "cost_price": 10.0, "stock": 5}]
        }))
        .expect("构造带视频商品");
        let assets = collect_product_assets(&product);
        let video_count = assets.iter().filter(|a| a.kind == "head_video").count();
        assert_eq!(video_count, 1, "应收集到 1 个主图视频素材");
        let video = assets
            .iter()
            .find(|a| a.kind == "head_video")
            .expect("应有 head_video 素材");
        assert_eq!(video.source_url.as_str(), video_url, "视频素材应保留淘宝原始 URL");

        // 无 main_video 的商品不产生 head_video 素材
        let no_video: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "id-2",
            "title": "无视频商品",
            "source_url": "https://item.taobao.com/item.htm?id=2",
            "images": ["https://img.alicdn.com/h0.jpg"],
            "skus": [{"external_sku_id": "sku-1", "cost_price": 10.0, "stock": 5}]
        }))
        .expect("构造无视频商品");
        assert!(
            collect_product_assets(&no_video)
                .iter()
                .all(|a| a.kind != "head_video"),
            "无 main_video 时不应产生 head_video 素材"
        );
    }

    #[test]
    fn build_payload_fills_head_video_and_omits_when_absent() {
        // 锁死「主图视频搬运」铺货端契约：已上传的 head_video 素材 → payload.head_videos[0].video_url
        // 填入微信播放 URL（head_videos 须为数组，见 apply_head_video）；无 head_video 素材 →
        // 不输出 head_videos 字段（视频为可选增强，缺失不阻断）。
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "https://item.taobao.com/item.htm?id=9",
            "title": "测试带视频商品",
            "source_url": "https://item.taobao.com/item.htm?id=9",
            "images": [
                "https://img.alicdn.com/h0.jpg",
                "https://img.alicdn.com/h1.jpg",
                "https://img.alicdn.com/h2.jpg"
            ],
            "detail_images": ["https://img.alicdn.com/d0.jpg"],
            "skus": [{"external_sku_id": "sku-1", "cost_price": 10.0, "stock": 5}],
            "metadata": {
                "wechat_add_product_payload": {
                    "deliver_method": 0,
                    "extra_service": { "seven_day_return": 1, "freight_insurance": 0 },
                    "cats_v2": [{ "cat_id": 1 }, { "cat_id": 2 }, { "cat_id": 3 }],
                    "skus": [{ "out_sku_id": "sku-1", "sale_price": 100, "stock_num": 5 }]
                }
            }
        }))
        .expect("构造测试商品");

        let mut assets: Vec<PreparedAsset> = (0..3)
            .map(|index| PreparedAsset {
                kind: "head_image".to_string(),
                sort_order: index,
                wechat_url: format!("https://mmecimage.cn/p/h{index}.jpg"),
                source_url: format!("https://img.alicdn.com/h{index}.jpg"),
            })
            .collect();
        assets.push(PreparedAsset {
            kind: "detail_image".to_string(),
            sort_order: 0,
            wechat_url: "https://mmecimage.cn/p/d0.jpg".to_string(),
            source_url: "https://img.alicdn.com/d0.jpg".to_string(),
        });

        // 无 head_video 素材：payload 不应输出 head_videos
        let without_video = build_add_product_payload_relaxed_after_sale(&product, &assets)
            .expect("应构建 payload 成功");
        assert!(
            without_video.get("head_videos").is_none(),
            "无视频素材时不应输出 head_videos"
        );

        // 含 head_video 素材：payload.head_videos[0].video_url 应为微信播放 URL
        let wechat_video_url =
            "https://173.wxapp.tc.qq.com/play/abc.f0.mp4?dis_k=k&dis_t=1";
        assets.push(PreparedAsset {
            kind: "head_video".to_string(),
            sort_order: 0,
            wechat_url: wechat_video_url.to_string(),
            source_url: "https://cloud.video.taobao.com/play/u/1/457890291899.mp4?appKey=38829"
                .to_string(),
        });
        let with_video = build_add_product_payload_relaxed_after_sale(&product, &assets)
            .expect("应构建 payload 成功");
        assert_eq!(
            with_video
                .pointer("/head_videos/0/video_url")
                .and_then(Value::as_str),
            Some(wechat_video_url),
            "head_videos[0].video_url 应填入已上传的微信视频 URL"
        );
        // head_videos 必须是数组（微信服务端要求，object 会报 addproduct 47001）
        assert!(
            with_video
                .get("head_videos")
                .map(Value::is_array)
                .unwrap_or(false),
            "head_videos 必须是数组"
        );
    }

    #[test]
    fn detail_image_resolved_when_kind_overwritten_by_sku_image() {
        // 回归 bug：同一张淘宝图既作详情图又作 SKU 颜色图时，publish_assets 的
        // ON CONFLICT(item_id, source_url) 把这条记录的 asset_kind 覆盖成后写入的 sku_image，
        // 旧逻辑按 asset_kind='detail_image' 取详情图会取不到、误报「详情图为空」。
        // 本测试模拟「被覆盖现场」：共享图在 assets 里 kind=sku_image，验证 submit 端仍能
        // 按 source_url 反查到该详情图（不再依赖会被覆盖的 kind）。
        let shared = "https://img.alicdn.com/shared-color-and-detail.jpg";
        let shared_wechat = "https://mmecimage.cn/p/shared.jpg";
        let product: ExternalProductInput = serde_json::from_value(serde_json::json!({
            "external_product_id": "item-shared",
            "title": "详情图与颜色图同图商品",
            "source_url": "https://item.taobao.com/item.htm?id=2",
            "images": [
                "https://img.alicdn.com/h0.jpg",
                "https://img.alicdn.com/h1.jpg",
                "https://img.alicdn.com/h2.jpg"
            ],
            "detail_images": [shared],
            "skus": [
                {"external_sku_id": "sku-1", "cost_price": 10.0, "stock": 5, "sku_image": shared}
            ],
            "metadata": {
                "wechat_add_product_payload": {
                    "deliver_method": 0,
                    "extra_service": { "seven_day_return": 1, "freight_insurance": 0 },
                    "cats_v2": [{ "cat_id": 1 }, { "cat_id": 2 }, { "cat_id": 3 }],
                    "skus": [{ "out_sku_id": "sku-1", "sale_price": 100, "stock_num": 5 }]
                }
            }
        }))
        .expect("构造测试商品");

        let mut assets: Vec<PreparedAsset> = (0..3)
            .map(|index| PreparedAsset {
                kind: "head_image".to_string(),
                sort_order: index,
                wechat_url: format!("https://mmecimage.cn/p/h{index}.jpg"),
                source_url: format!("https://img.alicdn.com/h{index}.jpg"),
            })
            .collect();
        // 共享图：详情图先写、SKU 图后写，ON CONFLICT 后这条记录 asset_kind 已是 sku_image（仅一条）。
        assets.push(PreparedAsset {
            kind: "sku_image".to_string(),
            sort_order: 0,
            wechat_url: shared_wechat.to_string(),
            source_url: shared.to_string(),
        });

        let payload = build_add_product_payload_relaxed_after_sale(&product, &assets)
            .expect("详情图被 SKU 图覆盖 kind 后仍应能按 source_url 反查到，不应报详情图为空");
        // 详情图按 source_url 反查到（即便 asset_kind 已是 sku_image）
        let desc_imgs = payload
            .pointer("/desc_info/imgs")
            .and_then(Value::as_array)
            .expect("payload 应含 desc_info.imgs");
        assert_eq!(
            desc_imgs
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>(),
            vec![shared_wechat],
            "详情图应按 source_url 反查到共享图的微信 URL"
        );
        // 同图作 SKU 颜色图也应正常填 thumb_img
        let thumb = payload
            .get("skus")
            .and_then(Value::as_array)
            .and_then(|skus| skus.first())
            .and_then(|sku| sku.get("thumb_img"))
            .and_then(Value::as_str);
        assert_eq!(thumb, Some(shared_wechat), "共享图应同时作为 SKU thumb_img");
    }
}
