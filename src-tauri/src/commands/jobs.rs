use super::*;

/// 创建铺货任务：把已审查的商品数据按目标店铺展开为 pipeline 流水线记录。
/// 原为本机 HTTP API 的对外入口，HTTP API 移除后仅供采集链路（collection.rs）内部复用。
pub(in crate::commands) fn create_external_publish_job(
    app: AppHandle,
    request: ExternalPublishJobRequest,
) -> AppResult<PublishJobCreated> {
    validate_publish_request(&request)?;
    let mut conn = open_connection(&app)?;
    let targets = resolve_target_shops(&conn, &request)?;
    if targets.is_empty() {
        return Err(AppError::Validation(
            "目标店铺为空，请先在店铺组中添加店铺".to_string(),
        ));
    }

    let tx = conn.transaction()?;
    let created_at = now_shanghai();
    let mut failed_items = 0usize;
    let total_items = request.products.len() * targets.len();

    for product in &request.products {
        // 幂等：同一 external_product_id 已有在途流水线（未上架且未异常）则跳过，
        // 避免外部 API 网络重试用相同 request 重复创建商品
        let inflight: Option<String> = tx
            .query_row(
                "SELECT id FROM pipeline_products
                 WHERE external_product_id = ?1 AND status NOT IN ('listed', 'error')
                 LIMIT 1",
                params![product.external_product_id],
                |row| row.get(0),
            )
            .optional()?;
        if inflight.is_some() {
            continue;
        }
        let product_id = format!("prod_{}", Uuid::new_v4().simple());
        let reviewed_data = serde_json::to_string(product).unwrap_or_else(|_| "{}".to_string());
        // 外部 API 直接提供已采集/已审查的商品数据，跳过采集与审查，直接进入铺货阶段
        tx.execute(
            "INSERT INTO pipeline_products
             (id, external_product_id, title, source_url, category_path,
              status, stage, attention, collected_data, reviewed_data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'publishing', 'publish', 'none', ?6, ?6, ?7, ?7)",
            params![
                product_id,
                product.external_product_id,
                product.title,
                product.source_url,
                product.category_hint.clone().unwrap_or_default(),
                reviewed_data,
                created_at
            ],
        )?;

        for target in &targets {
            let duplicate = tx
                .query_row(
                    "SELECT id FROM shop_products WHERE shop_id = ?1 AND external_product_id = ?2",
                    params![target.id, product.external_product_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            // 入口期校验失败的 target 直接置 blocked（带退避时间），由商品级聚合显示待确认/异常
            let (status, error_code, error_summary, next_retry_at) = if duplicate.is_some() {
                failed_items += 1;
                (
                    "blocked",
                    Some("DUPLICATE_EXTERNAL_PRODUCT_IN_SHOP"),
                    Some("同一个 external_product_id 已经铺过该店铺"),
                    Some(created_at.clone()),
                )
            } else if product.images.len() < 3 {
                failed_items += 1;
                (
                    "blocked",
                    Some("INSUFFICIENT_HEAD_IMAGES"),
                    Some("商品主图少于 3 张，无法进入微信发品"),
                    Some(created_at.clone()),
                )
            } else {
                ("pending", None, None, None)
            };

            tx.execute(
                "INSERT INTO pipeline_shop_targets
                 (id, product_id, shop_id, shop_name, stage, status, error_code, error_summary,
                  retry_count, next_retry_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'precheck', ?5, ?6, ?7, 0, ?8, ?9, ?9)",
                params![
                    format!("tgt_{}", Uuid::new_v4().simple()),
                    product_id,
                    target.id,
                    target.name,
                    status,
                    error_code,
                    error_summary,
                    next_retry_at,
                    created_at
                ],
            )?;
        }
    }

    tx.commit()?;

    let job_status = if failed_items == total_items {
        "failed"
    } else if failed_items > 0 {
        "partial_success"
    } else {
        "queued"
    };

    Ok(PublishJobCreated {
        task_id: request.request_id.clone(),
        status: job_status.to_string(),
        accepted_product_count: request.products.len(),
        target_shop_count: targets.len(),
    })
}

#[tauri::command]
pub fn get_publish_job(app: AppHandle, task_id: String) -> AppResult<PublishJobView> {
    let conn = open_connection(&app)?;
    let mut job = conn
        .query_row(
            "SELECT id, request_id, status, accepted_product_count, target_shop_count, created_at
             FROM publish_jobs WHERE id = ?1",
            [task_id.as_str()],
            |row| {
                Ok(PublishJobView {
                    id: row.get(0)?,
                    request_id: row.get(1)?,
                    status: row.get(2)?,
                    accepted_product_count: row.get(3)?,
                    target_shop_count: row.get(4)?,
                    created_at: row.get(5)?,
                    products: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("铺货任务不存在".to_string()))?;

    let mut product_stmt = conn.prepare(
        "SELECT id, external_product_id, title, status, error_summary
         FROM publish_products WHERE job_id = ?1 ORDER BY created_at ASC",
    )?;
    let product_rows = product_stmt
        .query_map([job.id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (product_row_id, external_product_id, title, status, error_summary) in product_rows {
        let items = load_job_items(&conn, &product_row_id)?;
        let success_count = items
            .iter()
            .filter(|item| matches!(item.status.as_str(), "success" | "audit_passed"))
            .count() as i64;
        let failed_count = items.iter().filter(|item| item.status == "failed").count() as i64;
        let pending_count = items
            .iter()
            .filter(|item| {
                matches!(
                    item.status.as_str(),
                    "pending"
                        | "prechecking"
                        | "category_prechecking"
                        | "asset_uploading"
                        | "publishing"
                        | "listing"
                        | "running"
                        | "submitted"
                        | "audit_pending"
                        | "ready_to_publish"
                        | "category_prechecked"
                        | "assets_ready"
                )
            })
            .count() as i64;
        job.products.push(PublishProductView {
            external_product_id,
            title,
            status,
            success_count,
            failed_count,
            pending_count,
            error_summary,
            items,
        });
    }

    Ok(job)
}

#[tauri::command]
pub fn create_price_update_job(
    app: AppHandle,
    request: PriceUpdateJobRequest,
) -> AppResult<PriceUpdateJobCreated> {
    validate_price_update_request(&request)?;
    let mut conn = open_connection(&app)?;
    let targets = resolve_target_shops_for_scope(
        &conn,
        &request.target_shop_group_ids,
        &request.target_shop_ids,
    )?;
    if targets.is_empty() {
        return Err(AppError::Validation(
            "目标店铺为空，请先选择店铺或店铺组".to_string(),
        ));
    }

    let existing_request: Option<String> = conn
        .query_row(
            "SELECT id FROM price_update_jobs WHERE request_id = ?1",
            [request.request_id.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    if existing_request.is_some() {
        return Err(AppError::Validation(
            "request_id 已存在，不能重复创建改价任务".to_string(),
        ));
    }

    let tx = conn.transaction()?;
    let job_id = format!("price_{}", Uuid::new_v4().simple());
    let created_at = now_shanghai();
    let mut failed_items = 0usize;
    let total_items = request.products.len() * targets.len();

    tx.execute(
        "INSERT INTO price_update_jobs
         (id, request_id, status, accepted_product_count, target_shop_count, created_at)
         VALUES (?1, ?2, 'queued', ?3, ?4, ?5)",
        params![
            job_id,
            request.request_id,
            request.products.len() as i64,
            targets.len() as i64,
            created_at
        ],
    )?;
    tx.execute(
        "INSERT INTO task_runs (id, task_type, status, progress, created_at)
         VALUES (?1, 'price.create_update_job', 'pending', 0, ?2)",
        params![job_id, created_at],
    )?;

    for product in &request.products {
        for target in &targets {
            let shop_product = tx
                .query_row(
                    "SELECT wechat_product_id
                     FROM shop_products
                     WHERE shop_id = ?1 AND external_product_id = ?2",
                    params![target.id, product.external_product_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?;
            let (status, error_code, error_summary, wechat_product_id) = match shop_product {
                None => {
                    failed_items += 1;
                    (
                        "failed",
                        Some("SHOP_PRODUCT_NOT_FOUND"),
                        Some("该店铺未找到 external_product_id 对应的已铺货商品"),
                        None,
                    )
                }
                Some(None) => {
                    failed_items += 1;
                    (
                        "failed",
                        Some("MISSING_WECHAT_PRODUCT_ID"),
                        Some("该店铺商品缺少微信 product_id，不能进入改价"),
                        None,
                    )
                }
                Some(Some(wechat_product_id)) if wechat_product_id.trim().is_empty() => {
                    failed_items += 1;
                    (
                        "failed",
                        Some("MISSING_WECHAT_PRODUCT_ID"),
                        Some("该店铺商品缺少微信 product_id，不能进入改价"),
                        None,
                    )
                }
                Some(Some(wechat_product_id)) => ("pending", None, None, Some(wechat_product_id)),
            };
            tx.execute(
                "INSERT INTO price_update_items
                 (id, job_id, shop_id, shop_name, external_product_id, target_price_cents,
                  status, error_code, error_summary, wechat_product_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    format!("price-item-{}", Uuid::new_v4()),
                    job_id,
                    target.id,
                    target.name,
                    product.external_product_id,
                    product.target_price_cents,
                    status,
                    error_code,
                    error_summary,
                    wechat_product_id,
                    created_at
                ],
            )?;
        }
    }

    let job_status = if failed_items == total_items {
        "failed"
    } else if failed_items > 0 {
        "partial_success"
    } else {
        "queued"
    };
    tx.execute(
        "UPDATE price_update_jobs SET status = ?1 WHERE id = ?2",
        params![job_status, job_id],
    )?;
    tx.execute(
        "UPDATE task_runs SET status = ?1 WHERE id = ?2",
        params![job_status, job_id],
    )?;
    tx.commit()?;

    Ok(PriceUpdateJobCreated {
        task_id: job_id,
        status: job_status.to_string(),
        accepted_product_count: request.products.len(),
        target_shop_count: targets.len(),
    })
}

#[tauri::command]
pub fn get_price_update_job(app: AppHandle, task_id: String) -> AppResult<PriceUpdateJobView> {
    let conn = open_connection(&app)?;
    let mut job = conn
        .query_row(
            "SELECT id, request_id, status, accepted_product_count, target_shop_count, created_at
             FROM price_update_jobs WHERE id = ?1",
            [task_id.as_str()],
            |row| {
                Ok(PriceUpdateJobView {
                    id: row.get(0)?,
                    request_id: row.get(1)?,
                    status: row.get(2)?,
                    accepted_product_count: row.get(3)?,
                    target_shop_count: row.get(4)?,
                    created_at: row.get(5)?,
                    items: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("改价任务不存在".to_string()))?;
    job.items = load_price_update_items(&conn, &job.id)?;
    Ok(job)
}
