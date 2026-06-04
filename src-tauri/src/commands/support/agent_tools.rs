// Agent 工具查询函数 — 供 local_api.rs 的 /api/agent/ 端点调用。
// 每个函数接收 &AppHandle + &Connection + 参数，返回序列化 JSON。

use crate::models::ExternalProductInput;
use crate::storage::{open_connection, AppError, AppResult};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;

// ── 商品查询 ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentProductDetail {
    pub task_id: String,
    pub title: String,
    pub source_url: String,
    pub category_hint: Option<String>,
    pub brand_hint: Option<String>,
    pub images: Vec<String>,
    pub detail_images: Vec<String>,
    pub skus: Vec<AgentSkuSummary>,
    pub review_status: String,
    pub review_summary: Option<String>,
    pub wechat_category_ids: Option<Vec<i64>>,
    pub wechat_category_path: Option<String>,
    pub ai_attr_suggestions: Option<Value>,
    pub taobao_item_params: Option<Value>,
    pub supplier_name: Option<String>,
    pub weight_gram: Option<i64>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct AgentSkuSummary {
    pub external_sku_id: String,
    pub specs: Value,
    pub cost_price: f64,
    pub stock: i64,
}

pub(crate) fn get_agent_product_detail(
    app: &AppHandle,
    task_id: &str,
) -> AppResult<AgentProductDetail> {
    let conn = open_connection(app)?;
    let row = conn.query_row(
        "SELECT id, title, source_url, category_path, status, error_reason,
                collected_data, stage, progress_text, reviewed_data, review_result_json,
                '[]', '[]', NULL, created_at, updated_at
         FROM pipeline_products WHERE id = ?1",
        params![task_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
            ))
        },
    )?;

    let (
        id,
        title,
        source_url,
        _category_path,
        _status,
        _error_summary,
        collected_data,
        review_status,
        review_summary,
        reviewed_data,
        _review_result_json,
        _published_shop_ids,
        _publish_job_ids,
        _published_at,
        created_at,
        _updated_at,
    ) = row;

    // 优先用 reviewed_data，其次 collected_data
    let data_json = reviewed_data.or(collected_data);

    // 尝试解析 ExternalProductInput
    let product: Option<ExternalProductInput> = data_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok());

    let product_ref = product.as_ref();

    // 提取微信类目信息
    let (wechat_category_ids, wechat_category_path) = product_ref
        .and_then(|p| p.metadata.as_object())
        .map(|m| {
            let ids = m
                .get("wechat_category_ids")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>());
            let path = m
                .get("wechat_category_infer_source")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (ids, path)
        })
        .unwrap_or((None, None));

    let ai_attr_suggestions = product_ref
        .and_then(|p| p.metadata.as_object())
        .and_then(|m| m.get("ai_attr_suggestions").cloned());

    let taobao_item_params = product_ref
        .and_then(|p| p.metadata.as_object())
        .and_then(|m| m.get("taobao_item_params").cloned());

    let images = product_ref.map(|p| p.images.clone()).unwrap_or_default();
    let detail_images = product_ref
        .map(|p| p.detail_images.clone())
        .unwrap_or_default();
    let category_hint = product_ref.and_then(|p| p.category_hint.clone());
    let brand_hint = product_ref.and_then(|p| p.brand_hint.clone());
    let supplier_name = product_ref.and_then(|p| p.supplier_name.clone());
    let weight_gram = product_ref.and_then(|p| p.weight_gram);

    let skus = product
        .map(|p| {
            p.skus
                .into_iter()
                .map(|sku| AgentSkuSummary {
                    external_sku_id: sku.external_sku_id,
                    specs: sku.specs,
                    cost_price: sku.cost_price,
                    stock: sku.stock,
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(AgentProductDetail {
        task_id: id,
        title,
        source_url,
        category_hint,
        brand_hint,
        images,
        detail_images,
        skus,
        review_status,
        review_summary,
        wechat_category_ids,
        wechat_category_path,
        ai_attr_suggestions,
        taobao_item_params,
        supplier_name,
        weight_gram,
        created_at,
    })
}

// ── 微信类目查询 ────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentCategorySearchResult {
    pub cat_id: i64,
    pub name: String,
    pub full_path: String,
    pub level: Option<i64>,
    pub is_leaf: bool,
    pub is_available: bool,
}

pub(crate) fn search_agent_categories(
    app: &AppHandle,
    shop_id: &str,
    query: &str,
    limit: i64,
) -> AppResult<Vec<AgentCategorySearchResult>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "WITH RECURSIVE path_cte(cat_id, parent_cat_id, name, level, path_name, depth) AS (
           SELECT c.cat_id, c.parent_cat_id, c.name, c.level, CAST(c.name AS TEXT), 1
           FROM wechat_categories c
           WHERE c.shop_id = ?1 AND c.name LIKE '%' || ?2 || '%'
           UNION ALL
           SELECT c.cat_id, c.parent_cat_id, c.name, c.level,
                  CAST(c.name || ' > ' || p.path_name AS TEXT), p.depth + 1
           FROM wechat_categories c
           JOIN path_cte p ON c.cat_id = p.parent_cat_id
           WHERE c.shop_id = ?1 AND p.depth < 5
         )
         SELECT cat_id, name, path_name, level,
                NOT EXISTS(SELECT 1 FROM wechat_categories child
                           WHERE child.shop_id = ?1 AND child.parent_cat_id = path_cte.cat_id) AS is_leaf,
                EXISTS(SELECT 1 FROM wechat_category_relations rel
                       WHERE rel.shop_id = ?1 AND rel.cat_id = path_cte.cat_id AND rel.status = 1) AS is_available
         FROM path_cte
         ORDER BY depth DESC, name ASC
         LIMIT ?3",
    )?;
    let results = stmt
        .query_map(params![shop_id, query, limit], |row| {
            Ok(AgentCategorySearchResult {
                cat_id: row.get(0)?,
                name: row.get(1)?,
                full_path: row.get(2)?,
                level: row.get(3)?,
                is_leaf: row.get::<_, i64>(4)? != 0,
                is_available: row.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

// ── 类目详情查询 ────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentCategoryDetail {
    pub cat_id: i64,
    pub name: String,
    pub parent_cat_id: Option<i64>,
    pub product_attrs: Vec<AgentCategoryAttr>,
    pub sale_attrs: Vec<AgentCategoryAttr>,
    pub all_pass: Option<bool>,
    pub fail_reasons: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct AgentCategoryAttr {
    pub key: String,
    pub attr_type: Option<String>,
    pub is_required: bool,
    pub allowed_values: Vec<String>,
    pub append_allowed: bool,
}

pub(crate) fn get_agent_category_detail(
    app: &AppHandle,
    shop_id: &str,
    cat_id: i64,
) -> AppResult<Option<AgentCategoryDetail>> {
    let conn = open_connection(app)?;
    let cat_row = conn
        .query_row(
            "SELECT cat_id, name, parent_cat_id, raw_payload
             FROM wechat_categories WHERE shop_id = ?1 AND cat_id = ?2",
            params![shop_id, cat_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((cat_id, name, parent_cat_id, _)) = cat_row else {
        return Ok(None);
    };

    // 查类目详情（属性列表）
    let detail_row = conn
        .query_row(
            "SELECT raw_payload FROM wechat_category_details WHERE shop_id = ?1 AND cat_id = ?2",
            params![shop_id, cat_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?;

    let (product_attrs, sale_attrs) = detail_row
        .flatten()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .map(|detail| {
            (
                extract_category_attr_list(&detail, "product_attr_list"),
                extract_category_attr_list(&detail, "sale_attr_list"),
            )
        })
        .unwrap_or_default();

    // 查预检结果
    let precheck = conn
        .query_row(
            "SELECT raw_payload FROM wechat_category_prechecks WHERE shop_id = ?1 AND cat_id = ?2",
            params![shop_id, cat_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());

    let all_pass = precheck
        .as_ref()
        .and_then(|v| v.get("all_pass"))
        .and_then(|v| v.as_bool());
    let fail_reasons: Option<Vec<String>> = precheck
        .as_ref()
        .and_then(|v| v.get("fail_reasons"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    Ok(Some(AgentCategoryDetail {
        cat_id,
        name,
        parent_cat_id,
        product_attrs,
        sale_attrs,
        all_pass,
        fail_reasons,
    }))
}

fn extract_category_attr_list(detail: &Value, list_key: &str) -> Vec<AgentCategoryAttr> {
    let mut attrs = Vec::new();
    collect_category_attrs(detail, list_key, &mut attrs);
    attrs
}

fn collect_category_attrs(value: &Value, list_key: &str, attrs: &mut Vec<AgentCategoryAttr>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_category_attrs(item, list_key, attrs);
            }
        }
        Value::Object(object) => {
            if let Some(items) = object.get(list_key).and_then(Value::as_array) {
                for item in items {
                    if let Some(obj) = item.as_object() {
                        let key = ["name", "attr_key", "attr_name", "key"]
                            .iter()
                            .find_map(|k| {
                                obj.get(*k)
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.trim().to_string())
                            })
                            .unwrap_or_default();
                        if key.is_empty() {
                            continue;
                        }
                        let is_required = ["is_required", "required", "mandatory", "is_mandatory"]
                            .iter()
                            .any(|k| obj.get(*k).and_then(|v| v.as_bool()).unwrap_or(false))
                            || obj
                                .get("required_rule")
                                .and_then(Value::as_object)
                                .and_then(|rule| rule.get("rule_type"))
                                .and_then(|v| v.as_i64())
                                == Some(1);
                        let allowed_values = ["value_list", "values", "attr_values"]
                            .iter()
                            .find_map(|k| obj.get(*k).and_then(Value::as_array))
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| match v {
                                        Value::String(s) => {
                                            let parts: Vec<String> = s
                                                .split(|ch| matches!(ch, ';' | '；'))
                                                .map(|s| s.trim().to_string())
                                                .filter(|s| !s.is_empty())
                                                .collect();
                                            Some(parts)
                                        }
                                        Value::Object(o) => {
                                            ["attr_value", "value", "name", "value_name"]
                                                .iter()
                                                .find_map(|k| o.get(*k).and_then(|v| v.as_str()))
                                                .map(|s| vec![s.trim().to_string()])
                                        }
                                        _ => None,
                                    })
                                    .flatten()
                                    .collect()
                            })
                            .unwrap_or_default();
                        let attr_type = ["type_v2", "type"].iter().find_map(|k| {
                            obj.get(*k)
                                .and_then(|v| v.as_str())
                                .map(|s| s.trim().to_lowercase())
                        });
                        let append_allowed = ["append_allowed", "allow_append", "custom_allowed"]
                            .iter()
                            .any(|k| obj.get(*k).and_then(|v| v.as_bool()).unwrap_or(false));
                        attrs.push(AgentCategoryAttr {
                            key,
                            attr_type,
                            is_required,
                            allowed_values,
                            append_allowed,
                        });
                    }
                }
            }
            for child in object.values() {
                collect_category_attrs(child, list_key, attrs);
            }
        }
        _ => {}
    }
}

// ── 店铺查询 ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentShopInfo {
    pub shop_id: String,
    pub name: String,
    pub appid: String,
    pub status: String,
    pub wechat_nickname: Option<String>,
    pub wechat_status: Option<String>,
    pub wechat_subject_type: Option<String>,
    pub is_local_life: bool,
    pub has_secret: bool,
    pub category_relation_count: i64,
    pub active_category_relation_count: i64,
    pub freight_template_count: i64,
    pub last_health_check_at: Option<String>,
}

pub(crate) fn get_agent_shop_info(
    app: &AppHandle,
    shop_id: &str,
) -> AppResult<Option<AgentShopInfo>> {
    let conn = open_connection(app)?;
    let row = conn
        .query_row(
            "SELECT s.id, s.name, s.appid, s.status,
                    s.wechat_nickname, s.wechat_status, s.wechat_subject_type,
                    COALESCE(s.is_local_life, 0),
                    c.shop_id IS NOT NULL AS has_secret,
                    s.last_health_check_at
             FROM shops s
             LEFT JOIN shop_credentials c ON c.shop_id = s.id
             WHERE s.id = ?1",
            params![shop_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()?;

    let Some((
        shop_id,
        name,
        appid,
        status,
        wechat_nickname,
        wechat_status,
        wechat_subject_type,
        is_local_life,
        has_secret,
        last_health_check_at,
    )) = row
    else {
        return Ok(None);
    };

    let (category_relation_count, active_category_relation_count) = conn
        .query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN status = 1 THEN 1 ELSE 0 END)
             FROM wechat_category_relations WHERE shop_id = ?1",
            params![&shop_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap_or((0, 0));

    let freight_template_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM wechat_freight_templates WHERE shop_id = ?1",
            params![&shop_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(Some(AgentShopInfo {
        shop_id,
        name,
        appid,
        status,
        wechat_nickname,
        wechat_status,
        wechat_subject_type,
        is_local_life: is_local_life != 0,
        has_secret: has_secret != 0,
        category_relation_count,
        active_category_relation_count,
        freight_template_count,
        last_health_check_at,
    }))
}

// ── 微信文档查询 ────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentDocResult {
    pub title: String,
    pub snippet: String,
    pub category: String,
}

/// 本地微信小店属性知识库 — 覆盖最常用的童装类目属性定义、枚举和规则
const WECHAT_DOC_KNOWLEDGE: &[(&str, &str, &str, &str)] = &[
    // 安全等级
    (
        "安全等级",
        "A类/B类/C类",
        "安全技术类别",
        "GB 18401 纺织品安全技术规范：A类=婴幼儿用品（0-36个月），B类=直接接触皮肤产品，C类=非直接接触皮肤产品。童装类目通常要求 A类 或 B类。",
    ),
    (
        "适用年龄",
        "婴幼儿/儿童/成人",
        "适用年龄",
        "微信小店的通用年龄分段：婴幼儿(0-3岁)、小童(3-6岁)、中童(6-12岁)、大童(12-16岁)。根据商品标题中的'婴幼''宝宝''小童''中童''大童'等关键词和 SKU 尺码判断。",
    ),
    (
        "面料材质",
        "纯棉/涤纶/锦纶/氨纶/棉麻…",
        "面料材质",
        "微信类目要求的 select_one 属性，必须从类目详情 allowed_values 中选取。常见材质映射：棉→纯棉，涤→涤纶(聚酯纤维)，尼龙→锦纶。",
    ),
    (
        "面料材质成分含量",
        "棉100%/涤纶65%,棉35%…",
        "面料成分含量",
        "string 类型属性，格式：'{材质名称}{百分比}%'。多个材质用逗号分隔。可根据已知面料材质推断：纯棉→棉100%，涤棉→涤纶65%,棉35%。",
    ),
    (
        "颜色",
        "黑/白/红/蓝/多色…",
        "颜色",
        "微信类目 select_one 属性。如果商品有多个颜色 SKU 且无法归纳为单一颜色，应选择'多色'。如果 SKU 颜色为编码（如 A01/A02），需根据商品图片和标题判断实际颜色。",
    ),
    (
        "风格",
        "韩版/欧美/日系/田园/运动…",
        "风格",
        "微信类目 select_one 属性。童装常见风格：韩版、运动、田园/小清新风、日系、英伦、休闲。从标题和商品图片判断。",
    ),
    (
        "版型",
        "宽松/修身/直筒…",
        "版型",
        "童装版型属性，常见值：宽松、修身、直筒、常规。从标题关键词判断：'宽松''肥大'→宽松，'修身''紧身'→修身。",
    ),
    (
        "适用季节",
        "春季/夏季/秋季/冬季/四季通用",
        "适用季节",
        "从标题和商品图片判断。含'夏季''夏装''短袖'→夏季，含'冬季''冬装''加厚''加绒'→冬季，含'春季''春秋'→春秋季。",
    ),
    (
        "尺码",
        "73cm/80cm/90cm…",
        "尺码",
        "销售属性(select_one)，每个 SKU 分别填写。如果 SKU 已有'身高'或'尺码'规格值（如 73CM、80cm），直接映射到最近似的微信尺码选项。允许 append 自定义尺码值。",
    ),
    (
        "商品图片要求",
        "主图≥3张 详情图≥1张",
        "微信发品图片规范",
        "微信 addproduct 要求：主图( head_imgs )最少3张最多9张，详情图( detail_imgs )最少1张最多50张。图片URL必须是微信素材上传接口返回的微信域名URL，不能直接使用外部CDN链接。",
    ),
    (
        "售后地址",
        "快递/自提",
        "微信发品 address_id",
        "快递发货(deliver_method=0)必须提供 after_sale_info.after_sale_address_id。如果未指定，系统会自动从店铺地址列表中选择默认退货地址。",
    ),
    (
        "SKU 价格",
        "market_price/sale_price",
        "微信 SKU 定价",
        "market_price=市场价(划线价)，sale_price=实际销售价。cost_price 来自采集时的成本价（不含利润），系统会根据定价策略自动计算 sale_price。",
    ),
];

pub(crate) fn search_agent_docs(_app: &AppHandle, query: &str) -> AppResult<Vec<AgentDocResult>> {
    let query_lower = query.to_lowercase();
    let results: Vec<AgentDocResult> = WECHAT_DOC_KNOWLEDGE
        .iter()
        .filter(|(title, _values, _category, content)| {
            title.to_lowercase().contains(&query_lower)
                || _values.to_lowercase().contains(&query_lower)
                || _category.to_lowercase().contains(&query_lower)
                || content.to_lowercase().contains(&query_lower)
                || query_lower.contains(&title.to_lowercase())
        })
        .take(10)
        .map(|(title, _values, category, content)| AgentDocResult {
            title: title.to_string(),
            snippet: content.to_string(),
            category: category.to_string(),
        })
        .collect();
    Ok(results)
}

// ═══════════════════════════════════════════════════════════
// P1 工具：类目树 / 运费 / 地址 / 快递
// ═══════════════════════════════════════════════════════════

// ── 已开通类目列表 ─────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentActiveCategory {
    pub cat_id: i64,
    pub name: String,
    pub full_path: String,
    pub level: Option<i64>,
    pub is_leaf: bool,
    pub status: i64,
}

pub(crate) fn get_agent_active_categories(
    app: &AppHandle,
    shop_id: &str,
) -> AppResult<Vec<AgentActiveCategory>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "WITH RECURSIVE path_cte(cat_id, parent_cat_id, name, level, path_name, depth) AS (
           SELECT c.cat_id, c.parent_cat_id, c.name, c.level, CAST(c.name AS TEXT), 1
           FROM wechat_categories c
           JOIN wechat_category_relations r ON r.shop_id = c.shop_id AND r.cat_id = c.cat_id AND r.status = 1
           WHERE c.shop_id = ?1
             AND NOT EXISTS(SELECT 1 FROM wechat_categories child WHERE child.shop_id = ?1 AND child.parent_cat_id = c.cat_id)
           UNION ALL
           SELECT c.cat_id, c.parent_cat_id, c.name, c.level,
                  CAST(c.name || ' > ' || p.path_name AS TEXT), p.depth + 1
           FROM wechat_categories c
           JOIN path_cte p ON c.cat_id = p.parent_cat_id
           WHERE c.shop_id = ?1 AND p.depth < 5
         )
         SELECT cat_id, name, path_name, level,
                NOT EXISTS(SELECT 1 FROM wechat_categories child WHERE child.shop_id = ?1 AND child.parent_cat_id = path_cte.cat_id) AS is_leaf,
                (SELECT r.status FROM wechat_category_relations r WHERE r.shop_id = ?1 AND r.cat_id = path_cte.cat_id)
         FROM path_cte
         ORDER BY depth DESC, name ASC",
    )?;
    let results = stmt
        .query_map(params![shop_id], |row| {
            Ok(AgentActiveCategory {
                cat_id: row.get(0)?,
                name: row.get(1)?,
                full_path: row.get(2)?,
                level: row.get(3)?,
                is_leaf: row.get::<_, i64>(4)? != 0,
                status: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

// ── 类目树 ─────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentCategoryTreeNode {
    pub cat_id: i64,
    pub name: String,
    pub level: Option<i64>,
    pub is_available: bool,
    pub has_children: bool,
    pub children: Vec<AgentCategoryTreeNode>,
}

pub(crate) fn get_agent_category_tree(
    app: &AppHandle,
    shop_id: &str,
    parent_cat_id: Option<i64>,
) -> AppResult<Vec<AgentCategoryTreeNode>> {
    let conn = open_connection(app)?;
    let parent_filter = match parent_cat_id {
        Some(id) => format!("AND c.parent_cat_id = {id}"),
        None => "AND (c.parent_cat_id IS NULL OR c.parent_cat_id = 0)".to_string(),
    };
    let sql = format!(
        "SELECT c.cat_id, c.name, c.level,
                EXISTS(SELECT 1 FROM wechat_category_relations r WHERE r.shop_id = ?1 AND r.cat_id = c.cat_id AND r.status = 1) AS is_available,
                EXISTS(SELECT 1 FROM wechat_categories child WHERE child.shop_id = ?1 AND child.parent_cat_id = c.cat_id) AS has_children
         FROM wechat_categories c
         WHERE c.shop_id = ?1 {parent_filter}
         ORDER BY c.cat_id ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<(i64, String, Option<i64>, bool, bool)> = stmt
        .query_map(params![shop_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, i64>(4)? != 0,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(
            |(cat_id, name, level, is_available, has_children)| AgentCategoryTreeNode {
                cat_id,
                name,
                level,
                is_available,
                has_children,
                children: Vec::new(),
            },
        )
        .collect())
}

// ── 运费模板 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentFreightTemplate {
    pub template_id: String,
    pub name: Option<String>,
    pub synced_at: String,
}

pub(crate) fn get_agent_freight_templates(
    app: &AppHandle,
    shop_id: &str,
) -> AppResult<Vec<AgentFreightTemplate>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT template_id, raw_payload, synced_at
         FROM wechat_freight_templates WHERE shop_id = ?1
         ORDER BY synced_at DESC",
    )?;
    let results = stmt
        .query_map(params![shop_id], |row| {
            let raw: String = row.get(1)?;
            let name = serde_json::from_str::<Value>(&raw).ok().and_then(|v| {
                v.get("name")
                    .or_else(|| v.get("template_name"))
                    .or_else(|| v.get("title"))
                    .and_then(|n| n.as_str())
                    .map(String::from)
            });
            Ok(AgentFreightTemplate {
                template_id: row.get(0)?,
                name,
                synced_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

// ── 售后地址 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentAfterSaleAddress {
    pub address_id: Option<i64>,
    pub note: String,
}

pub(crate) fn get_agent_after_sale_addresses(
    _app: &AppHandle,
    _shop_id: &str,
) -> AppResult<Vec<AgentAfterSaleAddress>> {
    // 售后地址不持久化到本地 DB，需通过微信 API 实时获取。
    // 铺货时 `run_publish_category_prechecks_once` 会自动从微信拉取并选择默认退货地址。
    // Agent 不需要手动管理售后地址 — 交给铺货流水线自动处理。
    Ok(vec![AgentAfterSaleAddress {
        address_id: None,
        note: "售后地址由铺货流水线自动从微信 API 获取并选择默认退货地址，无需 Agent 手动指定"
            .to_string(),
    }])
}

// ── 快递公司列表 ───────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentDeliveryCompany {
    pub delivery_id: String,
    pub delivery_name: String,
    pub synced_at: String,
}

pub(crate) fn get_agent_delivery_companies(
    app: &AppHandle,
    shop_id: &str,
) -> AppResult<Vec<AgentDeliveryCompany>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT delivery_id, delivery_name, synced_at
         FROM delivery_companies WHERE shop_id = ?1
         ORDER BY delivery_name ASC",
    )?;
    let results = stmt
        .query_map(params![shop_id], |row| {
            Ok(AgentDeliveryCompany {
                delivery_id: row.get(0)?,
                delivery_name: row.get(1)?,
                synced_at: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

// ═══════════════════════════════════════════════════════════
// P2 工具：订单 / 售后 / 采购 / 商品 / 分析
// ═══════════════════════════════════════════════════════════

// ── 订单详情 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentOrderDetail {
    pub order_id: String,
    pub shop_id: String,
    pub wechat_order_id: String,
    pub wechat_status: Option<i64>,
    pub order_created_at: Option<i64>,
    pub items: Vec<AgentOrderItem>,
    pub purchase_tasks: Vec<AgentPurchaseTaskSummary>,
    pub shipments: Vec<AgentShipmentSummary>,
}

#[derive(Serialize)]
pub struct AgentOrderItem {
    pub id: String,
    pub wechat_product_id: Option<String>,
    pub wechat_sku_id: Option<String>,
    pub external_product_id: Option<String>,
    pub external_sku_id: Option<String>,
    pub title: Option<String>,
    pub quantity: i64,
    pub sale_price: Option<i64>,
    pub real_price: Option<i64>,
}

#[derive(Serialize)]
pub struct AgentPurchaseTaskSummary {
    pub id: String,
    pub status: String,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub supplier_waybill_id: Option<String>,
    pub quantity: i64,
    pub estimated_cost: Option<f64>,
}

#[derive(Serialize)]
pub struct AgentShipmentSummary {
    pub id: String,
    pub status: String,
    pub delivery_name: Option<String>,
    pub waybill_id: Option<String>,
    pub submitted_at: Option<String>,
}

pub(crate) fn get_agent_order(
    app: &AppHandle,
    order_id: &str,
) -> AppResult<Option<AgentOrderDetail>> {
    let conn = open_connection(app)?;
    let row = conn
        .query_row(
            "SELECT id, shop_id, wechat_order_id, wechat_status, order_created_at
             FROM orders WHERE id = ?1",
            params![order_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((order_id, shop_id, wechat_order_id, wechat_status, order_created_at)) = row else {
        return Ok(None);
    };

    let mut item_stmt = conn.prepare(
        "SELECT id, wechat_product_id, wechat_sku_id, external_product_id, external_sku_id,
                title, sku_count, sale_price, real_price
         FROM order_items WHERE order_id = ?1",
    )?;
    let items = item_stmt
        .query_map(params![&order_id], |row| {
            Ok(AgentOrderItem {
                id: row.get(0)?,
                wechat_product_id: row.get(1)?,
                wechat_sku_id: row.get(2)?,
                external_product_id: row.get(3)?,
                external_sku_id: row.get(4)?,
                title: row.get(5)?,
                quantity: row.get(6)?,
                sale_price: row.get(7)?,
                real_price: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut pt_stmt = conn.prepare(
        "SELECT id, status, supplier_name, supplier_product_id, supplier_waybill_id,
                quantity, estimated_cost
         FROM purchase_tasks WHERE order_id = ?1",
    )?;
    let purchase_tasks = pt_stmt
        .query_map(params![&order_id], |row| {
            Ok(AgentPurchaseTaskSummary {
                id: row.get(0)?,
                status: row.get(1)?,
                supplier_name: row.get(2)?,
                supplier_product_id: row.get(3)?,
                supplier_waybill_id: row.get(4)?,
                quantity: row.get(5)?,
                estimated_cost: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut sh_stmt = conn.prepare(
        "SELECT id, status, delivery_name, waybill_id, submitted_at
         FROM shipments WHERE order_id = ?1",
    )?;
    let shipments = sh_stmt
        .query_map(params![&order_id], |row| {
            Ok(AgentShipmentSummary {
                id: row.get(0)?,
                status: row.get(1)?,
                delivery_name: row.get(2)?,
                waybill_id: row.get(3)?,
                submitted_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(AgentOrderDetail {
        order_id,
        shop_id,
        wechat_order_id,
        wechat_status,
        order_created_at,
        items,
        purchase_tasks,
        shipments,
    }))
}

// ── 订单列表 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentOrderSummary {
    pub order_id: String,
    pub shop_id: String,
    pub wechat_order_id: String,
    pub wechat_status: Option<i64>,
    pub item_count: i64,
    pub revenue_cents: i64,
    pub order_created_at: Option<i64>,
}

pub(crate) fn list_agent_orders(
    app: &AppHandle,
    shop_id: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AgentOrderSummary>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT o.id, o.shop_id, o.wechat_order_id, o.wechat_status,
                (SELECT COUNT(*) FROM order_items WHERE order_id = o.id) AS item_count,
                COALESCE((SELECT SUM(real_price * sku_count) FROM order_items WHERE order_id = o.id), 0) AS revenue,
                o.order_created_at
         FROM orders o
         ORDER BY o.order_created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| map_order_summary(row))?
        .filter_map(|r| r.ok())
        .filter(|row| {
            if let Some(sid) = shop_id {
                if row.shop_id != sid {
                    return false;
                }
            }
            if let Some(st) = status.filter(|s| !s.is_empty()) {
                if row.wechat_status.map(|v| v.to_string()) != Some(st.to_string()) {
                    return false;
                }
            }
            true
        })
        .take(limit as usize)
        .collect();
    Ok(rows)
}

fn map_order_summary(row: &rusqlite::Row) -> rusqlite::Result<AgentOrderSummary> {
    Ok(AgentOrderSummary {
        order_id: row.get(0)?,
        shop_id: row.get(1)?,
        wechat_order_id: row.get(2)?,
        wechat_status: row.get(3)?,
        item_count: row.get(4)?,
        revenue_cents: row.get(5)?,
        order_created_at: row.get(6)?,
    })
}

// ── 售后单详情 ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentAftersaleDetail {
    pub id: String,
    pub shop_id: String,
    pub wechat_aftersale_id: String,
    pub order_id: Option<String>,
    pub wechat_order_id: Option<String>,
    pub status: String,
    pub aftersale_type: Option<String>,
    pub reason: Option<String>,
    pub refund_amount_cents: Option<i64>,
    pub responsibility_party: Option<String>,
    pub supplier_compensation_cents: i64,
    pub last_action: Option<String>,
    pub last_action_status: Option<String>,
    pub synced_at: String,
}

pub(crate) fn get_agent_aftersale(
    app: &AppHandle,
    aftersale_id: &str,
) -> AppResult<Option<AgentAftersaleDetail>> {
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT id, shop_id, wechat_aftersale_id, order_id, wechat_order_id, status,
                aftersale_type, reason, refund_amount_cents, responsibility_party,
                supplier_compensation_cents, last_action, last_action_status, synced_at
         FROM aftersales WHERE id = ?1",
        params![aftersale_id],
        |row| {
            Ok(AgentAftersaleDetail {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                wechat_aftersale_id: row.get(2)?,
                order_id: row.get(3)?,
                wechat_order_id: row.get(4)?,
                status: row.get(5)?,
                aftersale_type: row.get(6)?,
                reason: row.get(7)?,
                refund_amount_cents: row.get(8)?,
                responsibility_party: row.get(9)?,
                supplier_compensation_cents: row.get(10)?,
                last_action: row.get(11)?,
                last_action_status: row.get(12)?,
                synced_at: row.get(13)?,
            })
        },
    )
    .optional()
    .map_err(AppError::from)
}

// ── 售后列表 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentAftersaleSummary {
    pub id: String,
    pub shop_id: String,
    pub wechat_aftersale_id: String,
    pub wechat_order_id: Option<String>,
    pub status: String,
    pub aftersale_type: Option<String>,
    pub refund_amount_cents: Option<i64>,
    pub responsibility_party: Option<String>,
    pub synced_at: String,
}

pub(crate) fn list_agent_aftersales(
    app: &AppHandle,
    shop_id: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AgentAftersaleSummary>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, wechat_aftersale_id, wechat_order_id, status,
                aftersale_type, refund_amount_cents, responsibility_party, synced_at
         FROM aftersales ORDER BY synced_at DESC",
    )?;
    let rows: Vec<AgentAftersaleSummary> = stmt
        .query_map([], |row| map_aftersale_summary(row))?
        .filter_map(|r| r.ok())
        .filter(|row| {
            if let Some(sid) = shop_id {
                if row.shop_id != sid {
                    return false;
                }
            }
            if let Some(st) = status.filter(|s| !s.is_empty()) {
                if row.status != st {
                    return false;
                }
            }
            true
        })
        .take(limit as usize)
        .collect();
    Ok(rows)
}

fn map_aftersale_summary(row: &rusqlite::Row) -> rusqlite::Result<AgentAftersaleSummary> {
    Ok(AgentAftersaleSummary {
        id: row.get(0)?,
        shop_id: row.get(1)?,
        wechat_aftersale_id: row.get(2)?,
        wechat_order_id: row.get(3)?,
        status: row.get(4)?,
        aftersale_type: row.get(5)?,
        refund_amount_cents: row.get(6)?,
        responsibility_party: row.get(7)?,
        synced_at: row.get(8)?,
    })
}

// ── 拒绝原因枚举 ───────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentRejectReason {
    pub reason_type: i64,
    pub reason_type_text: String,
    pub reason: String,
    pub reject_scene: Option<i64>,
}

pub(crate) fn get_agent_reject_reasons(
    app: &AppHandle,
    shop_id: &str,
) -> AppResult<Vec<AgentRejectReason>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT reject_reason_type, reject_reason_type_text, reject_reason, reject_scene
         FROM aftersale_reject_reasons WHERE shop_id = ?1
         ORDER BY reject_scene, reject_reason_type ASC",
    )?;
    let results = stmt
        .query_map(params![shop_id], |row| {
            Ok(AgentRejectReason {
                reason_type: row.get(0)?,
                reason_type_text: row.get(1)?,
                reason: row.get(2)?,
                reject_scene: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(results)
}

// ── 采购任务详情 ───────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentPurchaseTaskDetail {
    pub id: String,
    pub order_id: String,
    pub shop_id: String,
    pub status: String,
    pub external_product_id: Option<String>,
    pub external_sku_id: Option<String>,
    pub source_url: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_product_id: Option<String>,
    pub supplier_delivery_name: Option<String>,
    pub supplier_waybill_id: Option<String>,
    pub quantity: i64,
    pub estimated_cost: Option<f64>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub(crate) fn get_agent_purchase_task(
    app: &AppHandle,
    task_id: &str,
) -> AppResult<Option<AgentPurchaseTaskDetail>> {
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT id, order_id, shop_id, status, external_product_id, external_sku_id,
                source_url, supplier_name, supplier_product_id, supplier_delivery_name,
                supplier_waybill_id, quantity, estimated_cost, error_summary, created_at, updated_at
         FROM purchase_tasks WHERE id = ?1",
        params![task_id],
        |row| {
            Ok(AgentPurchaseTaskDetail {
                id: row.get(0)?,
                order_id: row.get(1)?,
                shop_id: row.get(2)?,
                status: row.get(3)?,
                external_product_id: row.get(4)?,
                external_sku_id: row.get(5)?,
                source_url: row.get(6)?,
                supplier_name: row.get(7)?,
                supplier_product_id: row.get(8)?,
                supplier_delivery_name: row.get(9)?,
                supplier_waybill_id: row.get(10)?,
                quantity: row.get(11)?,
                estimated_cost: row.get(12)?,
                error_summary: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
            })
        },
    )
    .optional()
    .map_err(AppError::from)
}

// ── 已铺货商品 ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentShopProduct {
    pub id: String,
    pub shop_id: String,
    pub external_product_id: String,
    pub wechat_product_id: Option<String>,
    pub status: String,
    pub wechat_status: Option<i64>,
    pub current_price_cents: Option<i64>,
    pub created_at: String,
}

pub(crate) fn get_agent_shop_product(
    app: &AppHandle,
    shop_id: &str,
    external_product_id: &str,
) -> AppResult<Option<AgentShopProduct>> {
    let conn = open_connection(app)?;
    conn.query_row(
        "SELECT id, shop_id, external_product_id, wechat_product_id, status,
                wechat_status, current_price_cents, created_at
         FROM shop_products WHERE shop_id = ?1 AND external_product_id = ?2",
        params![shop_id, external_product_id],
        |row| {
            Ok(AgentShopProduct {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                external_product_id: row.get(2)?,
                wechat_product_id: row.get(3)?,
                status: row.get(4)?,
                wechat_status: row.get(5)?,
                current_price_cents: row.get(6)?,
                created_at: row.get(7)?,
            })
        },
    )
    .optional()
    .map_err(AppError::from)
}

// ── 采集任务列表 ───────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentCollectionTask {
    pub id: String,
    pub title: String,
    pub source_url: String,
    pub status: String,
    pub review_status: String,
    pub created_at: String,
}

pub(crate) fn list_agent_collections(
    app: &AppHandle,
    status: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AgentCollectionTask>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT id, title, source_url, status, review_status, created_at
         FROM collection_tasks ORDER BY created_at DESC",
    )?;
    let rows: Vec<AgentCollectionTask> = stmt
        .query_map([], |row| map_collection_summary(row))?
        .filter_map(|r| r.ok())
        .filter(|row| {
            if let Some(st) = status.filter(|s| !s.is_empty()) {
                if row.status != st {
                    return false;
                }
            }
            true
        })
        .take(limit as usize)
        .collect();
    Ok(rows)
}

fn map_collection_summary(row: &rusqlite::Row) -> rusqlite::Result<AgentCollectionTask> {
    Ok(AgentCollectionTask {
        id: row.get(0)?,
        title: row.get(1)?,
        source_url: row.get(2)?,
        status: row.get(3)?,
        review_status: row.get(4)?,
        created_at: row.get(5)?,
    })
}

// ── 商品销售统计 ───────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentProductSales {
    pub external_product_id: String,
    pub title: Option<String>,
    pub active_shop_count: i64,
    pub order_count: i64,
    pub units_sold: i64,
    pub revenue_cents: i64,
    pub inventory_risk_status: Option<String>,
}

pub(crate) fn get_agent_product_sales(
    app: &AppHandle,
    external_product_id: &str,
    _shop_id: Option<&str>,
) -> AppResult<Option<AgentProductSales>> {
    let conn = open_connection(app)?;
    let row = conn
        .query_row(
            "SELECT
               p.external_product_id,
               MAX(p.title),
               COUNT(DISTINCT sp.shop_id) AS active_shops,
               COUNT(DISTINCT oi.order_id) AS order_count,
               COALESCE(SUM(oi.sku_count), 0) AS units_sold,
               COALESCE(SUM(oi.real_price * oi.sku_count), 0) AS revenue_cents
             FROM publish_products p
             LEFT JOIN shop_products sp ON sp.external_product_id = p.external_product_id AND sp.status NOT IN ('failed','cancelled')
             LEFT JOIN order_items oi ON oi.out_product_id = p.external_product_id
             WHERE p.external_product_id = ?1
             GROUP BY p.external_product_id",
            params![external_product_id],
            |row| {
                Ok(AgentProductSales {
                    external_product_id: row.get(0)?,
                    title: row.get(1)?,
                    active_shop_count: row.get(2)?,
                    order_count: row.get(3)?,
                    units_sold: row.get(4)?,
                    revenue_cents: row.get(5)?,
                    inventory_risk_status: None,
                })
            },
        )
        .optional()?;
    Ok(row)
}

// ── 库存风险 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentInventoryRisk {
    pub external_product_id: String,
    pub title: Option<String>,
    pub active_shop_count: i64,
    pub total_stock: i64,
    pub available_stock: i64,
    pub pending_purchase_quantity: i64,
    pub risk_status: String,
}

pub(crate) fn get_agent_inventory_risk(
    app: &AppHandle,
    external_product_id: Option<&str>,
) -> AppResult<Vec<AgentInventoryRisk>> {
    let conn = open_connection(app)?;
    let mut stmt = conn.prepare(
        "SELECT sp.external_product_id, MAX(sp.source_url) AS title_hint,
                COUNT(*) AS total_records,
                SUM(CASE WHEN sp.status IN ('active','submitted','audit_pending','audit_passed') THEN 1 ELSE 0 END) AS active_count,
                SUM(CASE WHEN sp.status = 'out_of_stock' THEN 1 ELSE 0 END) AS oos_count,
                COALESCE(SUM(pt.quantity), 0) AS pending_qty
         FROM shop_products sp
         LEFT JOIN purchase_tasks pt ON pt.external_product_id = sp.external_product_id
           AND pt.status IN ('pending','in_progress')
         GROUP BY sp.external_product_id
         ORDER BY sp.external_product_id ASC
         LIMIT 50",
    )?;
    let rows: Vec<AgentInventoryRisk> = stmt
        .query_map([], |row| map_inventory_risk(row))?
        .filter_map(|r| r.ok())
        .filter(|row| {
            if let Some(pid) = external_product_id {
                if row.external_product_id != pid {
                    return false;
                }
            }
            true
        })
        .collect();
    Ok(rows)
}

fn map_inventory_risk(row: &rusqlite::Row) -> rusqlite::Result<AgentInventoryRisk> {
    let total: i64 = row.get(2)?;
    let active: i64 = row.get(3)?;
    let oos: i64 = row.get(4)?;
    let available = (active - oos).max(0);
    let risk = if oos >= total && total > 0 {
        "out_of_stock"
    } else if oos > 0 {
        "partial_stock"
    } else if active == 0 {
        "not_listed"
    } else {
        "normal"
    };
    Ok(AgentInventoryRisk {
        external_product_id: row.get(0)?,
        title: row.get(1)?,
        active_shop_count: active,
        total_stock: total,
        available_stock: available,
        pending_purchase_quantity: row.get(5)?,
        risk_status: risk.to_string(),
    })
}

// ── 利润汇总 ───────────────────────────────────────────────

#[derive(Serialize)]
pub struct AgentProfitSummary {
    pub order_id: String,
    pub wechat_order_id: String,
    pub shop_id: Option<String>,
    pub revenue_cents: i64,
    pub purchase_cost_cents: i64,
    pub refund_cents: i64,
    pub estimated_profit_cents: i64,
}

pub(crate) fn get_agent_profit_summary(
    app: &AppHandle,
    order_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<AgentProfitSummary>> {
    let conn = open_connection(app)?;
    let mut results = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT opa.order_id, o.wechat_order_id, o.shop_id,
                COALESCE(SUM(CASE WHEN opa.kind = 'revenue' THEN opa.amount_cents ELSE 0 END), 0) AS revenue,
                COALESCE(SUM(CASE WHEN opa.kind = 'purchase_cost' OR opa.kind = 'purchase_freight' THEN opa.amount_cents ELSE 0 END), 0) AS cost,
                COALESCE(SUM(CASE WHEN opa.kind = 'refund' THEN opa.amount_cents ELSE 0 END), 0) AS refund,
                COALESCE(SUM(opa.amount_cents), 0) AS profit
         FROM order_profit_adjustments opa
         JOIN orders o ON o.id = opa.order_id
         GROUP BY opa.order_id
         ORDER BY opa.order_id DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(AgentProfitSummary {
                order_id: row.get(0)?,
                wechat_order_id: row.get(1)?,
                shop_id: row.get(2)?,
                revenue_cents: row.get(3)?,
                purchase_cost_cents: row.get(4)?,
                refund_cents: row.get(5)?,
                estimated_profit_cents: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for row in rows {
        if let Some(oid) = order_id {
            if row.order_id != oid {
                continue;
            }
        }
        results.push(row);
    }
    Ok(results)
}
