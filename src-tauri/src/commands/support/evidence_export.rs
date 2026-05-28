use super::*;

pub(in crate::commands) fn normalize_aftersale_evidence_export_format(
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

pub(in crate::commands) fn aftersale_evidence_export_json(row: &AftersaleEvidenceView) -> Value {
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

pub(in crate::commands) fn render_aftersale_evidence_markdown(
    rows: &[AftersaleEvidenceView],
) -> String {
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

pub(in crate::commands) fn markdown_table_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('\n', " ")
        .replace('\r', " ")
}
