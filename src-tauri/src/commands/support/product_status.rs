use super::*;

/// 从 getproduct 返回的 audit_info 里尽量解析微信驳回理由文案。不同接口字段不一，多路径兜底；
/// 解析不到返回 None（调用方降级保留原 summary），绝不 panic。
fn extract_audit_reject_reason(audit_info: Option<&Value>) -> Option<String> {
    let info = audit_info?;
    for key in ["reject_reason", "audit_reason", "reason", "audit_desc", "desc"] {
        if let Some(text) = info.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(truncate_for_summary(trimmed, 200));
            }
        }
    }
    let mut collected: Vec<String> = Vec::new();
    let mut push_from = |item: &Value| {
        if let Some(text) = item
            .get("reason")
            .and_then(Value::as_str)
            .or_else(|| item.as_str())
        {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                collected.push(trimmed.to_string());
            }
        }
    };
    if let Some(arr) = info.as_array() {
        for item in arr {
            push_from(item);
        }
    }
    for key in ["reasons", "audit_reasons", "reject_reasons"] {
        if let Some(arr) = info.get(key).and_then(Value::as_array) {
            for item in arr {
                push_from(item);
            }
        }
    }
    if collected.is_empty() {
        None
    } else {
        Some(truncate_for_summary(&collected.join("；"), 200))
    }
}

pub(in crate::commands) fn resolve_wechat_product_status(
    info: &ProductGetInfo,
) -> ProductAuditResolution {
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
        // 解析微信驳回理由写进 summary，避免运营只看到「状态异常」却不知为何被驳。
        let summary = match extract_audit_reject_reason(info.audit_info.as_ref()) {
            Some(reason) => format!("{summary}。微信驳回原因：{reason}"),
            None => format!("{summary}。需要按微信返回原因处理后重试"),
        };
        return ProductAuditResolution {
            status: "failed",
            error_code: Some(format!("WECHAT_PRODUCT_STATUS_{code}")),
            summary,
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

pub(in crate::commands) fn resolve_price_update_confirmation(
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

pub(in crate::commands) fn snapshot_sku_sale_prices(
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

pub(in crate::commands) fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
}

pub(in crate::commands) fn all_prices_match(prices: &[i64], target_price_cents: i64) -> bool {
    !prices.is_empty() && prices.iter().all(|price| *price == target_price_cents)
}

pub(in crate::commands) fn format_price_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    format!("{sign}{}.{:02}", cents / 100, cents % 100)
}

pub(in crate::commands) fn wechat_terminal_failure_code(
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

pub(in crate::commands) fn wechat_product_status_label(status: Option<i64>) -> String {
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

pub(in crate::commands) fn wechat_product_edit_status_label(status: Option<i64>) -> String {
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
