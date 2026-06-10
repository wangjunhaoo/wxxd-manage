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
///
/// 状态机完整词表（按生命周期）：`pending_collect`（待采集）→ `collecting`（采集中）→
/// `collected` → `publishing` → `listed`，分支态 `need_confirm` / `error`。
/// 其中 `pending_collect` 与 `collecting` 两个采集入口态的字面量由 collection.rs
/// 以 SQL 字面量维护（本文件的重试/重采 SQL 同样直接写字面量），不设符号常量。
pub mod product_status {
    /// 已采集审查完成、但还没选店：待选店铺货（解耦采集与铺货）。
    pub const COLLECTED: &str = "collected";
    pub const NEED_CONFIRM: &str = "need_confirm";
    pub const PUBLISHING: &str = "publishing";
    pub const LISTED: &str = "listed";
    pub const ERROR: &str = "error";
}

/// 商品级内部阶段。
///
/// 阶段完整词表（推进顺序）：`collect` → `review` → `publish` → `done`。
/// 其中 `collect` / `review` 两个阶段的字面量由 collection.rs 以 SQL 字面量推进
/// （本文件的重试/重采分流 match 同样直接写字面量），不设符号常量。
pub mod product_stage {
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
        "INSUFFICIENT_HEAD_IMAGES"
        | "INSUFFICIENT_DETAIL_IMAGES"
        | "INVALID_IMAGE_SOURCE_URL"
        | "IMAGE_PREPROCESS_FAILED"
        | "PRODUCT_ASSETS_EMPTY" => "IMAGE",
        "MISSING_AFTER_SALE_ADDRESS"
        | "AMBIGUOUS_AFTER_SALE_ADDRESS"
        | "MISSING_FREIGHT_TEMPLATE" => "SHOP_SETTING",
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
    error_summary: Option<String>,
    wechat_product_id: Option<String>,
    audit_summary: Option<String>,
    updated_at: String,
}

fn load_targets(conn: &Connection, product_id: &str) -> AppResult<Vec<TargetRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, shop_id, shop_name, stage, status, error_code, error_summary,
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
                error_summary: row.get(6)?,
                wechat_product_id: row.get(7)?,
                audit_summary: row.get(8)?,
                updated_at: row.get(9)?,
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
pub(in crate::commands) fn mark_target_running(
    conn: &Connection,
    target_id: &str,
) -> AppResult<()> {
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
        params![
            next_stage,
            target_status::PENDING,
            now_shanghai(),
            target_id
        ],
    )?;
    Ok(())
}

/// target 全链路完成（已上架）：stage=done, status=done。
pub(in crate::commands) fn finish_target(conn: &Connection, target_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET stage = ?1, status = ?2, error_code = NULL, error_summary = NULL, updated_at = ?3
         WHERE id = ?4",
        params![
            target_stage::DONE,
            target_status::DONE,
            now_shanghai(),
            target_id
        ],
    )?;
    Ok(())
}

/// target 阻塞（失败，等 driver 退避重试或人工处理）：status=blocked + 错误码，
/// 并按重试次数写入下次可重试时间 next_retry_at（指数退避）。
///
/// retry_count 语义为「同一错误码的连续失败次数」：换了错误码视为新错误链、从 1 重计
///（退避也从 1 分钟重新起步）。这保证 MAX_SKU_SWALLOW_RETRY、auto_retry_limit 等
/// 上限判定不被早前无关阶段的失败侵蚀。
/// 商品级 attention（need_confirm / error）由 recompute_pipeline_product 从 error_code
/// 经 classify_error_code 聚合，原语只记店级事实；是否真的会被重试由退避扫描决定。
pub(in crate::commands) fn block_target(
    conn: &Connection,
    target_id: &str,
    error_code: &str,
    error_summary: &str,
) -> AppResult<()> {
    let previous: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT retry_count, error_code FROM pipeline_shop_targets WHERE id = ?1",
            [target_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let same_code_retries = match &previous {
        Some((count, Some(prev_code))) if prev_code == error_code => *count,
        _ => 0,
    };
    let next_retry_at =
        format_shanghai(Utc::now() + Duration::seconds(retry_backoff_secs(same_code_retries)));
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET status = ?1, error_code = ?2, error_summary = ?3,
             retry_count = ?4, next_retry_at = ?5, updated_at = ?6
         WHERE id = ?7",
        params![
            target_status::BLOCKED,
            error_code,
            error_summary,
            same_code_retries + 1,
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

/// 把 target 置回 pending 同 stage（清退避时间），用于退避自动重试。
///
/// 刻意保留 error_code/error_summary/retry_count：它们构成「同一错误码连续失败」
/// 计数链（block_target 据此累加），是吞 SKU 上限 MAX_SKU_SWALLOW_RETRY 与
/// auto_retry_limit 超限判定的依据——若在此清掉，重试一次计数就归零，上限永远
/// 不会触发形成无限循环。错误信息在阶段成功推进（advance/finish）时才清。
pub(in crate::commands) fn requeue_target(conn: &Connection, target_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET status = ?1, next_retry_at = NULL, updated_at = ?2
         WHERE id = ?3",
        params![target_status::PENDING, now_shanghai(), target_id],
    )?;
    Ok(())
}

/// 把 target 置回 pending 并回退到指定阶段（清退避时间），用于自动重试时
/// 把「纯检查、无修复能力」阶段 blocked 的属性类错误送回有 AI 兜底的 category_precheck。
/// 与 requeue_target 同理保留 error_code/retry_count 计数链，超限判定才能生效。
fn requeue_target_at_stage(conn: &Connection, target_id: &str, stage: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET stage = ?1, status = ?2, next_retry_at = NULL, updated_at = ?3
         WHERE id = ?4",
        params![stage, target_status::PENDING, now_shanghai(), target_id],
    )?;
    Ok(())
}

/// driver 退避扫描：把已到期、可自动重试的 blocked target 置回 pending 重试。
///
/// 两类可自动重试：
/// 1. retriable=true（瞬时故障/配置自愈类）：无限次，按指数退避节奏；
/// 2. auto_retry_limit=Some(n)（纯技术性缺属性类）：retry_count ≤ n 时有限次重试，
///    且若 blocked 在 asset_upload/submit（这两阶段的属性检查是纯检查、无 AI 兜底，
///    原地重试必然原地再 block 空转），回退到 category_precheck——那里有在线类目详情
///    与 AI 兜底补齐；已传图片经 pipeline_assets 唯一键缓存全部复用，回退幂等无重复上传。
/// 其余（need_confirm 纯人工 / fatal）保持 blocked 等人工。返回本轮重新激活的数量。
pub(in crate::commands) fn reactivate_retriable_blocked_targets(
    conn: &Connection,
) -> AppResult<usize> {
    struct BlockedRow {
        id: String,
        product_id: String,
        stage: String,
        error_code: Option<String>,
        retry_count: i64,
    }
    let now = now_shanghai();
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT id, product_id, stage, error_code, retry_count
             FROM pipeline_shop_targets
             WHERE status = ?1
               AND (next_retry_at IS NULL OR next_retry_at <= ?2)",
        )?;
        let mapped = stmt
            .query_map(params![target_status::BLOCKED, now], |row| {
                Ok(BlockedRow {
                    id: row.get(0)?,
                    product_id: row.get(1)?,
                    stage: row.get(2)?,
                    error_code: row.get(3)?,
                    retry_count: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        mapped
    };

    let mut reactivated = 0usize;
    let mut affected_products = BTreeSet::new();
    for row in rows {
        let Some(code) = row.error_code.as_deref() else {
            continue;
        };
        let classification = classify_error_code(code);
        let limited_retry = classification
            .auto_retry_limit
            .is_some_and(|limit| row.retry_count <= limit);
        if !classification.retriable && !limited_retry {
            continue;
        }
        // 属性类错误在纯检查阶段 blocked：回退 category_precheck 走 AI 兜底，避免原地空转
        let needs_stage_rollback = limited_retry
            && matches!(
                code,
                "CATEGORY_ATTRS_NEED_AI_FILL" | "WECHAT_PAYLOAD_NEEDS_AI_FILL"
            )
            && matches!(
                row.stage.as_str(),
                target_stage::ASSET_UPLOAD | target_stage::SUBMIT
            );
        if needs_stage_rollback {
            requeue_target_at_stage(conn, &row.id, target_stage::CATEGORY_PRECHECK)?;
        } else {
            requeue_target(conn, &row.id)?;
        }
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
        // 铺货阶段失败：按错误类型分流重试
        _ => {
            // ① 审核驳回/上架失败类（微信侧已存在被驳草稿）：原地 requeue 回 audit 只会再次
            //    轮询到同样的驳回结果死循环；重采又因 wechat_product_id 非空被跳过永久卡死。
            //    正确恢复路径：回退 submit 重新提交——保留 wechat_product_id，submit 入口的
            //    「删残留草稿」会先删微信侧旧草稿再全新 addproduct（防孤儿复用现有机制）。
            //    人工重试视为新一轮，清错误码重置「同码连续失败」计数链。
            let rolled_back = conn.execute(
                "UPDATE pipeline_shop_targets
                 SET stage = ?1, status = ?2, error_code = NULL, error_summary = NULL,
                     next_retry_at = NULL, updated_at = ?3
                 WHERE product_id = ?4 AND status = ?5
                   AND stage IN (?6, ?7)
                   AND (error_code LIKE 'WECHAT_PRODUCT_STATUS_%'
                        OR error_code LIKE 'WECHAT_LISTINGPRODUCT_%')",
                params![
                    target_stage::SUBMIT,
                    target_status::PENDING,
                    now_shanghai(),
                    product_id,
                    target_status::BLOCKED,
                    target_stage::AUDIT,
                    target_stage::LISTING
                ],
            )?;
            // ② 其余 blocked：原地推回 pending 同 stage，由 driver 续跑
            let requeued = conn.execute(
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
            rolled_back + requeued
        }
    };
    Ok(affected as i64)
}

/// 重新采集：把商品退回采集流程重抓数据、重走审查铺货。保留已选目标店，已上架(stage=done)的
/// 店保持不动、不重复铺货，仅未上架的 target 重置为 await_review 等重新审查通过后激活。清空
/// 采集结果与审查中间态强制重抓。用于采集数据不满意、或采集/审查/铺货失败想从头重来。
#[tauri::command]
pub fn recollect_pipeline_product(app: AppHandle, product_id: String) -> AppResult<()> {
    let conn = open_connection(&app)?;
    let now = now_shanghai();
    // 未提交微信的 target 重置为 await_review（等重新审查通过激活）；已提交微信的
    // (wechat_product_id 非空：submit 成功/audit/listing/done) 保持不动——它们在微信侧已建商品，
    // 打回会丢失同步链路且重采后必然撞 DUPLICATE，只能继续走原同步流程。同时清错误码与退避时间。
    conn.execute(
        "UPDATE pipeline_shop_targets
         SET stage = ?1, status = ?2, error_code = NULL, error_summary = NULL,
             next_retry_at = NULL, updated_at = ?3
         WHERE product_id = ?4 AND wechat_product_id IS NULL",
        params![
            target_stage::AWAIT_REVIEW,
            target_status::PENDING,
            now,
            product_id
        ],
    )?;
    // 商品退回待采集，清空采集结果与审查中间态强制重抓重审，由 driver 自动重新采集。
    let affected = conn.execute(
        "UPDATE pipeline_products
         SET status = 'pending_collect', stage = 'collect', attention = 'none',
             error_code = NULL, error_reason = NULL, progress_text = NULL,
             collected_data = NULL, reviewed_data = NULL, review_result_json = NULL,
             updated_at = ?1
         WHERE id = ?2",
        params![now, product_id],
    )?;
    if affected == 0 {
        return Err(AppError::Validation(format!(
            "流水线商品 {product_id} 不存在"
        )));
    }
    Ok(())
}

/// 拉取单个流水线商品的采集明细（点商品标题打开详情抽屉用）。
/// 解析 pipeline_products.collected_data（采集原始 ExternalProductInput，无则回退
/// reviewed_data），category_path 取主表。商品不存在或无采集数据时返回校验错误。
#[tauri::command]
pub fn get_pipeline_product_detail(
    app: AppHandle,
    product_id: String,
) -> AppResult<PipelineProductDetailView> {
    let conn = open_connection(&app)?;
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT category_path, COALESCE(collected_data, reviewed_data)
             FROM pipeline_products WHERE id = ?1",
            params![product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (category_path, collected) =
        row.ok_or_else(|| AppError::Validation(format!("流水线商品 {product_id} 不存在")))?;
    let Some(collected) = collected else {
        return Err(AppError::Validation(
            "该商品暂无采集数据（可能尚未采集或已被重置）".to_string(),
        ));
    };
    let input: ExternalProductInput = serde_json::from_str(&collected)
        .map_err(|error| AppError::Validation(format!("采集数据解析失败：{error}")))?;

    // 淘宝商品参数存在采集 metadata.taobao_item_params（产地/面料/安全等级等），单独提取展示
    let item_params = input
        .metadata
        .get("taobao_item_params")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let skus = input
        .skus
        .into_iter()
        .map(|sku| PipelineProductDetailSku {
            external_sku_id: sku.external_sku_id,
            specs: sku.specs,
            cost_price: sku.cost_price,
            stock: sku.stock,
        })
        .collect();

    Ok(PipelineProductDetailView {
        id: product_id,
        external_product_id: Some(input.external_product_id).filter(|id| !id.is_empty()),
        title: input.title,
        source_url: input.source_url,
        category_path,
        images: input.images,
        detail_images: input.detail_images,
        supplier_name: input.supplier_name,
        brand_hint: input.brand_hint,
        category_hint: input.category_hint,
        weight_gram: input.weight_gram,
        item_params,
        skus,
    })
}

/// 状态统计：一条聚合 SQL 算出各状态计数，不受列表 LIMIT 截断影响。
/// batch_id 非空时只统计该批次内商品（前端 Pill 数字 = 当前批次内各状态数）。
fn load_pipeline_stats(conn: &Connection, batch_id: Option<&str>) -> AppResult<PipelineStats> {
    conn.query_row(
        "SELECT
           SUM(CASE WHEN archived_at IS NULL AND status IN ('pending_collect','collecting') THEN 1 ELSE 0 END),
           SUM(CASE WHEN archived_at IS NULL AND status = 'collected' THEN 1 ELSE 0 END),
           SUM(CASE WHEN archived_at IS NULL AND status = 'need_confirm' THEN 1 ELSE 0 END),
           SUM(CASE WHEN archived_at IS NULL AND status = 'publishing' THEN 1 ELSE 0 END),
           SUM(CASE WHEN archived_at IS NULL AND status = 'listed' THEN 1 ELSE 0 END),
           SUM(CASE WHEN archived_at IS NULL AND status = 'error' THEN 1 ELSE 0 END),
           SUM(CASE WHEN archived_at IS NOT NULL THEN 1 ELSE 0 END)
         FROM pipeline_products
         WHERE (?1 IS NULL OR import_batch_id = ?1)",
        params![batch_id],
        |row| {
            Ok(PipelineStats {
                collecting: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                collected: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                need_confirm: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                publishing: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                listed: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                error: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                archived: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            })
        },
    )
    .map_err(Into::into)
}

/// 全部导入批次（含商品计数），新批次在前，「历史数据」等旧批次靠后。
fn load_import_batches(conn: &Connection) -> AppResult<Vec<ImportBatchView>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.name, b.source, b.created_at, COUNT(p.id)
         FROM import_batches b
         LEFT JOIN pipeline_products p ON p.import_batch_id = b.id
         GROUP BY b.id, b.name, b.source, b.created_at
         ORDER BY b.created_at DESC, b.id DESC",
    )?;
    let batches = stmt
        .query_map([], |row| {
            Ok(ImportBatchView {
                id: row.get(0)?,
                name: row.get(1)?,
                source: row.get(2)?,
                created_at: row.get(3)?,
                product_count: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(batches)
}

/// 一次性批量加载多个商品的 targets（消除每商品一查的 N+1），按 product_id 分组返回。
fn load_targets_grouped(
    conn: &Connection,
    product_ids: &[String],
) -> AppResult<std::collections::HashMap<String, Vec<TargetRow>>> {
    let mut grouped: std::collections::HashMap<String, Vec<TargetRow>> =
        std::collections::HashMap::with_capacity(product_ids.len());
    if product_ids.is_empty() {
        return Ok(grouped);
    }
    let placeholders = vec!["?"; product_ids.len()].join(",");
    let sql = format!(
        "SELECT product_id, id, shop_id, shop_name, stage, status, error_code, error_summary,
                wechat_product_id, audit_summary, updated_at
         FROM pipeline_shop_targets WHERE product_id IN ({placeholders})
         ORDER BY shop_name"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(product_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                TargetRow {
                    id: row.get(1)?,
                    shop_id: row.get(2)?,
                    shop_name: row.get(3)?,
                    stage: row.get(4)?,
                    status: row.get(5)?,
                    error_code: row.get(6)?,
                    error_summary: row.get(7)?,
                    wechat_product_id: row.get(8)?,
                    audit_summary: row.get(9)?,
                    updated_at: row.get(10)?,
                },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (product_id, target) in rows {
        grouped.entry(product_id).or_default().push(target);
    }
    Ok(grouped)
}

/// 统一流水线视图查询：前端工作台数据源（状态统计 + 当前视图商品列表 + 批次列表）。
/// filter：None/"active"=未归档全部（默认），"archived"=仅已归档。
/// batch_id：非空时列表与统计都只看该导入批次（批量操作随筛选天然限定在批次内）。
#[tauri::command]
pub fn list_pipeline_products(
    app: AppHandle,
    filter: Option<String>,
    batch_id: Option<String>,
) -> AppResult<PipelineWorkbenchView> {
    let conn = open_connection(&app)?;
    let stats = load_pipeline_stats(&conn, batch_id.as_deref())?;
    let batches = load_import_batches(&conn)?;

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
        import_batch_id: Option<String>,
        updated_at: String,
    }

    let archived_view = filter.as_deref() == Some("archived");
    let archived_clause = if archived_view {
        "archived_at IS NOT NULL"
    } else {
        "archived_at IS NULL"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT id, external_product_id, title, source_url, category_path,
                stage, status, attention, progress_text, error_code, error_reason,
                import_batch_id, updated_at
         FROM pipeline_products
         WHERE {archived_clause}
           AND (?1 IS NULL OR import_batch_id = ?1)
         ORDER BY updated_at DESC, created_at DESC
         LIMIT 300"
    ))?;
    let products = stmt
        .query_map(params![batch_id], |row| {
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
                import_batch_id: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let product_ids: Vec<String> = products.iter().map(|p| p.id.clone()).collect();
    let mut targets_by_product = load_targets_grouped(&conn, &product_ids)?;

    let mut views = Vec::with_capacity(products.len());
    for product in products {
        let targets = targets_by_product.remove(&product.id).unwrap_or_default();
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
            // 错误信息只在 blocked 态透出：pending 态保留的 error_code 是「同码连续失败」
            // 计数链的内部依据（见 requeue_target），重试中不应在前端显示为错误。
            let blocked = target_is_failed(&target.status);
            let classification = target
                .error_code
                .as_deref()
                .filter(|_| blocked)
                .map(classify_error_code);
            shops.push(PipelineShopTargetView {
                id: target.id.clone(),
                shop_id: target.shop_id.clone(),
                shop_name: target.shop_name.clone(),
                status_text: target_status_text(
                    target.stage.as_str(),
                    target.status.as_str(),
                    target.error_code.as_deref(),
                ),
                error_code: target.error_code.clone().filter(|_| blocked),
                error_reason: classification.map(|c| c.human_reason.to_string()),
                suggested_action: classification.map(|c| c.suggested_action.to_string()),
                error_detail: target.error_summary.clone().filter(|_| blocked),
                can_retry: blocked,
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
        let suggested_action = error_code
            .as_deref()
            .map(|code| classify_error_code(code).suggested_action.to_string());
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
            suggested_action,
            total_shops: targets.len() as i64,
            listed_shops: agg.listed,
            failed_shops: agg.failed,
            pending_shops: agg.pending,
            can_retry,
            can_confirm,
            confirm_kind,
            import_batch_id: product.import_batch_id,
            updated_at: product.updated_at,
            shops,
        });
    }
    Ok(PipelineWorkbenchView {
        stats,
        products: views,
        batches,
    })
}

/// 重命名导入批次（名称由导入时自动生成，此处允许用户改成有业务含义的名字）。
#[tauri::command]
pub fn rename_import_batch(app: AppHandle, batch_id: String, name: String) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("批次名称不能为空".to_string()));
    }
    let conn = open_connection(&app)?;
    let updated = conn.execute(
        "UPDATE import_batches SET name = ?1 WHERE id = ?2",
        params![name, batch_id],
    )?;
    if updated == 0 {
        return Err(AppError::Validation("批次不存在或已被删除".to_string()));
    }
    Ok(())
}

/// 批量归档已上架商品：从工作台默认视图隐藏（仅 status=listed 可归档，防误归进行中商品）。
/// 返回实际归档数量。
#[tauri::command]
pub fn archive_pipeline_products(app: AppHandle, product_ids: Vec<String>) -> AppResult<i64> {
    if product_ids.is_empty() {
        return Ok(0);
    }
    let conn = open_connection(&app)?;
    let now = now_shanghai();
    let mut archived = 0usize;
    for product_id in &product_ids {
        archived += conn.execute(
            "UPDATE pipeline_products
             SET archived_at = ?1, updated_at = ?1
             WHERE id = ?2 AND status = 'listed' AND archived_at IS NULL",
            params![now, product_id],
        )?;
    }
    Ok(archived as i64)
}

/// 批量取消归档：商品回到工作台默认视图。返回实际恢复数量。
#[tauri::command]
pub fn unarchive_pipeline_products(app: AppHandle, product_ids: Vec<String>) -> AppResult<i64> {
    if product_ids.is_empty() {
        return Ok(0);
    }
    let conn = open_connection(&app)?;
    let now = now_shanghai();
    let mut restored = 0usize;
    for product_id in &product_ids {
        restored += conn.execute(
            "UPDATE pipeline_products
             SET archived_at = NULL, updated_at = ?1
             WHERE id = ?2 AND archived_at IS NOT NULL",
            params![now, product_id],
        )?;
    }
    Ok(restored as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内存库 + 最小化 pipeline_shop_targets 表（仅 block/requeue 原语涉及的列）。
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("内存库");
        conn.execute_batch(
            "CREATE TABLE pipeline_shop_targets (
               id TEXT PRIMARY KEY,
               product_id TEXT NOT NULL DEFAULT 'prod-1',
               stage TEXT NOT NULL DEFAULT 'submit',
               status TEXT NOT NULL DEFAULT 'pending',
               error_code TEXT,
               error_summary TEXT,
               retry_count INTEGER NOT NULL DEFAULT 0,
               next_retry_at TEXT,
               updated_at TEXT NOT NULL DEFAULT ''
             );
             INSERT INTO pipeline_shop_targets (id) VALUES ('t1');",
        )
        .expect("建表");
        conn
    }

    fn read_target(conn: &Connection) -> (i64, Option<String>, String) {
        conn.query_row(
            "SELECT retry_count, error_code, status FROM pipeline_shop_targets WHERE id = 't1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("读行")
    }

    #[test]
    fn block_target_counts_consecutive_same_code_failures() {
        let conn = test_conn();
        // 同一错误码连续 block：计数递增
        block_target(&conn, "t1", "WECHAT_ADDPRODUCT_SKU_SWALLOWED", "吞1").unwrap();
        assert_eq!(read_target(&conn).0, 1);
        block_target(&conn, "t1", "WECHAT_ADDPRODUCT_SKU_SWALLOWED", "吞2").unwrap();
        assert_eq!(read_target(&conn).0, 2);
        // 换错误码：新错误链，从 1 重计
        block_target(&conn, "t1", "ACCESS_TOKEN_FAILED", "令牌失败").unwrap();
        let (count, code, _) = read_target(&conn);
        assert_eq!(count, 1, "换码应重计");
        assert_eq!(code.as_deref(), Some("ACCESS_TOKEN_FAILED"));
    }

    #[test]
    fn requeue_keeps_error_chain_for_limit_judgement() {
        let conn = test_conn();
        block_target(&conn, "t1", "CATEGORY_ATTRS_NEED_AI_FILL", "缺属性").unwrap();
        block_target(&conn, "t1", "CATEGORY_ATTRS_NEED_AI_FILL", "缺属性").unwrap();
        // requeue 重试：status 回 pending，但 error_code 与 retry_count 保留（计数链不断）
        requeue_target(&conn, "t1").unwrap();
        let (count, code, status) = read_target(&conn);
        assert_eq!(status, "pending");
        assert_eq!(count, 2, "requeue 不应清计数");
        assert_eq!(code.as_deref(), Some("CATEGORY_ATTRS_NEED_AI_FILL"));
        // 再次同码失败：计数继续累加（auto_retry_limit 超限判定依赖此行为）
        block_target(&conn, "t1", "CATEGORY_ATTRS_NEED_AI_FILL", "缺属性").unwrap();
        assert_eq!(read_target(&conn).0, 3);
    }
}
