use super::*;

pub(in crate::commands) fn load_category_catalog_shop_summaries(
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
           (SELECT COUNT(*) FROM wechat_category_relations rel WHERE rel.shop_id = s.id),
           (SELECT COUNT(*) FROM wechat_category_relations rel WHERE rel.shop_id = s.id AND rel.status = 1),
           (SELECT COUNT(*) FROM wechat_freight_templates f WHERE f.shop_id = s.id),
           (SELECT MAX(synced_at) FROM wechat_categories c WHERE c.shop_id = s.id),
           (SELECT MAX(synced_at) FROM wechat_category_relations rel WHERE rel.shop_id = s.id),
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
                category_relation_count: row.get(6)?,
                active_category_relation_count: row.get(7)?,
                freight_template_count: row.get(8)?,
                last_category_sync_at: row.get(9)?,
                last_relation_sync_at: row.get(10)?,
                last_rule_sync_at: row.get(11)?,
                last_freight_sync_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(in crate::commands) fn load_category_relation_views(
    conn: &Connection,
    shop_id: Option<&str>,
    keyword: Option<&str>,
    limit: i64,
) -> AppResult<Vec<CategoryRelationView>> {
    let mut stmt = conn.prepare(
        "SELECT
           rel.shop_id,
           s.name,
           rel.cat_id,
           c.name,
           rel.status,
           rel.uneffective_reason,
           rel.effective_time,
           rel.uneffective_time,
           rel.qua_id,
           rel.synced_at
         FROM wechat_category_relations rel
         JOIN shops s ON s.id = rel.shop_id
         LEFT JOIN wechat_categories c
           ON c.shop_id = rel.shop_id AND c.cat_id = rel.cat_id
         WHERE (?1 IS NULL OR rel.shop_id = ?1)
           AND (
             ?2 IS NULL
             OR c.name LIKE ?2
             OR CAST(rel.cat_id AS TEXT) LIKE ?2
             OR rel.uneffective_reason LIKE ?2
           )
         ORDER BY rel.status ASC, COALESCE(c.name, '') ASC, rel.cat_id ASC
         LIMIT ?3",
    )?;
    let items = stmt
        .query_map(params![shop_id, keyword, limit], |row| {
            Ok(CategoryRelationView {
                shop_id: row.get(0)?,
                shop_name: row.get(1)?,
                cat_id: row.get(2)?,
                category_name: row.get(3)?,
                status: row.get(4)?,
                uneffective_reason: row.get(5)?,
                effective_time: row.get(6)?,
                uneffective_time: row.get(7)?,
                qua_id: row.get(8)?,
                synced_at: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(in crate::commands) fn load_category_cache_views(
    conn: &Connection,
    shop_id: Option<&str>,
    keyword: Option<&str>,
    limit: i64,
) -> AppResult<Vec<CategoryCacheView>> {
    let sql = if keyword.is_some() {
        "WITH RECURSIVE
           matched(shop_id, cat_id) AS (
             SELECT c.shop_id, c.cat_id
             FROM wechat_categories c
             WHERE (?1 IS NULL OR c.shop_id = ?1)
               AND (c.name LIKE ?2 OR CAST(c.cat_id AS TEXT) LIKE ?2)
           ),
           ancestor_scope(shop_id, cat_id) AS (
             SELECT shop_id, cat_id FROM matched
             UNION
             SELECT parent.shop_id, parent.cat_id
             FROM wechat_categories parent
             JOIN wechat_categories child
               ON child.shop_id = parent.shop_id AND child.parent_cat_id = parent.cat_id
             JOIN ancestor_scope scope
               ON scope.shop_id = child.shop_id AND scope.cat_id = child.cat_id
           ),
           descendant_scope(shop_id, cat_id) AS (
             SELECT shop_id, cat_id FROM matched
             UNION
             SELECT child.shop_id, child.cat_id
             FROM wechat_categories child
             JOIN descendant_scope scope
               ON child.shop_id = scope.shop_id AND child.parent_cat_id = scope.cat_id
           ),
           category_scope(shop_id, cat_id) AS (
             SELECT shop_id, cat_id FROM ancestor_scope
             UNION
             SELECT shop_id, cat_id FROM descendant_scope
           )
         SELECT
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
           COALESCE(rel.status, 0) = 1,
           c.synced_at,
           d.synced_at
         FROM wechat_categories c
         JOIN category_scope scope
           ON scope.shop_id = c.shop_id AND scope.cat_id = c.cat_id
         JOIN shops s ON s.id = c.shop_id
         LEFT JOIN wechat_category_details d
           ON d.shop_id = c.shop_id AND d.cat_id = c.cat_id
         LEFT JOIN wechat_category_rules pr
           ON pr.shop_id = c.shop_id AND pr.cat_id = c.cat_id AND pr.rule_type = 'product'
         LEFT JOIN wechat_category_rules dr
           ON dr.shop_id = c.shop_id AND dr.cat_id = c.cat_id AND dr.rule_type = 'delivery'
         LEFT JOIN wechat_category_relations rel
           ON rel.shop_id = c.shop_id AND rel.cat_id = c.cat_id
         ORDER BY COALESCE(c.level, 0) ASC, COALESCE(c.parent_cat_id, 0) ASC, c.name ASC
         LIMIT ?3"
    } else {
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
           COALESCE(rel.status, 0) = 1,
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
         LEFT JOIN wechat_category_relations rel
           ON rel.shop_id = c.shop_id AND rel.cat_id = c.cat_id
         WHERE (?1 IS NULL OR c.shop_id = ?1)
         ORDER BY COALESCE(c.level, 0) ASC, COALESCE(c.parent_cat_id, 0) ASC, c.name ASC
         LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql)?;
    let items = if keyword.is_some() {
        stmt.query_map(
            params![shop_id, keyword, limit],
            category_cache_view_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![shop_id, limit], category_cache_view_from_row)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(items)
}

fn category_cache_view_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CategoryCacheView> {
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
        is_available_for_shop: row.get::<_, i64>(12)? == 1,
        synced_at: row.get(13)?,
        detail_synced_at: row.get(14)?,
    })
}

pub(in crate::commands) fn load_freight_template_views(
    conn: &Connection,
    shop_id: Option<&str>,
) -> AppResult<Vec<FreightTemplateView>> {
    let mut stmt = conn.prepare(
        "SELECT f.shop_id, s.name, f.template_id, f.synced_at, f.raw_payload
         FROM wechat_freight_templates f
         JOIN shops s ON s.id = f.shop_id
         WHERE (?1 IS NULL OR f.shop_id = ?1)
         ORDER BY s.name ASC, f.template_id ASC
         LIMIT 200",
    )?;
    let defaults = load_publish_default_freight_templates(conn)?;
    let items = stmt
        .query_map(params![shop_id], |row| {
            let row_shop_id: String = row.get(0)?;
            let row_template_id: String = row.get(2)?;
            let is_default = defaults
                .get(&row_shop_id)
                .map(|tid| tid == &row_template_id)
                .unwrap_or(false);
            // 模板名称从 raw_payload（缓存的 freight_template 详情对象）里解析，存量缺详情则为 None。
            let raw_payload_text: String = row.get(4)?;
            let template_name = serde_json::from_str::<Value>(&raw_payload_text)
                .ok()
                .as_ref()
                .and_then(freight_template_name_from_payload);
            Ok(FreightTemplateView {
                shop_id: row_shop_id,
                shop_name: row.get(1)?,
                template_id: row_template_id,
                template_name,
                synced_at: row.get(3)?,
                is_default,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub(in crate::commands) async fn sync_freight_template_ids(
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
                let synced_at = now_shanghai();
                let conn = open_connection(app)?;
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
                // 列表只给 template_id，逐个查询详情拿模板名称（及计费方式等）整体存入 raw_payload。
                // 详情查询失败（如模板已删 10020005）降级为只存 ID，不中断整页同步。
                for template_id in &template_ids {
                    let detail_payload = match client
                        .get_freight_template_detail(access_token, template_id)
                        .await
                    {
                        Ok(detail_call) => {
                            let detail_endpoint = detail_call.meta.endpoint;
                            let detail_method = detail_call.meta.method;
                            match detail_call.result {
                                WechatCallResult::Success(detail) => {
                                    extract_freight_template_detail(&detail.raw_payload)
                                        .unwrap_or_else(|| {
                                            serde_json::json!({ "template_id": template_id })
                                        })
                                }
                                WechatCallResult::ApiError(error) => {
                                    insert_api_call_log(
                                        &conn,
                                        Some(shop_id),
                                        detail_endpoint,
                                        detail_method,
                                        "api_error",
                                        Some(error.errcode),
                                        Some(&error.errmsg),
                                        Some(&format!(
                                            "freight template detail api error template_id={template_id}"
                                        )),
                                    )?;
                                    serde_json::json!({ "template_id": template_id })
                                }
                            }
                        }
                        Err(error) => {
                            insert_task_log(
                                &conn,
                                task_id,
                                None,
                                "warning",
                                &format!(
                                    "运费模板 {template_id} 详情查询失败，仅保留模板 ID：{error}"
                                ),
                                None,
                            )?;
                            serde_json::json!({ "template_id": template_id })
                        }
                    };
                    upsert_freight_template(
                        &conn,
                        shop_id,
                        template_id,
                        &detail_payload,
                        &synced_at,
                    )?;
                }
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

pub(in crate::commands) fn upsert_wechat_categories(
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

pub(in crate::commands) fn replace_wechat_categories_for_shop(
    conn: &Connection,
    shop_id: &str,
    categories: &[CachedWechatCategory],
    synced_at: &str,
) -> AppResult<()> {
    conn.execute(
        "DELETE FROM wechat_categories WHERE shop_id = ?1",
        params![shop_id],
    )?;
    upsert_wechat_categories(conn, shop_id, categories, synced_at)
}

pub(in crate::commands) fn upsert_wechat_category_relations(
    conn: &Connection,
    shop_id: &str,
    relations: &[CachedWechatCategoryRelation],
    synced_at: &str,
) -> AppResult<()> {
    conn.execute(
        "DELETE FROM wechat_category_relations WHERE shop_id = ?1",
        params![shop_id],
    )?;
    for relation in relations {
        conn.execute(
            "INSERT INTO wechat_category_relations
             (shop_id, cat_id, status, uneffective_reason, effective_time, uneffective_time, qua_id, raw_payload, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                shop_id,
                relation.cat_id,
                relation.status,
                relation.uneffective_reason,
                relation.effective_time,
                relation.uneffective_time,
                relation.qua_id,
                relation.raw_payload.to_string(),
                synced_at
            ],
        )?;
    }
    Ok(())
}

pub(in crate::commands) fn upsert_freight_template(
    conn: &Connection,
    shop_id: &str,
    template_id: &str,
    raw_payload: &Value,
    synced_at: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO wechat_freight_templates (shop_id, template_id, raw_payload, synced_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(shop_id, template_id) DO UPDATE SET
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at",
        params![shop_id, template_id, raw_payload.to_string(), synced_at],
    )?;
    Ok(())
}

pub(in crate::commands) fn upsert_category_detail(
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

pub(in crate::commands) fn upsert_category_rule(
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

pub(in crate::commands) fn upsert_category_precheck_result(
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
