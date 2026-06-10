use super::*;

// ============================================================================
// 铺货链路加载层（统一流水线版）
//
// 数据源从旧表（publish_job_items / publish_products / task_runs）切换到
// 新表 pipeline_shop_targets（店级推进单位）+ pipeline_products（商品主表）。
// 每个 loader 按 target.stage 取「正落在该阶段、待 runner 推进」的店级单位：
//   status = 'pending'（新到达本阶段） 或 'running'（上次被中断，幂等重做）。
// status = 'blocked' 的失败项不在此捞取，统一交由 driver 退避后置回 pending
//（与采集链路靠 retry_collection_task 手动重试同理），避免 runner 无退避狂重试。
//
// raw_payload 取 COALESCE(target.raw_payload, product.reviewed_data, '')：
// precheck 阶段 target 尚未生成店级发品 payload，回退到商品级审查结果
// （reviewed_data 即序列化后的 ExternalProductInput）；后续 runner 把生成的
// 店级发品 payload 写回 target.raw_payload，形成「读当前态 → 写当前态」闭环。
//
// PendingPublishItem 字段映射：
//   item_id        ← target.id        （店级推进单位主键）
//   job_id         ← target.product_id（新模型无 job 概念，占位为商品 id，
//                                       便于 runner 收尾时调 recompute_pipeline_product）
//   product_row_id ← target.product_id
//   shop_id / shop_status / shop_has_secret / external_product_id 同旧语义
// ============================================================================

pub(in crate::commands) fn load_pending_publish_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.product_id,
           t.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           COALESCE(p.external_product_id, ''),
           COALESCE(t.raw_payload, p.reviewed_data, '')
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         JOIN shops s ON s.id = t.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         WHERE t.stage = 'precheck'
           AND t.status IN ('pending', 'running')
         ORDER BY t.created_at ASC
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

pub(in crate::commands) fn load_category_precheck_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.product_id,
           t.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           COALESCE(p.external_product_id, ''),
           COALESCE(t.raw_payload, p.reviewed_data, '')
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         JOIN shops s ON s.id = t.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         WHERE t.stage = 'category_precheck'
           AND t.status IN ('pending', 'running')
         ORDER BY t.created_at ASC
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

pub(in crate::commands) fn load_attribute_fill_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.product_id,
           t.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           COALESCE(p.external_product_id, ''),
           COALESCE(t.raw_payload, p.reviewed_data, '')
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         JOIN shops s ON s.id = t.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         WHERE t.stage = 'attr_fill'
           AND t.status IN ('pending', 'running')
         ORDER BY t.created_at ASC
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

pub(in crate::commands) fn load_pending_price_update_items(
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

pub(in crate::commands) fn load_ready_price_update_items(
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

pub(in crate::commands) fn load_confirmable_price_update_items(
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

pub(in crate::commands) fn load_asset_upload_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.product_id,
           t.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           COALESCE(p.external_product_id, ''),
           COALESCE(t.raw_payload, p.reviewed_data, '')
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         JOIN shops s ON s.id = t.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         WHERE t.stage = 'asset_upload'
           AND t.status IN ('pending', 'running')
         ORDER BY t.created_at ASC
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

pub(in crate::commands) fn load_product_submit_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<PendingPublishItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.product_id,
           t.shop_id,
           s.status,
           c.shop_id IS NOT NULL AS shop_has_secret,
           COALESCE(p.external_product_id, ''),
           COALESCE(t.raw_payload, p.reviewed_data, '')
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         JOIN shops s ON s.id = t.shop_id
         LEFT JOIN shop_credentials c ON c.shop_id = s.id
         WHERE t.stage = 'submit'
           AND t.status IN ('pending', 'running')
         ORDER BY t.created_at ASC
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

/// audit 阶段状态轮询的最小间隔（秒）。微信审核以分钟计，driver 8s 一跳若每跳都
/// getproduct 纯属空烧 API 配额；新提交项（last_status_sync_at 为 NULL）立即查首轮，
/// 之后每 ≥60s 查一次，上架确认最多晚 60s，换来 getproduct 调用量降约 85%。
const STATUS_SYNC_MIN_INTERVAL_SECS: i64 = 60;

pub(in crate::commands) fn load_product_status_sync_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<StatusSyncItem>> {
    let stale_before =
        format_shanghai(Utc::now() - Duration::seconds(STATUS_SYNC_MIN_INTERVAL_SECS));
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.shop_id,
           COALESCE(p.external_product_id, ''),
           t.wechat_product_id
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         WHERE t.stage = 'audit'
           AND t.status IN ('pending', 'running')
           AND t.wechat_product_id IS NOT NULL
           AND t.wechat_product_id != ''
           AND (t.last_status_sync_at IS NULL OR t.last_status_sync_at <= ?2)
         ORDER BY t.last_status_sync_at IS NOT NULL ASC, t.created_at ASC
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map(params![limit, stale_before], |row| {
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

pub(in crate::commands) fn load_product_listing_items(
    conn: &Connection,
    limit: i64,
) -> AppResult<Vec<StatusSyncItem>> {
    let mut stmt = conn.prepare(
        "SELECT
           t.id,
           t.product_id,
           t.shop_id,
           COALESCE(p.external_product_id, ''),
           t.wechat_product_id
         FROM pipeline_shop_targets t
         JOIN pipeline_products p ON p.id = t.product_id
         WHERE t.stage = 'listing'
           AND t.status IN ('pending', 'running')
           AND t.wechat_product_id IS NOT NULL
           AND t.wechat_product_id != ''
         ORDER BY t.last_status_sync_at ASC, t.created_at ASC
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
