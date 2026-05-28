use super::*;

pub(in crate::commands) struct AiSkillDefinition {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub runtime: &'static str,
    pub skill_path: &'static str,
    pub schema_path: &'static str,
    pub instructions: &'static str,
    pub output_schema: &'static str,
}

pub(in crate::commands) const PRODUCT_REVIEW_SKILL: AiSkillDefinition = AiSkillDefinition {
    name: "wx-xd-product-review",
    version: "1.0.0",
    description:
        "审查采集商品，清洗标题、识别不能用于微信小店铺货的图片，并从本地微信类目候选中选择类目。",
    runtime: "pi_coding_agent",
    skill_path: "agent-skills/wx-xd-product-review/SKILL.md",
    schema_path: "agent-skills/wx-xd-product-review/output_schema.json",
    instructions: include_str!("../../../../agent-skills/wx-xd-product-review/SKILL.md"),
    output_schema: include_str!("../../../../agent-skills/wx-xd-product-review/output_schema.json"),
};

pub(in crate::commands) const ATTRIBUTE_SUGGESTION_SKILL: AiSkillDefinition = AiSkillDefinition {
    name: "wx-xd-attribute-suggestion",
    version: "1.0.0",
    description: "基于微信类目详情、商品资料和 SKU 规格生成必填属性候选值。",
    runtime: "pi_coding_agent",
    skill_path: "agent-skills/wx-xd-attribute-suggestion/SKILL.md",
    schema_path: "agent-skills/wx-xd-attribute-suggestion/output_schema.json",
    instructions: include_str!("../../../../agent-skills/wx-xd-attribute-suggestion/SKILL.md"),
    output_schema: include_str!(
        "../../../../agent-skills/wx-xd-attribute-suggestion/output_schema.json"
    ),
};

pub(in crate::commands) const SUPPLIER_BRIDGE_SKILL_NAME: &str = "wx-xd-supplier-bridge";
pub(in crate::commands) const SUPPLIER_BRIDGE_SKILL_VERSION: &str = "1.0.0";

pub(in crate::commands) fn builtin_agent_skills() -> Vec<&'static AiSkillDefinition> {
    vec![&PRODUCT_REVIEW_SKILL, &ATTRIBUTE_SUGGESTION_SKILL]
}

pub(in crate::commands) fn load_agent_skill_enabled(
    conn: &Connection,
    skill: &AiSkillDefinition,
) -> AppResult<bool> {
    let enabled = conn
        .query_row(
            "SELECT enabled FROM agent_skill_settings WHERE name = ?1",
            [skill.name],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(1);
    Ok(enabled != 0)
}
