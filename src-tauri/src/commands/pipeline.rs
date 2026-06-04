//! 统一商品流水线：把采集→审查→铺货收敛为「一个商品 = 一条流水线」的统一模型。
//!
//! 本模块提供：
//! - 阶段/状态常量（对外 6 态、内部 stage）
//! - 商品级状态聚合 [`recompute_pipeline_product`]（铺货阶段由各店 target 上卷写回主表）
//! - 统一视图查询命令 [`list_pipeline_products`]（前端零适配层）
//!
//! 注：driver 推进引擎在阶段 2 接入。采集/审查阶段的商品状态由各自 handler 直接维护，
//! 本模块的聚合只负责铺货阶段的多店上卷。

use super::*;

/// 对外商品状态（人话 6 态）。
pub mod product_status {
    pub const PENDING_COLLECT: &str = "pending_collect";
    pub const COLLECTING: &str = "collecting";
    /// 已采集审查完成、但还没选店：待选店铺货（解耦采集与铺货）。
    pub const COLLECTED: &str = "collected";
    pub const NEED_CONFIRM: &str = "need_confirm";
    pub const PUBLISHING: &str = "publishing";
    pub const LISTED: &str = "listed";
    pub const ERROR: &str = "error";
}

/// 商品级内部阶段。
pub mod product_stage {
    pub const COLLECT: &str = "collect";
    pub const REVIEW: &str = "review";
    pub const PUBLISH: &str = "publish";
    pub const DONE: &str = "done";
}

/// 店级（target）推进阶段。
pub mod target_stage {
    pub const AWAIT_REVIEW: &str = "await_review";
    pub const PRECHECK: &str = "precheck";
    pub const ATTR_FILL: &str = "attr_fill";
    pub const CATEGORY_PRECHECK: &str = "category_precheck";
    pub const ASSET_UPLOAD: &str = "asset_upload";
    pub const SUBMIT: &str = "submit";
    pub const AUDIT: &str = "audit";
    pub const LISTING: &str = "listing";
    pub const DONE: &str = "done";
}

/// 店级（target）推进状态（方案Y 四态）。
/// stage 表达「在哪个阶段」，status 表达「该阶段的处理状态」，两维正交。
pub mod target_status {
    /// 新到达本 stage，待 runner 处理。
    pub const PENDING: &str = "pending";
    /// runner 处理中（被中断后 loader 仍会按 pending/running 幂等重做）。
    pub const RUNNING: &str = "running";
    /// 本 stage 失败，带 error_code；retriable 由 driver 退避后置回 pending，
    /// need_confirm/fatal 留待人工。
    pub const BLOCKED: &str = "blocked";
    /// 全链路完成（listing 后）。
    pub const DONE: &str = "done";
}

/// 店级 target 完成判定：stage 推进到 done 即视为已上架。
fn target_is_listed(stage: &str) -> bool {
    stage == target_stage::DONE
}
/// 店级 target 失败判定：status=blocked 即处于失败/待重试态。
fn target_is_failed(status: &str) -> bool {
    status == target_status::BLOCKED
}

/// 关注级别排序：None < NeedConfirm < Error，用于多店聚合取"最严重"。
fn attention_rank(attention: Attention) -> u8 {
    match attention {
        Attention::None => 0,
        Attention::NeedConfirm => 1,
        Attention::Error => 2,
    }
}

/// 待确认子类型：决定前端弹哪种确认 UI。
fn confirm_kind_for(error_code: &str) -> &'static str {
    match error_code {
        "MISSING_WECHAT_LEAF_CATEGORY_ID" | "CATEGORY_NEEDS_AI_FILL" => "CATEGORY",
        "CATEGORY_ATTRS_NEED_AI_FILL" | "WECHAT_PAYLOAD_NEEDS_AI_FILL" => "ATTR",
        "INSUFFICIENT_HEAD_IMAGES" | "INSUFFICIENT_DETAIL_IMAGES" | "INVALID_IMAGE_SOURCE_URL"
        | "IMAGE_PREPROCESS_FAILED" | "PRODUCT_ASSETS_EMPTY" => "IMAGE",
        "MISSING_AFTER_SALE_ADDRESS" | "AMBIGUOUS_AFTER_SALE_ADDRESS" | "MISSING_FREIGHT_TEMPLATE" => {
            "SHOP_SETTING"
        }
        "REVIEW_NEEDS_CONFIRM" | "REVIEW_BLOCKED" => "REVIEW",
        _ => "GENERIC",
    }
}

/// 店级人话状态文案（方案Y：blocked 看错误码，done 即已上架，其余按 stage 出"正在做什么"）。
fn target_status_text(stage: &str, status: &str, error_code: Option<&str>) -> String {
    if status == target_status::BLOCKED {
        return error_code
            .map(|code| classify_error_code(code).human_reason.to_string())
            .unwrap_or_else(|| "处理失败".to_string());
    }
    if status == target_status::DONE || stage == target_stage::DONE {
        return "已上架".to_string();
    }
    match stage {
        target_stage::AWAIT_REVIEW => "等待采集审查",
        target_stage::PRECHECK => "发品前校验中",
        target_stage::ATTR_FILL => "补齐必填属性中",
        target_stage::CATEGORY_PRECHECK => "类目预检中",
        target_stage::ASSET_UPLOAD => "上传商品图片中",
        target_stage::SUBMIT => "提交微信发品中",
        target_stage::AUDIT => "等待微信审核",
        target_stage::LISTING => "上架中",
        _ => "处理中",
    }
    .to_string()
}

/// 单个 target 的原始行。
struct TargetRow {
    id: String,
    shop_id: String,
    shop_name: String,
    stage: String,
    status: String,
    error_code: Option<String>,
    wechat_product_id: Option<String>,
    audit_summary: Option<String>,
    updated_at: String,
}

fn load_targets(conn: &Connection, product_id: &str) -> AppResult<Vec<TargetRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, shop_name, stage, status, error_code,
                wechat_product_id, audit_summary, updated_at
         FROM pipeline_shop_targets WHERE product_id = ?1
         ORDER BY shop_name",
    )?;
    let rows = stmt
        .query_map([product_id], |row| {
            Ok(TargetRow {
                id: row.get(0)?,
                shop_id: row.get(1)?,
                shop_name: row.get(2)?,
                stage: row.get(3)?,
                status: row.get(4)?,
                error_code: row.get(5)?,
                wechat_product_id: row.get(6)?,
                audit_summary: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// 铺货阶段的多店聚合结果。
struct PublishAggregate {
    total: i64,
    listed: i64,
    failed: i64,
    pending: i64,
    worst_attention: Attention,
    worst_error_code: Option<String>,
    worst_reason: Option<String>,
}

fn aggregate_targets(targets: &[TargetRow]) -> PublishAggregate {
    let mut agg = PublishAggregate {
        total: 0,
        listed: 0,
        failed: 0,
        pending: 0,
        worst_attention: Attention::None,
        worst_error_code: None,
        worst_reason: None,
    };
    for target in targets {
        agg.total += 1;
        if target_is_listed(&target.stage) {
            agg.listed += 1;
        } else if target_is_failed(&target.status) {
            agg.failed += 1;
            if let Some(code) = target.error_code.as_deref() {
                let c = classify_error_code(code);
                if attention_rank(c.attention) > attention_rank(agg.worst_attention) {
                    agg.worst_attention = c.attention;
                    agg.worst_error_code = Some(code.to_string());
                    agg.worst_reason = Some(c.human_reason.to_string());
                }
            }
        } else {
            agg.pending += 1;
        }
    }
    agg
}

/// 铺货阶段多店聚合 → 商品级 (status, attention, error_code, error_reason, progress_text)。
fn derive_publish_state(
    agg: &PublishAggregate,
) -> (
    &'static str,
    &'static str,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    if agg.total == 0 {
        return (
            product_status::ERROR,
            "error",
            Some("NO_TARGET_SHOP".to_string()),
            Some("没有目标店铺".to_string()),
            None,
        );
    }
    let progress = Some(format!("{}/{} 店已上架", agg.listed, agg.total));
    match agg.worst_attention {
        Attention::Error => (
            product_status::ERROR,
            "error",
            agg.worst_error_code.clone(),
            agg.worst_reason.clone(),
            progress,
        ),
        Attention::NeedConfirm => (
            product_status::NEED_CONFIRM,
            "need_confirm",
            agg.worst_error_code.clone(),
            agg.worst_reason.clone(),
            progress,
        ),
        Attention::None => {
            if agg.listed == agg.total {
                (
                    product_status::LISTED,
                    "none",
                    None,
                    None,
                    Some(format!("已上架 {}/{} 店", agg.listed, agg.total)),
                )
            } else {
                (product_status::PUBLISHING, "none", None, None, progress)
            }
        }
    }
}

/// 重算商品级状态：仅铺货阶段聚合多店 target 写回主表。
/// 采集/审查阶段的状态由各自 handler 直接维护，这里直接返回。
pub(in crate::commands) fn recompute_pipeline_product(
    conn: &Connection,
    product_id: &str,
) -> AppResult<()> {
    let stage: String = conn.query_row(
        "SELECT stage FROM pipeline_products WHERE id = ?1",
        [product_id],
        |row| row.get(0),
    )?;
    if stage != product_stage::PUBLISH && stage != product_stage::DONE {
        return Ok(());
    }
    let targets = load_targets(conn, product_id)?;
    if targets.is_empty() {
        // 审查已通过但还没选店：稳定在「待铺货」，等用户补店后再进铺货。
        // （能走到 stage=publish 且无 target，只可能是「只采集」模式审查通过的商品）
        conn.execute(
            "UPDATE pipeline_products
             SET status = ?1, attention = 'none', error_code = NULL, error_reason = NULL,
                 progress_text = '待选店铺货', stage = ?2, updated_at = ?3
             WHERE id = ?4",
            params![
                product_status::COLLECTED,
                product_stage::PUBLISH,
                now_shanghai(),
                product_id
            ],
        )?;
        return Ok(());
    }
    // 能走到这说明商品已进入铺货阶段（stage=publish/done，即采集审查已通过）。
    // 把任何仍停在 await_review 的 target 统一提升为 precheck：这是 await_review→铺货的
    // 唯一可靠激活点，消除「采集中提前补店 + 审查并发完成（按旧快照走了无 target 分支）」
    // 导致该 target 永远停在 await_review、无任何 loader 捞取的竞态。
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET stage = 'precheck', status = 'pending', error_code = NULL,
             error_summary = NULL, updated_at = ?1
         WHERE product_id = ?2 AND stage = 'await_review'",
        params![now_shanghai(), product_id],
    )?;
    let agg = aggregate_targets(&targets);
    let (status, attention, error_code, error_reason, progress_text) = derive_publish_state(&agg);
    // 全部目标店上架后，把商品级 stage 推进到 done
    let next_stage = if status == product_status::LISTED {
        product_stage::DONE
    } else {
        product_stage::PUBLISH
    };
    conn.execute(
        "UPDATE pipeline_products
         SET status = ?1, attention = ?2, error_code = ?3, error_reason = ?4,
             progress_text = ?5, stage = ?6, updated_at = ?7
         WHERE id = ?8",
        params![
            status,
            attention,
            error_code,
            error_reason,
            progress_text,
            next_stage,
            now_shanghai(),
            product_id
        ],
    )?;
    Ok(())
}

// ============================================================================
// 店级 target 推进原语：铺货 runner 的统一状态写入出口。
//
// 取代旧模型散落各处的「写细粒度 status 串到 publish_job_items」。新模型下
// target 用两维表达进度：stage（在哪个阶段）+ status（pending/running/blocked/done）。
// 商品级对外 6 态与 attention 由 recompute_pipeline_product 从各店 target 经
// classify_error_code 聚合算出，原语本身不碰商品级，保持职责单一。
// ============================================================================

/// runner 开始处理某 target：置 status=running（stage 不变），标记「正在做」。
pub(in crate::commands) fn mark_target_running(conn: &Connection, target_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![target_status::RUNNING, now_shanghai(), target_id],
    )?;
    Ok(())
}

/// 推进 target 到下一阶段：stage=next_stage, status=pending，清空上一阶段的错误。
pub(in crate::commands) fn advance_target(
    conn: &Connection,
    target_id: &str,
    next_stage: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET stage = ?1, status = ?2, error_code = NULL, error_summary = NULL, updated_at = ?3
         WHERE id = ?4",
        params![next_stage, target_status::PENDING, now_shanghai(), target_id],
    )?;
    Ok(())
}

/// target 全链路完成（已上架）：stage=done, status=done。
pub(in crate::commands) fn finish_target(conn: &Connection, target_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET stage = ?1, status = ?2, error_code = NULL, error_summary = NULL, updated_at = ?3
         WHERE id = ?4",
        params![target_stage::DONE, target_status::DONE, now_shanghai(), target_id],
    )?;
    Ok(())
}

/// target 阻塞（失败，等 driver 退避重试或人工处理）：status=blocked + 错误码，retry_count+1，
/// 并按当前重试次数写入下次可重试时间 next_retry_at（指数退避）。
/// 商品级 attention（need_confirm / error）由 recompute_pipeline_product 从 error_code
/// 经 classify_error_code 聚合，原语只记店级事实；是否真的会被重试由退避扫描按 retriable 决定。
pub(in crate::commands) fn block_target(
    conn: &Connection,
    target_id: &str,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let retry_count: i64 = conn
        .query_row(
            "SELECT retry_count FROM pipeline_shop_targets WHERE id = ?1",
            [target_id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);
    let next_retry_at = format_shanghai(Utc::now() + Duration::seconds(retry_backoff_secs(retry_count)));
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET status = ?1, error_code = ?2, error_summary = ?3,
             retry_count = retry_count + 1, next_retry_at = ?4, updated_at = ?5
         WHERE id = ?6",
        params![
            target_status::BLOCKED,
            error_code,
            error_summary,
            next_retry_at,
            now_shanghai(),
            target_id
        ],
    )?;
    Ok(())
}

/// 指数退避秒数：1/2/4/8/16 分钟逐级翻倍，封顶 30 分钟。
fn retry_backoff_secs(retry_count: i64) -> i64 {
    let minutes = 1i64
        .checked_shl(retry_count.clamp(0, 5) as u32)
        .unwrap_or(32)
        .min(30);
    minutes * 60
}

/// 把 target 置回 pending 同 stage（清空错误与退避时间），用于轮询未完成或退避重试。
pub(in crate::commands) fn requeue_target(conn: &Connection, target_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET status = ?1, error_code = NULL, error_summary = NULL,
             next_retry_at = NULL, updated_at = ?2
         WHERE id = ?3",
        params![target_status::PENDING, now_shanghai(), target_id],
    )?;
    Ok(())
}

/// driver 退避扫描：把 retriable 且已到期的 blocked target 置回 pending 同 stage 重试。
/// need_confirm / fatal 类错误（retriable=false）保持 blocked，等待人工处理。
/// 返回本轮重新激活的 target 数量。
pub(in crate::commands) fn reactivate_retriable_blocked_targets(
    conn: &Connection,
) -> AppResult<usize> {
    struct BlockedRow {
        id: String,
        product_id: String,
        error_code: Option<String>,
    }
    let now = now_shanghai();
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT id, product_id, error_code
             FROM pipeline_shop_targets
             WHERE status = ?1
               AND (next_retry_at IS NULL OR next_retry_at <= ?2)",
        )?;
        let mapped = stmt
            .query_map(params![target_status::BLOCKED, now], |row| {
                Ok(BlockedRow {
                    id: row.get(0)?,
                    product_id: row.get(1)?,
                    error_code: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        mapped
    };

    let mut reactivated = 0usize;
    let mut affected_products = BTreeSet::new();
    for row in rows {
        let retriable = row
            .error_code
            .as_deref()
            .map(|code| classify_error_code(code).retriable)
            .unwrap_or(false);
        if !retriable {
            continue;
        }
        requeue_target(conn, &row.id)?;
        affected_products.insert(row.product_id);
        reactivated += 1;
    }
    for product_id in &affected_products {
        recompute_pipeline_product(conn, product_id)?;
    }
    Ok(reactivated)
}

/// 人工处理后重试：把指定商品所有 blocked 的 target 置回 pending 同 stage（清错误与退避时间），
/// 让 driver 下一轮重新推进。用于 need_confirm/error 商品在用户补救（换图/选类目/改属性/配置店铺）后手动重试。
/// 与 driver 自动退避(reactivate)互补：自动退避只重激活 retriable 错误，本命令覆盖需人工的 blocked。
#[tauri::command]
pub fn retry_pipeline_product(app: AppHandle, product_id: String) -> AppResult<i64> {
    let conn = open_connection(&app)?;
    // 按商品当前阶段分流重试：采集/审查阶段失败是商品级（主表直接表达状态），
    // 铺货阶段失败是店级（blocked target）。此前只处理 blocked target，
    // 导致采集失败的商品点「重试」匹配 0 行、毫无效果。
    let stage: String = conn.query_row(
        "SELECT stage FROM pipeline_products WHERE id = ?1",
        params![product_id],
        |row| row.get(0),
    )?;

    let affected = match stage.as_str() {
        // 采集阶段失败：退回待采集，清掉采集/审查残留，由 driver 自动重新采集
        "collect" => conn.execute(
            "UPDATE pipeline_products
             SET status = 'pending_collect', stage = 'collect', attention = 'none',
                 error_code = NULL, error_reason = NULL, progress_text = NULL,
                 reviewed_data = NULL, review_result_json = NULL, updated_at = ?1
             WHERE id = ?2",
            params![now_shanghai(), product_id],
        )?,
        // 审查阶段失败：退回待审查（status=collecting，审查 loader 据此重新捞），无需重采
        "review" => conn.execute(
            "UPDATE pipeline_products
             SET status = 'collecting', attention = 'none',
                 error_code = NULL, error_reason = NULL, progress_text = NULL,
                 review_result_json = NULL, updated_at = ?1
             WHERE id = ?2",
            params![now_shanghai(), product_id],
        )?,
        // 铺货阶段失败：把 blocked 的店级 target 推回 pending，由 driver 续跑
        _ => {
            let n = conn.execute(
                "UPDATE pipeline_shop_targets
                 SET status = ?1, error_code = NULL, error_summary = NULL,
                     next_retry_at = NULL, updated_at = ?2
                 WHERE product_id = ?3 AND status = ?4",
                params![
                    target_status::PENDING,
                    now_shanghai(),
                    product_id,
                    target_status::BLOCKED
                ],
            )?;
            recompute_pipeline_product(&conn, &product_id)?;
            n
        }
    };
    Ok(affected as i64)
}

/// 统一流水线视图查询：前端工作台数据源，前端零适配层。
#[tauri::command]
pub fn list_pipeline_products(app: AppHandle) -> AppResult<Vec<PipelineProductView>> {
    let conn = open_connection(&app)?;

    struct ProductRow {
        id: String,
        external_product_id: Option<String>,
        title: String,
        source_url: String,
        category_path: String,
        stage: String,
        status: String,
        attention: String,
        progress_text: Option<String>,
        error_code: Option<String>,
        error_reason: Option<String>,
        updated_at: String,
    }

    let mut stmt = conn.prepare(
        "SELECT id, external_product_id, title, source_url, category_path,
                stage, status, attention, progress_text, error_code, error_reason, updated_at
         FROM pipeline_products
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 500",
    )?;
    let products = stmt
        .query_map([], |row| {
            Ok(ProductRow {
                id: row.get(0)?,
                external_product_id: row.get(1)?,
                title: row.get(2)?,
                source_url: row.get(3)?,
                category_path: row.get(4)?,
                stage: row.get(5)?,
                status: row.get(6)?,
                attention: row.get(7)?,
                progress_text: row.get(8)?,
                error_code: row.get(9)?,
                error_reason: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut views = Vec::with_capacity(products.len());
    for product in products {
        let targets = load_targets(&conn, &product.id)?;
        let agg = aggregate_targets(&targets);

        // 铺货阶段(publish/done)实时聚合商品级状态；采集/审查阶段用主表已存状态
        let (status, attention, error_code, error_reason, progress_text) =
            if product.stage == product_stage::PUBLISH || product.stage == product_stage::DONE {
                if targets.is_empty() {
                    // 「只采集」审查通过但还没选店：稳定显示「待铺货」，与
                    // recompute_pipeline_product 的空 target 分支一致，
                    // 不能让 derive_publish_state 的 total==0 把它误算成「异常」。
                    (
                        product_status::COLLECTED.to_string(),
                        "none".to_string(),
                        None,
                        None,
                        Some("待选店铺货".to_string()),
                    )
                } else {
                    let (s, a, ec, er, pt) = derive_publish_state(&agg);
                    (s.to_string(), a.to_string(), ec, er, pt)
                }
            } else {
                (
                    product.status.clone(),
                    product.attention.clone(),
                    product.error_code.clone(),
                    product.error_reason.clone(),
                    product.progress_text.clone(),
                )
            };

        let mut shops = Vec::with_capacity(targets.len());
        for target in &targets {
            let target_error_reason = target
                .error_code
                .as_deref()
                .map(|code| classify_error_code(code).human_reason.to_string());
            shops.push(PipelineShopTargetView {
                id: target.id.clone(),
                shop_id: target.shop_id.clone(),
                shop_name: target.shop_name.clone(),
                status_text: target_status_text(
                    target.stage.as_str(),
                    target.status.as_str(),
                    target.error_code.as_deref(),
                ),
                error_code: target.error_code.clone(),
                error_reason: target_error_reason,
                can_retry: target_is_failed(&target.status),
                wechat_product_id: target.wechat_product_id.clone(),
                audit_summary: target.audit_summary.clone(),
                updated_at: target.updated_at.clone(),
            });
        }

        let can_confirm = attention == "need_confirm";
        let can_retry = attention == "error";
        let confirm_kind = if can_confirm {
            error_code
                .as_deref()
                .map(|code| confirm_kind_for(code).to_string())
        } else {
            None
        };
        views.push(PipelineProductView {
            id: product.id,
            external_product_id: product.external_product_id,
            title: product.title,
            source_url: product.source_url,
            category_path: product.category_path,
            status,
            attention,
            progress_text,
            error_code,
            error_reason,
            total_shops: targets.len() as i64,
            listed_shops: agg.listed,
            failed_shops: agg.failed,
            pending_shops: agg.pending,
            can_retry,
            can_confirm,
            confirm_kind,
            updated_at: product.updated_at,
            shops,
        });
    }
    Ok(views)
}
