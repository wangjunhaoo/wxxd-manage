use super::*;

pub(in crate::commands) fn validate_publish_request(
    request: &ExternalPublishJobRequest,
) -> AppResult<()> {
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

pub(in crate::commands) fn validate_price_update_request(
    request: &PriceUpdateJobRequest,
) -> AppResult<()> {
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

pub(in crate::commands) fn validate_product(product: &ExternalProductInput) -> AppResult<()> {
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

pub(in crate::commands) fn resolve_target_shops(
    conn: &Connection,
    request: &ExternalPublishJobRequest,
) -> AppResult<Vec<TargetShop>> {
    resolve_target_shops_for_scope(
        conn,
        &request.target_shop_group_ids,
        &request.target_shop_ids,
    )
}

pub(in crate::commands) fn resolve_target_shops_for_scope(
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

pub(in crate::commands) fn load_job_items(
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

pub(in crate::commands) fn load_price_update_items(
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
