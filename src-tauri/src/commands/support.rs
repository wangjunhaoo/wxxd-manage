use super::*;

pub(super) fn upsert_shop_secret(
    app: &AppHandle,
    conn: &Connection,
    shop_id: &str,
    secret: &str,
    updated_at: &str,
) -> AppResult<()> {
    let encrypted = encrypt_secret(app, secret)?;
    let fingerprint = secret_fingerprint(secret);
    conn.execute(
        "INSERT INTO shop_credentials
         (shop_id, encrypted_secret, secret_nonce, key_version, secret_fingerprint, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(shop_id) DO UPDATE SET
           encrypted_secret = excluded.encrypted_secret,
           secret_nonce = excluded.secret_nonce,
           key_version = excluded.key_version,
           secret_fingerprint = excluded.secret_fingerprint,
           updated_at = excluded.updated_at",
        params![
            shop_id,
            encrypted.ciphertext,
            encrypted.nonce,
            encrypted.key_version,
            fingerprint,
            updated_at
        ],
    )?;
    Ok(())
}

pub(super) async fn ensure_access_token(
    app: &AppHandle,
    shop_id: &str,
    client: &WechatShopClient,
) -> AppResult<String> {
    let cached_token = {
        let conn = open_connection(app)?;
        conn.query_row(
            "SELECT encrypted_access_token, token_nonce, expires_at
             FROM access_tokens
             WHERE shop_id = ?1",
            [shop_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
    };
    if let Some((encrypted_access_token, token_nonce, expires_at)) = cached_token {
        if is_future_rfc3339(&expires_at) {
            return decrypt_access_token(app, &encrypted_access_token, &token_nonce);
        }
    }

    let (appid, encrypted_secret, secret_nonce) = {
        let conn = open_connection(app)?;
        conn.query_row(
            "SELECT s.appid, c.encrypted_secret, c.secret_nonce
             FROM shops s
             JOIN shop_credentials c ON c.shop_id = s.id
             WHERE s.id = ?1",
            [shop_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("店铺不存在或未保存 app_secret".to_string()))?
    };
    let secret = decrypt_secret(app, &encrypted_secret, &secret_nonce)?;
    let call = client
        .get_stable_access_token(&appid, &secret, false)
        .await?;
    let conn = open_connection(app)?;
    match &call.result {
        WechatCallResult::Success(token) => {
            let encrypted_token = encrypt_access_token(app, &token.access_token)?;
            let expires_at = expires_at_shanghai(token.expires_in);
            let refreshed_at = now_shanghai();
            conn.execute(
                "INSERT INTO access_tokens
                 (shop_id, encrypted_access_token, token_nonce, key_version, expires_at, refreshed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(shop_id) DO UPDATE SET
                   encrypted_access_token = excluded.encrypted_access_token,
                   token_nonce = excluded.token_nonce,
                   key_version = excluded.key_version,
                   expires_at = excluded.expires_at,
                   refreshed_at = excluded.refreshed_at",
                params![
                    shop_id,
                    encrypted_token.ciphertext,
                    encrypted_token.nonce,
                    encrypted_token.key_version,
                    expires_at,
                    refreshed_at
                ],
            )?;
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "stable_token refreshed, expires_in={}s",
                    token.expires_in
                )),
            )?;
            Ok(token.access_token.clone())
        }
        WechatCallResult::ApiError(error) => {
            conn.execute(
                "UPDATE shops SET status = 'auth_failed' WHERE id = ?1",
                [shop_id],
            )?;
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("stable_token refresh api error"),
            )?;
            Err(AppError::WechatApi {
                errcode: error.errcode,
                errmsg: error.errmsg.clone(),
            })
        }
    }
}

pub(super) fn insert_api_call_log(
    conn: &Connection,
    shop_id: Option<&str>,
    endpoint: &str,
    method: &str,
    status: &str,
    errcode: Option<i64>,
    errmsg: Option<&str>,
    response_summary: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO api_call_logs
         (id, shop_id, provider, endpoint, method, status, errcode, errmsg, response_summary, created_at)
         VALUES (?1, ?2, 'wechat_shop', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            format!("api-{}", Uuid::new_v4()),
            shop_id,
            endpoint,
            method,
            status,
            errcode,
            errmsg,
            response_summary,
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(super) fn validate_publish_request(request: &ExternalPublishJobRequest) -> AppResult<()> {
    if request.request_id.trim().is_empty() {
        return Err(AppError::Validation("request_id 不能为空".to_string()));
    }
    if request.target_shop_group_ids.is_empty() && request.target_shop_ids.is_empty() {
        return Err(AppError::Validation(
            "必须提供目标店铺组或目标店铺".to_string(),
        ));
    }
    if request.products.is_empty() {
        return Err(AppError::Validation("products 不能为空".to_string()));
    }
    for product in &request.products {
        validate_product(product)?;
    }
    Ok(())
}

pub(super) fn validate_price_update_request(request: &PriceUpdateJobRequest) -> AppResult<()> {
    if request.request_id.trim().is_empty() {
        return Err(AppError::Validation("request_id 不能为空".to_string()));
    }
    if request.target_shop_group_ids.is_empty() && request.target_shop_ids.is_empty() {
        return Err(AppError::Validation(
            "必须提供目标店铺组或目标店铺".to_string(),
        ));
    }
    if request.products.is_empty() {
        return Err(AppError::Validation("products 不能为空".to_string()));
    }
    for product in &request.products {
        if product.external_product_id.trim().is_empty() {
            return Err(AppError::Validation(
                "external_product_id 不能为空".to_string(),
            ));
        }
        if product.target_price_cents <= 0 {
            return Err(AppError::Validation(format!(
                "商品 {} 的目标售价必须大于 0 分",
                product.external_product_id
            )));
        }
        if product.target_price_cents > 10_000_000 {
            return Err(AppError::Validation(format!(
                "商品 {} 的目标售价超过安全上限",
                product.external_product_id
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_product(product: &ExternalProductInput) -> AppResult<()> {
    if product.external_product_id.trim().is_empty() {
        return Err(AppError::Validation(
            "external_product_id 不能为空".to_string(),
        ));
    }
    if product.title.trim().is_empty() {
        return Err(AppError::Validation("title 不能为空".to_string()));
    }
    if product.source_url.trim().is_empty() {
        return Err(AppError::Validation("source_url 不能为空".to_string()));
    }
    if product.skus.is_empty() {
        return Err(AppError::Validation(format!(
            "商品 {} 缺少 skus",
            product.external_product_id
        )));
    }
    for sku in &product.skus {
        if sku.external_sku_id.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "商品 {} 存在空 external_sku_id",
                product.external_product_id
            )));
        }
        if sku.cost_price <= 0.0 {
            return Err(AppError::Validation(format!(
                "商品 {} 的 SKU {} 成本价必须大于 0",
                product.external_product_id, sku.external_sku_id
            )));
        }
        if sku.stock < 0 {
            return Err(AppError::Validation(format!(
                "商品 {} 的 SKU {} 库存不能为负数",
                product.external_product_id, sku.external_sku_id
            )));
        }
    }
    Ok(())
}

pub(super) fn resolve_target_shops(
    conn: &Connection,
    request: &ExternalPublishJobRequest,
) -> AppResult<Vec<TargetShop>> {
    resolve_target_shops_for_scope(
        conn,
        &request.target_shop_group_ids,
        &request.target_shop_ids,
    )
}

pub(super) fn resolve_target_shops_for_scope(
    conn: &Connection,
    target_shop_group_ids: &[String],
    target_shop_ids: &[String],
) -> AppResult<Vec<TargetShop>> {
    let mut targets = Vec::new();

    for group_id in target_shop_group_ids {
        let mut stmt =
            conn.prepare("SELECT id, name FROM shops WHERE group_id = ?1 ORDER BY created_at ASC")?;
        for row in stmt.query_map([group_id], |row| {
            Ok(TargetShop {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })? {
            targets.push(row?);
        }
    }

    for shop_id in target_shop_ids {
        let shop = conn
            .query_row(
                "SELECT id, name FROM shops WHERE id = ?1",
                [shop_id.as_str()],
                |row| {
                    Ok(TargetShop {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    })
                },
            )
            .optional()?;
        if let Some(shop) = shop {
            targets.push(shop);
        }
    }

    targets.sort_by(|a, b| a.id.cmp(&b.id));
    targets.dedup_by(|a, b| a.id == b.id);
    Ok(targets)
}

pub(super) fn load_job_items(
    conn: &Connection,
    product_row_id: &str,
) -> AppResult<Vec<PublishJobItemView>> {
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, shop_name, status, error_code, error_summary, created_at
         FROM publish_job_items
         WHERE product_row_id = ?1
         ORDER BY shop_name ASC",
    )?;
    let items = stmt
        .query_map([product_row_id], |row| {
            Ok(PublishJobItemView {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                shop_name: row.get(2)?,
                status: row.get(3)?,
                error_code: row.get(4)?,
                error_summary: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_price_update_items(
    conn: &Connection,
    job_id: &str,
) -> AppResult<Vec<PriceUpdateItemView>> {
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, shop_name, external_product_id, target_price_cents, status,
                error_code, error_summary, wechat_product_id, created_at, updated_at
         FROM price_update_items
         WHERE job_id = ?1
         ORDER BY external_product_id ASC, shop_name ASC",
    )?;
    let items = stmt
        .query_map([job_id], |row| {
            Ok(PriceUpdateItemView {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                shop_name: row.get(2)?,
                external_product_id: row.get(3)?,
                target_price_cents: row.get(4)?,
                status: row.get(5)?,
                error_code: row.get(6)?,
                error_summary: row.get(7)?,
                wechat_product_id: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_category_catalog_shop_summaries(
    conn: &Connection,
) -> AppResult<Vec<CategoryCatalogShopSummary>> {
    let mut stmt = conn.prepare(
        "SELECT
           s.id,
           s.name,
           (SELECT COUNT(*) FROM wechat_categories c WHERE c.shop_id = s.id),
           (SELECT COUNT(*) FROM wechat_category_details d WHERE d.shop_id = s.id),
           (SELECT COUNT(*) FROM wechat_category_rules r WHERE r.shop_id = s.id AND r.rule_type = 'product'),
           (SELECT COUNT(*) FROM wechat_category_rules r WHERE r.shop_id = s.id AND r.rule_type = 'delivery'),
           (SELECT COUNT(*) FROM wechat_freight_templates f WHERE f.shop_id = s.id),
           (SELECT MAX(synced_at) FROM wechat_categories c WHERE c.shop_id = s.id),
           (SELECT MAX(synced_at) FROM wechat_category_rules r WHERE r.shop_id = s.id),
           (SELECT MAX(synced_at) FROM wechat_freight_templates f WHERE f.shop_id = s.id)
         FROM shops s
         ORDER BY s.created_at ASC",
    )?;
    let items = stmt
        .query_map([], |row| {
            Ok(CategoryCatalogShopSummary {
                shop_id: row.get(0)?,
                shop_name: row.get(1)?,
                category_count: row.get(2)?,
                detail_count: row.get(3)?,
                product_rule_count: row.get(4)?,
                delivery_rule_count: row.get(5)?,
                freight_template_count: row.get(6)?,
                last_category_sync_at: row.get(7)?,
                last_rule_sync_at: row.get(8)?,
                last_freight_sync_at: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_category_cache_views(
    conn: &Connection,
    shop_id: Option<&str>,
    keyword: Option<&str>,
    limit: i64,
) -> AppResult<Vec<CategoryCacheView>> {
    let mut stmt = conn.prepare(
        "SELECT
           c.shop_id,
           s.name,
           c.cat_id,
           c.parent_cat_id,
           c.level,
           c.name,
           COALESCE(d.product_attr_count, 0),
           COALESCE(d.sale_attr_count, 0),
           COALESCE(d.product_qua_count, 0),
           d.cat_id IS NOT NULL,
           pr.cat_id IS NOT NULL,
           dr.cat_id IS NOT NULL,
           c.synced_at,
           d.synced_at
         FROM wechat_categories c
         JOIN shops s ON s.id = c.shop_id
         LEFT JOIN wechat_category_details d
           ON d.shop_id = c.shop_id AND d.cat_id = c.cat_id
         LEFT JOIN wechat_category_rules pr
           ON pr.shop_id = c.shop_id AND pr.cat_id = c.cat_id AND pr.rule_type = 'product'
         LEFT JOIN wechat_category_rules dr
           ON dr.shop_id = c.shop_id AND dr.cat_id = c.cat_id AND dr.rule_type = 'delivery'
         WHERE (?1 IS NULL OR c.shop_id = ?1)
           AND (?2 IS NULL OR c.name LIKE ?2 OR CAST(c.cat_id AS TEXT) LIKE ?2)
         ORDER BY c.level DESC, c.name ASC
         LIMIT ?3",
    )?;
    let items = stmt
        .query_map(params![shop_id, keyword, limit], |row| {
            Ok(CategoryCacheView {
                shop_id: row.get(0)?,
                shop_name: row.get(1)?,
                cat_id: row.get(2)?,
                parent_cat_id: row.get(3)?,
                level: row.get(4)?,
                name: row.get(5)?,
                product_attr_count: row.get(6)?,
                sale_attr_count: row.get(7)?,
                product_qua_count: row.get(8)?,
                has_detail: row.get::<_, i64>(9)? == 1,
                has_product_rule: row.get::<_, i64>(10)? == 1,
                has_delivery_rule: row.get::<_, i64>(11)? == 1,
                synced_at: row.get(12)?,
                detail_synced_at: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_freight_template_views(
    conn: &Connection,
    shop_id: Option<&str>,
) -> AppResult<Vec<FreightTemplateView>> {
    let mut stmt = conn.prepare(
        "SELECT f.shop_id, s.name, f.template_id, f.synced_at
         FROM wechat_freight_templates f
         JOIN shops s ON s.id = f.shop_id
         WHERE (?1 IS NULL OR f.shop_id = ?1)
         ORDER BY s.name ASC, f.template_id ASC
         LIMIT 200",
    )?;
    let items = stmt
        .query_map(params![shop_id], |row| {
            Ok(FreightTemplateView {
                shop_id: row.get(0)?,
                shop_name: row.get(1)?,
                template_id: row.get(2)?,
                synced_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) async fn sync_freight_template_ids(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    shop_id: &str,
    task_id: &str,
) -> AppResult<i64> {
    let mut offset = 0i64;
    let limit = 100i64;
    let mut total = 0i64;
    for _ in 0..20 {
        let call = client
            .get_freight_template_list(access_token, offset, limit)
            .await?;
        match &call.result {
            WechatCallResult::Success(raw) => {
                let template_ids = extract_freight_template_ids(&raw.raw_payload);
                let conn = open_connection(app)?;
                upsert_freight_template_ids(&conn, shop_id, &template_ids, &now_shanghai())?;
                insert_api_call_log(
                    &conn,
                    Some(shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "success",
                    None,
                    None,
                    Some(&format!(
                        "synced freight templates page offset={}, count={}",
                        offset,
                        template_ids.len()
                    )),
                )?;
                total += template_ids.len() as i64;
                if template_ids.len() < limit as usize {
                    break;
                }
                offset += limit;
            }
            WechatCallResult::ApiError(error) => {
                let conn = open_connection(app)?;
                insert_api_call_log(
                    &conn,
                    Some(shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("freight template list api error"),
                )?;
                insert_task_log(
                    &conn,
                    task_id,
                    None,
                    "error",
                    &format!("运费模板同步失败：{}", error.errmsg),
                    Some(&serde_json::json!({ "errcode": error.errcode, "offset": offset })),
                )?;
                return Err(AppError::WechatApi {
                    errcode: error.errcode,
                    errmsg: error.errmsg.clone(),
                });
            }
        }
    }

    let conn = open_connection(app)?;
    insert_task_log(
        &conn,
        task_id,
        None,
        "info",
        &format!("运费模板同步完成：{total} 个模板 ID"),
        None,
    )?;
    Ok(total)
}

pub(super) fn upsert_wechat_categories(
    conn: &Connection,
    shop_id: &str,
    categories: &[CachedWechatCategory],
    synced_at: &str,
) -> AppResult<()> {
    for category in categories {
        conn.execute(
            "INSERT INTO wechat_categories
             (shop_id, cat_id, parent_cat_id, level, name, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(shop_id, cat_id) DO UPDATE SET
               parent_cat_id = excluded.parent_cat_id,
               level = excluded.level,
               name = excluded.name,
               raw_payload = excluded.raw_payload,
               synced_at = excluded.synced_at",
            params![
                shop_id,
                category.cat_id,
                category.parent_cat_id,
                category.level,
                category.name,
                category.raw_payload.to_string(),
                synced_at
            ],
        )?;
    }
    Ok(())
}

pub(super) fn upsert_freight_template_ids(
    conn: &Connection,
    shop_id: &str,
    template_ids: &[String],
    synced_at: &str,
) -> AppResult<()> {
    for template_id in template_ids {
        conn.execute(
            "INSERT INTO wechat_freight_templates (shop_id, template_id, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(shop_id, template_id) DO UPDATE SET
               raw_payload = excluded.raw_payload,
               synced_at = excluded.synced_at",
            params![
                shop_id,
                template_id,
                serde_json::json!({ "template_id": template_id }).to_string(),
                synced_at
            ],
        )?;
    }
    Ok(())
}

pub(super) fn upsert_category_detail(
    conn: &Connection,
    shop_id: &str,
    cat_id: i64,
    raw_payload: &Value,
    counts: &CategoryDetailCounts,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO wechat_category_details
         (shop_id, cat_id, product_attr_count, sale_attr_count, product_qua_count, raw_payload, synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(shop_id, cat_id) DO UPDATE SET
           product_attr_count = excluded.product_attr_count,
           sale_attr_count = excluded.sale_attr_count,
           product_qua_count = excluded.product_qua_count,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at",
        params![
            shop_id,
            cat_id,
            counts.product_attr_count,
            counts.sale_attr_count,
            counts.product_qua_count,
            raw_payload.to_string(),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(super) fn upsert_category_rule(
    conn: &Connection,
    shop_id: &str,
    cat_id: i64,
    rule_type: &str,
    raw_payload: &Value,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO wechat_category_rules (shop_id, cat_id, rule_type, raw_payload, synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(shop_id, cat_id, rule_type) DO UPDATE SET
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at",
        params![
            shop_id,
            cat_id,
            rule_type,
            raw_payload.to_string(),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(super) fn upsert_category_precheck_result(
    conn: &Connection,
    shop_id: &str,
    cat_id: i64,
    all_pass: bool,
    fail_reasons: &[String],
    raw_payload: &Value,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO wechat_category_prechecks
         (shop_id, cat_id, all_pass, fail_reasons, raw_payload, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(shop_id, cat_id) DO UPDATE SET
           all_pass = excluded.all_pass,
           fail_reasons = excluded.fail_reasons,
           raw_payload = excluded.raw_payload,
           checked_at = excluded.checked_at",
        params![
            shop_id,
            cat_id,
            if all_pass { 1 } else { 0 },
            serde_json::to_string(fail_reasons).unwrap_or_else(|_| "[]".to_string()),
            raw_payload.to_string(),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(super) fn check_cached_category_requirements(
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

pub(super) fn load_cached_category_detail_payload(
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

pub(super) fn required_category_attrs(raw_payload: &Value, list_key: &str) -> BTreeSet<String> {
    let mut attrs = BTreeSet::new();
    collect_required_category_attrs(raw_payload, list_key, &mut attrs);
    attrs
}

pub(super) fn collect_required_category_attrs(
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

pub(super) fn is_required_category_attr(object: &serde_json::Map<String, Value>) -> bool {
    ["is_required", "required", "mandatory", "is_mandatory"]
        .iter()
        .any(|key| json_value_to_bool(object.get(*key)).unwrap_or(false))
        || object
            .get("required_rule")
            .and_then(Value::as_object)
            .and_then(|rule| json_value_to_i64(rule.get("rule_type")))
            == Some(1)
}

pub(super) fn category_attr_name(object: &serde_json::Map<String, Value>) -> Option<String> {
    ["name", "attr_key", "attr_name", "key"]
        .iter()
        .find_map(|key| json_value_to_string(object.get(*key)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn payload_product_attr_keys(payload: &Value) -> BTreeSet<String> {
    payload
        .get("attrs")
        .and_then(Value::as_array)
        .map(|items| payload_attr_keys_from_array(items))
        .unwrap_or_default()
}

pub(super) fn payload_sale_attr_keys(payload: &Value) -> BTreeSet<String> {
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

pub(super) fn payload_attr_keys_from_array(items: &[Value]) -> BTreeSet<String> {
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

pub(super) fn build_attribute_fill_plan(
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

pub(super) fn required_category_attr_specs(
    raw_payload: &Value,
    list_key: &str,
) -> BTreeMap<String, CategoryRequiredAttr> {
    let mut specs = BTreeMap::new();
    collect_required_category_attr_specs(raw_payload, list_key, &mut specs);
    specs
}

pub(super) fn collect_required_category_attr_specs(
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

pub(super) fn category_attr_options(object: &serde_json::Map<String, Value>) -> Vec<String> {
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

pub(super) fn category_option_value(value: &Value) -> Option<String> {
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

pub(super) fn build_product_attr_suggestion(
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

pub(super) fn build_sale_attr_suggestion(
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

pub(super) fn metadata_attr_suggestion(
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

pub(super) fn infer_product_attr_value(
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

pub(super) fn normalize_suggested_value(value: &str, options: &[String]) -> String {
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

pub(super) fn infer_sale_attr_values_from_payload(
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

pub(super) fn is_sale_attr_synonym(existing_key: &str, required_key: &str) -> bool {
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

pub(super) fn build_attribute_prompt_json(
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

pub(super) fn apply_attribute_fill_plan_to_payload(
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

pub(super) fn ensure_payload_product_attr(
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

pub(super) fn ensure_payload_sku_attrs(
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

pub(super) fn ensure_payload_sku_attr_for_all(
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

pub(super) fn ensure_single_sku_attr(
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

pub(super) fn persist_filled_add_product_payload(
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

pub(super) fn upsert_publish_attribute_suggestion(
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

pub(super) fn attribute_suggestions_json(suggestions: &[AttributeFillSuggestion]) -> Value {
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

pub(super) fn sku_values_json(values: &[SkuAttrFill]) -> String {
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

pub(super) fn parse_sku_attr_fill_values(raw: Option<&str>) -> AppResult<Vec<SkuAttrFill>> {
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

pub(super) fn normalize_attribute_suggestion_status(
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

pub(super) fn count_attribute_suggestions_by_status(
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

pub(super) fn load_publish_attribute_suggestion_views(
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

pub(super) fn attribute_suggestion_allowed_values(prompt_json: &Value) -> Vec<String> {
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

pub(super) fn attribute_suggestion_reason(prompt_json: &Value) -> Option<String> {
    prompt_json
        .get("reason")
        .or_else(|| prompt_json.pointer("/response/reason"))
        .and_then(|value| json_value_to_string(Some(value)))
        .map(|value| truncate_for_summary(&value, 120))
}

pub(super) fn load_attribute_suggestion_for_apply(
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

pub(super) fn insert_success_api_and_task_log(
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

pub(super) fn insert_api_error_and_task_log(
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

pub(super) fn extract_wechat_categories(raw_payload: &Value) -> Vec<CachedWechatCategory> {
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

pub(super) fn collect_wechat_categories(value: &Value, categories: &mut Vec<CachedWechatCategory>) {
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

pub(super) fn category_from_object(
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

pub(super) fn extract_freight_template_ids(raw_payload: &Value) -> Vec<String> {
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

pub(super) fn category_detail_counts(raw_payload: &Value) -> CategoryDetailCounts {
    CategoryDetailCounts {
        product_attr_count: count_named_arrays(raw_payload, "product_attr_list"),
        sale_attr_count: count_named_arrays(raw_payload, "sale_attr_list"),
        product_qua_count: count_named_arrays(raw_payload, "product_qua_list"),
    }
}

pub(super) fn count_named_arrays(value: &Value, key: &str) -> i64 {
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

pub(super) fn load_pending_publish_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.product_row_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           p.external_product_id,
           p.raw_payload
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'pending'
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('pending', 'queued', 'running', 'partial_success')
         ORDER BY i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPublishItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                product_row_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_status: row.get(4)?,
                shop_has_secret: row.get::<_, i64>(5)? == 1,
                external_product_id: row.get(6)?,
                raw_payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_category_precheck_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.product_row_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           p.external_product_id,
           p.raw_payload
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'ready_to_publish'
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('ready_to_publish', 'partial_success', 'running')
         ORDER BY i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPublishItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                product_row_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_status: row.get(4)?,
                shop_has_secret: row.get::<_, i64>(5)? == 1,
                external_product_id: row.get(6)?,
                raw_payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_attribute_fill_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.product_row_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           p.external_product_id,
           p.raw_payload
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'failed'
           AND i.error_code = 'CATEGORY_ATTRS_NEED_AI_FILL'
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('failed', 'partial_success', 'running', 'ready_to_publish')
         ORDER BY i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPublishItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                product_row_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_status: row.get(4)?,
                shop_has_secret: row.get::<_, i64>(5)? == 1,
                external_product_id: row.get(6)?,
                raw_payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_pending_price_update_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPriceUpdateItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           i.external_product_id,
           i.wechat_product_id,
           i.target_price_cents
         FROM price_update_items i
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'pending'
           AND t.task_type = 'price.create_update_job'
           AND t.status IN ('pending', 'queued', 'running', 'partial_success')
         ORDER BY i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPriceUpdateItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                shop_id: row.get(2)?,
                shop_status: row.get(3)?,
                shop_has_secret: row.get::<_, i64>(4)? == 1,
                external_product_id: row.get(5)?,
                wechat_product_id: row.get(6)?,
                target_price_cents: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_ready_price_update_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPriceUpdateItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           i.external_product_id,
           i.wechat_product_id,
           i.target_price_cents
         FROM price_update_items i
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'ready_to_update'
           AND t.task_type = 'price.create_update_job'
           AND t.status IN ('ready_to_update', 'running', 'partial_success')
         ORDER BY i.updated_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPriceUpdateItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                shop_id: row.get(2)?,
                shop_status: row.get(3)?,
                shop_has_secret: row.get::<_, i64>(4)? == 1,
                external_product_id: row.get(5)?,
                wechat_product_id: row.get(6)?,
                target_price_cents: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_confirmable_price_update_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPriceUpdateItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           i.external_product_id,
           i.wechat_product_id,
           i.target_price_cents
         FROM price_update_items i
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status IN ('submitted', 'audit_pending')
           AND t.task_type = 'price.create_update_job'
           AND t.status IN ('submitted', 'audit_pending', 'running', 'partial_success')
         ORDER BY i.updated_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPriceUpdateItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                shop_id: row.get(2)?,
                shop_status: row.get(3)?,
                shop_has_secret: row.get::<_, i64>(4)? == 1,
                external_product_id: row.get(5)?,
                wechat_product_id: row.get(6)?,
                target_price_cents: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_asset_upload_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.product_row_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           p.external_product_id,
           p.raw_payload
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status IN ('ready_to_publish', 'category_prechecked')
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('ready_to_publish', 'partial_success', 'running')
         ORDER BY i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPublishItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                product_row_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_status: row.get(4)?,
                shop_has_secret: row.get::<_, i64>(5)? == 1,
                external_product_id: row.get(6)?,
                raw_payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_product_submit_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.product_row_id,
           i.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           p.external_product_id,
           p.raw_payload
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN shops s ON s.id = i.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'assets_ready'
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('assets_ready', 'partial_success', 'running')
         ORDER BY i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PendingPublishItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                product_row_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_status: row.get(4)?,
                shop_has_secret: row.get::<_, i64>(5)? == 1,
                external_product_id: row.get(6)?,
                raw_payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_product_status_sync_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<StatusSyncItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.shop_id,
           p.external_product_id,
           i.wechat_product_id
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status IN ('submitted', 'audit_pending')
           AND i.wechat_product_id IS NOT NULL
           AND i.wechat_product_id != ''
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('submitted', 'audit_pending', 'running', 'partial_success')
         ORDER BY i.last_status_sync_at IS NOT NULL ASC, i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(StatusSyncItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                shop_id: row.get(2)?,
                external_product_id: row.get(3)?,
                wechat_product_id: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_product_listing_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<StatusSyncItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           i.id,
           i.job_id,
           i.shop_id,
           p.external_product_id,
           i.wechat_product_id
         FROM publish_job_items i
         JOIN publish_products p ON p.id = i.product_row_id
         JOIN task_runs t ON t.id = i.job_id
         WHERE i.status = 'audit_passed'
           AND i.wechat_product_id IS NOT NULL
           AND i.wechat_product_id != ''
           AND t.task_type = 'publish.create_external_job'
           AND t.status IN ('audit_passed', 'running', 'partial_success')
         ORDER BY i.last_status_sync_at ASC, i.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(StatusSyncItem {
                item_id: row.get(0)?,
                job_id: row.get(1)?,
                shop_id: row.get(2)?,
                external_product_id: row.get(3)?,
                wechat_product_id: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn load_order_sync_shops(conn: &Connection) -> AppResult<Vec<OrderSyncShop>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name
         FROM shops s
         JOIN shop_credentials c ON c.shop_id = s.id
         WHERE s.status = 'active'
         ORDER BY s.created_at ASC",
    )?;
    let shops = stmt
        .query_map([], |row| {
            Ok(OrderSyncShop {
                shop_id: row.get(0)?,
                shop_name: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(shops)
}

pub(super) fn load_order_detail_sync_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<OrderDetailSyncItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, wechat_order_id
         FROM orders
         WHERE wechat_order_id IS NOT NULL
           AND shop_id IS NOT NULL
           AND status IN ('pending_shipment', 'synced', 'pending_purchase')
           AND (detail_synced_at IS NULL OR detail_synced_at = '')
         ORDER BY synced_at ASC, created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(OrderDetailSyncItem {
                order_id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_order_id: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn count_aftersales(conn: &Connection, status: Option<&str>) -> AppResult<i64> {
    match status {
        Some("active") => Ok(conn.query_row(
            "SELECT COUNT(*)
             FROM aftersales
             WHERE status NOT IN (
               'USER_CANCELD', 'USER_CANCELLED', 'RETURN_CLOSED',
               'MERCHANT_REFUND_SUCCESS', 'MERCHANT_RETURN_SUCCESS',
               'MERCHANT_REFUND_RETRY_FAIL', 'MERCHANT_FAIL',
               'MERCHANT_EXCHANGE_SUCCESS', 'sync_failed'
             )",
            [],
            |row| row.get(0),
        )?),
        Some(status) => Ok(conn.query_row(
            "SELECT COUNT(*) FROM aftersales WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?),
        None => Ok(conn.query_row("SELECT COUNT(*) FROM aftersales", [], |row| row.get(0))?),
    }
}

pub(super) fn load_aftersale_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AftersaleView>> {
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(AftersaleView {
            id: row.get(0)?,
            shop_id: row.get(1)?,
            shop_name: row.get(2)?,
            order_id: row.get(3)?,
            wechat_order_id: row.get(4)?,
            wechat_aftersale_id: row.get(5)?,
            status: row.get(6)?,
            aftersale_type: row.get(7)?,
            reason: row.get(8)?,
            refund_amount_cents: row.get(9)?,
            responsibility_party: row.get(10)?,
            responsibility_note: row.get(11)?,
            supplier_compensation_cents: row.get(12)?,
            handled_at: row.get(13)?,
            last_action: row.get(14)?,
            last_action_status: row.get(15)?,
            last_action_error: row.get(16)?,
            last_action_note: row.get(17)?,
            last_action_at: row.get(18)?,
            evidence_count: row.get(19)?,
            synced_at: row.get(20)?,
            updated_at: row.get(21)?,
        })
    };
    let base_select = "SELECT
           a.id,
           a.shop_id,
           COALESCE(s.name, a.shop_id),
           a.order_id,
           a.wechat_order_id,
           a.wechat_aftersale_id,
           a.status,
           a.aftersale_type,
           a.reason,
           a.refund_amount_cents,
           a.responsibility_party,
           a.responsibility_note,
           COALESCE(a.supplier_compensation_cents, 0),
           a.handled_at,
           a.last_action,
           a.last_action_status,
           a.last_action_error,
           a.last_action_note,
           a.last_action_at,
           (
             SELECT COUNT(*)
             FROM aftersale_evidence_records e
             WHERE e.target_type = 'aftersale'
               AND (e.target_id = a.id OR e.external_target_id = a.wechat_aftersale_id)
           ),
           a.synced_at,
           a.updated_at
         FROM aftersales a
         LEFT JOIN shops s ON s.id = a.shop_id";
    match status {
        Some("active") => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE a.status NOT IN (
                   'USER_CANCELD', 'USER_CANCELLED', 'RETURN_CLOSED',
                   'MERCHANT_REFUND_SUCCESS', 'MERCHANT_RETURN_SUCCESS',
                   'MERCHANT_REFUND_RETRY_FAIL', 'MERCHANT_FAIL',
                   'MERCHANT_EXCHANGE_SUCCESS', 'sync_failed'
                 )
                 ORDER BY a.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        Some(status) => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE a.status = ?1
                 ORDER BY a.updated_at DESC
                 LIMIT ?2"
            ))?;
            let rows = stmt
                .query_map(params![status, limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 ORDER BY a.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

pub(super) fn count_aftersale_evidence(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM aftersale_evidence_records
         WHERE (?1 IS NULL OR target_type = ?1)
           AND (?2 IS NULL OR target_id = ?2 OR external_target_id = ?2)
           AND (?3 IS NULL OR status = ?3)",
        params![target_type, target_id, status],
        |row| row.get(0),
    )?)
}

pub(super) fn load_aftersale_evidence_views(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AftersaleEvidenceView>> {
    let mut stmt = conn.prepare(
        "SELECT
           e.id,
           e.target_type,
           e.target_id,
           e.shop_id,
           COALESCE(s.name, e.shop_id),
           e.external_target_id,
           e.evidence_type,
           e.title,
           e.content_text,
           e.local_file_path,
           e.source_url,
           e.status,
           e.created_at,
           e.updated_at
         FROM aftersale_evidence_records e
         LEFT JOIN shops s ON s.id = e.shop_id
         WHERE (?1 IS NULL OR e.target_type = ?1)
           AND (?2 IS NULL OR e.target_id = ?2 OR e.external_target_id = ?2)
           AND (?3 IS NULL OR e.status = ?3)
         ORDER BY e.updated_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(params![target_type, target_id, status, limit], |row| {
            let evidence_type: String = row.get(6)?;
            let status: String = row.get(11)?;
            Ok(AftersaleEvidenceView {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_name: row.get(4)?,
                external_target_id: row.get(5)?,
                evidence_type: evidence_type.clone(),
                evidence_type_text: aftersale_evidence_type_text(&evidence_type).to_string(),
                title: row.get(7)?,
                content_text: row.get(8)?,
                local_file_path: row.get(9)?,
                source_url: row.get(10)?,
                status: status.clone(),
                status_text: aftersale_evidence_status_text(&status).to_string(),
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn count_supplier_aftersale_followups(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM supplier_aftersale_followups
         WHERE (?1 IS NULL OR target_type = ?1)
           AND (?2 IS NULL OR target_id = ?2 OR external_target_id = ?2)
           AND (?3 IS NULL OR status = ?3)",
        params![target_type, target_id, status],
        |row| row.get(0),
    )?)
}

pub(super) fn load_supplier_aftersale_followup_views(
    conn: &Connection,
    target_type: Option<&str>,
    target_id: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<SupplierAftersaleFollowupView>> {
    let mut stmt = conn.prepare(
        "SELECT
           f.id,
           f.target_type,
           f.target_id,
           f.shop_id,
           COALESCE(s.name, f.shop_id),
           f.external_target_id,
           f.purchase_task_id,
           f.supplier_name,
           f.followup_type,
           f.status,
           f.note,
           f.created_at,
           f.updated_at
         FROM supplier_aftersale_followups f
         LEFT JOIN shops s ON s.id = f.shop_id
         WHERE (?1 IS NULL OR f.target_type = ?1)
           AND (?2 IS NULL OR f.target_id = ?2 OR f.external_target_id = ?2)
           AND (?3 IS NULL OR f.status = ?3)
         ORDER BY f.updated_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt
        .query_map(params![target_type, target_id, status, limit], |row| {
            let followup_type: String = row.get(8)?;
            let status: String = row.get(9)?;
            Ok(SupplierAftersaleFollowupView {
                id: row.get(0)?,
                target_type: row.get(1)?,
                target_id: row.get(2)?,
                shop_id: row.get(3)?,
                shop_name: row.get(4)?,
                external_target_id: row.get(5)?,
                purchase_task_id: row.get(6)?,
                supplier_name: row.get(7)?,
                followup_type: followup_type.clone(),
                followup_type_text: supplier_aftersale_followup_type_text(&followup_type)
                    .to_string(),
                status: status.clone(),
                status_text: supplier_aftersale_followup_status_text(&status).to_string(),
                note: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn count_guarantee_orders(conn: &Connection, status: Option<&str>) -> AppResult<i64> {
    match status {
        Some("active") => Ok(conn.query_row(
            "SELECT COUNT(*)
             FROM guarantee_orders
             WHERE status NOT IN (
               'STATUS_NO_NEED_PAY', 'STATUS_PAY_SUCC', 'STATUS_USER_CANCEL', 'sync_failed'
             )",
            [],
            |row| row.get(0),
        )?),
        Some(status) => Ok(conn.query_row(
            "SELECT COUNT(*) FROM guarantee_orders WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?),
        None => Ok(
            conn.query_row("SELECT COUNT(*) FROM guarantee_orders", [], |row| {
                row.get(0)
            })?,
        ),
    }
}

pub(super) fn load_guarantee_order_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<GuaranteeOrderView>> {
    let mapper = |row: &rusqlite::Row<'_>| {
        let guarantee_type: Option<i64> = row.get(6)?;
        let status: String = row.get(7)?;
        Ok(GuaranteeOrderView {
            id: row.get(0)?,
            shop_id: row.get(1)?,
            shop_name: row.get(2)?,
            order_id: row.get(3)?,
            wechat_order_id: row.get(4)?,
            guarantee_order_id: row.get(5)?,
            guarantee_type,
            guarantee_type_text: guarantee_type_text(guarantee_type).to_string(),
            status: status.clone(),
            status_text: guarantee_status_text(&status).to_string(),
            apply_reason: row.get(8)?,
            pay_amount_cents: row.get(9)?,
            merchant_refuse_reason: row.get(10)?,
            handling_status: row.get(11)?,
            handling_note: row.get(12)?,
            responsibility_party: row.get(13)?,
            supplier_compensation_cents: row.get(14)?,
            handled_at: row.get(15)?,
            created_time: row.get(16)?,
            updated_time_unix: row.get(17)?,
            expire_time: row.get(18)?,
            complete_time: row.get(19)?,
            evidence_count: row.get(20)?,
            synced_at: row.get(21)?,
            updated_at: row.get(22)?,
        })
    };
    let base_select = "SELECT
           g.id,
           g.shop_id,
           COALESCE(s.name, g.shop_id),
           g.order_id,
           g.wechat_order_id,
           g.guarantee_order_id,
           g.guarantee_type,
           g.status,
           g.apply_reason,
           g.pay_amount_cents,
           g.merchant_refuse_reason,
           g.handling_status,
           g.handling_note,
           g.responsibility_party,
           COALESCE(g.supplier_compensation_cents, 0),
           g.handled_at,
           g.created_time,
           g.updated_time,
           g.expire_time,
           g.complete_time,
           (
             SELECT COUNT(*)
             FROM aftersale_evidence_records e
             WHERE e.target_type = 'guarantee'
               AND (e.target_id = g.id OR e.external_target_id = g.guarantee_order_id)
           ),
           g.synced_at,
           g.updated_at
         FROM guarantee_orders g
         LEFT JOIN shops s ON s.id = g.shop_id";
    match status {
        Some("active") => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE g.status NOT IN (
                   'STATUS_NO_NEED_PAY', 'STATUS_PAY_SUCC', 'STATUS_USER_CANCEL', 'sync_failed'
                 )
                 ORDER BY g.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        Some(status) => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 WHERE g.status = ?1
                 ORDER BY g.updated_at DESC
                 LIMIT ?2"
            ))?;
            let rows = stmt
                .query_map(params![status, limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "{base_select}
                 ORDER BY g.updated_at DESC
                 LIMIT ?1"
            ))?;
            let rows = stmt
                .query_map([limit], mapper)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

pub(super) fn load_purchase_candidates(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PurchaseCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT
           oi.order_id,
           oi.id,
           oi.shop_id,
           oi.out_product_id,
           oi.out_sku_id,
           COALESCE(
             sp.source_url,
             (
               SELECT pp.source_url
               FROM publish_products pp
               WHERE pp.external_product_id = oi.out_product_id
               ORDER BY pp.created_at DESC
               LIMIT 1
             )
           ),
           oi.sku_count
         FROM order_items oi
         JOIN orders o ON o.id = oi.order_id
         LEFT JOIN purchase_tasks pt ON pt.order_item_id = oi.id
         LEFT JOIN shop_products sp
           ON sp.shop_id = oi.shop_id
          AND sp.external_product_id = oi.out_product_id
         WHERE pt.id IS NULL
           AND o.status IN ('pending_shipment', 'pending_purchase', 'synced')
         ORDER BY oi.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(PurchaseCandidate {
                order_id: row.get(0)?,
                order_item_id: row.get(1)?,
                shop_id: row.get(2)?,
                out_product_id: row.get(3)?,
                out_sku_id: row.get(4)?,
                source_url: row.get(5)?,
                quantity: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn count_purchase_tasks(conn: &Connection, status: Option<&str>) -> AppResult<i64> {
    if let Some(status) = status {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM purchase_tasks WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?)
    } else {
        Ok(conn.query_row("SELECT COUNT(*) FROM purchase_tasks", [], |row| row.get(0))?)
    }
}

pub(super) fn load_purchase_task_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<PurchaseTaskView>> {
    let sql = format!(
        "SELECT
           pt.id,
           pt.order_id,
           COALESCE(o.wechat_order_id, ''),
           pt.shop_id,
           COALESCE(s.name, pt.shop_id),
           pt.status,
           pt.external_product_id,
           pt.external_sku_id,
           pt.source_url,
           oi.title,
           pt.quantity,
           oi.sale_price,
           oi.real_price,
           COALESCE(oi.real_price, oi.sale_price * pt.quantity),
           pt.estimated_cost,
           CASE
             WHEN pt.estimated_cost IS NULL THEN NULL
             ELSE (COALESCE(oi.real_price, oi.sale_price * pt.quantity) / 100.0) - pt.estimated_cost
           END,
           pt.supplier_name,
           pt.supplier_product_id,
           pt.supplier_delivery_id,
           pt.supplier_delivery_name,
           pt.supplier_waybill_id,
           pt.supplier_deliver_type,
           pt.supplier_shipped_at,
           pt.error_summary,
           pt.created_at,
           pt.updated_at
         FROM purchase_tasks pt
         JOIN orders o ON o.id = pt.order_id
         JOIN order_items oi ON oi.id = pt.order_item_id
         LEFT JOIN shops s ON s.id = pt.shop_id
         {}
         ORDER BY pt.created_at DESC
         LIMIT ?{}",
        if status.is_some() {
            "WHERE pt.status = ?1"
        } else {
            ""
        },
        if status.is_some() { 2 } else { 1 }
    );
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(PurchaseTaskView {
            id: row.get(0)?,
            order_id: row.get(1)?,
            wechat_order_id: row.get(2)?,
            shop_id: row.get(3)?,
            shop_name: row.get(4)?,
            status: row.get(5)?,
            external_product_id: row.get(6)?,
            external_sku_id: row.get(7)?,
            source_url: row.get(8)?,
            title: row.get(9)?,
            quantity: row.get(10)?,
            sale_price: row.get(11)?,
            real_price: row.get(12)?,
            estimated_revenue: row.get(13)?,
            estimated_cost: row.get(14)?,
            estimated_profit: row.get(15)?,
            supplier_name: row.get(16)?,
            supplier_product_id: row.get(17)?,
            supplier_delivery_id: row.get(18)?,
            supplier_delivery_name: row.get(19)?,
            supplier_waybill_id: row.get(20)?,
            supplier_deliver_type: row.get(21)?,
            supplier_shipped_at: row.get(22)?,
            error_summary: row.get(23)?,
            created_at: row.get(24)?,
            updated_at: row.get(25)?,
        })
    };
    let rows = if let Some(status) = status {
        stmt.query_map(params![status, limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub(super) fn load_order_profit_views(conn: &Connection) -> AppResult<Vec<OrderProfitView>> {
    let mut stmt = conn.prepare(
        "WITH item_totals AS (
           SELECT
             order_id,
             COUNT(*) AS item_count,
             COALESCE(SUM(sku_count), 0) AS quantity,
             COALESCE(SUM(COALESCE(real_price, sale_price * sku_count, 0)), 0) AS revenue_cents
           FROM order_items
           GROUP BY order_id
         ),
         purchase_totals AS (
           SELECT
             order_id,
             COUNT(*) AS purchase_task_count,
             COALESCE(SUM(CASE WHEN estimated_cost IS NULL THEN 1 ELSE 0 END), 0) AS missing_cost_count,
             COALESCE(SUM(CASE
               WHEN estimated_cost IS NULL THEN 0
               ELSE CAST(ROUND(estimated_cost * 100) AS INTEGER)
             END), 0) AS purchase_cost_cents
           FROM purchase_tasks
           GROUP BY order_id
         ),
         adjustment_totals AS (
           SELECT
             order_id,
             COALESCE(SUM(CASE WHEN kind = 'purchase_freight' THEN amount_cents ELSE 0 END), 0) AS purchase_freight_cents,
             COALESCE(SUM(CASE WHEN kind = 'refund' THEN amount_cents ELSE 0 END), 0) AS refund_cents,
             COALESCE(SUM(CASE WHEN kind = 'aftersale_compensation' THEN amount_cents ELSE 0 END), 0) AS aftersale_compensation_cents,
             COALESCE(SUM(CASE WHEN kind = 'other_cost' THEN amount_cents ELSE 0 END), 0) AS other_cost_cents,
             COALESCE(SUM(CASE WHEN kind = 'other_income' THEN amount_cents ELSE 0 END), 0) AS other_income_cents
           FROM order_profit_adjustments
           GROUP BY order_id
         )
         SELECT
           o.id,
           COALESCE(o.wechat_order_id, ''),
           COALESCE(o.shop_id, ''),
           COALESCE(s.name, o.shop_id, ''),
           o.status,
           COALESCE(it.item_count, 0),
           COALESCE(it.quantity, 0),
           COALESCE(it.revenue_cents, 0),
           COALESCE(pt.purchase_task_count, 0),
           COALESCE(pt.missing_cost_count, 0),
           COALESCE(pt.purchase_cost_cents, 0),
           COALESCE(at.purchase_freight_cents, 0),
           COALESCE(at.refund_cents, 0),
           COALESCE(at.aftersale_compensation_cents, 0),
           COALESCE(at.other_cost_cents, 0),
           COALESCE(at.other_income_cents, 0),
           o.detail_synced_at,
           o.updated_at
         FROM orders o
         LEFT JOIN shops s ON s.id = o.shop_id
         LEFT JOIN item_totals it ON it.order_id = o.id
         LEFT JOIN purchase_totals pt ON pt.order_id = o.id
         LEFT JOIN adjustment_totals at ON at.order_id = o.id
         ORDER BY COALESCE(o.updated_at, o.created_at) DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            let revenue_cents = row.get::<_, i64>(7)?;
            let purchase_task_count = row.get::<_, i64>(8)?;
            let missing_cost_count = row.get::<_, i64>(9)?;
            let purchase_cost_cents = row.get::<_, i64>(10)?;
            let purchase_freight_cents = row.get::<_, i64>(11)?;
            let refund_cents = row.get::<_, i64>(12)?;
            let aftersale_compensation_cents = row.get::<_, i64>(13)?;
            let other_cost_cents = row.get::<_, i64>(14)?;
            let other_income_cents = row.get::<_, i64>(15)?;
            let estimated_profit_cents = revenue_cents + other_income_cents
                - purchase_cost_cents
                - purchase_freight_cents
                - refund_cents
                - aftersale_compensation_cents
                - other_cost_cents;
            let actual_profit_cents = if purchase_task_count > 0 && missing_cost_count == 0 {
                Some(estimated_profit_cents)
            } else {
                None
            };
            Ok(OrderProfitView {
                order_id: row.get(0)?,
                wechat_order_id: row.get(1)?,
                shop_id: row.get(2)?,
                shop_name: row.get(3)?,
                order_status: row.get(4)?,
                item_count: row.get(5)?,
                quantity: row.get(6)?,
                revenue_cents,
                purchase_task_count,
                missing_cost_count,
                purchase_cost_cents,
                purchase_freight_cents,
                refund_cents,
                aftersale_compensation_cents,
                other_cost_cents,
                other_income_cents,
                estimated_profit_cents,
                actual_profit_cents,
                profit_status: resolve_profit_status(
                    purchase_task_count,
                    missing_cost_count,
                    actual_profit_cents,
                )
                .to_string(),
                detail_synced_at: row.get(16)?,
                updated_at: row.get(17)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn summarize_order_profit(items: &[OrderProfitView]) -> OrderProfitTotals {
    OrderProfitTotals {
        order_count: items.len() as i64,
        revenue_cents: items.iter().map(|item| item.revenue_cents).sum(),
        purchase_cost_cents: items.iter().map(|item| item.purchase_cost_cents).sum(),
        purchase_freight_cents: items.iter().map(|item| item.purchase_freight_cents).sum(),
        refund_cents: items.iter().map(|item| item.refund_cents).sum(),
        aftersale_compensation_cents: items
            .iter()
            .map(|item| item.aftersale_compensation_cents)
            .sum(),
        other_cost_cents: items.iter().map(|item| item.other_cost_cents).sum(),
        other_income_cents: items.iter().map(|item| item.other_income_cents).sum(),
        estimated_profit_cents: items.iter().map(|item| item.estimated_profit_cents).sum(),
        actual_profit_cents: items
            .iter()
            .filter_map(|item| item.actual_profit_cents)
            .sum(),
        unknown_actual_order_count: items
            .iter()
            .filter(|item| item.actual_profit_cents.is_none())
            .count() as i64,
    }
}

pub(super) fn resolve_profit_status(
    purchase_task_count: i64,
    missing_cost_count: i64,
    actual_profit_cents: Option<i64>,
) -> &'static str {
    if purchase_task_count == 0 {
        "missing_purchase_task"
    } else if missing_cost_count > 0 {
        "missing_cost"
    } else if actual_profit_cents.unwrap_or_default() < 0 {
        "loss"
    } else {
        "profitable"
    }
}

pub(super) fn normalize_profit_adjustment_kind(kind: &str) -> AppResult<&'static str> {
    match kind.trim() {
        "purchase_freight" => Ok("purchase_freight"),
        "refund" => Ok("refund"),
        "aftersale_compensation" => Ok("aftersale_compensation"),
        "other_cost" => Ok("other_cost"),
        "other_income" => Ok("other_income"),
        _ => Err(AppError::Validation(
            "调整类型必须是 purchase_freight/refund/aftersale_compensation/other_cost/other_income"
                .to_string(),
        )),
    }
}

pub(super) fn normalize_aftersale_responsibility_party(party: &str) -> AppResult<&'static str> {
    match party.trim() {
        "supplier" => Ok("supplier"),
        "merchant" => Ok("merchant"),
        "customer" => Ok("customer"),
        "platform" => Ok("platform"),
        "unknown" => Ok("unknown"),
        _ => Err(AppError::Validation(
            "责任方必须是 supplier/merchant/customer/platform/unknown".to_string(),
        )),
    }
}

pub(super) fn normalize_aftersale_evidence_target_type(
    target_type: &str,
) -> AppResult<&'static str> {
    match target_type.trim() {
        "aftersale" => Ok("aftersale"),
        "guarantee" => Ok("guarantee"),
        _ => Err(AppError::Validation(
            "凭证目标类型必须是 aftersale 或 guarantee".to_string(),
        )),
    }
}

pub(super) fn normalize_aftersale_evidence_type(evidence_type: &str) -> AppResult<&'static str> {
    match evidence_type.trim() {
        "image" => Ok("image"),
        "text" => Ok("text"),
        "chat_record" => Ok("chat_record"),
        "logistics" => Ok("logistics"),
        "supplier_proof" => Ok("supplier_proof"),
        "quality_check" => Ok("quality_check"),
        "other" => Ok("other"),
        _ => Err(AppError::Validation(
            "凭证类型必须是 image/text/chat_record/logistics/supplier_proof/quality_check/other"
                .to_string(),
        )),
    }
}

pub(super) fn normalize_aftersale_evidence_status(status: &str) -> AppResult<&'static str> {
    match status.trim() {
        "draft" => Ok("draft"),
        "ready" => Ok("ready"),
        "used" => Ok("used"),
        "archived" => Ok("archived"),
        _ => Err(AppError::Validation(
            "凭证状态必须是 draft/ready/used/archived".to_string(),
        )),
    }
}

pub(super) fn normalize_supplier_aftersale_followup_type(
    followup_type: &str,
) -> AppResult<&'static str> {
    match followup_type.trim() {
        "contact" => Ok("contact"),
        "evidence_request" => Ok("evidence_request"),
        "evidence_received" => Ok("evidence_received"),
        "compensation" => Ok("compensation"),
        "return_refund" => Ok("return_refund"),
        "other" => Ok("other"),
        _ => Err(AppError::Validation(
            "供应商协同类型必须是 contact/evidence_request/evidence_received/compensation/return_refund/other"
                .to_string(),
        )),
    }
}

pub(super) fn normalize_supplier_aftersale_followup_status(
    status: &str,
) -> AppResult<&'static str> {
    match status.trim() {
        "pending" => Ok("pending"),
        "contacted" => Ok("contacted"),
        "waiting_supplier" => Ok("waiting_supplier"),
        "evidence_ready" => Ok("evidence_ready"),
        "compensation_pending" => Ok("compensation_pending"),
        "closed" => Ok("closed"),
        _ => Err(AppError::Validation(
            "供应商协同状态必须是 pending/contacted/waiting_supplier/evidence_ready/compensation_pending/closed"
                .to_string(),
        )),
    }
}

pub(super) fn normalize_optional_note(
    note: Option<String>,
    max_len: usize,
    label: &str,
) -> AppResult<Option<String>> {
    let note = note
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if note.as_deref().map(str::len).unwrap_or_default() > max_len {
        return Err(AppError::Validation(format!(
            "{label}不能超过 {max_len} 个字符"
        )));
    }
    Ok(note)
}

pub(super) fn resolve_aftersale_evidence_target(
    conn: &Connection,
    target_type: &str,
    target_id: &str,
) -> AppResult<(String, String, String)> {
    match target_type {
        "aftersale" => conn
            .query_row(
                "SELECT id, shop_id, wechat_aftersale_id
                 FROM aftersales
                 WHERE id = ?1 OR wechat_aftersale_id = ?1
                 LIMIT 1",
                [target_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("售后单不存在".to_string())),
        "guarantee" => conn
            .query_row(
                "SELECT id, shop_id, guarantee_order_id
                 FROM guarantee_orders
                 WHERE id = ?1 OR guarantee_order_id = ?1
                 LIMIT 1",
                [target_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("纠纷单不存在".to_string())),
        _ => Err(AppError::Validation("凭证目标类型不合法".to_string())),
    }
}

pub(super) fn load_aftersale_action_target(
    app: &AppHandle,
    aftersale_id: &str,
) -> AppResult<AftersaleActionTarget> {
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT id, shop_id, wechat_aftersale_id, status
         FROM aftersales
         WHERE id = ?1 OR wechat_aftersale_id = ?1
         LIMIT 1",
        [aftersale_id],
        |row| {
            Ok(AftersaleActionTarget {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_aftersale_id: row.get(2)?,
                status: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Validation("售后单不存在".to_string()))
}

pub(super) fn validate_aftersale_action_status(target: &AftersaleActionTarget) -> AppResult<()> {
    if !is_active_aftersale_status(&target.status) {
        return Err(AppError::Validation(format!(
            "售后单当前状态为 {}，不允许提交同意或拒绝动作",
            target.status
        )));
    }
    Ok(())
}

pub(super) fn handle_aftersale_action_call(
    app: &AppHandle,
    target: AftersaleActionTarget,
    call: WechatRawCall,
    action: &str,
    note: Option<&str>,
) -> AppResult<AftersaleActionResult> {
    let conn = open_connection(app)?;
    match &call.result {
        WechatCallResult::Success(_) => {
            insert_api_call_log(
                &conn,
                Some(&target.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "aftersale {} ok, id={}",
                    action, target.wechat_aftersale_id
                )),
            )?;
            update_aftersale_action_state(&conn, &target.id, action, "success", None, note)?;
            upsert_notification(
                &conn,
                "info",
                "aftersale_action",
                &target.id,
                Some(&target.shop_id),
                &format!("售后{}已提交", aftersale_action_label(action)),
                &format!(
                    "售后单 {} 已提交{}动作，后续通过售后同步确认平台状态。",
                    target.wechat_aftersale_id,
                    aftersale_action_label(action)
                ),
                Some(&serde_json::json!({
                    "aftersale_id": &target.id,
                    "wechat_aftersale_id": &target.wechat_aftersale_id,
                    "action": action,
                    "note": note
                })),
            )?;
            Ok(AftersaleActionResult {
                aftersale_id: target.id,
                wechat_aftersale_id: target.wechat_aftersale_id,
                action: action.to_string(),
                status: "success".to_string(),
                errcode: None,
                errmsg: None,
                message: "售后动作已提交，等待后续同步确认平台状态".to_string(),
            })
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&target.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some(&format!("aftersale {} api error", action)),
            )?;
            update_aftersale_action_state(
                &conn,
                &target.id,
                action,
                "failed",
                Some(&error.errmsg),
                note,
            )?;
            upsert_notification(
                &conn,
                "warning",
                "aftersale_action",
                &target.id,
                Some(&target.shop_id),
                &format!("售后{}失败", aftersale_action_label(action)),
                &format!(
                    "售后单 {} 提交{}失败：{}",
                    target.wechat_aftersale_id,
                    aftersale_action_label(action),
                    error.errmsg
                ),
                Some(&serde_json::json!({
                    "aftersale_id": &target.id,
                    "wechat_aftersale_id": &target.wechat_aftersale_id,
                    "action": action,
                    "errcode": error.errcode
                })),
            )?;
            Ok(AftersaleActionResult {
                aftersale_id: target.id,
                wechat_aftersale_id: target.wechat_aftersale_id,
                action: action.to_string(),
                status: "failed".to_string(),
                errcode: Some(error.errcode),
                errmsg: Some(error.errmsg.clone()),
                message: format!("售后动作提交失败：{}", error.errmsg),
            })
        }
    }
}

pub(super) fn update_aftersale_action_state(
    conn: &Connection,
    aftersale_id: &str,
    action: &str,
    status: &str,
    error: Option<&str>,
    note: Option<&str>,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE aftersales
         SET last_action = ?1,
             last_action_status = ?2,
             last_action_error = ?3,
             last_action_note = ?4,
             last_action_at = ?5,
             updated_at = ?5
         WHERE id = ?6",
        params![action, status, error, note, now, aftersale_id],
    )?;
    Ok(())
}

pub(super) fn aftersale_action_label(action: &str) -> &'static str {
    match action {
        "accept" => "同意",
        "reject" => "拒绝",
        _ => "处理",
    }
}

pub(super) fn normalize_purchase_issue_type(
    issue_type: &str,
) -> AppResult<(&'static str, &'static str)> {
    match issue_type.trim() {
        "out_of_stock" => Ok(("supplier_out_of_stock", "供应商缺货")),
        "price_changed" => Ok(("supplier_price_changed", "供应商涨价")),
        "supplier_cancelled" => Ok(("supplier_cancelled", "供应商取消")),
        "quality_risk" => Ok(("supplier_quality_risk", "供应商质量风险")),
        "other" => Ok(("supplier_exception", "供应商其他异常")),
        _ => Err(AppError::Validation(
            "采购异常类型必须是 out_of_stock/price_changed/supplier_cancelled/quality_risk/other"
                .to_string(),
        )),
    }
}

pub(super) fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "all")
}

pub(super) fn count_notifications(
    conn: &Connection,
    status: Option<&str>,
    severity: Option<&str>,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM notifications
         WHERE (?1 IS NULL OR status = ?1)
           AND (?2 IS NULL OR severity = ?2)",
        params![status, severity],
        |row| row.get(0),
    )?)
}

pub(super) fn load_notification_views(
    conn: &Connection,
    status: Option<&str>,
    severity: Option<&str>,
    limit: i64,
) -> AppResult<Vec<NotificationView>> {
    let mut stmt = conn.prepare(
        "SELECT
           n.id,
           n.severity,
           n.source_type,
           n.source_id,
           n.shop_id,
           s.name,
           n.title,
           n.body,
           n.status,
           n.data_json,
           n.read_at,
           n.created_at,
           n.updated_at
         FROM notifications n
         LEFT JOIN shops s ON s.id = n.shop_id
         WHERE (?1 IS NULL OR n.status = ?1)
           AND (?2 IS NULL OR n.severity = ?2)
         ORDER BY
           CASE n.status WHEN 'unread' THEN 0 ELSE 1 END,
           CASE n.severity WHEN 'critical' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END,
           n.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt
        .query_map(params![status, severity, limit], |row| {
            Ok(NotificationView {
                id: row.get(0)?,
                severity: row.get(1)?,
                source_type: row.get(2)?,
                source_id: row.get(3)?,
                shop_id: row.get(4)?,
                shop_name: row.get(5)?,
                title: row.get(6)?,
                body: row.get(7)?,
                status: row.get(8)?,
                data_json: row.get(9)?,
                read_at: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn upsert_notification(
    conn: &Connection,
    severity: &str,
    source_type: &str,
    source_id: &str,
    shop_id: Option<&str>,
    title: &str,
    body: &str,
    data: Option<&Value>,
) -> AppResult<()> {
    let dedupe_key = format!("{source_type}:{source_id}");
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO notifications
         (id, severity, source_type, source_id, shop_id, title, body, status,
          dedupe_key, data_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'unread', ?8, ?9, ?10, ?10)
         ON CONFLICT(dedupe_key) DO UPDATE SET
           severity = excluded.severity,
           shop_id = excluded.shop_id,
           title = excluded.title,
           body = excluded.body,
           status = CASE
             WHEN notifications.status = 'read' AND notifications.body = excluded.body THEN notifications.status
             ELSE 'unread'
           END,
           read_at = CASE
             WHEN notifications.status = 'read' AND notifications.body = excluded.body THEN notifications.read_at
             ELSE NULL
           END,
           data_json = excluded.data_json,
           updated_at = excluded.updated_at",
        params![
            format!("notification-{}", Uuid::new_v4()),
            severity,
            source_type,
            source_id,
            shop_id,
            title,
            body,
            dedupe_key,
            data.map(|value| value.to_string()),
            now
        ],
    )?;
    Ok(())
}

pub(super) fn load_delivery_company_views(
    conn: &Connection,
    shop_id: Option<&str>,
) -> AppResult<Vec<DeliveryCompanyView>> {
    let sql = if shop_id.is_some() {
        "SELECT shop_id, delivery_id, delivery_name, synced_at
         FROM delivery_companies
         WHERE shop_id = ?1
         ORDER BY delivery_name ASC, delivery_id ASC"
    } else {
        "SELECT shop_id, delivery_id, delivery_name, synced_at
         FROM delivery_companies
         ORDER BY delivery_name ASC, delivery_id ASC"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(shop_id) = shop_id {
        stmt.query_map([shop_id], delivery_company_from_row)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], delivery_company_from_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub(super) fn delivery_company_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DeliveryCompanyView> {
    Ok(DeliveryCompanyView {
        shop_id: row.get(0)?,
        delivery_id: row.get(1)?,
        delivery_name: row.get(2)?,
        synced_at: row.get(3)?,
    })
}

pub(super) fn load_aftersale_reject_reason_views(
    conn: &Connection,
    shop_id: Option<&str>,
    reject_scene: Option<i64>,
) -> AppResult<Vec<AftersaleRejectReasonView>> {
    let mut sql = "SELECT shop_id,
                          reject_reason_type,
                          reject_reason_type_text,
                          reject_reason,
                          reject_scene,
                          synced_at
                   FROM aftersale_reject_reasons"
        .to_string();
    let mut conditions = Vec::new();
    let mut params_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(shop_id) = shop_id {
        conditions.push("shop_id = ?".to_string());
        params_values.push(Box::new(shop_id.to_string()));
    }
    if let Some(reject_scene) = reject_scene {
        conditions.push("reject_scene = ?".to_string());
        params_values.push(Box::new(reject_scene));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY shop_id ASC, reject_scene ASC, reject_reason_type ASC");

    let params_ref = params_values
        .iter()
        .map(|value| value.as_ref() as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_ref.as_slice(), |row| {
            let reject_scene: Option<i64> = row.get(4)?;
            Ok(AftersaleRejectReasonView {
                shop_id: row.get(0)?,
                reject_reason_type: row.get(1)?,
                reject_reason_type_text: row.get(2)?,
                reject_reason: row.get(3)?,
                reject_scene,
                reject_scene_text: reject_scene_text(reject_scene).to_string(),
                synced_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(super) fn upsert_aftersale_reject_reasons(
    conn: &mut Connection,
    shop_id: &str,
    raw_payload: &Value,
) -> AppResult<i64> {
    let reason_list = raw_payload
        .get("reason_list")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::Validation("微信售后拒绝原因响应缺少 reason_list".to_string()))?;
    let now = now_shanghai();
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM aftersale_reject_reasons WHERE shop_id = ?1",
        [shop_id],
    )?;
    let mut synced = 0i64;
    for reason in reason_list {
        let reject_reason_type = json_value_to_i64(reason.get("reject_reason_type"));
        let reject_reason_type_text = json_value_to_string(reason.get("reject_reason_type_text"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let reject_reason = json_value_to_string(reason.get("reject_reason"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (Some(reject_reason_type), Some(reject_reason_type_text), Some(reject_reason)) =
            (reject_reason_type, reject_reason_type_text, reject_reason)
        else {
            continue;
        };
        tx.execute(
            "INSERT INTO aftersale_reject_reasons
             (shop_id, reject_reason_type, reject_reason_type_text, reject_reason,
              reject_scene, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(shop_id, reject_reason_type) DO UPDATE SET
               reject_reason_type_text = excluded.reject_reason_type_text,
               reject_reason = excluded.reject_reason,
               reject_scene = excluded.reject_scene,
               raw_payload = excluded.raw_payload,
               synced_at = excluded.synced_at",
            params![
                shop_id,
                reject_reason_type,
                reject_reason_type_text,
                reject_reason,
                json_value_to_i64(reason.get("reject_scene")),
                reason.to_string(),
                now
            ],
        )?;
        synced += 1;
    }
    tx.commit()?;
    Ok(synced)
}

pub(super) fn reject_scene_text(reject_scene: Option<i64>) -> &'static str {
    match reject_scene {
        Some(1) => "拒绝仅退款",
        Some(4) => "拒绝退货退款",
        Some(5) => "拒绝换货",
        Some(6) => "拒绝换货发新商品",
        Some(7) => "极速换货收货处理",
        _ => "未知场景",
    }
}

pub(super) fn upsert_delivery_companies(
    conn: &mut Connection,
    shop_id: &str,
    raw_payload: &Value,
) -> AppResult<i64> {
    let company_list = raw_payload
        .get("company_list")
        .and_then(|value| value.as_array())
        .ok_or_else(|| AppError::Validation("微信快递公司列表响应缺少 company_list".to_string()))?;
    let now = now_shanghai();
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM delivery_companies WHERE shop_id = ?1",
        [shop_id],
    )?;
    let mut synced = 0i64;
    for company in company_list {
        let delivery_id = json_value_to_string(company.get("delivery_id"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let delivery_name = json_value_to_string(company.get("delivery_name"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (Some(delivery_id), Some(delivery_name)) = (delivery_id, delivery_name) else {
            continue;
        };
        tx.execute(
            "INSERT INTO delivery_companies
             (shop_id, delivery_id, delivery_name, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(shop_id, delivery_id) DO UPDATE SET
               delivery_name = excluded.delivery_name,
               raw_payload = excluded.raw_payload,
               synced_at = excluded.synced_at",
            params![
                shop_id,
                delivery_id,
                delivery_name,
                company.to_string(),
                now
            ],
        )?;
        synced += 1;
    }
    tx.commit()?;
    Ok(synced)
}

pub(super) fn build_inventory_risk_views(conn: &Connection) -> AppResult<Vec<InventoryRiskView>> {
    let mut products = load_inventory_source_products(conn)?;
    let purchase_aggregates = load_inventory_purchase_aggregates(conn)?;
    let shop_counts = load_inventory_shop_counts(conn)?;
    for external_product_id in purchase_aggregates.keys().chain(shop_counts.keys()) {
        products
            .entry(external_product_id.clone())
            .or_insert_with(|| InventorySourceProduct {
                title: external_product_id.clone(),
                updated_at: now_shanghai(),
                ..InventorySourceProduct::default()
            });
    }

    let mut items = products
        .into_iter()
        .map(|(external_product_id, product)| {
            let purchase = purchase_aggregates
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let active_shop_count = shop_counts
                .get(&external_product_id)
                .copied()
                .unwrap_or_default();
            let available_stock = product.total_stock - purchase.reserved_quantity;
            let (risk_status, recommendation) = resolve_inventory_risk(
                product.total_stock,
                purchase.reserved_quantity,
                available_stock,
                purchase.pending_purchase_quantity,
                purchase.supplier_issue_count,
                active_shop_count,
            );
            InventoryRiskView {
                external_product_id,
                title: product.title,
                supplier_name: product.supplier_name,
                supplier_product_id: product.supplier_product_id,
                total_stock: product.total_stock,
                reserved_quantity: purchase.reserved_quantity,
                available_stock,
                active_shop_count,
                pending_purchase_quantity: purchase.pending_purchase_quantity,
                supplier_issue_count: purchase.supplier_issue_count,
                risk_status: risk_status.to_string(),
                recommendation: recommendation.to_string(),
                updated_at: product.updated_at,
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        inventory_risk_rank(&left.risk_status)
            .cmp(&inventory_risk_rank(&right.risk_status))
            .then(left.available_stock.cmp(&right.available_stock))
            .then(left.external_product_id.cmp(&right.external_product_id))
    });
    Ok(items)
}

pub(super) fn load_inventory_source_products(
    conn: &Connection,
) -> AppResult<BTreeMap<String, InventorySourceProduct>> {
    let mut stmt = conn.prepare(
        "SELECT external_product_id, title, raw_payload, created_at
         FROM publish_products
         ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut products = BTreeMap::new();
    for (external_product_id, fallback_title, raw_payload, created_at) in rows {
        if products.contains_key(&external_product_id) {
            continue;
        }
        let parsed = serde_json::from_str::<ExternalProductInput>(&raw_payload).ok();
        let title = parsed
            .as_ref()
            .map(|product| product.title.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback_title.trim())
            .to_string();
        let total_stock = parsed
            .as_ref()
            .map(|product| product.skus.iter().map(|sku| sku.stock.max(0)).sum())
            .unwrap_or_default();
        products.insert(
            external_product_id,
            InventorySourceProduct {
                title,
                supplier_name: parsed
                    .as_ref()
                    .and_then(|product| product.supplier_name.clone()),
                supplier_product_id: parsed
                    .as_ref()
                    .and_then(|product| product.supplier_product_id.clone()),
                total_stock,
                updated_at: created_at,
            },
        );
    }
    Ok(products)
}

pub(super) fn load_inventory_purchase_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, InventoryPurchaseAggregate>> {
    let mut stmt = conn.prepare(
        "SELECT
           external_product_id,
           COALESCE(SUM(CASE
             WHEN status IN ('pending_purchase', 'supplier_shipped', 'wechat_shipped', 'completed')
             THEN quantity ELSE 0 END), 0) AS reserved_quantity,
           COALESCE(SUM(CASE WHEN status = 'pending_purchase' THEN quantity ELSE 0 END), 0)
             AS pending_purchase_quantity,
           COALESCE(SUM(CASE
             WHEN status IN ('supplier_out_of_stock', 'supplier_price_changed',
                             'supplier_cancelled', 'supplier_quality_risk', 'supplier_exception')
             THEN 1 ELSE 0 END), 0) AS supplier_issue_count
         FROM purchase_tasks
         WHERE external_product_id IS NOT NULL AND TRIM(external_product_id) != ''
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                InventoryPurchaseAggregate {
                    reserved_quantity: row.get(1)?,
                    pending_purchase_quantity: row.get(2)?,
                    supplier_issue_count: row.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(super) fn load_inventory_shop_counts(conn: &Connection) -> AppResult<BTreeMap<String, i64>> {
    let mut stmt = conn.prepare(
        "SELECT external_product_id, COUNT(DISTINCT shop_id)
         FROM shop_products
         WHERE external_product_id IS NOT NULL AND TRIM(external_product_id) != ''
           AND status NOT IN ('deleted', 'failed')
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(super) fn resolve_inventory_risk(
    total_stock: i64,
    reserved_quantity: i64,
    available_stock: i64,
    pending_purchase_quantity: i64,
    supplier_issue_count: i64,
    active_shop_count: i64,
) -> (&'static str, &'static str) {
    if supplier_issue_count > 0 {
        return (
            "supplier_issue",
            "存在供应商异常采购任务，先人工处理供应商缺货、涨价或取消",
        );
    }
    if total_stock <= 0 || available_stock <= 0 {
        return (
            "out_of_stock",
            "货源库存已不足，建议暂停继续铺货并评估下架或换源",
        );
    }
    if pending_purchase_quantity > available_stock {
        return (
            "stock_pressure",
            "待采购数量超过可用库存，建议优先补货或减少铺货范围",
        );
    }
    if available_stock <= 5 {
        return ("low_stock", "可用库存偏低，建议补库存或降低新店铺铺货节奏");
    }
    if active_shop_count == 0 && reserved_quantity == 0 {
        return ("not_listed", "尚未形成有效店铺商品，可继续进入铺货任务");
    }
    ("healthy", "库存风险正常，可继续观察真实订单和售后表现")
}

pub(super) fn inventory_risk_rank(status: &str) -> i64 {
    match status {
        "out_of_stock" => 0,
        "supplier_issue" => 1,
        "stock_pressure" => 2,
        "low_stock" => 3,
        "not_listed" => 4,
        _ => 5,
    }
}

pub(super) fn build_product_sales_analysis_views(
    conn: &Connection,
) -> AppResult<Vec<ProductSalesAnalysisView>> {
    let source_products = load_inventory_source_products(conn)?;
    let inventory_risks = build_inventory_risk_views(conn)?
        .into_iter()
        .map(|item| (item.external_product_id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let sales = load_product_sales_aggregates(conn)?;
    let purchase_costs = load_product_purchase_cost_aggregates(conn)?;
    let aftersales = load_product_aftersale_aggregates(conn)?;

    let mut product_ids = BTreeSet::new();
    product_ids.extend(source_products.keys().cloned());
    product_ids.extend(inventory_risks.keys().cloned());
    product_ids.extend(sales.keys().cloned());
    product_ids.extend(purchase_costs.keys().cloned());
    product_ids.extend(aftersales.keys().cloned());

    let mut items = product_ids
        .into_iter()
        .map(|external_product_id| {
            let source = source_products.get(&external_product_id);
            let inventory = inventory_risks.get(&external_product_id);
            let sale = sales.get(&external_product_id).cloned().unwrap_or_default();
            let purchase = purchase_costs
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let aftersale = aftersales
                .get(&external_product_id)
                .cloned()
                .unwrap_or_default();
            let title = source
                .map(|product| product.title.trim())
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    inventory
                        .map(|item| item.title.trim())
                        .filter(|value| !value.is_empty())
                })
                .or_else(|| (!sale.title.trim().is_empty()).then_some(sale.title.trim()))
                .unwrap_or(external_product_id.as_str())
                .to_string();
            let total_stock = inventory.map(|item| item.total_stock).unwrap_or_default();
            let available_stock = inventory
                .map(|item| item.available_stock)
                .unwrap_or_default();
            let active_shop_count = inventory
                .map(|item| item.active_shop_count)
                .unwrap_or_default();
            let inventory_risk_status = inventory
                .map(|item| item.risk_status.clone())
                .unwrap_or_else(|| "not_listed".to_string());
            let gross_profit_cents =
                sale.revenue_cents - purchase.purchase_cost_cents - aftersale.related_refund_cents;
            let (operation_status, recommendation) = resolve_product_operation_status(
                sale.units_sold,
                sale.order_count,
                active_shop_count,
                available_stock,
                gross_profit_cents,
                purchase.purchase_task_count,
                purchase.missing_cost_count,
                aftersale.related_aftersale_count,
                &inventory_risk_status,
            );
            ProductSalesAnalysisView {
                external_product_id,
                title,
                supplier_name: source.and_then(|product| product.supplier_name.clone()),
                active_shop_count,
                order_count: sale.order_count,
                units_sold: sale.units_sold,
                revenue_cents: sale.revenue_cents,
                purchase_task_count: purchase.purchase_task_count,
                missing_cost_count: purchase.missing_cost_count,
                purchase_cost_cents: purchase.purchase_cost_cents,
                related_aftersale_count: aftersale.related_aftersale_count,
                related_refund_cents: aftersale.related_refund_cents,
                total_stock,
                available_stock,
                inventory_risk_status,
                operation_status: operation_status.to_string(),
                recommendation: recommendation.to_string(),
                last_order_at: sale.last_order_at,
                updated_at: source
                    .map(|product| product.updated_at.clone())
                    .or_else(|| inventory.map(|item| item.updated_at.clone()))
                    .unwrap_or_else(now_shanghai),
            }
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        product_operation_rank(&left.operation_status)
            .cmp(&product_operation_rank(&right.operation_status))
            .then(right.units_sold.cmp(&left.units_sold))
            .then(right.revenue_cents.cmp(&left.revenue_cents))
            .then(left.external_product_id.cmp(&right.external_product_id))
    });
    Ok(items)
}

pub(super) fn load_product_sales_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductSalesAggregate>> {
    let mut stmt = conn.prepare(
        "WITH product_order_items AS (
           SELECT
             COALESCE(NULLIF(TRIM(oi.out_product_id), ''), NULLIF(TRIM(pt.external_product_id), '')) AS external_product_id,
             oi.order_id,
             COALESCE(NULLIF(TRIM(oi.title), ''), '') AS title,
             COALESCE(oi.sku_count, 0) AS sku_count,
             COALESCE(oi.real_price, oi.sale_price * oi.sku_count, 0) AS revenue_cents,
             COALESCE(o.updated_at, o.synced_at, o.created_at) AS order_time,
             COALESCE(o.status, '') AS order_status
           FROM order_items oi
           LEFT JOIN orders o ON o.id = oi.order_id
           LEFT JOIN purchase_tasks pt ON pt.order_item_id = oi.id
         )
         SELECT
           external_product_id,
           MAX(title),
           COUNT(DISTINCT order_id),
           COALESCE(SUM(sku_count), 0),
           COALESCE(SUM(revenue_cents), 0),
           MAX(order_time)
         FROM product_order_items
         WHERE external_product_id IS NOT NULL
           AND external_product_id != ''
           AND order_status NOT IN ('cancelled')
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductSalesAggregate {
                    title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    order_count: row.get(2)?,
                    units_sold: row.get(3)?,
                    revenue_cents: row.get(4)?,
                    last_order_at: row.get(5)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(super) fn load_product_purchase_cost_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductPurchaseCostAggregate>> {
    let mut stmt = conn.prepare(
        "SELECT
           external_product_id,
           COUNT(*) AS purchase_task_count,
           COALESCE(SUM(CASE WHEN estimated_cost IS NULL THEN 1 ELSE 0 END), 0) AS missing_cost_count,
           COALESCE(SUM(CASE
             WHEN estimated_cost IS NULL THEN 0
             ELSE CAST(ROUND(estimated_cost * 100) AS INTEGER)
           END), 0) AS purchase_cost_cents
         FROM purchase_tasks
         WHERE external_product_id IS NOT NULL AND TRIM(external_product_id) != ''
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductPurchaseCostAggregate {
                    purchase_task_count: row.get(1)?,
                    missing_cost_count: row.get(2)?,
                    purchase_cost_cents: row.get(3)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(super) fn load_product_aftersale_aggregates(
    conn: &Connection,
) -> AppResult<BTreeMap<String, ProductAftersaleAggregate>> {
    let mut stmt = conn.prepare(
        "WITH product_orders AS (
           SELECT DISTINCT
             COALESCE(NULLIF(TRIM(oi.out_product_id), ''), NULLIF(TRIM(pt.external_product_id), '')) AS external_product_id,
             oi.order_id,
             oi.wechat_order_id
           FROM order_items oi
           LEFT JOIN purchase_tasks pt ON pt.order_item_id = oi.id
         ),
         product_aftersales AS (
           SELECT DISTINCT
             po.external_product_id,
             a.id AS aftersale_id,
             COALESCE(a.refund_amount_cents, 0) AS refund_amount_cents
           FROM product_orders po
           JOIN aftersales a
             ON a.order_id = po.order_id
             OR (
               a.wechat_order_id IS NOT NULL
               AND po.wechat_order_id IS NOT NULL
               AND a.wechat_order_id = po.wechat_order_id
             )
           WHERE po.external_product_id IS NOT NULL AND po.external_product_id != ''
         )
         SELECT
           external_product_id,
           COUNT(aftersale_id),
           COALESCE(SUM(refund_amount_cents), 0)
         FROM product_aftersales
         GROUP BY external_product_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProductAftersaleAggregate {
                    related_aftersale_count: row.get(1)?,
                    related_refund_cents: row.get(2)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

pub(super) fn summarize_product_sales_analysis(
    items: &[ProductSalesAnalysisView],
) -> ProductSalesAnalysisTotals {
    ProductSalesAnalysisTotals {
        product_count: items.len() as i64,
        sold_product_count: items.iter().filter(|item| item.units_sold > 0).count() as i64,
        total_units_sold: items.iter().map(|item| item.units_sold).sum(),
        revenue_cents: items.iter().map(|item| item.revenue_cents).sum(),
        purchase_cost_cents: items.iter().map(|item| item.purchase_cost_cents).sum(),
        gross_profit_cents: items
            .iter()
            .map(|item| item.revenue_cents - item.purchase_cost_cents - item.related_refund_cents)
            .sum(),
        missing_cost_product_count: items
            .iter()
            .filter(|item| item.missing_cost_count > 0)
            .count() as i64,
        scale_candidate_count: items
            .iter()
            .filter(|item| item.operation_status == "scale_candidate")
            .count() as i64,
        risk_product_count: items
            .iter()
            .filter(|item| {
                matches!(
                    item.operation_status.as_str(),
                    "stock_risk" | "margin_risk" | "aftersale_watch"
                )
            })
            .count() as i64,
    }
}

pub(super) fn resolve_product_operation_status(
    units_sold: i64,
    order_count: i64,
    active_shop_count: i64,
    available_stock: i64,
    gross_profit_cents: i64,
    purchase_task_count: i64,
    missing_cost_count: i64,
    related_aftersale_count: i64,
    inventory_risk_status: &str,
) -> (&'static str, &'static str) {
    if matches!(
        inventory_risk_status,
        "out_of_stock" | "supplier_issue" | "stock_pressure" | "low_stock"
    ) {
        return (
            "stock_risk",
            "库存或供应商风险已触发，先处理补货、换源或暂停继续铺货",
        );
    }
    if units_sold > 0
        && purchase_task_count > 0
        && missing_cost_count == 0
        && gross_profit_cents < 0
    {
        return (
            "margin_risk",
            "已有真实订单但毛利为负，优先核对成本并进入批量改价",
        );
    }
    if units_sold > 0 && related_aftersale_count > 0 {
        return (
            "aftersale_watch",
            "已有订单关联售后，先观察原因和责任方再扩大铺货",
        );
    }
    if units_sold >= 3 && order_count >= 2 && gross_profit_cents > 0 && available_stock > 5 {
        return (
            "scale_candidate",
            "真实订单、毛利和库存都较健康，可优先追加店铺或测试放量",
        );
    }
    if active_shop_count > 0 && units_sold == 0 {
        return (
            "no_sales",
            "已铺货但暂无真实订单，建议检查价格、标题、素材或下架节奏",
        );
    }
    if active_shop_count == 0 {
        return ("not_listed", "尚未形成有效店铺商品，可进入铺货候选池");
    }
    if units_sold > 0 {
        return ("steady", "已有真实订单，继续观察毛利、库存和售后变化");
    }
    ("observe", "数据不足，先保持观察，不做自动动销动作")
}

pub(super) fn product_operation_rank(status: &str) -> i64 {
    match status {
        "stock_risk" => 0,
        "margin_risk" => 1,
        "aftersale_watch" => 2,
        "scale_candidate" => 3,
        "no_sales" => 4,
        "not_listed" => 5,
        "steady" => 6,
        _ => 7,
    }
}

pub(super) fn load_purchase_task_shipment_ref(
    conn: &Connection,
    purchase_task_id: &str,
) -> AppResult<PurchaseTaskShipmentRef> {
    let purchase = conn
        .query_row(
            "SELECT pt.id, pt.order_id, pt.shop_id, COALESCE(o.wechat_order_id, '')
             FROM purchase_tasks pt
             JOIN orders o ON o.id = pt.order_id
             WHERE pt.id = ?1",
            [purchase_task_id],
            |row| {
                Ok(PurchaseTaskShipmentRef {
                    purchase_task_id: row.get(0)?,
                    order: ShipmentOrderRef {
                        order_id: row.get(1)?,
                        shop_id: row.get(2)?,
                        wechat_order_id: row.get(3)?,
                    },
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("采购任务不存在".to_string()))?;
    if purchase.order.wechat_order_id.trim().is_empty() {
        return Err(AppError::Validation(
            "采购任务关联订单缺少微信订单号，无法生成发货单".to_string(),
        ));
    }
    Ok(purchase)
}

pub(super) fn normalize_shipment_fields(
    deliver_type: Option<i64>,
    delivery_id: Option<&str>,
    delivery_name: Option<&str>,
    waybill_id: Option<&str>,
) -> AppResult<ShipmentFields> {
    let deliver_type = deliver_type.unwrap_or(1);
    if ![1, 3].contains(&deliver_type) {
        return Err(AppError::Validation(
            "发货方式只支持 1 自寄快递或 3 虚拟商品无需物流".to_string(),
        ));
    }
    let delivery_id = delivery_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let delivery_name = delivery_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let waybill_id = waybill_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if deliver_type == 1 && (delivery_id.is_none() || waybill_id.is_none()) {
        return Err(AppError::Validation(
            "自寄快递发货必须填写快递公司 ID 和快递单号".to_string(),
        ));
    }
    Ok(ShipmentFields {
        delivery_id,
        delivery_name,
        waybill_id,
        deliver_type,
    })
}

pub(super) fn count_distinct_order_purchase_logistics(
    conn: &Connection,
    order_id: &str,
) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*)
         FROM (
           SELECT
             COALESCE(supplier_deliver_type, 1) AS deliver_type,
             COALESCE(supplier_delivery_id, '') AS delivery_id,
             COALESCE(supplier_waybill_id, '') AS waybill_id
           FROM purchase_tasks
           WHERE order_id = ?1
             AND status IN ('supplier_shipped', 'wechat_shipped', 'completed')
           GROUP BY deliver_type, delivery_id, waybill_id
         )",
        [order_id],
        |row| row.get(0),
    )?)
}

pub(super) fn upsert_order_shipment(
    conn: &Connection,
    order: &ShipmentOrderRef,
    fields: &ShipmentFields,
) -> AppResult<ShipmentUpsertResult> {
    let auto_send_enabled = get_bool_setting(conn, AUTO_SEND_DELIVERY_SETTING, false)?;
    let status = if auto_send_enabled {
        "ready_to_send"
    } else {
        "waiting_confirmation"
    };
    let shipment_id = build_shipment_id(
        &order.order_id,
        fields.deliver_type,
        fields.delivery_id.as_deref(),
        fields.waybill_id.as_deref(),
    );
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO shipments
         (id, order_id, shop_id, wechat_order_id, delivery_id, delivery_name, waybill_id,
          deliver_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(id) DO UPDATE SET
           delivery_id = excluded.delivery_id,
           delivery_name = excluded.delivery_name,
           waybill_id = excluded.waybill_id,
           deliver_type = excluded.deliver_type,
           status = excluded.status,
           error_code = NULL,
           error_summary = NULL,
           updated_at = excluded.updated_at",
        params![
            shipment_id.as_str(),
            order.order_id.as_str(),
            order.shop_id.as_str(),
            order.wechat_order_id.as_str(),
            fields.delivery_id.as_deref(),
            fields.delivery_name.as_deref(),
            fields.waybill_id.as_deref(),
            fields.deliver_type,
            status,
            now
        ],
    )?;
    conn.execute(
        "UPDATE orders
         SET status = CASE
           WHEN status IN ('completed', 'cancelled', 'wechat_shipped') THEN status
           ELSE 'supplier_shipped'
         END,
         updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), order.order_id],
    )?;
    let message = if auto_send_enabled {
        "物流已回填，自动发货开关已开启，等待提交微信发货".to_string()
    } else {
        "物流已回填，自动发货开关关闭，当前仅进入待确认发货".to_string()
    };
    Ok(ShipmentUpsertResult {
        shipment_id,
        status: status.to_string(),
        auto_send_enabled,
        message,
    })
}

pub(super) fn csv_escape(value: &str) -> String {
    if value
        .chars()
        .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub(super) fn normalize_aftersale_evidence_export_format(
    format: Option<String>,
) -> AppResult<String> {
    let format = format
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("md");
    match format {
        "jsonl" | "json" | "md" => Ok(format.to_string()),
        _ => Err(AppError::Validation(
            "凭证资料包导出格式必须是 jsonl/json/md".to_string(),
        )),
    }
}

pub(super) fn aftersale_evidence_export_json(row: &AftersaleEvidenceView) -> Value {
    serde_json::json!({
        "evidence_id": row.id,
        "target_type": row.target_type,
        "target_label": if row.target_type == "guarantee" { "纠纷单" } else { "售后单" },
        "target_id": row.target_id,
        "external_target_id": row.external_target_id,
        "shop_id": row.shop_id,
        "shop_name": row.shop_name,
        "evidence_type": row.evidence_type,
        "evidence_type_text": row.evidence_type_text,
        "title": row.title,
        "content_text": row.content_text,
        "local_file_path": row.local_file_path,
        "source_url": row.source_url,
        "status": row.status,
        "status_text": row.status_text,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
        "file_contents_included": false,
        "wechat_uploaded": false,
        "platform_action_submitted": false
    })
}

pub(super) fn render_aftersale_evidence_markdown(rows: &[AftersaleEvidenceView]) -> String {
    let mut content = format!(
        "# 售后/纠纷本地凭证资料包\n\n导出时间：{}\n\n本文件只包含本地凭证元数据和脱敏说明，不包含收件人姓名、手机号、地址、文件内容、微信 access_token 或平台处理结果。`已使用` 只代表内部整理进度，不代表已向微信上传或完成平台举证。\n\n| 状态 | 目标 | 单号 | 店铺 | 类型 | 标题 | 脱敏说明 | 本地文件路径 | 来源链接 | 更新时间 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
        now_shanghai()
    );
    for row in rows {
        let fields = [
            row.status_text.clone(),
            if row.target_type == "guarantee" {
                "纠纷单".to_string()
            } else {
                "售后单".to_string()
            },
            row.external_target_id.clone(),
            row.shop_name.clone(),
            row.evidence_type_text.clone(),
            row.title.clone(),
            row.content_text.clone().unwrap_or_default(),
            row.local_file_path.clone().unwrap_or_default(),
            row.source_url.clone().unwrap_or_default(),
            row.updated_at.clone(),
        ];
        content.push_str("| ");
        content.push_str(
            &fields
                .iter()
                .map(|value| markdown_table_cell(value))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        content.push_str(" |\n");
    }
    content
}

pub(super) fn markdown_table_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('\n', " ")
        .replace('\r', " ")
}

pub(super) struct SupplierAgentAction {
    pub(super) action: String,
    pub(super) purchase_task_id: String,
    pub(super) body: Value,
}

pub(super) fn normalize_supplier_agent_export_format(format: Option<String>) -> AppResult<String> {
    let format = format
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("jsonl");
    match format {
        "jsonl" | "json" | "md" => Ok(format.to_string()),
        _ => Err(AppError::Validation(
            "供应商 agent 导出格式必须是 jsonl/json/md".to_string(),
        )),
    }
}

pub(super) fn supplier_agent_task_json(row: &PurchaseTaskView) -> Value {
    serde_json::json!({
        "purchase_task_id": row.id,
        "wechat_order_id": row.wechat_order_id,
        "shop_id": row.shop_id,
        "shop_name": row.shop_name,
        "status": row.status,
        "external_product_id": row.external_product_id,
        "source_url": row.source_url,
        "external_sku_id": row.external_sku_id,
        "title": row.title,
        "quantity": row.quantity,
        "estimated_revenue": row.estimated_revenue,
        "estimated_cost": row.estimated_cost,
        "estimated_profit": row.estimated_profit,
        "supplier_name": row.supplier_name,
        "supplier_product_id": row.supplier_product_id,
        "error_summary": row.error_summary,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
        "needs_mapping": row.external_product_id.is_none() || row.external_sku_id.is_none(),
        "allowed_result_actions": ["shipment", "issue", "mapping"]
    })
}

pub(super) fn render_supplier_agent_markdown(records: &[Value]) -> String {
    let mut content = String::from(
        "# 供应商采购任务清单\n\n本文件只包含非敏采购字段，不包含收件人姓名、手机号、地址、密钥或供应商平台凭证。货源链接用于人工采购定位，不得包含登录态、Cookie、token 或一次性私密参数。\n\n| 采购任务 ID | 状态 | 店铺 | 微信订单号 | 外部商品 ID | 货源链接 | 外部 SKU | 商品 | 数量 | 预估成本 | 异常 |\n| --- | --- | --- | --- | --- | --- | --- | --- | ---: | ---: | --- |\n",
    );
    for record in records {
        let fields = [
            supplier_agent_value_string(record.get("purchase_task_id")).unwrap_or_default(),
            supplier_agent_value_string(record.get("status")).unwrap_or_default(),
            supplier_agent_value_string(record.get("shop_name")).unwrap_or_default(),
            supplier_agent_value_string(record.get("wechat_order_id")).unwrap_or_default(),
            supplier_agent_value_string(record.get("external_product_id")).unwrap_or_default(),
            supplier_agent_value_string(record.get("source_url")).unwrap_or_default(),
            supplier_agent_value_string(record.get("external_sku_id")).unwrap_or_default(),
            supplier_agent_value_string(record.get("title")).unwrap_or_default(),
            supplier_agent_value_string(record.get("quantity")).unwrap_or_default(),
            supplier_agent_value_string(record.get("estimated_cost")).unwrap_or_default(),
            supplier_agent_value_string(record.get("error_summary")).unwrap_or_default(),
        ];
        content.push_str("| ");
        content.push_str(
            &fields
                .iter()
                .map(|value| value.replace('|', "\\|").replace('\n', " "))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        content.push_str(" |\n");
    }
    content
}

pub(super) fn parse_supplier_agent_result_records(raw: &str) -> AppResult<Vec<Value>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    if raw.starts_with('[') || raw.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(raw) {
            return supplier_agent_records_from_value(value);
        }
    }
    let mut records = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(line).map_err(|error| {
            AppError::Validation(format!("JSONL 第 {} 行解析失败：{error}", index + 1))
        })?;
        records.push(value);
    }
    Ok(records)
}

pub(super) fn supplier_agent_records_from_value(value: Value) -> AppResult<Vec<Value>> {
    match value {
        Value::Array(items) => Ok(items),
        Value::Object(mut object) => {
            if let Some(Value::Array(items)) = object.remove("items") {
                return Ok(items);
            }
            if let Some(Value::Array(items)) = object.remove("results") {
                return Ok(items);
            }
            if object.contains_key("action") && object.contains_key("purchase_task_id") {
                return Ok(vec![Value::Object(object)]);
            }
            Err(AppError::Validation(
                "JSON 输入必须是数组、单条结果，或包含 items/results 数组".to_string(),
            ))
        }
        _ => Err(AppError::Validation(
            "供应商 agent 结果必须是 JSON object 或 array".to_string(),
        )),
    }
}

pub(super) fn build_supplier_agent_action(record: &Value) -> AppResult<SupplierAgentAction> {
    let object = record.as_object().ok_or_else(|| {
        AppError::Validation("每条供应商 agent 结果必须是 JSON object".to_string())
    })?;
    let forbidden = find_supplier_agent_forbidden_keys(record, "$");
    if !forbidden.is_empty() {
        return Err(AppError::Validation(format!(
            "结果记录包含禁止字段：{}；不得传递收件信息、密钥、Cookie 或供应商平台凭证",
            forbidden.join(", ")
        )));
    }
    let action = supplier_agent_required_string(object.get("action"), "action")?;
    let purchase_task_id =
        supplier_agent_required_string(object.get("purchase_task_id"), "purchase_task_id")?;
    let allowed_fields = supplier_agent_action_fields(&action)?;
    let unknown = object
        .keys()
        .filter(|key| !allowed_fields.contains(key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(AppError::Validation(format!(
            "{action} 结果包含未知字段：{}",
            unknown.join(", ")
        )));
    }

    let body = match action.as_str() {
        "shipment" => {
            let deliver_type = supplier_agent_optional_i64(object.get("deliver_type")).unwrap_or(1);
            if deliver_type == 1 {
                supplier_agent_required_string(object.get("delivery_id"), "delivery_id")?;
                supplier_agent_required_string(object.get("waybill_id"), "waybill_id")?;
            }
            serde_json::json!({
                "delivery_id": supplier_agent_value_string(object.get("delivery_id")),
                "delivery_name": supplier_agent_value_string(object.get("delivery_name")),
                "waybill_id": supplier_agent_value_string(object.get("waybill_id")),
                "deliver_type": deliver_type,
                "estimated_cost": supplier_agent_optional_f64(object.get("estimated_cost")),
            })
        }
        "issue" => {
            let issue_type =
                supplier_agent_required_string(object.get("issue_type"), "issue_type")?;
            if !matches!(
                issue_type.as_str(),
                "out_of_stock" | "price_changed" | "supplier_cancelled" | "quality_risk" | "other"
            ) {
                return Err(AppError::Validation(
                    "issue_type 必须是 out_of_stock/price_changed/supplier_cancelled/quality_risk/other".to_string(),
                ));
            }
            serde_json::json!({
                "issue_type": issue_type,
                "note": supplier_agent_value_string(object.get("note")),
            })
        }
        "mapping" => {
            supplier_agent_required_string(
                object.get("external_product_id"),
                "external_product_id",
            )?;
            supplier_agent_required_string(object.get("external_sku_id"), "external_sku_id")?;
            serde_json::json!({
                "external_product_id": supplier_agent_value_string(object.get("external_product_id")),
                "external_sku_id": supplier_agent_value_string(object.get("external_sku_id")),
                "source_url": supplier_agent_value_string(object.get("source_url")),
                "supplier_name": supplier_agent_value_string(object.get("supplier_name")),
                "supplier_product_id": supplier_agent_value_string(object.get("supplier_product_id")),
                "estimated_cost": supplier_agent_optional_f64(object.get("estimated_cost")),
                "note": supplier_agent_value_string(object.get("note")),
            })
        }
        _ => unreachable!("action normalized"),
    };
    Ok(SupplierAgentAction {
        action,
        purchase_task_id,
        body,
    })
}

pub(super) fn supplier_agent_action_fields(action: &str) -> AppResult<BTreeSet<&'static str>> {
    let fields = match action {
        "shipment" => [
            "purchase_task_id",
            "action",
            "delivery_id",
            "delivery_name",
            "waybill_id",
            "deliver_type",
            "estimated_cost",
        ]
        .into_iter()
        .collect(),
        "issue" => ["purchase_task_id", "action", "issue_type", "note"]
            .into_iter()
            .collect(),
        "mapping" => [
            "purchase_task_id",
            "action",
            "external_product_id",
            "external_sku_id",
            "source_url",
            "supplier_name",
            "supplier_product_id",
            "estimated_cost",
            "note",
        ]
        .into_iter()
        .collect(),
        _ => {
            return Err(AppError::Validation(
                "action 必须是 shipment/issue/mapping".to_string(),
            ))
        }
    };
    Ok(fields)
}

pub(super) fn apply_supplier_agent_action(
    app: &AppHandle,
    action: SupplierAgentAction,
    index: i64,
) -> SupplierAgentApplyItemResult {
    let purchase_task_id = action.purchase_task_id.clone();
    let action_name = action.action.clone();
    let result = match action.action.as_str() {
        "shipment" => record_purchase_task_shipment(
            app.clone(),
            PurchaseTaskShipmentRequest {
                purchase_task_id: action.purchase_task_id,
                delivery_id: supplier_agent_value_string(action.body.get("delivery_id")),
                delivery_name: supplier_agent_value_string(action.body.get("delivery_name")),
                waybill_id: supplier_agent_value_string(action.body.get("waybill_id")),
                deliver_type: supplier_agent_optional_i64(action.body.get("deliver_type")),
                estimated_cost: supplier_agent_optional_f64(action.body.get("estimated_cost")),
            },
        )
        .and_then(|value| {
            serde_json::to_value(value)
                .map_err(|error| AppError::Validation(format!("JSON 序列化失败：{error}")))
        }),
        "issue" => mark_purchase_task_issue(
            app.clone(),
            PurchaseTaskIssueRequest {
                purchase_task_id: action.purchase_task_id,
                issue_type: supplier_agent_value_string(action.body.get("issue_type"))
                    .unwrap_or_default(),
                note: supplier_agent_value_string(action.body.get("note")),
            },
        )
        .and_then(|value| {
            serde_json::to_value(value)
                .map_err(|error| AppError::Validation(format!("JSON 序列化失败：{error}")))
        }),
        "mapping" => resolve_purchase_task_mapping(
            app.clone(),
            PurchaseTaskMappingRequest {
                purchase_task_id: action.purchase_task_id,
                external_product_id: supplier_agent_value_string(
                    action.body.get("external_product_id"),
                )
                .unwrap_or_default(),
                external_sku_id: supplier_agent_value_string(action.body.get("external_sku_id"))
                    .unwrap_or_default(),
                source_url: supplier_agent_value_string(action.body.get("source_url")),
                supplier_name: supplier_agent_value_string(action.body.get("supplier_name")),
                supplier_product_id: supplier_agent_value_string(
                    action.body.get("supplier_product_id"),
                ),
                estimated_cost: supplier_agent_optional_f64(action.body.get("estimated_cost")),
                note: supplier_agent_value_string(action.body.get("note")),
            },
        )
        .and_then(|value| {
            serde_json::to_value(value)
                .map_err(|error| AppError::Validation(format!("JSON 序列化失败：{error}")))
        }),
        _ => Err(AppError::Validation(
            "action 必须是 shipment/issue/mapping".to_string(),
        )),
    };
    match result {
        Ok(response) => SupplierAgentApplyItemResult {
            index,
            purchase_task_id: Some(purchase_task_id),
            action: Some(action_name),
            status: "success".to_string(),
            error: None,
            response: Some(response),
        },
        Err(error) => SupplierAgentApplyItemResult {
            index,
            purchase_task_id: Some(purchase_task_id),
            action: Some(action_name),
            status: "failed".to_string(),
            error: Some(error.to_string()),
            response: None,
        },
    }
}

pub(super) fn supplier_agent_required_string(
    value: Option<&Value>,
    field: &str,
) -> AppResult<String> {
    supplier_agent_value_string(value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("{field} 不能为空")))
}

pub(super) fn supplier_agent_value_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Null => None,
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn supplier_agent_optional_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(super) fn supplier_agent_optional_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(super) fn find_supplier_agent_forbidden_keys(value: &Value, prefix: &str) -> Vec<String> {
    match value {
        Value::Object(object) => object
            .iter()
            .flat_map(|(key, child)| {
                let path = format!("{prefix}.{key}");
                let normalized = key.to_lowercase().replace(['-', ' '], "_");
                let mut paths = Vec::new();
                if [
                    "access_token",
                    "address",
                    "apikey",
                    "api_key",
                    "app_secret",
                    "cookie",
                    "mobile",
                    "openid",
                    "password",
                    "phone",
                    "recipient",
                    "receiver",
                    "session",
                    "tel",
                ]
                .iter()
                .any(|fragment| normalized.contains(fragment))
                {
                    paths.push(path.clone());
                }
                paths.extend(find_supplier_agent_forbidden_keys(child, &path));
                paths
            })
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .flat_map(|(index, child)| {
                find_supplier_agent_forbidden_keys(child, &format!("{prefix}[{index}]"))
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn resolve_shipment_order(
    conn: &Connection,
    request: &ShipmentRecordRequest,
) -> AppResult<ShipmentOrderRef> {
    if let Some(order_id) = request
        .order_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return conn
            .query_row(
                "SELECT id, shop_id, wechat_order_id
                 FROM orders
                 WHERE id = ?1",
                [order_id],
                |row| {
                    Ok(ShipmentOrderRef {
                        order_id: row.get(0)?,
                        shop_id: row.get(1)?,
                        wechat_order_id: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("订单不存在，无法回填物流".to_string()));
    }

    let shop_id = request
        .shop_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("缺少 shop_id 或 order_id".to_string()))?;
    let wechat_order_id = request
        .wechat_order_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation("缺少 wechat_order_id 或 order_id".to_string()))?;

    conn.query_row(
        "SELECT id, shop_id, wechat_order_id
         FROM orders
         WHERE shop_id = ?1 AND wechat_order_id = ?2",
        params![shop_id, wechat_order_id],
        |row| {
            Ok(ShipmentOrderRef {
                order_id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_order_id: row.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Validation("订单不存在，无法回填物流".to_string()))
}

pub(super) fn count_delivery_shipments(conn: &Connection, status: Option<&str>) -> AppResult<i64> {
    if let Some(status) = status {
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM shipments WHERE status = ?1",
            [status],
            |row| row.get(0),
        )?)
    } else {
        Ok(conn.query_row("SELECT COUNT(*) FROM shipments", [], |row| row.get(0))?)
    }
}

pub(super) fn load_delivery_shipment_views(
    conn: &Connection,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<ShipmentView>> {
    let sql = format!(
        "SELECT
           sh.id,
           sh.order_id,
           sh.shop_id,
           COALESCE(s.name, sh.shop_id),
           sh.wechat_order_id,
           sh.delivery_id,
           sh.delivery_name,
           sh.waybill_id,
           sh.deliver_type,
           sh.status,
           sh.error_code,
           sh.error_summary,
           sh.submitted_at,
           sh.created_at,
           sh.updated_at
         FROM shipments sh
         LEFT JOIN shops s ON s.id = sh.shop_id
         {}
         ORDER BY sh.updated_at DESC, sh.created_at DESC
         LIMIT ?{}",
        if status.is_some() {
            "WHERE sh.status = ?1"
        } else {
            ""
        },
        if status.is_some() { 2 } else { 1 }
    );
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(ShipmentView {
            id: row.get(0)?,
            order_id: row.get(1)?,
            shop_id: row.get(2)?,
            shop_name: row.get(3)?,
            wechat_order_id: row.get(4)?,
            delivery_id: row.get(5)?,
            delivery_name: row.get(6)?,
            waybill_id: row.get(7)?,
            deliver_type: row.get(8)?,
            status: row.get(9)?,
            error_code: row.get(10)?,
            error_summary: row.get(11)?,
            submitted_at: row.get(12)?,
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    };
    let rows = if let Some(status) = status {
        stmt.query_map(params![status, limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([limit], mapper)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

pub(super) fn load_delivery_submission_candidates(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<ShipmentCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT id, order_id, shop_id, wechat_order_id, delivery_id, waybill_id, deliver_type
         FROM shipments
         WHERE status = 'ready_to_send'
         ORDER BY created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map([limit], |row| {
            Ok(ShipmentCandidate {
                shipment_id: row.get(0)?,
                order_id: row.get(1)?,
                shop_id: row.get(2)?,
                wechat_order_id: row.get(3)?,
                delivery_id: row.get(4)?,
                waybill_id: row.get(5)?,
                deliver_type: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(super) fn build_send_delivery_payload(
    conn: &Connection,
    shipment: &ShipmentCandidate,
) -> AppResult<Value> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(wechat_product_id, ''), COALESCE(wechat_sku_id, ''), sku_count
         FROM order_items
         WHERE order_id = ?1
         ORDER BY created_at ASC",
    )?;
    let products = stmt
        .query_map([shipment.order_id.as_str()], |row| {
            Ok(ShipmentProductInfo {
                product_id: row.get(0)?,
                sku_id: row.get(1)?,
                product_cnt: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if products.is_empty() {
        return Err(AppError::Validation(
            "订单缺少订单项，无法构造发货商品列表".to_string(),
        ));
    }
    if products
        .iter()
        .any(|item| item.product_id.trim().is_empty() || item.sku_id.trim().is_empty())
    {
        return Err(AppError::Validation(
            "订单项缺少微信 product_id 或 sku_id，无法提交发货".to_string(),
        ));
    }
    if shipment.deliver_type == 1
        && (shipment
            .delivery_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
            || shipment
                .waybill_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty())
    {
        return Err(AppError::Validation(
            "自寄快递发货缺少快递公司 ID 或快递单号".to_string(),
        ));
    }

    let product_infos = products
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "product_id": item.product_id,
                "sku_id": item.sku_id,
                "product_cnt": item.product_cnt.max(1)
            })
        })
        .collect::<Vec<_>>();
    let mut delivery = serde_json::json!({
        "deliver_type": shipment.deliver_type,
        "product_infos": product_infos
    });
    if shipment.deliver_type == 1 {
        if let Some(delivery_id) = &shipment.delivery_id {
            delivery["delivery_id"] = serde_json::json!(delivery_id);
        }
        if let Some(waybill_id) = &shipment.waybill_id {
            delivery["waybill_id"] = serde_json::json!(waybill_id);
        }
    }

    Ok(serde_json::json!({
        "order_id": shipment.wechat_order_id,
        "delivery_list": [delivery]
    }))
}

pub(super) fn mark_shipment_failed(
    conn: &Connection,
    shipment: &ShipmentCandidate,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE shipments
         SET status = 'send_failed',
             error_code = ?1,
             error_summary = ?2,
             updated_at = ?3
         WHERE id = ?4",
        params![error_code, error_summary, now, shipment.shipment_id],
    )?;
    conn.execute(
        "UPDATE orders
         SET status = CASE WHEN status IN ('completed', 'cancelled') THEN status ELSE 'exception' END,
             updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), shipment.order_id],
    )?;
    upsert_notification(
        conn,
        "critical",
        "delivery_shipment",
        &shipment.shipment_id,
        Some(&shipment.shop_id),
        "微信发货提交失败",
        &format!(
            "订单 {} 的物流单提交微信失败：{}（{}）",
            shipment.wechat_order_id, error_summary, error_code
        ),
        Some(&serde_json::json!({
            "order_id": &shipment.order_id,
            "wechat_order_id": &shipment.wechat_order_id,
            "shipment_id": &shipment.shipment_id,
            "error_code": error_code
        })),
    )?;
    Ok(())
}

pub(super) fn mark_shipment_submitted(
    conn: &Connection,
    shipment: &ShipmentCandidate,
    payload: &Value,
    response: &Value,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "UPDATE shipments
         SET status = 'wechat_shipped',
             send_payload = ?1,
             error_code = NULL,
             error_summary = NULL,
             submitted_at = ?2,
             updated_at = ?2
         WHERE id = ?3",
        params![
            serde_json::json!({
                "request": payload,
                "response": response
            })
            .to_string(),
            now,
            shipment.shipment_id
        ],
    )?;
    conn.execute(
        "UPDATE orders SET status = 'wechat_shipped', updated_at = ?1 WHERE id = ?2",
        params![now_shanghai(), shipment.order_id],
    )?;
    Ok(())
}

pub(super) fn build_shipment_id(
    order_id: &str,
    deliver_type: i64,
    delivery_id: Option<&str>,
    waybill_id: Option<&str>,
) -> String {
    let delivery = delivery_id.unwrap_or("virtual").trim().replace('/', "_");
    let waybill = waybill_id.unwrap_or("no-waybill").trim().replace('/', "_");
    format!("shipment-{order_id}-{deliver_type}-{delivery}-{waybill}")
}

pub(super) fn normalize_ai_provider_type(value: &str) -> AppResult<String> {
    let provider_type = value.trim();
    if provider_type.is_empty() || provider_type == "openai_compatible" {
        Ok("openai_compatible".to_string())
    } else {
        Err(AppError::Validation(
            "当前仅支持 openai_compatible AI provider".to_string(),
        ))
    }
}

pub(super) fn normalize_ai_base_url(value: &str) -> AppResult<String> {
    let base_url = value.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Ok(base_url);
    }
    Url::parse(&base_url)
        .map_err(|error| AppError::Validation(format!("AI base_url 不是有效 URL：{error}")))?;
    Ok(base_url)
}

pub(super) fn load_ai_provider_settings(conn: &Connection) -> AppResult<AiProviderSettings> {
    let key = ai_api_key_record(conn)?;
    let updated_at = max_ai_setting_updated_at(conn)?;
    Ok(AiProviderSettings {
        enabled: get_bool_setting(conn, AI_PROVIDER_ENABLED_SETTING, false)?,
        provider_type: get_string_setting(conn, AI_PROVIDER_TYPE_SETTING)?
            .unwrap_or_else(|| "openai_compatible".to_string()),
        base_url: get_string_setting(conn, AI_PROVIDER_BASE_URL_SETTING)?.unwrap_or_default(),
        model: get_string_setting(conn, AI_PROVIDER_MODEL_SETTING)?.unwrap_or_default(),
        temperature: get_string_setting(conn, AI_PROVIDER_TEMPERATURE_SETTING)?
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.1),
        has_api_key: key.is_some(),
        api_key_hint: key.map(|record| {
            let fingerprint = record.api_key_fingerprint;
            format!("指纹 {}", fingerprint.chars().take(8).collect::<String>())
        }),
        updated_at,
    })
}

pub(super) fn max_ai_setting_updated_at(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT MAX(updated_at)
             FROM (
               SELECT updated_at FROM app_settings WHERE key LIKE 'ai_provider.%'
               UNION ALL
               SELECT updated_at FROM ai_provider_credentials WHERE id = ?1
             )",
            [AI_PROVIDER_ID],
            |row| row.get::<_, Option<String>>(0),
        )?
        .filter(|value| !value.trim().is_empty()))
}

pub(super) fn ai_api_key_record(conn: &Connection) -> AppResult<Option<AiApiKeyRecord>> {
    conn.query_row(
        "SELECT encrypted_api_key, api_key_nonce, api_key_fingerprint
         FROM ai_provider_credentials WHERE id = ?1",
        [AI_PROVIDER_ID],
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

pub(super) fn save_ai_api_key(app: &AppHandle, conn: &Connection, api_key: &str) -> AppResult<()> {
    let encrypted = encrypt_secret(app, api_key)?;
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
            AI_PROVIDER_ID,
            encrypted.ciphertext,
            encrypted.nonce,
            encrypted.key_version,
            secret_fingerprint(api_key),
            now_shanghai()
        ],
    )?;
    Ok(())
}

pub(super) fn load_optional_ai_provider_config(
    app: &AppHandle,
) -> AppResult<Option<AiProviderConfig>> {
    let conn = open_connection(app)?;
    if !get_bool_setting(&conn, AI_PROVIDER_ENABLED_SETTING, false)? {
        return Ok(None);
    }
    let Some(record) = ai_api_key_record(&conn)? else {
        return Ok(None);
    };
    let base_url = get_string_setting(&conn, AI_PROVIDER_BASE_URL_SETTING)?.unwrap_or_default();
    let model = get_string_setting(&conn, AI_PROVIDER_MODEL_SETTING)?.unwrap_or_default();
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(AiProviderConfig {
        provider_type: get_string_setting(&conn, AI_PROVIDER_TYPE_SETTING)?
            .unwrap_or_else(|| "openai_compatible".to_string()),
        base_url,
        model,
        temperature: get_string_setting(&conn, AI_PROVIDER_TEMPERATURE_SETTING)?
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.1),
        api_key: decrypt_secret(app, &record.encrypted_api_key, &record.api_key_nonce)?,
    }))
}

pub(super) fn load_ai_provider_config(app: &AppHandle) -> AppResult<AiProviderConfig> {
    load_optional_ai_provider_config(app)?.ok_or_else(|| {
        AppError::Validation(
            "AI provider 未启用或配置不完整，请先在桌面端保存 base_url、model 和 API Key"
                .to_string(),
        )
    })
}

pub(super) fn ai_chat_completions_url(config: &AiProviderConfig) -> String {
    if config.base_url.ends_with("/chat/completions") {
        config.base_url.clone()
    } else {
        format!("{}/chat/completions", config.base_url.trim_end_matches('/'))
    }
}

pub(super) async fn request_ai_chat_json(
    config: &AiProviderConfig,
    instruction: &str,
    input: &Value,
) -> AppResult<Value> {
    let request_body = serde_json::json!({
        "model": config.model,
        "temperature": config.temperature,
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": "你是微信小店商品发布属性补齐助手。只返回 JSON，不输出 Markdown。不能确定时返回 null 或低 confidence。"
            },
            {
                "role": "user",
                "content": format!("{}\n输入：{}", instruction, input)
            }
        ]
    });
    let response = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()?
        .post(ai_chat_completions_url(config))
        .bearer_auth(&config.api_key)
        .json(&request_body)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Validation(format!(
            "AI provider 调用失败：HTTP {} {}",
            status.as_u16(),
            truncate_for_summary(&body, 240)
        )));
    }
    let raw = response.json::<Value>().await?;
    let content = raw
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AppError::Validation("AI provider 响应缺少 choices[0].message.content".to_string())
        })?;
    parse_model_json_content(content)
}

pub(super) fn parse_model_json_content(content: &str) -> AppResult<Value> {
    let trimmed = content.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    serde_json::from_str::<Value>(without_fence)
        .map_err(|error| AppError::Validation(format!("AI provider 未返回合法 JSON：{error}")))
}

pub(super) fn response_summary_for_ai(value: &Value) -> String {
    truncate_for_summary(&value.to_string(), 120)
}

pub(super) fn truncate_for_summary(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let summary = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

pub(super) async fn fill_attribute_plan_with_ai(
    config: &AiProviderConfig,
    plan: &mut AttributeFillPlan,
) -> AppResult<i64> {
    let mut generated = 0i64;
    for suggestion in &mut plan.suggestions {
        if suggestion.applied || suggestion.source != "needs_ai" {
            continue;
        }
        if let Some(ai_suggestion) = request_ai_attribute_suggestion(config, suggestion).await? {
            *suggestion = ai_suggestion;
            generated += 1;
        }
    }
    Ok(generated)
}

pub(super) async fn request_ai_attribute_suggestion(
    config: &AiProviderConfig,
    base: &AttributeFillSuggestion,
) -> AppResult<Option<AttributeFillSuggestion>> {
    let response = request_ai_chat_json(
        config,
        "请补齐单个微信小店发品必填属性。返回格式：{\"value\":\"属性值或null\",\"sku_values\":[{\"sku_index\":0,\"value\":\"值\"}],\"confidence\":0-100,\"reason\":\"一句话原因\"}。销售属性如果每个 SKU 不同，优先返回 sku_values。",
        &base.prompt_json,
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

pub(super) fn response_allowed_values(prompt_json: &Value) -> Vec<String> {
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

pub(super) fn suggestion_values_allowed(
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

pub(super) fn get_string_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
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

pub(super) fn set_string_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
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

pub(super) fn get_bool_setting(
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

pub(super) fn default_automation_settings() -> OperationalAutomationSettings {
    OperationalAutomationSettings {
        order_sync_enabled: true,
        order_detail_sync_enabled: true,
        aftersale_sync_enabled: true,
        purchase_task_enabled: true,
        delivery_submission_enabled: true,
        publish_precheck_enabled: true,
        publish_attribute_fill_enabled: true,
        publish_category_precheck_enabled: true,
        publish_asset_upload_enabled: true,
        publish_submit_enabled: true,
        publish_status_sync_enabled: true,
        publish_listing_enabled: true,
        price_confirm_enabled: true,
    }
}

pub(super) fn load_automation_settings(
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
        publish_precheck_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_PRECHECK_SETTING,
            defaults.publish_precheck_enabled,
        )?,
        publish_attribute_fill_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_ATTRIBUTE_FILL_SETTING,
            defaults.publish_attribute_fill_enabled,
        )?,
        publish_category_precheck_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_CATEGORY_PRECHECK_SETTING,
            defaults.publish_category_precheck_enabled,
        )?,
        publish_asset_upload_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_ASSET_UPLOAD_SETTING,
            defaults.publish_asset_upload_enabled,
        )?,
        publish_submit_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_SUBMIT_SETTING,
            defaults.publish_submit_enabled,
        )?,
        publish_status_sync_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_STATUS_SYNC_SETTING,
            defaults.publish_status_sync_enabled,
        )?,
        publish_listing_enabled: get_bool_setting(
            conn,
            AUTOMATION_PUBLISH_LISTING_SETTING,
            defaults.publish_listing_enabled,
        )?,
        price_confirm_enabled: get_bool_setting(
            conn,
            AUTOMATION_PRICE_CONFIRM_SETTING,
            defaults.price_confirm_enabled,
        )?,
    })
}

pub(super) fn save_automation_settings(
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
        AUTOMATION_PUBLISH_PRECHECK_SETTING,
        settings.publish_precheck_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_ATTRIBUTE_FILL_SETTING,
        settings.publish_attribute_fill_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_CATEGORY_PRECHECK_SETTING,
        settings.publish_category_precheck_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_ASSET_UPLOAD_SETTING,
        settings.publish_asset_upload_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_SUBMIT_SETTING,
        settings.publish_submit_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_STATUS_SYNC_SETTING,
        settings.publish_status_sync_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PUBLISH_LISTING_SETTING,
        settings.publish_listing_enabled,
    )?;
    set_bool_setting(
        conn,
        AUTOMATION_PRICE_CONFIRM_SETTING,
        settings.price_confirm_enabled,
    )?;
    Ok(())
}

pub(super) fn push_automation_error(
    result: &mut OperationalAutomationRunResult,
    step: &str,
    error: AppError,
) {
    result.errors.push(AutomationStepError {
        step: step.to_string(),
        error: error.to_string(),
    });
}

pub(super) fn merge_attribute_fill_results(
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

pub(super) fn database_backup_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let database = database_path(app)?;
    let parent = database.parent().ok_or_else(|| {
        AppError::Validation("无法定位数据库所在目录，不能创建备份目录".to_string())
    })?;
    let dir = parent.join("backups");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(super) fn create_database_backup_file(app: &AppHandle, prefix: &str) -> AppResult<BackupInfo> {
    checkpoint_database(app)?;
    let database = database_path(app)?;
    if !database.exists() {
        return Err(AppError::Validation(
            "本地数据库文件不存在，无法备份".to_string(),
        ));
    }
    let backup_dir = database_backup_dir(app)?;
    let timestamp = safe_backup_timestamp();
    let mut backup_path = backup_dir.join(format!("wx-xd-{prefix}-{timestamp}.sqlite"));
    if backup_path.exists() {
        backup_path = backup_dir.join(format!(
            "wx-xd-{prefix}-{timestamp}-{}.sqlite",
            Uuid::new_v4()
        ));
    }
    fs::copy(&database, &backup_path)?;
    let backup = backup_info_from_path(&backup_path)?;
    if !backup.integrity_ok {
        let _ = fs::remove_file(&backup_path);
        return Err(AppError::Validation(format!(
            "备份完成后完整性校验失败：{}",
            backup.integrity_message
        )));
    }
    Ok(backup)
}

pub(super) fn checkpoint_database(app: &AppHandle) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

pub(super) fn backup_info_from_path(path: &Path) -> AppResult<BackupInfo> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()
        .map(DateTime::<Utc>::from)
        .map(format_shanghai)
        .unwrap_or_else(|_| now_shanghai());
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown.sqlite")
        .to_string();
    let id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&file_name)
        .to_string();
    let (integrity_ok, integrity_message) = check_sqlite_integrity(path);
    Ok(BackupInfo {
        id,
        file_name,
        file_path: path.display().to_string(),
        size_bytes: metadata.len(),
        sha256: sha256_file(path)?,
        created_at: modified,
        integrity_ok,
        integrity_message,
    })
}

pub(super) fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn check_sqlite_integrity(path: &Path) -> (bool, String) {
    match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|conn| sqlite_integrity_message(&conn))
    {
        Ok(message) => (message.eq_ignore_ascii_case("ok"), message),
        Err(error) => (false, error.to_string()),
    }
}

pub(super) fn sqlite_integrity_message(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
}

pub(super) fn safe_backup_timestamp() -> String {
    now_shanghai()
        .replace(':', "-")
        .replace('+', "plus")
        .replace('/', "-")
}

pub(super) fn shanghai_date_key() -> String {
    now_shanghai().chars().take(10).collect()
}

pub(super) fn remove_sqlite_sidecars(database: &Path) -> AppResult<()> {
    let file_name = database
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::Validation("数据库路径缺少文件名".to_string()))?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = database.with_file_name(format!("{file_name}{suffix}"));
        if sidecar.exists() {
            fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}

pub(super) fn set_bool_setting(conn: &Connection, key: &str, value: bool) -> AppResult<()> {
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

pub(super) fn save_synced_order(
    conn: &Connection,
    shop_id: &str,
    wechat_order_id: &str,
    wechat_status: i64,
) -> AppResult<()> {
    let now = now_shanghai();
    let internal_status = match wechat_status {
        20 | 21 => "pending_shipment",
        30 => "wechat_shipped",
        100 => "completed",
        250 => "cancelled",
        _ => "synced",
    };
    conn.execute(
        "INSERT INTO orders
         (id, shop_id, wechat_order_id, wechat_status, status, raw_payload, created_at, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7)
         ON CONFLICT(shop_id, wechat_order_id) DO UPDATE SET
           wechat_status = excluded.wechat_status,
           status = excluded.status,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("order-{}-{}", shop_id, wechat_order_id),
            shop_id,
            wechat_order_id,
            wechat_status,
            internal_status,
            serde_json::json!({
                "order_id": wechat_order_id,
                "status": wechat_status
            })
            .to_string(),
            now
        ],
    )?;
    Ok(())
}

pub(super) fn mark_order_detail_failed(
    app: &AppHandle,
    item: &OrderDetailSyncItem,
    detail_error: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE orders SET detail_error = ?1, updated_at = ?2 WHERE id = ?3",
        params![detail_error, now_shanghai(), item.order_id],
    )?;
    upsert_notification(
        &conn,
        "critical",
        "order_detail_sync",
        &item.order_id,
        Some(&item.shop_id),
        "订单详情同步失败",
        &format!(
            "订单 {} 详情同步失败：{}",
            item.wechat_order_id, detail_error
        ),
        Some(&serde_json::json!({
            "order_id": &item.order_id,
            "wechat_order_id": &item.wechat_order_id,
            "shop_id": &item.shop_id
        })),
    )?;
    Ok(())
}

pub(super) fn save_order_detail(
    conn: &Connection,
    item: &OrderDetailSyncItem,
    order: &Value,
) -> AppResult<i64> {
    let now = now_shanghai();
    let wechat_status = order.get("status").and_then(Value::as_i64);
    let order_created_at = order.get("create_time").and_then(Value::as_i64);
    let order_updated_at = order.get("update_time").and_then(Value::as_i64);
    let internal_status = wechat_status
        .map(order_status_from_wechat)
        .unwrap_or("pending_shipment");
    let summary_payload = sanitize_order_payload(order);

    conn.execute(
        "UPDATE orders
         SET wechat_status = COALESCE(?1, wechat_status),
             status = ?2,
             raw_payload = ?3,
             order_created_at = ?4,
             order_updated_at = ?5,
             detail_synced_at = ?6,
             detail_error = NULL,
             updated_at = ?6
         WHERE id = ?7",
        params![
            wechat_status,
            internal_status,
            summary_payload.to_string(),
            order_created_at,
            order_updated_at,
            now,
            item.order_id
        ],
    )?;

    let product_infos = order
        .pointer("/order_detail/product_infos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut saved_items = 0i64;
    for (index, product) in product_infos.iter().enumerate() {
        let wechat_product_id = json_value_to_string(product.get("product_id"));
        let wechat_sku_id = json_value_to_string(product.get("sku_id"));
        let out_product_id = product
            .get("out_product_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let out_sku_id = product
            .get("out_sku_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let title = product
            .get("title")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let sku_count = product
            .get("sku_cnt")
            .or_else(|| product.get("sku_count"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let sale_price = product.get("sale_price").and_then(Value::as_i64);
        let real_price = product.get("real_price").and_then(Value::as_i64);
        let item_id = format!("order-item-{}-{}", item.order_id, index);
        conn.execute(
            "INSERT INTO order_items
             (id, order_id, shop_id, wechat_order_id, wechat_product_id, wechat_sku_id,
              out_product_id, out_sku_id, title, sku_count, sale_price, real_price,
              raw_payload, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
             ON CONFLICT(id) DO UPDATE SET
               wechat_product_id = excluded.wechat_product_id,
               wechat_sku_id = excluded.wechat_sku_id,
               out_product_id = excluded.out_product_id,
               out_sku_id = excluded.out_sku_id,
               title = excluded.title,
               sku_count = excluded.sku_count,
               sale_price = excluded.sale_price,
               real_price = excluded.real_price,
               raw_payload = excluded.raw_payload,
               updated_at = excluded.updated_at",
            params![
                item_id,
                item.order_id,
                item.shop_id,
                item.wechat_order_id,
                wechat_product_id,
                wechat_sku_id,
                out_product_id,
                out_sku_id,
                title,
                sku_count,
                sale_price,
                real_price,
                sanitize_order_item_payload(product).to_string(),
                now
            ],
        )?;
        saved_items += 1;
    }
    Ok(saved_items)
}

pub(super) fn save_failed_aftersale(
    conn: &Connection,
    shop_id: &str,
    wechat_aftersale_id: &str,
    reason: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO aftersales
         (id, shop_id, wechat_aftersale_id, status, reason, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'sync_failed', ?4, ?5, ?6, ?6)
         ON CONFLICT(shop_id, wechat_aftersale_id) DO UPDATE SET
           status = excluded.status,
           reason = excluded.reason,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("aftersale-{shop_id}-{wechat_aftersale_id}"),
            shop_id,
            wechat_aftersale_id,
            reason,
            serde_json::json!({
                "after_sale_order_id": wechat_aftersale_id,
                "error": reason
            })
            .to_string(),
            now
        ],
    )?;
    upsert_notification(
        conn,
        "critical",
        "aftersale",
        wechat_aftersale_id,
        Some(shop_id),
        "售后详情同步失败",
        &format!("售后单 {wechat_aftersale_id} 同步失败：{reason}"),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "wechat_aftersale_id": wechat_aftersale_id,
            "reason": reason
        })),
    )?;
    Ok(())
}

pub(super) fn save_synced_aftersale(
    conn: &Connection,
    shop_id: &str,
    after_sale_order: &Value,
) -> AppResult<()> {
    let wechat_aftersale_id = json_value_to_string(after_sale_order.get("after_sale_order_id"))
        .or_else(|| json_value_to_string(after_sale_order.get("aftersale_order_id")))
        .ok_or_else(|| AppError::Validation("售后详情缺少 after_sale_order_id".to_string()))?;
    let status = json_value_to_string(after_sale_order.get("status"))
        .unwrap_or_else(|| "unknown".to_string());
    let aftersale_type = json_value_to_string(after_sale_order.get("type"))
        .or_else(|| json_value_to_string(after_sale_order.get("after_sale_type")));
    let reason = json_value_to_string(after_sale_order.get("reason_text"))
        .or_else(|| json_value_to_string(after_sale_order.get("reason")));
    let wechat_order_id = json_value_to_string(after_sale_order.get("order_id"));
    let local_order_id = match &wechat_order_id {
        Some(wechat_order_id) => conn
            .query_row(
                "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2 LIMIT 1",
                params![shop_id, wechat_order_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let refund_amount_cents = extract_refund_amount_cents(after_sale_order);
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO aftersales
         (id, shop_id, wechat_aftersale_id, order_id, wechat_order_id, status,
          aftersale_type, reason, refund_amount_cents, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(shop_id, wechat_aftersale_id) DO UPDATE SET
           order_id = excluded.order_id,
           wechat_order_id = excluded.wechat_order_id,
           status = excluded.status,
           aftersale_type = excluded.aftersale_type,
           reason = excluded.reason,
           refund_amount_cents = excluded.refund_amount_cents,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("aftersale-{shop_id}-{wechat_aftersale_id}"),
            shop_id,
            wechat_aftersale_id,
            local_order_id,
            wechat_order_id,
            status,
            aftersale_type,
            reason,
            refund_amount_cents,
            sanitize_aftersale_payload(after_sale_order).to_string(),
            now
        ],
    )?;

    if is_active_aftersale_status(&status) {
        upsert_notification(
            conn,
            "warning",
            "aftersale",
            &wechat_aftersale_id,
            Some(shop_id),
            "微信售后待处理",
            &format!(
                "售后单 {} 当前状态为 {}，需人工判断是否同意、拒绝或补充凭证。",
                wechat_aftersale_id, status
            ),
            Some(&serde_json::json!({
                "shop_id": shop_id,
                "wechat_aftersale_id": &wechat_aftersale_id,
                "wechat_order_id": &wechat_order_id,
                "status": &status,
                "reason": &reason
            })),
        )?;
    }

    let local_order_id: Option<String> = conn
        .query_row(
            "SELECT order_id FROM aftersales WHERE shop_id = ?1 AND wechat_aftersale_id = ?2",
            params![shop_id, wechat_aftersale_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if let Some(order_id) = local_order_id {
        if is_active_aftersale_status(&status) {
            conn.execute(
                "UPDATE orders
                 SET status = 'aftersale_active', updated_at = ?1
                 WHERE id = ?2 AND status != 'cancelled'",
                params![now_shanghai(), order_id],
            )?;
        }
        if is_refund_success_aftersale_status(&status) {
            if let Some(amount_cents) = refund_amount_cents.filter(|value| *value > 0) {
                conn.execute(
                    "INSERT INTO order_profit_adjustments
                     (id, order_id, kind, amount_cents, note, created_at)
                     VALUES (?1, ?2, 'refund', ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET
                       amount_cents = excluded.amount_cents,
                       note = excluded.note,
                       created_at = excluded.created_at",
                    params![
                        format!("aftersale-refund-{shop_id}-{wechat_aftersale_id}"),
                        order_id,
                        amount_cents,
                        format!("微信售后退款：{wechat_aftersale_id}"),
                        now_shanghai()
                    ],
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn save_failed_guarantee_order(
    conn: &Connection,
    shop_id: &str,
    guarantee_order_id: &str,
    reason: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO guarantee_orders
         (id, shop_id, guarantee_order_id, status, apply_reason, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'sync_failed', ?4, ?5, ?6, ?6)
         ON CONFLICT(shop_id, guarantee_order_id) DO UPDATE SET
           status = excluded.status,
           apply_reason = excluded.apply_reason,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("guarantee-{shop_id}-{guarantee_order_id}"),
            shop_id,
            guarantee_order_id,
            reason,
            serde_json::json!({
                "guarantee_order_id": guarantee_order_id,
                "error": reason
            })
            .to_string(),
            now
        ],
    )?;
    upsert_notification(
        conn,
        "critical",
        "guarantee_sync",
        guarantee_order_id,
        Some(shop_id),
        "纠纷单详情同步失败",
        &format!("纠纷单 {guarantee_order_id} 同步失败：{reason}"),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "guarantee_order_id": guarantee_order_id,
            "reason": reason
        })),
    )?;
    Ok(())
}

pub(super) fn save_synced_guarantee_order(
    conn: &Connection,
    shop_id: &str,
    guarantee_order: &Value,
) -> AppResult<()> {
    let guarantee_order_id = json_value_to_string(guarantee_order.get("guarantee_order_id"))
        .ok_or_else(|| AppError::Validation("纠纷单详情缺少 guarantee_order_id".to_string()))?;
    let status = json_value_to_string(guarantee_order.get("status"))
        .unwrap_or_else(|| "unknown".to_string());
    let guarantee_type = json_value_to_i64(guarantee_order.get("type"));
    let wechat_order_id = json_value_to_string(guarantee_order.get("order_id"));
    let local_order_id = match &wechat_order_id {
        Some(wechat_order_id) => conn
            .query_row(
                "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2 LIMIT 1",
                params![shop_id, wechat_order_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let apply_reason = json_value_to_string(guarantee_order.get("apply_reason"))
        .or_else(|| json_value_to_string(guarantee_order.get("refund_reason_text")));
    let pay_amount_cents = json_value_to_i64(guarantee_order.get("pay_amount"))
        .or_else(|| json_value_to_i64(guarantee_order.pointer("/bad_pay_info/pay_fee")))
        .or_else(|| {
            json_value_to_i64(guarantee_order.pointer("/fake_one_pay_four_info/total_pay_fee"))
        });
    let merchant_refuse_reason =
        json_value_to_string(guarantee_order.get("merchant_refuse_reason"));
    let created_time = json_value_to_i64(guarantee_order.get("create_time"));
    let updated_time = json_value_to_i64(guarantee_order.get("update_time"));
    let expire_time = json_value_to_i64(guarantee_order.get("expire_time"));
    let complete_time = json_value_to_i64(guarantee_order.get("complete_time"));
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO guarantee_orders
         (id, shop_id, guarantee_order_id, order_id, wechat_order_id, guarantee_type,
          status, apply_reason, pay_amount_cents, merchant_refuse_reason, raw_payload,
          created_time, updated_time, expire_time, complete_time, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)
         ON CONFLICT(shop_id, guarantee_order_id) DO UPDATE SET
           order_id = excluded.order_id,
           wechat_order_id = excluded.wechat_order_id,
           guarantee_type = excluded.guarantee_type,
           status = excluded.status,
           apply_reason = excluded.apply_reason,
           pay_amount_cents = excluded.pay_amount_cents,
           merchant_refuse_reason = excluded.merchant_refuse_reason,
           raw_payload = excluded.raw_payload,
           created_time = excluded.created_time,
           updated_time = excluded.updated_time,
           expire_time = excluded.expire_time,
           complete_time = excluded.complete_time,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("guarantee-{shop_id}-{guarantee_order_id}"),
            shop_id,
            guarantee_order_id,
            local_order_id,
            wechat_order_id,
            guarantee_type,
            status,
            apply_reason,
            pay_amount_cents,
            merchant_refuse_reason,
            sanitize_aftersale_payload(guarantee_order).to_string(),
            created_time,
            updated_time,
            expire_time,
            complete_time,
            now
        ],
    )?;

    if is_active_guarantee_status(&status) {
        upsert_notification(
            conn,
            "critical",
            "guarantee_order",
            &guarantee_order_id,
            Some(shop_id),
            "微信纠纷单待处理",
            &format!(
                "纠纷单 {} 当前状态为 {}（{}），需人工核对凭证、责任方和处理时限。",
                guarantee_order_id,
                status,
                guarantee_status_text(&status)
            ),
            Some(&serde_json::json!({
                "shop_id": shop_id,
                "guarantee_order_id": &guarantee_order_id,
                "wechat_order_id": &wechat_order_id,
                "status": &status,
                "status_text": guarantee_status_text(&status),
                "apply_reason": &apply_reason,
                "pay_amount_cents": pay_amount_cents
            })),
        )?;
    }

    if let Some(order_id) = conn
        .query_row(
            "SELECT order_id FROM guarantee_orders WHERE shop_id = ?1 AND guarantee_order_id = ?2",
            params![shop_id, guarantee_order_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
    {
        if is_active_guarantee_status(&status) {
            conn.execute(
                "UPDATE orders
                 SET status = 'aftersale_active', updated_at = ?1
                 WHERE id = ?2 AND status != 'cancelled'",
                params![now_shanghai(), order_id],
            )?;
        }
    }
    Ok(())
}

pub(super) fn order_status_from_wechat(status: i64) -> &'static str {
    match status {
        20 | 21 => "pending_shipment",
        30 => "wechat_shipped",
        100 => "completed",
        250 => "cancelled",
        _ => "synced",
    }
}

pub(super) fn sanitize_order_payload(order: &Value) -> Value {
    serde_json::json!({
        "order_id": json_value_to_string(order.get("order_id")),
        "status": order.get("status").and_then(Value::as_i64),
        "create_time": order.get("create_time").and_then(Value::as_i64),
        "update_time": order.get("update_time").and_then(Value::as_i64),
        "product_infos": order
            .pointer("/order_detail/product_infos")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(sanitize_order_item_payload).collect::<Vec<_>>())
            .unwrap_or_default()
    })
}

pub(super) fn sanitize_order_item_payload(product: &Value) -> Value {
    serde_json::json!({
        "product_id": product.get("product_id").cloned().unwrap_or(Value::Null),
        "sku_id": product.get("sku_id").cloned().unwrap_or(Value::Null),
        "out_product_id": product.get("out_product_id").cloned().unwrap_or(Value::Null),
        "out_sku_id": product.get("out_sku_id").cloned().unwrap_or(Value::Null),
        "title": product.get("title").cloned().unwrap_or(Value::Null),
        "sku_cnt": product.get("sku_cnt").cloned().unwrap_or(Value::Null),
        "sale_price": product.get("sale_price").cloned().unwrap_or(Value::Null),
        "real_price": product.get("real_price").cloned().unwrap_or(Value::Null),
        "thumb_img": product.get("thumb_img").cloned().unwrap_or(Value::Null)
    })
}

pub(super) fn sanitize_aftersale_payload(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (key, child) in map {
                if is_sensitive_json_key(key) {
                    sanitized.insert(key.clone(), Value::String("[redacted]".to_string()));
                } else {
                    sanitized.insert(key.clone(), sanitize_aftersale_payload(child));
                }
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_aftersale_payload).collect()),
        _ => value.clone(),
    }
}

pub(super) fn is_sensitive_json_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "openid", "open_id", "unionid", "name", "tel", "mobile", "phone", "address", "receiver",
        "contact",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(super) fn json_value_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn json_value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(value)) => value.as_i64(),
        Some(Value::String(value)) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(super) fn json_value_to_bool(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Number(value)) => value.as_i64().map(|value| value != 0),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn json_value_to_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(super) fn extract_refund_amount_cents(after_sale_order: &Value) -> Option<i64> {
    [
        "/refund_info/refund_fee",
        "/refund_info/refund_amount",
        "/refund_info/refund_price",
        "/refund_resp/refund_fee",
        "/refund_resp/refund_amount",
        "/refund_amount",
        "/refund_fee",
    ]
    .iter()
    .find_map(|path| json_value_to_i64(after_sale_order.pointer(path)))
    .or_else(|| {
        after_sale_order
            .pointer("/refund_info/refund_product_infos")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        json_value_to_i64(item.get("refund_amount"))
                            .or_else(|| json_value_to_i64(item.get("refund_fee")))
                    })
                    .sum::<i64>()
            })
            .filter(|sum| *sum > 0)
    })
}

pub(super) fn is_active_aftersale_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_uppercase().as_str(),
        "USER_CANCELD"
            | "USER_CANCELLED"
            | "RETURN_CLOSED"
            | "MERCHANT_REFUND_SUCCESS"
            | "MERCHANT_RETURN_SUCCESS"
            | "MERCHANT_REFUND_RETRY_FAIL"
            | "MERCHANT_FAIL"
            | "MERCHANT_EXCHANGE_SUCCESS"
            | "SYNC_FAILED"
    )
}

pub(super) fn is_refund_success_aftersale_status(status: &str) -> bool {
    matches!(
        status.to_ascii_uppercase().as_str(),
        "MERCHANT_REFUND_SUCCESS" | "MERCHANT_RETURN_SUCCESS"
    )
}

pub(super) fn is_active_guarantee_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_uppercase().as_str(),
        "STATUS_NO_NEED_PAY" | "STATUS_PAY_SUCC" | "STATUS_USER_CANCEL" | "SYNC_FAILED"
    )
}

pub(super) fn guarantee_type_text(guarantee_type: Option<i64>) -> &'static str {
    match guarantee_type {
        Some(1) => "假一赔三/四",
        Some(2) => "坏损包退",
        Some(0) => "全部类型",
        _ => "未知类型",
    }
}

pub(super) fn guarantee_status_text(status: &str) -> &'static str {
    match status.to_ascii_uppercase().as_str() {
        "STATUS_WAIT_MERCHANT_HANDLE" => "等待商家处理",
        "STATUS_WAIT_PLATFORM_HANDLE" => "等待平台处理",
        "STATUS_WAIT_USER_CONFIRM" => "等待用户确认",
        "STATUS_WAIT_MERCHANT_PROOF" => "等待商家举证",
        "STATUS_WAIT_USER_PROOF" => "等待用户举证",
        "STATUS_WAIT_BOTH_PROOF" => "等待双方举证",
        "STATUS_WAIT_OP_COMFIRM" => "等待 OP 确认",
        "STATUS_WAIT_PAYSCORE_DONE" => "等待支付分付款",
        "STATUS_NO_NEED_PAY" => "无需赔付",
        "STATUS_PAYING" => "赔付中",
        "STATUS_PAY_BLOCK" => "赔付金额异常待确认",
        "STATUS_PAY_SUCC" => "赔付成功",
        "STATUS_PAY_FAIL" => "赔付失败",
        "STATUS_USER_CANCEL" => "用户取消申请",
        "SYNC_FAILED" => "同步失败",
        _ => "未知状态",
    }
}

pub(super) fn normalize_guarantee_handling_status(status: &str) -> AppResult<&'static str> {
    match status.trim() {
        "pending" => Ok("pending"),
        "in_progress" => Ok("in_progress"),
        "waiting_supplier" => Ok("waiting_supplier"),
        "evidence_ready" => Ok("evidence_ready"),
        "resolved" => Ok("resolved"),
        "ignored" => Ok("ignored"),
        _ => Err(AppError::Validation(
            "纠纷跟进状态必须是 pending/in_progress/waiting_supplier/evidence_ready/resolved/ignored"
                .to_string(),
        )),
    }
}

pub(super) fn guarantee_handling_status_text(status: &str) -> &'static str {
    match status {
        "pending" => "待跟进",
        "in_progress" => "跟进中",
        "waiting_supplier" => "等供应商",
        "evidence_ready" => "凭证已整理",
        "resolved" => "已处理",
        "ignored" => "无需处理",
        _ => "未知跟进状态",
    }
}

pub(super) fn aftersale_evidence_type_text(evidence_type: &str) -> &'static str {
    match evidence_type {
        "image" => "图片",
        "text" => "文字说明",
        "chat_record" => "沟通记录",
        "logistics" => "物流凭证",
        "supplier_proof" => "供应商凭证",
        "quality_check" => "质检凭证",
        "other" => "其他",
        _ => "未知凭证",
    }
}

pub(super) fn aftersale_evidence_status_text(status: &str) -> &'static str {
    match status {
        "draft" => "草稿",
        "ready" => "已整理",
        "used" => "已使用",
        "archived" => "已归档",
        _ => "未知状态",
    }
}

pub(super) fn supplier_aftersale_followup_type_text(followup_type: &str) -> &'static str {
    match followup_type {
        "contact" => "联系供应商",
        "evidence_request" => "索要凭证",
        "evidence_received" => "收到凭证",
        "compensation" => "赔付沟通",
        "return_refund" => "退货退款",
        "other" => "其他协同",
        _ => "未知协同",
    }
}

pub(super) fn supplier_aftersale_followup_status_text(status: &str) -> &'static str {
    match status {
        "pending" => "待处理",
        "contacted" => "已联系",
        "waiting_supplier" => "等供应商",
        "evidence_ready" => "凭证已备",
        "compensation_pending" => "赔付待确认",
        "closed" => "已关闭",
        _ => "未知状态",
    }
}

pub(super) fn precheck_publish_item(
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

pub(super) fn prepare_add_product_payload_for_publish(
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

pub(super) fn publish_payload_ready_summary(prepare: &AddProductPayloadPrepare) -> String {
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

pub(super) fn persist_generated_add_product_payload(
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

pub(super) fn mark_publish_item_failed(
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

pub(super) fn conn_update_publish_item_error(
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

pub(super) fn set_publish_item_status_in_conn(
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

pub(super) fn publish_failure_notification_severity(error_code: &str) -> &'static str {
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

pub(super) fn mark_publish_item_failed_for_app(
    app: &AppHandle,
    item: &PendingPublishItem,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    mark_publish_item_failed(&conn, item, error_code, error_summary)
}

pub(super) fn set_publish_item_status(
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

pub(super) fn set_publish_status_sync_state(
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

pub(super) fn set_shop_product_item_state(
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

pub(super) fn mark_listing_item_failed(
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

pub(super) fn insert_task_log_for_app(
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

pub(super) fn mark_price_update_item_failed_for_app(
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

pub(super) fn set_price_update_item_pending_for_app(
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

pub(super) fn set_price_update_item_confirmed_for_app(
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

pub(super) fn build_price_update_product_payload(
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

pub(super) fn sanitize_product_update_payload(payload: &mut serde_json::Map<String, Value>) {
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

pub(super) fn require_price_update_field(
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

pub(super) fn collect_product_assets(product: &ExternalProductInput) -> Vec<ProductAsset> {
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

pub(super) fn load_prepared_assets(
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

pub(super) fn resolve_add_product_base_payload(
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

pub(super) fn generate_add_product_payload_draft(
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

pub(super) fn product_metadata_object(
    product: &ExternalProductInput,
) -> Result<Option<&serde_json::Map<String, Value>>, String> {
    match &product.metadata {
        Value::Object(metadata) => Ok(Some(metadata)),
        Value::Null => Ok(None),
        _ => Err("metadata 必须是对象；请传 {} 或包含微信发品参数的对象".to_string()),
    }
}

pub(super) fn apply_add_product_defaults(
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

pub(super) fn insert_add_product_categories(
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
        "缺少微信类目 ID。AI/外部系统需要根据 category_hint={category_hint}、标题和图片产出 metadata.wechat_category_ids，例如 [一级cat_id, 二级cat_id, 三级或叶子cat_id]；也可以直接传 metadata.wechat_add_product_payload"
    ))
}

pub(super) fn normalize_category_objects(value: &Value, field: &str) -> Result<Value, String> {
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

pub(super) fn category_ids_from_value(value: &Value) -> Vec<i64> {
    match value {
        Value::Array(items) => items.iter().filter_map(category_id_from_value).collect(),
        _ => Vec::new(),
    }
}

pub(super) fn extract_leaf_category_id_from_payload(payload: &Value) -> Option<i64> {
    payload
        .get("cats_v2")
        .or_else(|| payload.get("cats"))
        .map(category_ids_from_value)
        .and_then(|ids| ids.into_iter().last())
}

pub(super) fn category_id_from_value(value: &Value) -> Option<i64> {
    match value {
        Value::Object(object) => json_value_to_i64(object.get("cat_id")),
        _ => json_value_to_i64(Some(value)),
    }
    .filter(|cat_id| *cat_id > 0)
}

pub(super) fn resolve_extra_service(metadata: Option<&serde_json::Map<String, Value>>) -> Value {
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

pub(super) fn resolve_express_info(
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

pub(super) fn build_add_product_skus(
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

pub(super) fn sku_attrs_from_specs(
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

pub(super) fn sku_attr_value_to_string(value: &Value) -> Option<String> {
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

pub(super) fn validate_sku_attr_text(
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

pub(super) fn resolve_sku_sale_price_cents(
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

pub(super) fn validate_sale_price_cents(price: i64, external_sku_id: &str) -> Result<i64, String> {
    if price <= 0 {
        return Err(format!("SKU {external_sku_id} 的 sale_price 必须大于 0"));
    }
    if price > 1_000_000_000 {
        return Err(format!("SKU {external_sku_id} 的 sale_price 超过微信上限"));
    }
    Ok(price)
}

pub(super) fn validate_add_product_base_payload(
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

pub(super) fn validate_add_product_skus(
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

pub(super) fn metadata_value<'a>(
    metadata: Option<&'a serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<&'a Value> {
    let metadata = metadata?;
    keys.iter().find_map(|key| metadata.get(*key))
}

pub(super) fn metadata_object_clone(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<serde_json::Map<String, Value>> {
    metadata_value(metadata, keys)
        .and_then(Value::as_object)
        .cloned()
}

pub(super) fn metadata_array_clone(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<Vec<Value>> {
    metadata_value(metadata, keys)
        .and_then(Value::as_array)
        .cloned()
}

pub(super) fn metadata_string(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    metadata_value(metadata, keys)
        .and_then(|value| json_value_to_string(Some(value)))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn metadata_i64(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<i64> {
    metadata_value(metadata, keys).and_then(|value| json_value_to_i64(Some(value)))
}

pub(super) fn metadata_f64(
    metadata: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<f64> {
    metadata_value(metadata, keys).and_then(|value| json_value_to_f64(Some(value)))
}

pub(super) fn build_add_product_payload(
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

pub(super) fn prepared_asset_urls(assets: &[PreparedAsset], kind: &str) -> Vec<String> {
    let mut values = assets
        .iter()
        .filter(|asset| asset.kind == kind)
        .map(|asset| (asset.sort_order, asset.wechat_url.clone()))
        .collect::<Vec<_>>();
    values.sort_by_key(|(sort_order, _)| *sort_order);
    values.into_iter().map(|(_, url)| url).collect()
}

pub(super) fn upsert_desc_images(
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

pub(super) fn require_add_product_field(
    payload: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match payload.get(field) {
        Some(Value::Null) | None => Err(format!("缺少 {field}，需先补齐微信发品参数")),
        _ => Ok(()),
    }
}

pub(super) fn validate_array_field(
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

pub(super) fn validate_extra_service(
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

pub(super) fn resolve_wechat_product_status(info: &ProductGetInfo) -> ProductAuditResolution {
    let wechat_status = info
        .product
        .as_ref()
        .and_then(|product| product.status)
        .or_else(|| {
            info.edit_product
                .as_ref()
                .and_then(|product| product.status)
        });
    let wechat_edit_status = info
        .edit_product
        .as_ref()
        .and_then(|product| product.edit_status)
        .or_else(|| {
            info.product
                .as_ref()
                .and_then(|product| product.edit_status)
        });

    let status_label = wechat_product_status_label(wechat_status);
    let edit_status_label = wechat_product_edit_status_label(wechat_edit_status);
    let summary = format!("微信商品状态：{status_label}；编辑状态：{edit_status_label}");

    if wechat_status == Some(5) {
        return ProductAuditResolution {
            status: "success",
            error_code: None,
            summary: format!("{summary}。商品已上架"),
            wechat_status,
            wechat_edit_status,
        };
    }

    if let Some(code) = wechat_terminal_failure_code(wechat_status, wechat_edit_status) {
        return ProductAuditResolution {
            status: "failed",
            error_code: Some(format!("WECHAT_PRODUCT_STATUS_{code}")),
            summary: format!("{summary}。需要按微信返回原因处理后重试"),
            wechat_status,
            wechat_edit_status,
        };
    }

    if wechat_status == Some(4) || wechat_edit_status == Some(4) {
        return ProductAuditResolution {
            status: "audit_passed",
            error_code: None,
            summary: format!("{summary}。审核已通过，后续可进入上架"),
            wechat_status,
            wechat_edit_status,
        };
    }

    ProductAuditResolution {
        status: "audit_pending",
        error_code: None,
        summary: format!("{summary}。仍需继续轮询"),
        wechat_status,
        wechat_edit_status,
    }
}

pub(super) fn resolve_price_update_confirmation(
    info: &ProductGetInfo,
    target_price_cents: i64,
) -> PriceUpdateConfirmationResolution {
    let wechat_status = info
        .product
        .as_ref()
        .and_then(|product| product.status)
        .or_else(|| {
            info.edit_product
                .as_ref()
                .and_then(|product| product.status)
        });
    let wechat_edit_status = info
        .edit_product
        .as_ref()
        .and_then(|product| product.edit_status)
        .or_else(|| {
            info.product
                .as_ref()
                .and_then(|product| product.edit_status)
        });

    let status_label = wechat_product_status_label(wechat_status);
    let edit_status_label = wechat_product_edit_status_label(wechat_edit_status);
    let target_price = format_price_cents(target_price_cents);
    let summary_prefix = format!(
        "微信商品状态：{status_label}；编辑状态：{edit_status_label}；目标价：{target_price} 元"
    );

    if let Some(code) = wechat_terminal_failure_code(wechat_status, wechat_edit_status) {
        return PriceUpdateConfirmationResolution {
            status: "failed",
            error_code: Some(format!("WECHAT_PRODUCT_STATUS_{code}")),
            summary: format!(
                "{summary_prefix}。微信商品或编辑状态已进入失败状态，需要人工处理后重试"
            ),
            wechat_status,
            wechat_edit_status,
        };
    }

    let online_prices = info
        .product
        .as_ref()
        .map(|product| snapshot_sku_sale_prices(product));
    if matches!(online_prices.as_ref(), Some(Ok(prices)) if all_prices_match(prices, target_price_cents))
    {
        return PriceUpdateConfirmationResolution {
            status: "success",
            error_code: None,
            summary: format!("{summary_prefix}。线上 product.skus[].sale_price 已全部匹配目标价"),
            wechat_status,
            wechat_edit_status,
        };
    }

    let draft_prices = info
        .edit_product
        .as_ref()
        .map(|product| snapshot_sku_sale_prices(product));
    if matches!(draft_prices.as_ref(), Some(Ok(prices)) if all_prices_match(prices, target_price_cents))
    {
        return PriceUpdateConfirmationResolution {
            status: "audit_pending",
            error_code: None,
            summary: format!(
                "{summary_prefix}。edit_product.skus[].sale_price 已匹配目标价，线上 product 价格尚未生效，继续轮询"
            ),
            wechat_status,
            wechat_edit_status,
        };
    }

    let online_error = online_prices
        .as_ref()
        .and_then(|result| result.as_ref().err());
    let draft_error = draft_prices
        .as_ref()
        .and_then(|result| result.as_ref().err());
    if online_prices.is_none() && draft_prices.is_none() {
        return PriceUpdateConfirmationResolution {
            status: "failed",
            error_code: Some("PRICE_CONFIRM_PRODUCT_MISSING".to_string()),
            summary: format!("{summary_prefix}。微信 getproduct 未返回 product 或 edit_product，无法确认改价结果"),
            wechat_status,
            wechat_edit_status,
        };
    }
    if online_prices.is_none() && draft_error.is_some() {
        let detail = draft_error
            .map(|error| error.as_str())
            .unwrap_or("SKU 价格不可读");
        return PriceUpdateConfirmationResolution {
            status: "failed",
            error_code: Some("PRICE_CONFIRM_SKUS_MISSING".to_string()),
            summary: format!("{summary_prefix}。{detail}，无法确认改价结果"),
            wechat_status,
            wechat_edit_status,
        };
    }
    if online_error.is_some() && (draft_prices.is_none() || draft_error.is_some()) {
        let detail = online_error
            .or(draft_error)
            .map(|error| error.as_str())
            .unwrap_or("SKU 价格不可读");
        return PriceUpdateConfirmationResolution {
            status: "failed",
            error_code: Some("PRICE_CONFIRM_SKUS_MISSING".to_string()),
            summary: format!("{summary_prefix}。{detail}，无法确认改价结果"),
            wechat_status,
            wechat_edit_status,
        };
    }

    PriceUpdateConfirmationResolution {
        status: "audit_pending",
        error_code: None,
        summary: format!(
            "{summary_prefix}。线上 product.skus[].sale_price 暂未全部匹配目标价，稍后重试"
        ),
        wechat_status,
        wechat_edit_status,
    }
}

pub(super) fn snapshot_sku_sale_prices(
    snapshot: &WechatProductSnapshot,
) -> Result<Vec<i64>, String> {
    let skus = snapshot
        .extra
        .get("skus")
        .and_then(Value::as_array)
        .ok_or_else(|| "微信商品缺少 skus".to_string())?;
    if skus.is_empty() {
        return Err("微信商品 SKU 为空".to_string());
    }
    skus.iter()
        .enumerate()
        .map(|(index, sku)| {
            sku.get("sale_price")
                .and_then(value_to_i64)
                .ok_or_else(|| format!("第 {} 个 SKU 缺少可解析的 sale_price", index + 1))
        })
        .collect()
}

pub(super) fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
}

pub(super) fn all_prices_match(prices: &[i64], target_price_cents: i64) -> bool {
    !prices.is_empty() && prices.iter().all(|price| *price == target_price_cents)
}

pub(super) fn format_price_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

pub(super) fn wechat_terminal_failure_code(
    status: Option<i64>,
    edit_status: Option<i64>,
) -> Option<i64> {
    if let Some(code) = edit_status {
        if matches!(code, 3 | 8 | 72 | 73) {
            return Some(code);
        }
    }
    if let Some(code) = status {
        if matches!(
            code,
            3 | 8 | 10 | 13 | 14 | 15 | 20 | 21 | 30 | 71 | 72 | 73
        ) {
            return Some(code);
        }
    }
    None
}

pub(super) fn wechat_product_status_label(status: Option<i64>) -> String {
    match status {
        Some(0) => "0 初始值".to_string(),
        Some(1) => "1 编辑中".to_string(),
        Some(2) => "2 审核中".to_string(),
        Some(3) => "3 审核失败".to_string(),
        Some(4) => "4 审核成功".to_string(),
        Some(5) => "5 已上架".to_string(),
        Some(6) => "6 回收站".to_string(),
        Some(7) => "7 异步上传中".to_string(),
        Some(8) => "8 异步上传失败".to_string(),
        Some(9) => "9 彻底删除".to_string(),
        Some(10) => "10 冻结，审核通过但不能上架".to_string(),
        Some(11) => "11 自主下架".to_string(),
        Some(12) => "12 售罄下架".to_string(),
        Some(13) => "13 违规/风控下架".to_string(),
        Some(14) => "14 保证金不足下架".to_string(),
        Some(15) => "15 品牌过期下架".to_string(),
        Some(20) => "20 商品被封禁".to_string(),
        Some(21) => "21 SKU 逻辑删除".to_string(),
        Some(30) => "30 商品不存在".to_string(),
        Some(70) => "70 异步提审中".to_string(),
        Some(71) => "71 质检不通过".to_string(),
        Some(72) => "72 当日 quota 不足".to_string(),
        Some(73) => "73 限频触发".to_string(),
        Some(code) => format!("{code} 未知状态"),
        None => "未返回".to_string(),
    }
}

pub(super) fn wechat_product_edit_status_label(status: Option<i64>) -> String {
    match status {
        Some(0) => "0 初始值".to_string(),
        Some(1) => "1 编辑中".to_string(),
        Some(2) => "2 审核中".to_string(),
        Some(3) => "3 审核失败".to_string(),
        Some(4) => "4 审核成功".to_string(),
        Some(7) => "7 异步上传中".to_string(),
        Some(8) => "8 异步上传失败".to_string(),
        Some(70) => "70 异步提审中".to_string(),
        Some(72) => "72 当日 quota 不足".to_string(),
        Some(73) => "73 限频触发".to_string(),
        Some(code) => format!("{code} 未知编辑状态"),
        None => "未返回".to_string(),
    }
}

pub(super) async fn upload_or_reuse_asset(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    item: &PendingPublishItem,
    asset: &ProductAsset,
) -> AppResult<AssetUploadOutcome> {
    if let Some(error_summary) = validate_image_source_url(&asset.source_url) {
        record_asset_failure(app, item, asset, "INVALID_IMAGE_SOURCE_URL", &error_summary)?;
        mark_publish_item_failed_for_app(app, item, "INVALID_IMAGE_SOURCE_URL", &error_summary)?;
        return Ok(AssetUploadOutcome::Failed);
    }

    if is_wechat_image_url(&asset.source_url) {
        record_asset_success(app, item, asset, &asset.source_url, "reused")?;
        return Ok(AssetUploadOutcome::Reused);
    }

    if let Some(wechat_url) = find_cached_asset_url(app, &item.shop_id, &asset.source_url)? {
        record_asset_success(app, item, asset, &wechat_url, "reused")?;
        return Ok(AssetUploadOutcome::Reused);
    }

    let prepared = match prepare_image_for_upload(app, &asset.source_url).await {
        Ok(prepared) => prepared,
        Err(error_summary) => {
            record_asset_failure(app, item, asset, "IMAGE_PREPROCESS_FAILED", &error_summary)?;
            mark_publish_item_failed_for_app(app, item, "IMAGE_PREPROCESS_FAILED", &error_summary)?;
            return Ok(AssetUploadOutcome::Failed);
        }
    };

    let call = match client
        .upload_image_bytes(
            access_token,
            prepared.bytes,
            &prepared.file_name,
            prepared.mime_type,
            prepared.width,
            prepared.height,
        )
        .await
    {
        Ok(call) => call,
        Err(error) => {
            let error_summary = format!("微信图片上传请求失败：{error}");
            record_asset_failure(
                app,
                item,
                asset,
                "WECHAT_IMAGE_UPLOAD_HTTP_FAILED",
                &error_summary,
            )?;
            mark_publish_item_failed_for_app(
                app,
                item,
                "WECHAT_IMAGE_UPLOAD_HTTP_FAILED",
                &error_summary,
            )?;
            return Ok(AssetUploadOutcome::Failed);
        }
    };

    let conn = open_connection(app)?;
    match &call.result {
        WechatCallResult::Success(result) => {
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!("image upload ok, {}", prepared.summary)),
            )?;
            drop(conn);
            record_asset_success(app, item, asset, &result.img_url, "success")?;
            insert_task_log_for_app(
                app,
                &item.job_id,
                Some(&item.item_id),
                "info",
                &format!("素材预处理完成：{}", prepared.summary),
                Some(&serde_json::json!({
                    "asset_kind": asset.kind,
                    "sort_order": asset.sort_order,
                    "local_path": prepared.local_path.display().to_string()
                })),
            )?;
            Ok(AssetUploadOutcome::Uploaded)
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&item.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("image upload api error"),
            )?;
            drop(conn);
            let error_code = format!("WECHAT_IMAGE_UPLOAD_{}", error.errcode);
            let error_summary = format!("微信图片上传失败：{}", error.errmsg);
            record_asset_failure(app, item, asset, &error_code, &error_summary)?;
            mark_publish_item_failed_for_app(app, item, &error_code, &error_summary)?;
            Ok(AssetUploadOutcome::Failed)
        }
    }
}

pub(super) fn validate_image_source_url(source_url: &str) -> Option<String> {
    let parsed = match Url::parse(source_url) {
        Ok(parsed) => parsed,
        Err(error) => return Some(format!("图片 URL 不合法：{error}")),
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Some("图片 URL 必须使用 http 或 https".to_string());
    }
    if parsed.host_str().is_none() {
        return Some("图片 URL 缺少域名".to_string());
    }
    if parsed.path().contains("//") {
        return Some("图片 URL 路径不能包含连续 //，微信图片接口不支持该格式".to_string());
    }
    if let Some(host) = parsed.host_str().map(|host| host.to_ascii_lowercase()) {
        if host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" {
            return Some("图片 URL 不能指向 localhost 或 127.0.0.1".to_string());
        }
    }
    None
}

pub(super) async fn prepare_image_for_upload(
    app: &AppHandle,
    source_url: &str,
) -> Result<PreparedImageUpload, String> {
    let http = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(StdDuration::from_secs(IMAGE_DOWNLOAD_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("初始化图片下载客户端失败：{error}"))?;
    let response = http
        .get(source_url)
        .header(USER_AGENT, IMAGE_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("下载图片失败：{error}"))?;
    let status = response.status();
    if status.is_redirection() {
        let location = response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        return Err(if location.is_empty() {
            format!("图片 URL 返回 {status} 跳转，微信 URL 上传不支持 301/302")
        } else {
            format!("图片 URL 返回 {status} 跳转到 {location}，微信 URL 上传不支持 301/302")
        });
    }
    if !status.is_success() {
        return Err(format!("图片 URL 打开失败：HTTP {status}"));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or(value)
                .trim()
                .to_ascii_lowercase()
        });
    if matches!(content_type.as_deref(), Some("text/xml" | "image/svg+xml")) {
        return Err("SVG 暂不支持本地规范化，请先转为 PNG/JPEG 再铺货".to_string());
    }
    if let Some(content_length) = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        if content_length > IMAGE_DOWNLOAD_MAX_BYTES {
            return Err(format!(
                "图片过大：{}，超过本地下载上限 {}",
                format_bytes_short(content_length as usize),
                format_bytes_short(IMAGE_DOWNLOAD_MAX_BYTES as usize)
            ));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取图片内容失败：{error}"))?;
    if bytes.is_empty() {
        return Err("图片内容为空".to_string());
    }
    if bytes.len() > IMAGE_DOWNLOAD_MAX_BYTES as usize {
        return Err(format!(
            "图片过大：{}，超过本地下载上限 {}",
            format_bytes_short(bytes.len()),
            format_bytes_short(IMAGE_DOWNLOAD_MAX_BYTES as usize)
        ));
    }

    let format =
        image::guess_format(&bytes).map_err(|error| format!("图片格式无法识别：{error}"))?;
    let image = image::load_from_memory_with_format(&bytes, format)
        .map_err(|error| format!("图片解码失败：{error}"))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("图片宽高无效".to_string());
    }

    let (upload_bytes, upload_width, upload_height, mime_type, extension, summary_suffix) =
        if let Some(mime_type) = original_upload_mime(format) {
            if bytes.len() <= WECHAT_IMAGE_MAX_BYTES {
                (
                    bytes.to_vec(),
                    width,
                    height,
                    mime_type,
                    original_upload_extension(format).unwrap_or("img"),
                    format!("保留原格式 {}", image_format_label(format)),
                )
            } else {
                let normalized = encode_jpeg_under_limit(image)?;
                let summary_suffix = format!(
                    "已压缩转 JPEG，{} -> {}",
                    format_bytes_short(bytes.len()),
                    format_bytes_short(normalized.0.len())
                );
                (
                    normalized.0,
                    normalized.1,
                    normalized.2,
                    "image/jpeg",
                    "jpg",
                    summary_suffix,
                )
            }
        } else {
            let normalized = encode_jpeg_under_limit(image)?;
            let summary_suffix = format!(
                "{} 已转 JPEG，{} -> {}",
                image_format_label(format),
                format_bytes_short(bytes.len()),
                format_bytes_short(normalized.0.len())
            );
            (
                normalized.0,
                normalized.1,
                normalized.2,
                "image/jpeg",
                "jpg",
                summary_suffix,
            )
        };

    if upload_bytes.len() > WECHAT_IMAGE_MAX_BYTES {
        return Err(format!(
            "图片规范化后仍超过微信 10MB 限制：{}",
            format_bytes_short(upload_bytes.len())
        ));
    }

    let hash = hex_sha256(source_url.as_bytes());
    let dir = image_cache_dir(app).map_err(|error| format!("创建图片缓存目录失败：{error}"))?;
    let local_path = dir.join(format!("{hash}.{extension}"));
    fs::write(&local_path, &upload_bytes).map_err(|error| format!("写入图片缓存失败：{error}"))?;
    let file_name = local_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("wx-xd-image.jpg")
        .to_string();
    let summary = format!(
        "{}x{}，{}，{}",
        upload_width,
        upload_height,
        format_bytes_short(upload_bytes.len()),
        summary_suffix
    );

    Ok(PreparedImageUpload {
        bytes: upload_bytes,
        width: upload_width,
        height: upload_height,
        mime_type,
        file_name,
        local_path,
        summary,
    })
}

pub(super) fn image_cache_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app.path().app_data_dir()?.join("image-cache");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub(super) fn original_upload_mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Png => Some("image/png"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

pub(super) fn original_upload_extension(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("jpg"),
        ImageFormat::Png => Some("png"),
        ImageFormat::WebP => Some("webp"),
        ImageFormat::Bmp => Some("bmp"),
        _ => None,
    }
}

pub(super) fn image_format_label(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Png => "PNG",
        ImageFormat::WebP => "WEBP",
        ImageFormat::Bmp => "BMP",
        ImageFormat::Gif => "GIF",
        _ => "未知格式",
    }
}

pub(super) fn encode_jpeg_under_limit(image: DynamicImage) -> Result<(Vec<u8>, u32, u32), String> {
    let mut last_bytes = Vec::new();
    let mut last_width = 0u32;
    let mut last_height = 0u32;
    for max_side in [2400u32, 2000, 1600, 1200, 900] {
        let working = resize_to_max_side(&image, max_side);
        let (width, height) = working.dimensions();
        for quality in [86u8, 78, 70, 62] {
            let encoded = encode_jpeg_with_white_background(&working, quality)?;
            if encoded.len() <= WECHAT_IMAGE_TARGET_BYTES {
                return Ok((encoded, width, height));
            }
            last_bytes = encoded;
            last_width = width;
            last_height = height;
        }
    }
    if last_bytes.len() <= WECHAT_IMAGE_MAX_BYTES {
        Ok((last_bytes, last_width, last_height))
    } else {
        Err(format!(
            "图片压缩后仍过大：{}",
            format_bytes_short(last_bytes.len())
        ))
    }
}

pub(super) fn resize_to_max_side(image: &DynamicImage, max_side: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= max_side && height <= max_side {
        return image.clone();
    }
    image.resize(max_side, max_side, image::imageops::FilterType::Lanczos3)
}

pub(super) fn encode_jpeg_with_white_background(
    image: &DynamicImage,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(width, height);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as u16;
        let blend = |channel: u8| -> u8 {
            (((channel as u16 * alpha) + (255u16 * (255 - alpha))) / 255) as u8
        };
        rgb.put_pixel(
            x,
            y,
            image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, quality);
    encoder
        .encode(&rgb, width, height, image::ColorType::Rgb8.into())
        .map_err(|error| format!("图片转 JPEG 失败：{error}"))?;
    Ok(bytes)
}

pub(super) fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn format_bytes_short(value: usize) -> String {
    if value >= 1024 * 1024 {
        format!("{:.2}MB", value as f64 / 1024.0 / 1024.0)
    } else if value >= 1024 {
        format!("{:.2}KB", value as f64 / 1024.0)
    } else {
        format!("{value}B")
    }
}

pub(super) fn is_wechat_image_url(source_url: &str) -> bool {
    Url::parse(source_url)
        .ok()
        .map(|url| {
            url.host_str()
                .map(|host| host == "mmecimage.cn" || host.ends_with(".mmecimage.cn"))
                .unwrap_or(false)
                && url.path().starts_with("/p/")
        })
        .unwrap_or(false)
}

pub(super) fn find_cached_asset_url(
    app: &AppHandle,
    shop_id: &str,
    source_url: &str,
) -> AppResult<Option<String>> {
    let conn = open_connection(app)?;
    let wechat_url = conn
        .query_row(
            "SELECT wechat_url
             FROM publish_assets
             WHERE shop_id = ?1
               AND source_url = ?2
               AND status IN ('success', 'reused')
               AND wechat_url IS NOT NULL
             ORDER BY uploaded_at DESC, updated_at DESC
             LIMIT 1",
            params![shop_id, source_url],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(wechat_url)
}

pub(super) fn record_asset_success(
    app: &AppHandle,
    item: &PendingPublishItem,
    asset: &ProductAsset,
    wechat_url: &str,
    status: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO publish_assets
         (id, job_id, item_id, product_row_id, shop_id, source_url, asset_kind, sort_order,
          wechat_url, status, uploaded_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?11)
         ON CONFLICT(item_id, source_url) DO UPDATE SET
           asset_kind = excluded.asset_kind,
           sort_order = excluded.sort_order,
           wechat_url = excluded.wechat_url,
           status = excluded.status,
           error_code = NULL,
           error_summary = NULL,
           uploaded_at = excluded.uploaded_at,
           updated_at = excluded.updated_at",
        params![
            format!("asset-{}", Uuid::new_v4()),
            item.job_id.as_str(),
            item.item_id.as_str(),
            item.product_row_id.as_str(),
            item.shop_id.as_str(),
            asset.source_url.as_str(),
            asset.kind,
            asset.sort_order,
            wechat_url,
            status,
            now
        ],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "info",
        "素材已准备为微信图片链接",
        Some(&serde_json::json!({
            "asset_kind": asset.kind,
            "sort_order": asset.sort_order,
            "status": status
        })),
    )?;
    Ok(())
}

pub(super) fn record_asset_failure(
    app: &AppHandle,
    item: &PendingPublishItem,
    asset: &ProductAsset,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO publish_assets
         (id, job_id, item_id, product_row_id, shop_id, source_url, asset_kind, sort_order,
          status, error_code, error_summary, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'failed', ?9, ?10, ?11, ?11)
         ON CONFLICT(item_id, source_url) DO UPDATE SET
           asset_kind = excluded.asset_kind,
           sort_order = excluded.sort_order,
           status = 'failed',
           error_code = excluded.error_code,
           error_summary = excluded.error_summary,
           updated_at = excluded.updated_at",
        params![
            format!("asset-{}", Uuid::new_v4()),
            item.job_id.as_str(),
            item.item_id.as_str(),
            item.product_row_id.as_str(),
            item.shop_id.as_str(),
            asset.source_url.as_str(),
            asset.kind,
            asset.sort_order,
            error_code,
            error_summary,
            now
        ],
    )?;
    insert_task_log(
        &conn,
        &item.job_id,
        Some(&item.item_id),
        "error",
        error_summary,
        Some(&serde_json::json!({
            "error_code": error_code,
            "asset_kind": asset.kind,
            "sort_order": asset.sort_order
        })),
    )?;
    Ok(())
}

pub(super) fn recompute_publish_job(conn: &Connection, job_id: &str) -> AppResult<()> {
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

pub(super) fn recompute_price_update_job(conn: &Connection, job_id: &str) -> AppResult<()> {
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
pub(super) struct PublishStatusCounts {
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

pub(super) fn load_publish_status_counts(
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

pub(super) fn resolve_publish_status(counts: &PublishStatusCounts) -> &'static str {
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

pub(super) fn compute_publish_progress(counts: &PublishStatusCounts) -> i64 {
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

pub(super) fn insert_task_log(
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

pub(super) fn count_by_sql(conn: &Connection, sql: &str) -> AppResult<i64> {
    Ok(conn.query_row(sql, [], |row| row.get(0))?)
}
