use super::*;

pub(in crate::commands) fn load_pending_publish_items(
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

pub(in crate::commands) fn load_category_precheck_items(
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

pub(in crate::commands) fn load_attribute_fill_items(
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
           AND i.error_code IN (
             'CATEGORY_NEEDS_AI_FILL',
             'WECHAT_PAYLOAD_NEEDS_AI_FILL',
             'CATEGORY_ATTRS_NEED_AI_FILL'
           )
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

pub(in crate::commands) fn load_product_submit_items(
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

pub(in crate::commands) fn load_product_status_sync_items(
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

pub(in crate::commands) fn load_product_listing_items(
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
