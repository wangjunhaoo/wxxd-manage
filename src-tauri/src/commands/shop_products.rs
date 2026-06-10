//! 微信小店真实商品管理：直连官方 API 同步商品到本地缓存，并支持上架/下架/删除/库存调整。
//!
//! 数据模式：`sync_shop_products` 调微信接口把商品写进 `wechat_shop_products` / `wechat_shop_product_skus`
//! 缓存表；列表页读缓存（快、可搜索）；上下架/删除/改库存等写操作走 API，成功后立即回写缓存行保持一致。
//!
//! 微信无「列表带概要」接口：`get_product_list` 只返回 product_id 列表，标题/价格/状态/SKU 必须逐个
//! `get_product` 拉取，因此同步 = 游标翻页拿全部 id + 有限并发拉详情入缓存。

use super::*;
use tokio::task::JoinSet;

/// 同步时拉取商品详情的并发度（避免对微信接口瞬时压力过大触发限频）。
const SYNC_DETAIL_CONCURRENCY: usize = 5;
/// 列表游标翻页的最大轮数兜底（30/页 × 300 = 9000 个商品上限），防 next_key 异常导致死循环。
const MAX_LIST_PAGES: usize = 300;

// ===== JSON 取值辅助 =====

/// 从 JSON 值中稳健取 i64（兼容数字、浮点、字符串数字）。
fn json_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|float| float as i64)),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 从 JSON 值中稳健取非空字符串（兼容数字 id 转字符串）。
fn json_str(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

// ===== 商品 JSON 解析 =====

struct ParsedSku {
    sku_id: String,
    out_sku_id: Option<String>,
    sku_code: Option<String>,
    sale_price_cents: Option<i64>,
    stock_num: Option<i64>,
    sku_attrs: Option<String>,
    thumb_img: Option<String>,
}

struct ParsedProduct {
    out_product_id: Option<String>,
    title: String,
    head_img: Option<String>,
    status: i64,
    edit_status: Option<i64>,
    min_price_cents: Option<i64>,
    cat_id: Option<i64>,
    total_stock: i64,
    skus: Vec<ParsedSku>,
}

/// 从 get_product 返回的 product / edit_product 对象解析出缓存所需字段。
fn parse_product(product: &Value, fallback_id: &str) -> ParsedProduct {
    let title = json_str(product.get("title")).unwrap_or_else(|| fallback_id.to_string());
    let head_img = product
        .get("head_imgs")
        .and_then(Value::as_array)
        .and_then(|images| images.iter().find_map(|image| json_str(Some(image))));
    let status = json_i64(product.get("status")).unwrap_or_default();
    let edit_status = json_i64(product.get("edit_status"));
    let min_price_cents = json_i64(product.get("min_price"));
    // 类目优先取新版多级类目树 cats_v2 的叶子（最后一级），回退旧 cats。
    let cat_id = product
        .get("cats_v2")
        .and_then(Value::as_array)
        .filter(|cats| !cats.is_empty())
        .or_else(|| product.get("cats").and_then(Value::as_array))
        .and_then(|cats| cats.last())
        .and_then(|cat| json_i64(cat.get("cat_id")));
    let out_product_id = json_str(product.get("out_product_id"));

    let skus = product
        .get("skus")
        .and_then(Value::as_array)
        .map(|skus| {
            skus.iter()
                .filter_map(|sku| {
                    let sku_id = json_str(sku.get("sku_id"))?;
                    let sku_attrs = sku
                        .get("sku_attrs")
                        .filter(|value| !value.is_null())
                        .map(|value| value.to_string());
                    Some(ParsedSku {
                        sku_id,
                        out_sku_id: json_str(sku.get("out_sku_id")),
                        sku_code: json_str(sku.get("sku_code")),
                        sale_price_cents: json_i64(sku.get("sale_price")),
                        stock_num: json_i64(sku.get("stock_num")),
                        sku_attrs,
                        thumb_img: json_str(sku.get("thumb_img")),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let total_stock = skus.iter().filter_map(|sku| sku.stock_num).sum();

    ParsedProduct {
        out_product_id,
        title,
        head_img,
        status,
        edit_status,
        min_price_cents,
        cat_id,
        total_stock,
        skus,
    }
}

/// 从 ProductGetInfo 取线上 product，缺失时回退草稿 edit_product。
fn product_value_from_info(info: &ProductGetInfo) -> Value {
    info.raw_payload
        .get("product")
        .filter(|value| !value.is_null())
        .or_else(|| {
            info.raw_payload
                .get("edit_product")
                .filter(|value| !value.is_null())
        })
        .cloned()
        .unwrap_or(Value::Null)
}

// ===== 缓存写入 =====

/// 把单个商品（及其 SKU）upsert 进缓存表。synced_at 统一用本批时间戳，便于同步后清理陈旧商品。
fn upsert_product(
    conn: &Connection,
    shop_id: &str,
    shop_name: &str,
    wechat_product_id: &str,
    info: &ProductGetInfo,
    batch_ts: &str,
) -> AppResult<()> {
    let product_value = product_value_from_info(info);
    let parsed = parse_product(&product_value, wechat_product_id);
    let row_id = format!("wsp-{shop_id}-{wechat_product_id}");
    // 缓存原始返回供排障，但剥离体积最大的详情图字段，避免缓存表无谓膨胀。
    let mut slim_payload = product_value.clone();
    if let Some(object) = slim_payload.as_object_mut() {
        object.remove("desc_info");
    }

    conn.execute(
        "INSERT INTO wechat_shop_products
           (id, shop_id, shop_name, wechat_product_id, out_product_id, title, head_img,
            status, edit_status, min_price_cents, cat_id, total_stock, sku_count,
            audit_summary, raw_payload, synced_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16, ?16)
         ON CONFLICT(shop_id, wechat_product_id) DO UPDATE SET
           shop_name = excluded.shop_name,
           out_product_id = excluded.out_product_id,
           title = excluded.title,
           head_img = excluded.head_img,
           status = excluded.status,
           edit_status = excluded.edit_status,
           min_price_cents = excluded.min_price_cents,
           cat_id = excluded.cat_id,
           total_stock = excluded.total_stock,
           sku_count = excluded.sku_count,
           audit_summary = excluded.audit_summary,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            row_id,
            shop_id,
            shop_name,
            wechat_product_id,
            parsed.out_product_id,
            parsed.title,
            parsed.head_img,
            parsed.status,
            parsed.edit_status,
            parsed.min_price_cents,
            parsed.cat_id,
            parsed.total_stock,
            parsed.skus.len() as i64,
            Option::<String>::None,
            slim_payload.to_string(),
            batch_ts,
        ],
    )?;

    // SKU 全量重写：先删旧再插新，保证删掉微信侧已移除的 SKU。
    conn.execute(
        "DELETE FROM wechat_shop_product_skus WHERE shop_id = ?1 AND wechat_product_id = ?2",
        params![shop_id, wechat_product_id],
    )?;
    for sku in &parsed.skus {
        conn.execute(
            "INSERT INTO wechat_shop_product_skus
               (id, product_row_id, shop_id, wechat_product_id, sku_id, out_sku_id, sku_code,
                sale_price_cents, stock_num, sku_attrs, thumb_img, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                format!("wsps-{shop_id}-{wechat_product_id}-{}", sku.sku_id),
                row_id,
                shop_id,
                wechat_product_id,
                sku.sku_id,
                sku.out_sku_id,
                sku.sku_code,
                sku.sale_price_cents,
                sku.stock_num,
                sku.sku_attrs,
                sku.thumb_img,
                batch_ts,
            ],
        )?;
    }
    Ok(())
}

/// 取店铺名（缺失时回退 shop_id）。
fn shop_name_of(app: &AppHandle, shop_id: &str) -> AppResult<String> {
    let conn = open_connection(app)?;
    Ok(conn
        .query_row(
            "SELECT name FROM shops WHERE id = ?1",
            [shop_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| shop_id.to_string()))
}

/// 写操作成功后重新拉取该商品详情回写缓存，保证页面与微信一致。回写失败不影响主操作结果。
async fn refresh_product_cache(
    app: &AppHandle,
    client: &WechatShopClient,
    access_token: &str,
    shop_id: &str,
    shop_name: &str,
    wechat_product_id: &str,
) -> AppResult<()> {
    let call = client.get_product(access_token, wechat_product_id, 3).await?;
    match call.result {
        WechatCallResult::Success(info) => {
            let batch_ts = now_shanghai();
            let mut conn = open_connection(app)?;
            let tx = conn.transaction()?;
            upsert_product(&tx, shop_id, shop_name, wechat_product_id, &info, &batch_ts)?;
            tx.commit()?;
        }
        WechatCallResult::ApiError(error) => {
            let conn = open_connection(app)?;
            insert_api_call_log(
                &conn,
                Some(shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("refresh product cache after action"),
            )?;
        }
    }
    Ok(())
}

// ===== 缓存读取（行 → 视图）=====

fn row_to_product_view(row: &rusqlite::Row) -> rusqlite::Result<WechatShopProductView> {
    Ok(WechatShopProductView {
        id: row.get("id")?,
        shop_id: row.get("shop_id")?,
        shop_name: row.get("shop_name")?,
        wechat_product_id: row.get("wechat_product_id")?,
        out_product_id: row.get("out_product_id")?,
        title: row.get("title")?,
        head_img: row.get("head_img")?,
        status: row.get("status")?,
        edit_status: row.get("edit_status")?,
        min_price_cents: row.get("min_price_cents")?,
        cat_id: row.get("cat_id")?,
        total_stock: row.get("total_stock")?,
        sku_count: row.get("sku_count")?,
        audit_summary: row.get("audit_summary")?,
        synced_at: row.get("synced_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_sku_view(row: &rusqlite::Row) -> rusqlite::Result<WechatShopProductSkuView> {
    Ok(WechatShopProductSkuView {
        sku_id: row.get("sku_id")?,
        out_sku_id: row.get("out_sku_id")?,
        sku_code: row.get("sku_code")?,
        sale_price_cents: row.get("sale_price_cents")?,
        stock_num: row.get("stock_num")?,
        sku_attrs: row.get("sku_attrs")?,
        thumb_img: row.get("thumb_img")?,
    })
}

// ===== Tauri 命令 =====

/// 同步指定店铺的真实商品到本地缓存：游标翻页拿全部 product_id → 有限并发拉详情 → upsert → 清理陈旧。
#[tauri::command]
pub async fn sync_shop_products(
    app: AppHandle,
    shop_id: String,
) -> AppResult<SyncShopProductsResult> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let shop_name = shop_name_of(&app, &shop_id)?;

    // 1) 游标翻页拉取全部商品 id。
    // 微信 getproductlist：status 不填默认排除「从未上架的草稿(0)和回收站(6)」，
    // 故第二轮显式传 status=0 补拉从未上架的草稿商品；两轮互斥，合并去重统一纳入管理。
    let mut product_ids: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut total_num: i64 = 0;
    for status in [None, Some(0_i64)] {
        let is_draft_round = status.is_some();
        // 日志 detail 区分两轮，便于排查草稿补拉是否被店铺/权限限制。
        let log_detail = if is_draft_round {
            "get_product_list(draft)"
        } else {
            "get_product_list(all)"
        };
        let mut next_key: Option<String> = None;
        let mut round_total: i64 = 0;
        'pages: for _ in 0..MAX_LIST_PAGES {
            let call = client
                .get_product_list(&access_token, status, 30, next_key.as_deref())
                .await?;
            match call.result {
                WechatCallResult::Success(data) => {
                    // 两轮 status 互斥，各取该轮最后一页的总数，轮末累加进 total_num。
                    round_total = data.total_num;
                    let got = data.product_ids.len();
                    // 循环内即时去重，避免翻页边界与跨轮重复污染唯一计数与终止判断。
                    for id in data.product_ids {
                        if seen.insert(id.clone()) {
                            product_ids.push(id);
                        }
                    }
                    next_key = data.next_key;
                    // 仅依赖微信契约的正常终止信号（空页 / 无 next_key）；不用「含重复计数 >= total_num」
                    // 提前终止，否则边界重复会少翻一页漏拉商品；MAX_LIST_PAGES 兜底防死循环。
                    if got == 0 || next_key.is_none() {
                        break 'pages;
                    }
                }
                WechatCallResult::ApiError(error) => {
                    let conn = open_connection(&app)?;
                    insert_api_call_log(
                        &conn,
                        Some(&shop_id),
                        call.meta.endpoint,
                        call.meta.method,
                        "api_error",
                        Some(error.errcode),
                        Some(&error.errmsg),
                        Some(log_detail),
                    )?;
                    // 默认轮是主数据来源，失败致命；草稿轮为增量补充，记日志后跳过不阻断主同步。
                    if is_draft_round {
                        round_total = 0;
                        break 'pages;
                    }
                    return Err((&error).into());
                }
            }
        }
        total_num += round_total;
    }
    // 2) 有限并发拉详情（分块 spawn，每块 join 完再下一块）。
    let mut details: Vec<(String, ProductGetInfo)> = Vec::new();
    let mut failed_count: i64 = 0;
    for chunk in product_ids.chunks(SYNC_DETAIL_CONCURRENCY) {
        let mut set: JoinSet<(String, AppResult<crate::wechat::ProductGetCall>)> = JoinSet::new();
        for product_id in chunk {
            let client = client.clone();
            let token = access_token.clone();
            let product_id = product_id.clone();
            set.spawn(async move {
                // data_type=3：同时取线上 product 与草稿 edit_product，覆盖审核中/未上架商品。
                let result = client.get_product(&token, &product_id, 3).await;
                (product_id, result)
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((product_id, Ok(call))) => match call.result {
                    WechatCallResult::Success(info) => details.push((product_id, info)),
                    WechatCallResult::ApiError(_) => failed_count += 1,
                },
                Ok((_, Err(_))) => failed_count += 1,
                Err(_join_error) => failed_count += 1,
            }
        }
    }

    // 3) 入库（事务）+ 同步成功时清理本次未出现的陈旧商品。
    let batch_ts = now_shanghai();
    let synced_count = details.len() as i64;
    let mut conn = open_connection(&app)?;
    let tx = conn.transaction()?;
    for (product_id, info) in &details {
        upsert_product(&tx, &shop_id, &shop_name, product_id, info, &batch_ts)?;
    }
    if failed_count == 0 {
        tx.execute(
            "DELETE FROM wechat_shop_product_skus WHERE shop_id = ?1 AND synced_at <> ?2",
            params![shop_id, batch_ts],
        )?;
        tx.execute(
            "DELETE FROM wechat_shop_products WHERE shop_id = ?1 AND synced_at <> ?2",
            params![shop_id, batch_ts],
        )?;
    }
    tx.commit()?;

    Ok(SyncShopProductsResult {
        synced_count,
        total_num,
        failed_count,
    })
}

/// 清理孤儿草稿：根治草稿箱堆积（根因见记忆 wechat-listing-sku-zero-6600099 / 草稿箱排查）。
/// 微信对同 out_product_id 重复 addproduct 不去重、每次新建独立草稿，叠加历史退避换 ID 重试，
/// 致同一淘宝商品在草稿箱(status=0)堆多份未上架草稿。本命令：① 先 sync 刷新本地缓存；② 读 status=0
/// 草稿与 status=5 已上架的 out_product_id base（去 -r{n} 后缀归并同一商品）；③ base 已上架的草稿=
/// 重复→删除，base 独有的→按 base 归并保 SKU 最多 1 个尝试 listing 上架（配额已解除可转正），上架
/// 失败或空草稿一律删除；④ 再 sync 刷新并统计。网络调用均在数据库连接关闭后进行（conn 不可跨 await）。
#[tauri::command]
pub async fn cleanup_orphan_drafts(
    app: AppHandle,
    shop_id: String,
) -> AppResult<CleanupOrphanDraftsResult> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;

    // 1) 先同步刷新本地缓存（复用 sync_shop_products 的翻页 + 详情拉取，含 status=0 草稿补拉）。
    sync_shop_products(app.clone(), shop_id.clone()).await?;

    // 2) 读本地草稿(status=0)与已上架(status=5)的 base 集合（base = 去退避换 ID 残留 -r{n} 后缀）。
    struct DraftRow {
        product_id: String,
        base: String,
        sku_count: i64,
    }
    let (drafts, listed_bases, draft_total) = {
        let conn = open_connection(&app)?;
        let drafts: Vec<DraftRow> = {
            let mut stmt = conn.prepare(
                "SELECT wechat_product_id, IFNULL(out_product_id, ''), sku_count
                 FROM wechat_shop_products WHERE shop_id = ?1 AND status = 0",
            )?;
            // collect 结果先绑定 let（rusqlite MappedRows 借用 stmt，不能作 block 末尾表达式
            // 直接返回，否则 stmt 临时先于借用 drop 触发 E0597）。
            let rows = stmt
                .query_map([&shop_id], |row| {
                    let product_id: String = row.get(0)?;
                    let out_id: String = row.get(1)?;
                    let sku_count: i64 = row.get(2)?;
                    Ok(DraftRow {
                        product_id,
                        base: strip_retry_suffix(&out_id),
                        sku_count,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let listed_bases: std::collections::HashSet<String> = {
            let mut stmt = conn.prepare(
                "SELECT IFNULL(out_product_id, '') FROM wechat_shop_products
                 WHERE shop_id = ?1 AND status = 5",
            )?;
            let bases = stmt
                .query_map([&shop_id], |row| row.get::<_, String>(0))?
                .filter_map(Result::ok)
                .map(|out_id| strip_retry_suffix(&out_id))
                .filter(|base| !base.is_empty())
                .collect();
            bases
        };
        let draft_total = drafts.len() as i64;
        (drafts, listed_bases, draft_total)
    };

    // 3) 分组决策（纯内存）：重复草稿入删除集；独有按 base 归并，保 SKU 最多 1 个尝试上架、其余删。
    let mut by_base: std::collections::BTreeMap<String, Vec<DraftRow>> =
        std::collections::BTreeMap::new();
    let mut to_delete: Vec<String> = Vec::new();
    for draft in drafts {
        if draft.base.is_empty() || listed_bases.contains(&draft.base) {
            to_delete.push(draft.product_id);
        } else {
            by_base.entry(draft.base.clone()).or_default().push(draft);
        }
    }
    let mut to_list: Vec<String> = Vec::new();
    for (_base, mut group) in by_base {
        group.sort_by(|a, b| b.sku_count.cmp(&a.sku_count));
        let mut iter = group.into_iter();
        if let Some(best) = iter.next() {
            if best.sku_count > 0 {
                to_list.push(best.product_id);
            } else {
                to_delete.push(best.product_id);
            }
        }
        for rest in iter {
            to_delete.push(rest.product_id);
        }
    }

    // 4) 执行：先上架独有草稿（成功转正、失败转删），再批量删除。网络调用均在 conn 关闭后。
    let mut listed_promoted = 0i64;
    for product_id in to_list {
        let promoted = matches!(
            client.listing_product(&access_token, &product_id).await,
            Ok(call) if matches!(call.result, WechatCallResult::Success(_))
        );
        if promoted {
            listed_promoted += 1;
        } else {
            to_delete.push(product_id);
        }
    }
    let mut deleted = 0i64;
    for product_id in &to_delete {
        if let Ok(call) = client.delete_product(&access_token, product_id).await {
            if matches!(call.result, WechatCallResult::Success(_)) {
                deleted += 1;
            }
        }
    }

    // 5) 再次同步刷新缓存、统计清理后剩余草稿数，并记一条 api_call_log 留痕。
    sync_shop_products(app.clone(), shop_id.clone()).await?;
    let remaining_after = {
        let conn = open_connection(&app)?;
        let remaining: i64 = conn.query_row(
            "SELECT COUNT(*) FROM wechat_shop_products WHERE shop_id = ?1 AND status = 0",
            [&shop_id],
            |row| row.get(0),
        )?;
        insert_api_call_log(
            &conn,
            Some(&shop_id),
            "cleanup_orphan_drafts",
            "LOCAL",
            "success",
            None,
            None,
            Some(&format!(
                "清理孤儿草稿：清理前 {draft_total}，删除 {deleted}，上架转正 {listed_promoted}，剩余 {remaining}"
            )),
        )?;
        remaining
    };

    Ok(CleanupOrphanDraftsResult {
        shop_id,
        draft_total,
        deleted,
        listed_promoted,
        remaining_after,
    })
}

/// 去掉退避换 ID 残留的 `-r{n}` 后缀，把同一淘宝商品的多份草稿归并到同一 base。
/// out_product_id 形如 `https://item.taobao.com/item.htm?id=123`（淘宝 URL，不含 -r）或历史
/// `...id=123-r2`（旧版换 ID 产物）。仅当 `-r` 后全为数字才视作重试后缀，避免误伤 URL 内容。
fn strip_retry_suffix(out_id: &str) -> String {
    let trimmed = out_id.trim();
    if let Some(idx) = trimmed.rfind("-r") {
        let suffix = &trimmed[idx + 2..];
        if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            return trimmed[..idx].to_string();
        }
    }
    trimmed.to_string()
}

/// 读缓存商品列表，支持按店铺、状态、关键词筛选。
#[tauri::command]
pub fn list_cached_shop_products(
    app: AppHandle,
    shop_id: Option<String>,
    status: Option<i64>,
    keyword: Option<String>,
    limit: Option<i64>,
) -> AppResult<ShopProductListResult> {
    let conn = open_connection(&app)?;
    // 上限与同步能力（MAX_LIST_PAGES × 30 = 9000）对齐，避免大店商品被列表截断而不可见。
    let limit = limit.unwrap_or(10_000).clamp(1, 10_000);
    let shop_id = shop_id.filter(|value| !value.trim().is_empty() && value != "all");
    let keyword = keyword
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(shop_id) = &shop_id {
        conditions.push(format!("shop_id = ?{}", binds.len() + 1));
        binds.push(Box::new(shop_id.clone()));
    }
    if let Some(status) = status {
        conditions.push(format!("status = ?{}", binds.len() + 1));
        binds.push(Box::new(status));
    }
    if let Some(keyword) = &keyword {
        let like = format!("%{keyword}%");
        conditions.push(format!(
            "(title LIKE ?{0} OR wechat_product_id LIKE ?{0} OR IFNULL(out_product_id, '') LIKE ?{0})",
            binds.len() + 1
        ));
        binds.push(Box::new(like));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let bind_refs: Vec<&dyn rusqlite::types::ToSql> =
        binds.iter().map(|value| value.as_ref()).collect();

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM wechat_shop_products {where_clause}"),
        bind_refs.as_slice(),
        |row| row.get(0),
    )?;

    let sql = format!(
        "SELECT id, shop_id, shop_name, wechat_product_id, out_product_id, title, head_img,
                status, edit_status, min_price_cents, cat_id, total_stock, sku_count,
                audit_summary, synced_at, updated_at
         FROM wechat_shop_products
         {where_clause}
         ORDER BY updated_at DESC
         LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(bind_refs.as_slice(), row_to_product_view)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ShopProductListResult { items, total })
}

/// 读单个缓存商品详情（含 SKU 列表）。
#[tauri::command]
pub fn get_cached_shop_product_detail(
    app: AppHandle,
    row_id: String,
) -> AppResult<WechatShopProductDetailView> {
    let conn = open_connection(&app)?;
    let product = conn
        .query_row(
            "SELECT id, shop_id, shop_name, wechat_product_id, out_product_id, title, head_img,
                    status, edit_status, min_price_cents, cat_id, total_stock, sku_count,
                    audit_summary, synced_at, updated_at
             FROM wechat_shop_products WHERE id = ?1",
            [&row_id],
            row_to_product_view,
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("商品缓存不存在，请先同步".to_string()))?;

    let mut stmt = conn.prepare(
        "SELECT sku_id, out_sku_id, sku_code, sale_price_cents, stock_num, sku_attrs, thumb_img
         FROM wechat_shop_product_skus WHERE product_row_id = ?1 ORDER BY sku_id",
    )?;
    let skus = stmt
        .query_map([&row_id], row_to_sku_view)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(WechatShopProductDetailView { product, skus })
}

/// 商品上架，成功后回写缓存。
#[tauri::command]
pub async fn listing_shop_product(
    app: AppHandle,
    shop_id: String,
    wechat_product_id: String,
) -> AppResult<()> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client.listing_product(&access_token, &wechat_product_id).await?;
    {
        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(_) => insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("listing_product"),
            )?,
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("listing_product"),
                )?;
                return Err(error.into());
            }
        }
    }
    let shop_name = shop_name_of(&app, &shop_id)?;
    // 回写缓存失败不应让主操作（上/下架已在微信侧生效）变成错误，避免误导用户重复提交。
    let _ = refresh_product_cache(
        &app,
        &client,
        &access_token,
        &shop_id,
        &shop_name,
        &wechat_product_id,
    )
    .await;
    Ok(())
}

/// 商品下架，成功后回写缓存。审核中的商品需先撤回审核（微信返回 10020047/10020049）。
#[tauri::command]
pub async fn delisting_shop_product(
    app: AppHandle,
    shop_id: String,
    wechat_product_id: String,
) -> AppResult<()> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client
        .delisting_product(&access_token, &wechat_product_id)
        .await?;
    {
        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(_) => insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("delisting_product"),
            )?,
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("delisting_product"),
                )?;
                return Err(error.into());
            }
        }
    }
    let shop_name = shop_name_of(&app, &shop_id)?;
    // 回写缓存失败不应让主操作（上/下架已在微信侧生效）变成错误，避免误导用户重复提交。
    let _ = refresh_product_cache(
        &app,
        &client,
        &access_token,
        &shop_id,
        &shop_name,
        &wechat_product_id,
    )
    .await;
    Ok(())
}

/// 删除商品，成功后清掉本地缓存。审核中的商品无法删除（微信返回 10020049）。
#[tauri::command]
pub async fn delete_shop_product(
    app: AppHandle,
    shop_id: String,
    wechat_product_id: String,
) -> AppResult<()> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client
        .delete_product(&access_token, &wechat_product_id)
        .await?;
    let conn = open_connection(&app)?;
    match &call.result {
        WechatCallResult::Success(_) => {
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("delete_product"),
            )?;
            conn.execute(
                "DELETE FROM wechat_shop_product_skus WHERE shop_id = ?1 AND wechat_product_id = ?2",
                params![shop_id, wechat_product_id],
            )?;
            conn.execute(
                "DELETE FROM wechat_shop_products WHERE shop_id = ?1 AND wechat_product_id = ?2",
                params![shop_id, wechat_product_id],
            )?;
            Ok(())
        }
        WechatCallResult::ApiError(error) => {
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("delete_product"),
            )?;
            Err(error.into())
        }
    }
}

/// 调整单个 SKU 库存（diff_type：1 增 / 2 减 / 3 设置），成功后回读真实库存并回写缓存，返回新库存。
#[tauri::command]
pub async fn update_shop_product_stock(
    app: AppHandle,
    shop_id: String,
    wechat_product_id: String,
    sku_id: String,
    diff_type: i64,
    num: i64,
) -> AppResult<i64> {
    if !(1..=3).contains(&diff_type) {
        return Err(AppError::Validation(
            "库存修改类型不合法（1 增 / 2 减 / 3 设置）".to_string(),
        ));
    }
    if num < 0 {
        return Err(AppError::Validation("库存数量不能为负".to_string()));
    }

    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;
    let call = client
        .update_stock(&access_token, &wechat_product_id, &sku_id, diff_type, num)
        .await?;
    {
        let conn = open_connection(&app)?;
        match &call.result {
            WechatCallResult::Success(_) => insert_api_call_log(
                &conn,
                Some(&shop_id),
                call.meta.endpoint,
                call.meta.method,
                "success",
                None,
                None,
                Some("update_stock"),
            )?,
            WechatCallResult::ApiError(error) => {
                insert_api_call_log(
                    &conn,
                    Some(&shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("update_stock"),
                )?;
                return Err(error.into());
            }
        }
    }

    // 回读真实库存（normal_stock_num=通用库存）。只有回读成功才拿到权威绝对值。
    let stock_call = client
        .get_stock(&access_token, &wechat_product_id, &sku_id)
        .await?;
    let resolved_stock: Option<i64> = match &stock_call.result {
        WechatCallResult::Success(info) => Some(info.normal_stock_num),
        WechatCallResult::ApiError(error) => {
            let conn = open_connection(&app)?;
            insert_api_call_log(
                &conn,
                Some(&shop_id),
                stock_call.meta.endpoint,
                stock_call.meta.method,
                "api_error",
                Some(error.errcode),
                Some(&error.errmsg),
                Some("get_stock after update"),
            )?;
            // 回读失败：仅「设置(3)」语义能确定绝对库存；增/减(1/2) 是相对量，
            // 不知旧值无法算出新库存，绝不能把增量 num 当绝对库存写缓存，留待「刷新库存」或下次同步纠正。
            if diff_type == 3 {
                Some(num)
            } else {
                None
            }
        }
    };

    match resolved_stock {
        Some(stock) => {
            let conn = open_connection(&app)?;
            let row_id = format!("wsp-{shop_id}-{wechat_product_id}");
            conn.execute(
                "UPDATE wechat_shop_product_skus SET stock_num = ?1, synced_at = ?2
                 WHERE shop_id = ?3 AND wechat_product_id = ?4 AND sku_id = ?5",
                params![stock, now_shanghai(), shop_id, wechat_product_id, sku_id],
            )?;
            conn.execute(
                "UPDATE wechat_shop_products
                 SET total_stock = (
                       SELECT COALESCE(SUM(stock_num), 0)
                       FROM wechat_shop_product_skus WHERE product_row_id = ?1
                     ),
                     updated_at = ?2
                 WHERE id = ?1",
                params![row_id, now_shanghai()],
            )?;
            Ok(stock)
        }
        None => Err(AppError::Validation(
            "库存修改已提交到微信，但回读最新库存失败；请点「刷新库存」核对实际值".to_string(),
        )),
    }
}

/// 批量刷新指定店铺缓存商品的库存（batchgetstock，每 50 个一批），比全量同步快。返回更新的 SKU 数。
#[tauri::command]
pub async fn refresh_shop_stock(app: AppHandle, shop_id: String) -> AppResult<i64> {
    let client = WechatShopClient::default();
    let access_token = ensure_access_token(&app, &shop_id, &client).await?;

    let product_ids: Vec<String> = {
        let conn = open_connection(&app)?;
        let mut stmt =
            conn.prepare("SELECT wechat_product_id FROM wechat_shop_products WHERE shop_id = ?1")?;
        let ids = stmt
            .query_map([&shop_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids
    };
    if product_ids.is_empty() {
        return Ok(0);
    }

    let mut updated: i64 = 0;
    for chunk in product_ids.chunks(50) {
        let call = client
            .batch_get_stock(&access_token, chunk, None)
            .await?;
        match call.result {
            WechatCallResult::Success(info) => {
                let conn = open_connection(&app)?;
                let ts = now_shanghai();
                for spu in &info.spu_stock_list {
                    let wechat_product_id = match json_str(spu.get("product_id")) {
                        Some(value) => value,
                        None => continue,
                    };
                    if let Some(sku_list) = spu.get("sku_stock").and_then(Value::as_array) {
                        for sku in sku_list {
                            let sku_id = match json_str(sku.get("sku_id")) {
                                Some(value) => value,
                                None => continue,
                            };
                            let stock =
                                json_i64(sku.get("normal_stock_num")).unwrap_or_default();
                            updated += conn.execute(
                                "UPDATE wechat_shop_product_skus SET stock_num = ?1, synced_at = ?2
                                 WHERE shop_id = ?3 AND wechat_product_id = ?4 AND sku_id = ?5",
                                params![stock, ts, shop_id, wechat_product_id, sku_id],
                            )? as i64;
                        }
                    }
                    conn.execute(
                        "UPDATE wechat_shop_products
                         SET total_stock = (
                               SELECT COALESCE(SUM(stock_num), 0)
                               FROM wechat_shop_product_skus
                               WHERE shop_id = ?1 AND wechat_product_id = ?2
                             ),
                             updated_at = ?3
                         WHERE shop_id = ?1 AND wechat_product_id = ?2",
                        params![shop_id, wechat_product_id, ts],
                    )?;
                }
            }
            WechatCallResult::ApiError(error) => {
                let conn = open_connection(&app)?;
                insert_api_call_log(
                    &conn,
                    Some(&shop_id),
                    call.meta.endpoint,
                    call.meta.method,
                    "api_error",
                    Some(error.errcode),
                    Some(&error.errmsg),
                    Some("batch_get_stock"),
                )?;
                // 单批失败不中断整体：记日志后跳过该批，继续刷新其余批次（尽力而为）。
                continue;
            }
        }
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_i64_accepts_number_and_string() {
        assert_eq!(json_i64(Some(&json!(12))), Some(12));
        assert_eq!(json_i64(Some(&json!("34"))), Some(34));
        assert_eq!(json_i64(Some(&json!(5.9))), Some(5));
        assert_eq!(json_i64(Some(&json!(null))), None);
        assert_eq!(json_i64(None), None);
    }

    #[test]
    fn parse_product_extracts_core_fields_and_aggregates_stock() {
        let product = json!({
            "title": "测试商品",
            "out_product_id": "out-1",
            "head_imgs": ["https://img/a.jpg", "https://img/b.jpg"],
            "status": 5,
            "edit_status": 4,
            "min_price": 1990,
            "cats_v2": [{"cat_id": 1}, {"cat_id": 2}, {"cat_id": 3}],
            "skus": [
                {"sku_id": 100, "sale_price": 1990, "stock_num": 7, "sku_code": "A"},
                {"sku_id": "200", "sale_price": 2990, "stock_num": 3}
            ]
        });
        let parsed = parse_product(&product, "fallback");
        assert_eq!(parsed.title, "测试商品");
        assert_eq!(parsed.head_img.as_deref(), Some("https://img/a.jpg"));
        assert_eq!(parsed.status, 5);
        assert_eq!(parsed.min_price_cents, Some(1990));
        assert_eq!(parsed.cat_id, Some(3)); // 叶子类目取最后一级
        assert_eq!(parsed.skus.len(), 2);
        assert_eq!(parsed.total_stock, 10);
        assert_eq!(parsed.skus[0].sku_id, "100");
        assert_eq!(parsed.skus[1].sku_id, "200");
    }

    #[test]
    fn parse_product_uses_fallback_title_when_missing() {
        let parsed = parse_product(&json!({"status": 5}), "wpid-9");
        assert_eq!(parsed.title, "wpid-9");
        assert_eq!(parsed.total_stock, 0);
        assert!(parsed.skus.is_empty());
    }
}
