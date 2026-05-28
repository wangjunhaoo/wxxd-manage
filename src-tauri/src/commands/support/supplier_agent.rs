use super::*;

pub(in crate::commands) struct SupplierAgentAction {
    pub(in crate::commands) action: String,
    pub(in crate::commands) purchase_task_id: String,
    pub(in crate::commands) body: Value,
}

pub(in crate::commands) fn normalize_supplier_agent_export_format(
    format: Option<String>,
) -> AppResult<String> {
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

pub(in crate::commands) fn supplier_agent_task_json(row: &PurchaseTaskView) -> Value {
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

pub(in crate::commands) fn render_supplier_agent_markdown(records: &[Value]) -> String {
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

pub(in crate::commands) fn parse_supplier_agent_result_records(raw: &str) -> AppResult<Vec<Value>> {
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

pub(in crate::commands) fn supplier_agent_records_from_value(
    value: Value,
) -> AppResult<Vec<Value>> {
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

pub(in crate::commands) fn build_supplier_agent_action(
    record: &Value,
) -> AppResult<SupplierAgentAction> {
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

pub(in crate::commands) fn supplier_agent_action_fields(
    action: &str,
) -> AppResult<BTreeSet<&'static str>> {
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

pub(in crate::commands) fn apply_supplier_agent_action(
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

pub(in crate::commands) fn supplier_agent_required_string(
    value: Option<&Value>,
    field: &str,
) -> AppResult<String> {
    supplier_agent_value_string(value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("{field} 不能为空")))
}

pub(in crate::commands) fn supplier_agent_value_string(value: Option<&Value>) -> Option<String> {
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

pub(in crate::commands) fn supplier_agent_optional_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(in crate::commands) fn supplier_agent_optional_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(in crate::commands) fn find_supplier_agent_forbidden_keys(
    value: &Value,
    prefix: &str,
) -> Vec<String> {
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
