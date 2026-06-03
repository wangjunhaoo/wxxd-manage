use super::*;

#[tauri::command]
pub fn list_shop_groups(app: AppHandle) -> AppResult<Vec<ShopGroup>> {
    let conn = open_connection(&app)?;
    let mut stmt = conn.prepare(
        "SELECT g.id, g.name, g.status, g.created_at, COUNT(s.id) AS shop_count
         FROM shop_groups g
         LEFT JOIN shops s ON s.group_id = g.id
         GROUP BY g.id, g.name, g.status, g.created_at
         ORDER BY g.created_at ASC",
    )?;
    let groups = stmt
        .query_map([], |row| {
            Ok(ShopGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                shop_count: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(groups)
}

#[tauri::command]
pub fn list_shops(app: AppHandle) -> AppResult<Vec<ShopListItem>> {
    let conn = open_connection(&app)?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.appid, s.status, s.group_id, g.name, s.created_at,
                c.shop_id IS NOT NULL AS has_secret, t.expires_at,
                s.wechat_nickname, s.wechat_status, s.last_health_check_at,
                (
                  SELECT q.remain FROM api_quota_snapshots q
                  WHERE q.shop_id = s.id
                  ORDER BY q.checked_at DESC
                  LIMIT 1
                ) AS last_quota_remain
         FROM shops s
         JOIN shop_groups g ON g.id = s.group_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         LEFT JOIN access_tokens t ON t.shop_id = s.id
         ORDER BY s.created_at DESC",
    )?;
    let shops = stmt
        .query_map([], |row| {
            Ok(ShopListItem {
                id: row.get(0)?,
                name: row.get(1)?,
                appid: row.get(2)?,
                status: row.get(3)?,
                group_id: row.get(4)?,
                group_name: row.get(5)?,
                created_at: row.get(6)?,
                has_secret: row.get::<_, i64>(7)? == 1,
                token_expires_at: row.get(8)?,
                wechat_nickname: row.get(9)?,
                wechat_status: row.get(10)?,
                last_health_check_at: row.get(11)?,
                last_quota_remain: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(shops)
}

#[tauri::command]
pub fn create_shop_group(app: AppHandle, name: String) -> AppResult<ShopGroup> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("店铺组名称不能为空".to_string()));
    }

    let conn = open_connection(&app)?;
    let id = format!("group-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    conn.execute(
        "INSERT INTO shop_groups (id, name, status, created_at) VALUES (?1, ?2, 'active', ?3)",
        params![id, name, created_at],
    )?;
    Ok(ShopGroup {
        id,
        name: name.to_string(),
        status: "active".to_string(),
        shop_count: 0,
        created_at,
    })
}

#[tauri::command]
pub fn create_shop(app: AppHandle, request: CreateShopRequest) -> AppResult<Shop> {
    let name = request.name.trim();
    let appid = request.appid.trim();
    let app_secret = request.app_secret.unwrap_or_default();
    let app_secret = app_secret.trim();
    if name.is_empty() {
        return Err(AppError::Validation("店铺名称不能为空".to_string()));
    }
    if appid.is_empty() {
        return Err(AppError::Validation("appid 不能为空".to_string()));
    }
    if request.group_id.trim().is_empty() {
        return Err(AppError::Validation("店铺组不能为空".to_string()));
    }

    let mut conn = open_connection(&app)?;
    let group_exists: Option<String> = conn
        .query_row(
            "SELECT id FROM shop_groups WHERE id = ?1",
            [request.group_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if group_exists.is_none() {
        return Err(AppError::Validation("店铺组不存在".to_string()));
    }

    let id = format!("shop-{}", Uuid::new_v4());
    let created_at = now_shanghai();
    let status = if app_secret.is_empty() {
        "missing_secret"
    } else {
        "not_verified"
    };
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO shops (id, name, appid, status, group_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, appid, status, request.group_id, created_at],
    )?;
    if !app_secret.is_empty() {
        upsert_shop_secret(&app, &tx, &id, app_secret, &created_at)?;
    }
    tx.commit()?;

    Ok(Shop {
        id,
        name: name.to_string(),
        appid: appid.to_string(),
        status: status.to_string(),
        group_id: request.group_id,
        created_at,
    })
}

#[tauri::command]
pub async fn verify_shop_credentials(
    app: AppHandle,
    shop_id: String,
    force_refresh: bool,
) -> AppResult<ShopCredentialCheck> {
    let (appid, encrypted_secret, secret_nonce) = {
        let conn = open_connection(&app)?;
        conn.query_row(
            "SELECT s.appid, c.encrypted_secret, c.secret_nonce
             FROM shops s
             JOIN shop_credentials c ON c.shop_id = s.id
             WHERE s.id = ?1",
            [shop_id.as_str()],
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

    let secret = decrypt_secret(&app, &encrypted_secret, &secret_nonce)?;
    let client = WechatShopClient::default();
    let call = client
        .get_stable_access_token(&appid, &secret, force_refresh)
        .await?;
    let mut conn = open_connection(&app)?;
    let checked_at = now_shanghai();

    match &call.result {
        WechatCallResult::Success(token) => {
            let encrypted_token = encrypt_access_token(&app, &token.access_token)?;
            let expires_at = expires_at_shanghai(token.expires_in);
            let tx = conn.transaction()?;
            tx.execute(
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
                    checked_at
                ],
            )?;
            tx.execute(
                "UPDATE shops SET status = 'active', last_sync_at = ?1 WHERE id = ?2",
                params![checked_at, shop_id],
            )?;
            insert_api_call_log(
                &tx,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "stable_token ok, expires_in={}s",
                    token.expires_in
                )),
            )?;
            tx.commit()?;
            Ok(ShopCredentialCheck {
                shop_id,
                status: "active".to_string(),
                expires_at: Some(expires_at),
                errcode: None,
                errmsg: None,
            })
        }
        WechatCallResult::ApiError(error) => {
            conn.execute(
                "UPDATE shops SET status = 'auth_failed' WHERE id = ?1",
                [shop_id.as_str()],
            )?;
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("stable_token api error"),
            )?;
            Ok(ShopCredentialCheck {
                shop_id,
                status: "auth_failed".to_string(),
                expires_at: None,
                errcode: Some(error.errcode),
                errmsg: Some(error.errmsg.clone()),
            })
        }
    }
}

#[tauri::command]
pub async fn sync_shop_basic_info(
    app: AppHandle,
    shop_id: String,
) -> AppResult<ShopBasicInfoSyncResult> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client.get_shop_basic_info(&access_token).await?;
    let conn = open_connection(&app)?;
    let checked_at = now_shanghai();

    match &call.result {
        WechatCallResult::Success(info) => {
            conn.execute(
                "UPDATE shops SET
                   status = 'active',
                   wechat_nickname = ?1,
                   wechat_headimg_url = ?2,
                   wechat_subject_type = ?3,
                   wechat_status = ?4,
                   wechat_username = ?5,
                   is_local_life = ?6,
                   open_timestamp = ?7,
                   last_health_check_at = ?8,
                   last_sync_at = ?8
                 WHERE id = ?9",
                params![
                    info.nickname,
                    info.headimg_url,
                    info.subject_type,
                    info.status,
                    info.username,
                    info.is_local_life,
                    info.open_timestamp,
                    checked_at,
                    shop_id
                ],
            )?;
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("shop basic info synced"),
            )?;
            Ok(ShopBasicInfoSyncResult {
                shop_id,
                status: "active".to_string(),
                nickname: info.nickname.clone(),
                wechat_status: info.status.clone(),
                errcode: None,
                errmsg: None,
            })
        }
        WechatCallResult::ApiError(error) => {
            conn.execute(
                "UPDATE shops SET status = 'api_failed', last_health_check_at = ?1 WHERE id = ?2",
                params![checked_at, shop_id],
            )?;
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("shop basic info api error"),
            )?;
            Ok(ShopBasicInfoSyncResult {
                shop_id,
                status: "api_failed".to_string(),
                nickname: None,
                wechat_status: None,
                errcode: Some(error.errcode),
                errmsg: Some(error.errmsg.clone()),
            })
        }
    }
}

#[tauri::command]
pub async fn check_shop_api_quota(
    app: AppHandle,
    request: ApiQuotaCheckRequest,
) -> AppResult<ApiQuotaCheckResult> {
    let cgi_path = request.cgi_path.trim();
    if cgi_path.is_empty() {
        return Err(AppError::Validation("cgi_path 不能为空".to_string()));
    }
    if !cgi_path.starts_with('/') || cgi_path.starts_with("http") {
        return Err(AppError::Validation(
            "cgi_path 必须是以 / 开头的接口路径，不能包含域名".to_string(),
        ));
    }

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &request.shop_id, &client).await?;
    let call = client.get_api_quota(&access_token, cgi_path).await?;
    let conn = open_connection(&app)?;

    match &call.result {
        WechatCallResult::Success(info) => {
            let quota = info.quota.clone();
            let rate_limit = info.rate_limit.clone();
            conn.execute(
                "INSERT INTO api_quota_snapshots
                 (id, shop_id, cgi_path, daily_limit, used, remain, rate_call_count, rate_refresh_second, raw_payload, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    format!("quota-{}", Uuid::new_v4()),
                    request.shop_id,
                    cgi_path,
                    quota.as_ref().and_then(|item| item.daily_limit),
                    quota.as_ref().and_then(|item| item.used),
                    quota.as_ref().and_then(|item| item.remain),
                    rate_limit.as_ref().and_then(|item| item.call_count),
                    rate_limit.as_ref().and_then(|item| item.refresh_second),
                    serde_json::to_string(info).unwrap_or_else(|_| "{}".to_string()),
                    now_shanghai()
                ],
            )?;
            insert_api_call_log(
                &conn,
                Some(&request.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some(&format!("quota checked for {}", cgi_path)),
            )?;
            Ok(ApiQuotaCheckResult {
                shop_id: request.shop_id,
                cgi_path: cgi_path.to_string(),
                daily_limit: quota.as_ref().and_then(|item| item.daily_limit),
                used: quota.as_ref().and_then(|item| item.used),
                remain: quota.as_ref().and_then(|item| item.remain),
                rate_call_count: rate_limit.as_ref().and_then(|item| item.call_count),
                rate_refresh_second: rate_limit.as_ref().and_then(|item| item.refresh_second),
                errcode: None,
                errmsg: None,
            })
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&request.shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some(&format!("quota api error for {}", cgi_path)),
            )?;
            Ok(ApiQuotaCheckResult {
                shop_id: request.shop_id,
                cgi_path: cgi_path.to_string(),
                daily_limit: None,
                used: None,
                remain: None,
                rate_call_count: None,
                rate_refresh_second: None,
                errcode: Some(error.errcode),
                errmsg: Some(error.errmsg.clone()),
            })
        }
    }
}

#[tauri::command]
pub fn list_category_catalog(
    app: AppHandle,
    shop_id: Option<String>,
    keyword: Option<String>,
    limit: Option<i64>,
) -> AppResult<CategoryCatalogListResult> {
    let conn = open_connection(&app)?;
    let normalized_shop_id = shop_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let normalized_keyword = keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));
    let limit = limit.unwrap_or(100).clamp(1, 2_000);

    Ok(CategoryCatalogListResult {
        shops: load_category_catalog_shop_summaries(&conn)?,
        categories: load_category_cache_views(
            &conn,
            normalized_shop_id.as_deref(),
            normalized_keyword.as_deref(),
            limit,
        )?,
        category_relations: load_category_relation_views(
            &conn,
            normalized_shop_id.as_deref(),
            normalized_keyword.as_deref(),
            limit,
        )?,
        freight_templates: load_freight_template_views(&conn, normalized_shop_id.as_deref())?,
    })
}

#[tauri::command]
pub async fn sync_shop_category_catalog(
    app: AppHandle,
    shop_id: String,
) -> AppResult<CategoryCatalogSyncResult> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let task_id = format!("catalog-sync-{}", Uuid::new_v4());
    let started_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'catalog.sync_category_rules', 'running', 0, ?2, ?2)",
            params![task_id, started_at],
        )?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "开始同步店铺生效类目权限和运费模板",
            Some(&serde_json::json!({ "shop_id": shop_id })),
        )?;
    }

    let mut synced_categories = 0i64;
    let mut synced_category_relations = 0i64;
    let mut synced_freight_templates = 0i64;
    let mut failed_steps = Vec::new();
    let mut relation_list_synced = false;

    let relation_call = client
        .get_category_relation_list(&access_token, Some(1))
        .await?;
    let mut active_relations = Vec::new();
    match &relation_call.result {
        WechatCallResult::Success(result) => {
            let relations = extract_wechat_category_relations(&result.raw_payload);
            synced_category_relations = relations.len() as i64;
            let conn = open_connection(&app)?;
            upsert_wechat_category_relations(&conn, &shop_id, &relations, &now_shanghai())?;
            relation_list_synced = true;
            active_relations = relations;
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                relation_call.meta.endpoint,
                relation_call.meta.method,
                "success",
                None,
                None,
                Some(&format!(
                    "synced category relations={synced_category_relations}"
                )),
            )?;
            insert_task_log(
                &conn,
                &task_id,
                None,
                "info",
                &format!("店铺生效类目权限同步完成：{synced_category_relations} 个类目"),
                None,
            )?;
        }
        WechatCallResult::ApiError(error) => {
            failed_steps.push(format!("店铺类目权限同步失败：{}", error.errmsg));
            let conn = open_connection(&app)?;
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                relation_call.meta.endpoint,
                relation_call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("category relation list api error"),
            )?;
            insert_task_log(
                &conn,
                &task_id,
                None,
                "error",
                &format!("店铺类目权限同步失败：{}", error.errmsg),
                Some(&serde_json::json!({ "errcode": error.errcode })),
            )?;
        }
    }

    if relation_list_synced {
        let mut categories_by_id = std::collections::BTreeMap::<i64, CachedWechatCategory>::new();
        let active_leaf_ids = active_relations
            .iter()
            .map(|relation| relation.cat_id)
            .collect::<BTreeSet<_>>();
        for relation in &active_relations {
            let detail_call = client
                .get_category_relation_detail(&access_token, relation.cat_id)
                .await?;
            match &detail_call.result {
                WechatCallResult::Success(raw) => {
                    for category in extract_wechat_categories(&raw.raw_payload) {
                        categories_by_id.insert(category.cat_id, category);
                    }
                    let conn = open_connection(&app)?;
                    insert_api_call_log(
                        &conn,
                        Some(&shop_id),
                        detail_call.meta.endpoint,
                        detail_call.meta.method,
                        "success",
                        None,
                        None,
                        Some(&format!(
                            "synced category relation detail cat_id={}",
                            relation.cat_id
                        )),
                    )?;
                }
                WechatCallResult::ApiError(error) => {
                    failed_steps.push(format!(
                        "类目权限详情失败 {}：{}",
                        relation.cat_id, error.errmsg
                    ));
                    insert_api_error_and_task_log(
                        &app,
                        &task_id,
                        &shop_id,
                        &detail_call.meta,
                        error,
                    )?;
                }
            }
        }
        if active_leaf_ids
            .iter()
            .any(|cat_id| cached_category_path_len(&categories_by_id, *cat_id) < 3)
        {
            let all_category_call = client.get_all_categories(&access_token).await?;
            match &all_category_call.result {
                WechatCallResult::Success(raw) => {
                    let all_categories = extract_wechat_categories(&raw.raw_payload);
                    merge_active_category_paths(
                        &mut categories_by_id,
                        all_categories,
                        &active_leaf_ids,
                    );
                    let conn = open_connection(&app)?;
                    insert_api_call_log(
                        &conn,
                        Some(&shop_id),
                        all_category_call.meta.endpoint,
                        all_category_call.meta.method,
                        "success",
                        None,
                        None,
                        Some("used all category tree as path dictionary for active relations"),
                    )?;
                }
                WechatCallResult::ApiError(error) => {
                    failed_steps.push(format!("类目路径字典同步失败：{}", error.errmsg));
                    let conn = open_connection(&app)?;
                    insert_api_call_log(
                        &conn,
                        Some(&shop_id),
                        all_category_call.meta.endpoint,
                        all_category_call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some("all category dictionary api error"),
                    )?;
                    insert_task_log(
                        &conn,
                        &task_id,
                        None,
                        "warn",
                        &format!("类目路径字典同步失败：{}", error.errmsg),
                        Some(&serde_json::json!({ "errcode": error.errcode })),
                    )?;
                }
            }
        }
        let missing_leaf_ids = active_leaf_ids
            .iter()
            .filter(|cat_id| !categories_by_id.contains_key(cat_id))
            .copied()
            .collect::<Vec<_>>();
        for cat_id in missing_leaf_ids {
            let detail_call = client.get_category_detail(&access_token, cat_id).await?;
            match &detail_call.result {
                WechatCallResult::Success(raw) => {
                    if let Some(category) =
                        extract_wechat_category_detail_info(&raw.raw_payload, cat_id)
                    {
                        categories_by_id.insert(category.cat_id, category);
                    }
                    let conn = open_connection(&app)?;
                    insert_api_call_log(
                        &conn,
                        Some(&shop_id),
                        detail_call.meta.endpoint,
                        detail_call.meta.method,
                        "success",
                        None,
                        None,
                        Some(&format!("synced category leaf name cat_id={cat_id}")),
                    )?;
                }
                WechatCallResult::ApiError(error) => {
                    failed_steps.push(format!("类目名称兜底同步失败 {cat_id}：{}", error.errmsg));
                    insert_api_error_and_task_log(
                        &app,
                        &task_id,
                        &shop_id,
                        &detail_call.meta,
                        error,
                    )?;
                }
            }
        }
        let categories = categories_by_id.into_values().collect::<Vec<_>>();
        synced_categories = categories.len() as i64;
        let conn = open_connection(&app)?;
        replace_wechat_categories_for_shop(&conn, &shop_id, &categories, &now_shanghai())?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            &format!("店铺类目路径同步完成：{synced_categories} 个类目节点"),
            None,
        )?;
    }

    match sync_freight_template_ids(&app, &client, &access_token, &shop_id, &task_id).await {
        Ok(count) => synced_freight_templates = count,
        Err(error) => failed_steps.push(format!("运费模板同步失败：{error}")),
    }

    let final_status = if failed_steps.is_empty() {
        "success"
    } else if synced_categories == 0
        && synced_category_relations == 0
        && synced_freight_templates == 0
    {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;

    Ok(CategoryCatalogSyncResult {
        task_id,
        shop_id,
        synced_categories,
        synced_category_relations,
        synced_freight_templates,
        failed_steps,
    })
}

fn cached_category_path_len(
    categories_by_id: &BTreeMap<i64, CachedWechatCategory>,
    leaf_cat_id: i64,
) -> usize {
    let mut length = 0;
    let mut current = Some(leaf_cat_id);
    let mut visited = BTreeSet::new();
    for _ in 0..8 {
        let Some(cat_id) = current else {
            break;
        };
        if !visited.insert(cat_id) {
            break;
        }
        let Some(category) = categories_by_id.get(&cat_id) else {
            break;
        };
        length += 1;
        current = category.parent_cat_id.filter(|value| *value > 0);
    }
    length
}

fn merge_active_category_paths(
    categories_by_id: &mut BTreeMap<i64, CachedWechatCategory>,
    all_categories: Vec<CachedWechatCategory>,
    active_leaf_ids: &BTreeSet<i64>,
) {
    let mut all_categories_by_id = all_categories
        .into_iter()
        .map(|category| (category.cat_id, category))
        .collect::<BTreeMap<_, _>>();
    for leaf_cat_id in active_leaf_ids {
        let mut current = Some(*leaf_cat_id);
        let mut visited = BTreeSet::new();
        for _ in 0..8 {
            let Some(cat_id) = current else {
                break;
            };
            if !visited.insert(cat_id) {
                break;
            }
            if let Some(category) = all_categories_by_id.remove(&cat_id) {
                current = category.parent_cat_id.filter(|value| *value > 0);
                categories_by_id.insert(cat_id, category);
            } else if let Some(category) = categories_by_id.get(&cat_id) {
                current = category.parent_cat_id.filter(|value| *value > 0);
            } else {
                break;
            }
        }
    }
}

#[tauri::command]
pub async fn sync_category_rules(
    app: AppHandle,
    shop_id: String,
    cat_id: i64,
) -> AppResult<CategoryRuleSyncResult> {
    if cat_id <= 0 {
        return Err(AppError::Validation("cat_id 必须大于 0".to_string()));
    }

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let task_id = format!("category-rule-sync-{}", Uuid::new_v4());
    let started_at = now_shanghai();
    {
        let conn = open_connection(&app)?;
        conn.execute(
            "INSERT INTO task_runs (id, task_type, status, progress, started_at, created_at)
             VALUES (?1, 'catalog.sync_single_category_rule', 'running', 0, ?2, ?2)",
            params![task_id, started_at],
        )?;
        insert_task_log(
            &conn,
            &task_id,
            None,
            "info",
            "开始同步单个类目的详情和发品规则",
            Some(&serde_json::json!({ "shop_id": shop_id, "cat_id": cat_id })),
        )?;
    }

    let mut result = CategoryRuleSyncResult {
        task_id: task_id.clone(),
        shop_id: shop_id.clone(),
        cat_id,
        synced_detail: false,
        synced_product_rule: false,
        synced_delivery_rule: false,
        product_attr_count: 0,
        sale_attr_count: 0,
        product_qua_count: 0,
        failed_steps: Vec::new(),
    };

    let detail_call = client.get_category_detail(&access_token, cat_id).await?;
    match &detail_call.result {
        WechatCallResult::Success(raw) => {
            let counts = category_detail_counts(&raw.raw_payload);
            let conn = open_connection(&app)?;
            upsert_category_detail(&conn, &shop_id, cat_id, &raw.raw_payload, &counts)?;
            insert_success_api_and_task_log(
                &conn,
                &task_id,
                &shop_id,
                detail_call.meta.endpoint,
                detail_call.meta.method,
                &format!(
                    "类目详情同步完成：商品属性 {}，销售属性 {}，资质 {}",
                    counts.product_attr_count, counts.sale_attr_count, counts.product_qua_count
                ),
            )?;
            result.synced_detail = true;
            result.product_attr_count = counts.product_attr_count;
            result.sale_attr_count = counts.sale_attr_count;
            result.product_qua_count = counts.product_qua_count;
        }
        WechatCallResult::ApiError(error) => {
            result
                .failed_steps
                .push(format!("类目详情失败：{}", error.errmsg));
            insert_api_error_and_task_log(&app, &task_id, &shop_id, &detail_call.meta, error)?;
        }
    }

    let product_rule_call = client
        .get_category_product_rule(&access_token, cat_id, 0)
        .await?;
    match &product_rule_call.result {
        WechatCallResult::Success(raw) => {
            let conn = open_connection(&app)?;
            upsert_category_rule(&conn, &shop_id, cat_id, "product", &raw.raw_payload)?;
            insert_success_api_and_task_log(
                &conn,
                &task_id,
                &shop_id,
                product_rule_call.meta.endpoint,
                product_rule_call.meta.method,
                "类目商品发布规则同步完成",
            )?;
            result.synced_product_rule = true;
        }
        WechatCallResult::ApiError(error) => {
            result
                .failed_steps
                .push(format!("商品发布规则失败：{}", error.errmsg));
            insert_api_error_and_task_log(
                &app,
                &task_id,
                &shop_id,
                &product_rule_call.meta,
                error,
            )?;
        }
    }

    let delivery_rule_call = client
        .get_delivery_method_category_rule(&access_token, cat_id)
        .await?;
    match &delivery_rule_call.result {
        WechatCallResult::Success(raw) => {
            let conn = open_connection(&app)?;
            upsert_category_rule(&conn, &shop_id, cat_id, "delivery", &raw.raw_payload)?;
            insert_success_api_and_task_log(
                &conn,
                &task_id,
                &shop_id,
                delivery_rule_call.meta.endpoint,
                delivery_rule_call.meta.method,
                "类目发货方式规则同步完成",
            )?;
            result.synced_delivery_rule = true;
        }
        WechatCallResult::ApiError(error) => {
            result
                .failed_steps
                .push(format!("发货方式规则失败：{}", error.errmsg));
            insert_api_error_and_task_log(
                &app,
                &task_id,
                &shop_id,
                &delivery_rule_call.meta,
                error,
            )?;
        }
    }

    let final_status = if result.failed_steps.is_empty() {
        "success"
    } else if !result.synced_detail && !result.synced_product_rule && !result.synced_delivery_rule {
        "failed"
    } else {
        "partial_success"
    };
    let conn = open_connection(&app)?;
    conn.execute(
        "UPDATE task_runs SET status = ?1, progress = 100, finished_at = ?2 WHERE id = ?3",
        params![final_status, now_shanghai(), task_id],
    )?;
    Ok(result)
}
