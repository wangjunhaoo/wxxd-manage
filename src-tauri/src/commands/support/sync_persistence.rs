use super::*;

/// 本地履约状态 rank（订单履约重设计 §2.2）：数值越大越靠后，同步侧只允许单调前进。
/// cancelled rank 最高：官方 200（全部售后取消）可发生在 100 完成之后，必须放行 completed→cancelled。
/// exception 为业务侧人工异常态，rank 介于采购与微信发货之间：列表/详情同步的 pending_shipment(10)
/// 映射不得清掉它，但微信侧已发货(40)/完成(50)/取消(60) 必须能推进它。
/// 未知/遗留值（aftersale_active 等）记 -1：允许被任何映射覆盖修复。
pub(in crate::commands) fn fulfillment_status_rank(status: &str) -> i64 {
    match status {
        "unpaid" => 0,
        "pending_shipment" => 10,
        "pending_purchase" => 20,
        "exception" => 30,
        "supplier_shipped" => 30,
        "partially_shipped" => 35,
        "shipping_submitted" => 38,
        "wechat_shipped" => 40,
        "completed" => 50,
        "cancelled" => 60,
        // synced 是「列表新发现、详情未拉、真实状态未知」的入口占位态
        _ => -1,
    }
}

/// 同步侧订单状态的唯一写入口（订单履约重设计 §2.3），替换原先的无条件覆盖：
/// ① wechat_status 镜像无条件写；② status 仅当映射目标 rank 严格大于当前 rank 才写（永不回退）；
/// ③ 跳变钩子：取消单联动采购任务 + critical 通知；外部发货（官方后台发货）warning 提醒核对。
pub(in crate::commands) fn apply_wechat_status(
    conn: &Connection,
    local_order_id: &str,
    shop_id: &str,
    wechat_order_id: &str,
    wechat_status: i64,
) -> AppResult<()> {
    let now = now_shanghai();
    let Some(current) = conn
        .query_row(
            "SELECT status FROM orders WHERE id = ?1",
            params![local_order_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Ok(());
    };
    conn.execute(
        "UPDATE orders SET wechat_status = ?1, updated_at = ?2 WHERE id = ?3",
        params![wechat_status, now, local_order_id],
    )?;
    let mapped = order_status_from_wechat(wechat_status);
    if fulfillment_status_rank(mapped) <= fulfillment_status_rank(&current) {
        return Ok(());
    }
    conn.execute(
        "UPDATE orders SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![mapped, now, local_order_id],
    )?;

    // 跳变钩子①：订单被取消但本地已有未取消的采购任务 → 任务联动取消 + critical 通知（需上游拦截/退款）
    if mapped == "cancelled" {
        let open_tasks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM purchase_tasks WHERE order_id = ?1 AND status != 'cancelled'",
            params![local_order_id],
            |row| row.get(0),
        )?;
        if open_tasks > 0 {
            conn.execute(
                "UPDATE purchase_tasks
                 SET status = 'cancelled',
                     error_summary = '微信订单已取消，采购任务联动取消',
                     updated_at = ?1
                 WHERE order_id = ?2 AND status != 'cancelled'",
                params![now, local_order_id],
            )?;
            upsert_notification(
                conn,
                "critical",
                "order_cancelled",
                wechat_order_id,
                Some(shop_id),
                "已采购订单被取消",
                &format!(
                    "订单 {wechat_order_id} 在微信侧已取消，但本地存在 {open_tasks} 个进行中的采购任务，已联动取消；若上游已下单请尽快拦截或申请退款。"
                ),
                Some(&serde_json::json!({
                    "order_id": local_order_id,
                    "wechat_order_id": wechat_order_id,
                    "shop_id": shop_id,
                    "open_purchase_tasks": open_tasks
                })),
            )?;
        }
    }

    // 跳变钩子②：本地还在采购链却收到微信「已发货」→ 商家在官方后台/工具发过货（外部发货回流），提醒核对
    if mapped == "wechat_shipped"
        && matches!(
            current.as_str(),
            "pending_purchase" | "supplier_shipped" | "exception"
        )
    {
        upsert_notification(
            conn,
            "warning",
            "order_external_shipped",
            wechat_order_id,
            Some(shop_id),
            "订单已在系统外发货",
            &format!(
                "订单 {wechat_order_id} 微信侧已发货，但本地履约停在「{current}」，疑似经官方后台发货，请核对采购任务与运单归属。"
            ),
            Some(&serde_json::json!({
                "order_id": local_order_id,
                "wechat_order_id": wechat_order_id,
                "shop_id": shop_id,
                "local_status_before": current
            })),
        )?;
    }
    Ok(())
}

/// 列表同步落库（订单履约重设计 §2.3-⑤）：列表接口只知道过滤条件、不知道订单真实状态，
/// 因此彻底不写 status——新单以 'synced' 占位态进入，存量单只置 detail_dirty=1，
/// 真实状态一律由 order/get 详情经 apply_wechat_status 守卫落定。
/// 这同时根治了旧实现「重跑 status=20 同步把 pending_purchase 打回 pending_shipment」的回退 bug。
pub(in crate::commands) fn save_synced_order(
    conn: &Connection,
    shop_id: &str,
    wechat_order_id: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO orders
         (id, shop_id, wechat_order_id, status, raw_payload, detail_dirty, created_at, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'synced', ?4, 1, ?5, ?5, ?5)
         ON CONFLICT(shop_id, wechat_order_id) DO UPDATE SET
           detail_dirty = 1,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("order-{}-{}", shop_id, wechat_order_id),
            shop_id,
            wechat_order_id,
            serde_json::json!({
                "order_id": wechat_order_id
            })
            .to_string(),
            now
        ],
    )?;
    Ok(())
}

pub(in crate::commands) fn mark_order_detail_failed(
    app: &AppHandle,
    item: &OrderDetailSyncItem,
    detail_error: &str,
) -> AppResult<()> {
    let conn = open_connection(app)?;
    conn.execute(
        "UPDATE orders SET detail_error = ?1, updated_at = ?2 WHERE id = ?3",
        params![detail_error, now_shanghai(), item.order_id],
    )?;
    upsert_notification(
        &conn,
        "critical",
        "order_detail_sync",
        &item.order_id,
        Some(&item.shop_id),
        "订单详情同步失败",
        &format!(
            "订单 {} 详情同步失败：{}",
            item.wechat_order_id, detail_error
        ),
        Some(&serde_json::json!({
            "order_id": &item.order_id,
            "wechat_order_id": &item.wechat_order_id,
            "shop_id": &item.shop_id
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn save_order_detail(
    conn: &Connection,
    item: &OrderDetailSyncItem,
    order: &Value,
) -> AppResult<i64> {
    let now = now_shanghai();
    let wechat_status = order.get("status").and_then(Value::as_i64);
    let order_created_at = order.get("create_time").and_then(Value::as_i64);
    let order_updated_at = order.get("update_time").and_then(Value::as_i64);
    let summary_payload = sanitize_order_payload(order);

    // 状态经唯一守卫落定（镜像无条件、履约状态单调前进、含取消/外部发货钩子）
    if let Some(wechat_status) = wechat_status {
        apply_wechat_status(
            conn,
            &item.order_id,
            &item.shop_id,
            &item.wechat_order_id,
            wechat_status,
        )?;
    }

    // ===== 金额镜像（订单履约重设计 §3.4）=====
    let price_info = order.pointer("/order_detail/price_info");
    let order_price_cents = price_info.and_then(|p| json_value_to_i64(p.get("order_price")));
    // merchant_receieve_price 为官方文档原文拼写，勿纠正
    let merchant_receive_cents =
        price_info.and_then(|p| json_value_to_i64(p.get("merchant_receieve_price")));
    let freight_cents = price_info.and_then(|p| json_value_to_i64(p.get("freight")));
    let change_down_price = price_info
        .and_then(|p| json_value_to_i64(p.get("change_down_price")))
        .unwrap_or(0);
    let is_change_freight = price_info
        .and_then(|p| json_value_to_bool(p.get("is_change_freight")))
        .unwrap_or(false);

    let product_infos = order
        .pointer("/order_detail/product_infos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let any_item_change_price = product_infos
        .iter()
        .any(|p| json_value_to_bool(p.get("is_change_price")).unwrap_or(false));
    let is_change_price =
        (change_down_price > 0 || is_change_freight || any_item_change_price) as i64;

    // ===== 协商/履约镜像（改址申请、发货前换SKU、发货时效、备注、虚拟号）=====
    let delivery_info = order.pointer("/order_detail/delivery_info");
    // address_under_review 为对象：存在即表示有改址申请待商家审核（12h 不处理=自动同意，危险方向）
    let address_under_review = delivery_info
        .and_then(|d| d.get("address_under_review"))
        .map(|v| !v.is_null())
        .unwrap_or(false) as i64;
    let address_apply_time =
        delivery_info.and_then(|d| json_value_to_i64(d.get("address_apply_time")));
    let predict_delivery_time =
        delivery_info.and_then(|d| json_value_to_i64(d.get("predict_delivery_time")));
    let delivery_time_type =
        delivery_info.and_then(|d| json_value_to_i64(d.get("delivery_time_type")));
    let address_info = delivery_info.and_then(|d| d.get("address_info"));
    let use_tel_number = address_info.and_then(|a| json_value_to_i64(a.get("use_tel_number")));
    let virtual_tel_expire_time = address_info
        .and_then(|a| a.pointer("/tel_number_ext_info/virtual_tel_expire_time"))
        .and_then(|v| json_value_to_i64(Some(v)));
    // 虚拟号已延期次数（virtualnumber/delay 调用必须回传，防错配）
    let virtual_tel_delay_times = address_info
        .and_then(|a| a.pointer("/tel_number_ext_info/has_delay_times"))
        .and_then(|v| json_value_to_i64(Some(v)));

    // 换SKU 镜像：优先取「等待商家处理(3)」的申请（带处理截止时间），否则记录最近一次申请状态
    let mut change_sku_state: Option<i64> = None;
    let mut change_sku_ddl: Option<i64> = None;
    for product in &product_infos {
        if let Some(info) = product.get("change_sku_info").filter(|v| !v.is_null()) {
            let state = json_value_to_i64(info.get("preshipment_change_sku_state"));
            if state == Some(3) {
                change_sku_state = state;
                change_sku_ddl = json_value_to_i64(info.get("ddl_time_stamp"));
                break;
            }
            if change_sku_state.is_none() {
                change_sku_state = state;
            }
        }
    }
    // 发货时效：取各商品行最早的 delivery_deadline
    let delivery_deadline = product_infos
        .iter()
        .filter_map(|p| json_value_to_i64(p.get("delivery_deadline")))
        .min();
    let ext_info = order.pointer("/order_detail/ext_info");
    let merchant_notes = ext_info.and_then(|e| json_value_to_string(e.get("merchant_notes")));
    let customer_notes = ext_info.and_then(|e| json_value_to_string(e.get("customer_notes")));

    conn.execute(
        "UPDATE orders
         SET raw_payload = ?1,
             order_created_at = ?2,
             order_updated_at = ?3,
             order_price_cents = ?4,
             merchant_receive_cents = ?5,
             freight_cents = ?6,
             is_change_price = ?7,
             address_under_review = ?8,
             address_apply_time = ?9,
             change_sku_state = ?10,
             change_sku_ddl = ?11,
             delivery_deadline = ?12,
             predict_delivery_time = ?13,
             delivery_time_type = ?14,
             merchant_notes = ?15,
             customer_notes = ?16,
             use_tel_number = ?17,
             virtual_tel_expire_time = ?18,
             virtual_tel_delay_times = ?19,
             updated_at = ?20
         WHERE id = ?21",
        params![
            summary_payload.to_string(),
            order_created_at,
            order_updated_at,
            order_price_cents,
            merchant_receive_cents,
            freight_cents,
            is_change_price,
            address_under_review,
            address_apply_time,
            change_sku_state,
            change_sku_ddl,
            delivery_deadline,
            predict_delivery_time,
            delivery_time_type,
            merchant_notes,
            customer_notes,
            use_tel_number,
            virtual_tel_expire_time,
            virtual_tel_delay_times,
            now,
            item.order_id
        ],
    )?;

    // 包裹镜像（三期）：官方 delivery_product_info 数组 → order_packages/order_package_items
    if let Some(packages) = delivery_info
        .and_then(|d| d.get("delivery_product_info"))
        .and_then(Value::as_array)
    {
        mirror_wechat_packages(conn, item, packages, &now)?;
    }

    let saved_items = save_order_detail_items(conn, item, &product_infos, &now)?;

    // 审查修复：清脏标（detail_dirty=0 + detail_synced_at）放在包裹镜像与订单项
    // 全部落库成功「之后」——连接无事务，若中途出错/崩溃，旧实现已先标记「已同步」，
    // 半套快照会丢失 0/1 档重试触发器（终态单甚至永不回刷）。
    conn.execute(
        "UPDATE orders
         SET detail_dirty = 0, detail_synced_at = ?1, detail_error = NULL, updated_at = ?1
         WHERE id = ?2",
        params![now, item.order_id],
    )?;
    Ok(saved_items)
}

/// 把微信侧包裹物流镜像进 order_packages：本地提交的包裹（local_send）只更新发货时间与状态，
/// 官方后台/外部工具发的包裹以 wechat_mirror 入库（含包裹内商品行）。
fn mirror_wechat_packages(
    conn: &Connection,
    item: &OrderDetailSyncItem,
    packages: &[Value],
    now: &str,
) -> AppResult<()> {
    for package in packages {
        let delivery_id = json_value_to_string(package.get("delivery_id")).unwrap_or_default();
        let waybill_id = json_value_to_string(package.get("waybill_id")).unwrap_or_default();
        let delivery_name = json_value_to_string(package.get("delivery_name"));
        let deliver_type = json_value_to_i64(package.get("deliver_type")).unwrap_or(1);
        let delivery_time = json_value_to_i64(package.get("delivery_time"));
        // 身份键：有运单时 (delivery_id, waybill_id) 已唯一；无运单包裹（虚拟发货/无需物流）
        // 运单列全空，必须再用 deliver_type 区分，否则两个无运单包裹会互相吞并（审查修复）
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM order_packages
                 WHERE order_id = ?1 AND COALESCE(delivery_id, '') = ?2 AND COALESCE(waybill_id, '') = ?3
                   AND (?2 != '' OR ?3 != '' OR deliver_type = ?4)",
                params![item.order_id, delivery_id, waybill_id, deliver_type],
                |row| row.get(0),
            )
            .optional()?;
        let package_id = match existing {
            Some(package_id) => {
                conn.execute(
                    "UPDATE order_packages
                     SET status = 'confirmed', delivery_time = COALESCE(?1, delivery_time),
                         delivery_name = COALESCE(?2, delivery_name), updated_at = ?3
                     WHERE id = ?4",
                    params![delivery_time, delivery_name, now, package_id],
                )?;
                continue; // 已有包裹（本地提交）保留其商品映射
            }
            None => {
                let package_id = format!(
                    "package-{}-{}",
                    item.order_id,
                    Uuid::new_v4()
                );
                conn.execute(
                    "INSERT INTO order_packages
                     (id, order_id, shop_id, wechat_order_id, delivery_id, delivery_name, waybill_id,
                      deliver_type, source, status, delivery_time, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'wechat_mirror', 'confirmed', ?9, ?10, ?10)",
                    params![
                        package_id,
                        item.order_id,
                        item.shop_id,
                        item.wechat_order_id,
                        if delivery_id.is_empty() { None } else { Some(delivery_id.as_str()) },
                        delivery_name,
                        if waybill_id.is_empty() { None } else { Some(waybill_id.as_str()) },
                        deliver_type,
                        delivery_time,
                        now
                    ],
                )?;
                package_id
            }
        };
        if let Some(products) = package.get("product_infos").and_then(Value::as_array) {
            for product in products {
                let wechat_product_id = json_value_to_string(product.get("product_id"));
                let wechat_sku_id = json_value_to_string(product.get("sku_id"));
                let product_cnt = json_value_to_i64(product.get("product_cnt")).unwrap_or(1);
                let order_item_id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM order_items
                         WHERE order_id = ?1
                           AND COALESCE(wechat_product_id, '') = ?2
                           AND COALESCE(wechat_sku_id, '') = ?3
                         LIMIT 1",
                        params![
                            item.order_id,
                            wechat_product_id.clone().unwrap_or_default(),
                            wechat_sku_id.clone().unwrap_or_default()
                        ],
                        |row| row.get(0),
                    )
                    .optional()?;
                conn.execute(
                    "INSERT INTO order_package_items
                     (id, package_id, order_item_id, wechat_product_id, wechat_sku_id, product_cnt, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        format!("package-item-{}", Uuid::new_v4()),
                        package_id,
                        order_item_id,
                        wechat_product_id,
                        wechat_sku_id,
                        product_cnt,
                        now
                    ],
                )?;
            }
        }
    }
    Ok(())
}

/// 订单项落库：按稳定身份复用既有行 id（保住 purchase_tasks.order_item_id 外键），
/// 而非旧实现的「按数组下标拼 id」——换SKU/顺序变化时下标会错位串行。
/// 身份锚点优先级：① product_unique_id（官方注明下单后不变）② (wechat_product_id, wechat_sku_id)。
/// 人工补的 out_product_id/out_sku_id 映射只在微信侧给出非空值时才覆盖，回刷不得清掉人工映射。
fn save_order_detail_items(
    conn: &Connection,
    item: &OrderDetailSyncItem,
    product_infos: &[Value],
    now: &str,
) -> AppResult<i64> {
    let mut claimed: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut saved_items = 0i64;
    for (index, product) in product_infos.iter().enumerate() {
        let wechat_product_id = json_value_to_string(product.get("product_id"));
        let wechat_sku_id = json_value_to_string(product.get("sku_id"));
        let product_unique_id = json_value_to_string(product.get("product_unique_id"));
        let out_product_id = product
            .get("out_product_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let out_sku_id = product
            .get("out_sku_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let title = product
            .get("title")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let sku_count = product
            .get("sku_cnt")
            .or_else(|| product.get("sku_count"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let sale_price = product.get("sale_price").and_then(Value::as_i64);
        // 官方定义：real_price = min(estimate_price, change_price)，改价后金额以此为准
        let real_price = product.get("real_price").and_then(Value::as_i64);
        let estimate_price = product.get("estimate_price").and_then(Value::as_i64);
        let change_price = product.get("change_price").and_then(Value::as_i64);
        let raw_payload = sanitize_order_item_payload(product).to_string();

        // ① product_unique_id 精确锚定
        let mut existing_id: Option<String> = None;
        if let Some(unique_id) = &product_unique_id {
            existing_id = conn
                .query_row(
                    "SELECT id FROM order_items
                     WHERE order_id = ?1 AND product_unique_id = ?2
                     LIMIT 1",
                    params![item.order_id, unique_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .filter(|id| !claimed.contains(id));
        }
        // ② 微信商品身份回退（兼容 product_unique_id 列为空的历史行）
        if existing_id.is_none() {
            let mut stmt = conn.prepare(
                "SELECT id FROM order_items
                 WHERE order_id = ?1
                   AND COALESCE(wechat_product_id, '') = ?2
                   AND COALESCE(wechat_sku_id, '') = ?3
                 ORDER BY created_at ASC",
            )?;
            let candidates = stmt
                .query_map(
                    params![
                        item.order_id,
                        wechat_product_id.clone().unwrap_or_default(),
                        wechat_sku_id.clone().unwrap_or_default()
                    ],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<String>, _>>()?;
            existing_id = candidates.into_iter().find(|id| !claimed.contains(id));
        }

        match existing_id {
            Some(existing_id) => {
                conn.execute(
                    "UPDATE order_items
                     SET title = ?1,
                         sku_count = ?2,
                         sale_price = ?3,
                         real_price = ?4,
                         estimate_price = ?5,
                         change_price = ?6,
                         product_unique_id = COALESCE(?7, product_unique_id),
                         out_product_id = COALESCE(?8, out_product_id),
                         out_sku_id = COALESCE(?9, out_sku_id),
                         raw_payload = ?10,
                         updated_at = ?11
                     WHERE id = ?12",
                    params![
                        title,
                        sku_count,
                        sale_price,
                        real_price,
                        estimate_price,
                        change_price,
                        product_unique_id,
                        out_product_id,
                        out_sku_id,
                        raw_payload,
                        now,
                        existing_id
                    ],
                )?;
                claimed.insert(existing_id);
            }
            None => {
                // 新行：优先沿用历史 id 形态（order-item-{order}-{index}），被占用则退 uuid
                let mut item_id = format!("order-item-{}-{}", item.order_id, index);
                let id_taken = claimed.contains(&item_id)
                    || conn
                        .query_row(
                            "SELECT 1 FROM order_items WHERE id = ?1",
                            params![item_id],
                            |_| Ok(()),
                        )
                        .optional()?
                        .is_some();
                if id_taken {
                    item_id = format!("order-item-{}-{}", item.order_id, Uuid::new_v4());
                }
                conn.execute(
                    "INSERT INTO order_items
                     (id, order_id, shop_id, wechat_order_id, wechat_product_id, wechat_sku_id,
                      out_product_id, out_sku_id, title, sku_count, sale_price, real_price,
                      estimate_price, change_price, product_unique_id,
                      raw_payload, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)",
                    params![
                        item_id,
                        item.order_id,
                        item.shop_id,
                        item.wechat_order_id,
                        wechat_product_id,
                        wechat_sku_id,
                        out_product_id,
                        out_sku_id,
                        title,
                        sku_count,
                        sale_price,
                        real_price,
                        estimate_price,
                        change_price,
                        product_unique_id,
                        raw_payload,
                        now
                    ],
                )?;
                claimed.insert(item_id);
            }
        }
        saved_items += 1;
    }
    Ok(saved_items)
}

pub(in crate::commands) fn save_failed_aftersale(
    conn: &Connection,
    shop_id: &str,
    wechat_aftersale_id: &str,
    reason: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO aftersales
         (id, shop_id, wechat_aftersale_id, status, reason, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'sync_failed', ?4, ?5, ?6, ?6)
         ON CONFLICT(shop_id, wechat_aftersale_id) DO UPDATE SET
           status = excluded.status,
           reason = excluded.reason,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("aftersale-{shop_id}-{wechat_aftersale_id}"),
            shop_id,
            wechat_aftersale_id,
            reason,
            serde_json::json!({
                "after_sale_order_id": wechat_aftersale_id,
                "error": reason
            })
            .to_string(),
            now
        ],
    )?;
    upsert_notification(
        conn,
        "critical",
        "aftersale",
        wechat_aftersale_id,
        Some(shop_id),
        "售后详情同步失败",
        &format!("售后单 {wechat_aftersale_id} 同步失败：{reason}"),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "wechat_aftersale_id": wechat_aftersale_id,
            "reason": reason
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn save_synced_aftersale(
    conn: &Connection,
    shop_id: &str,
    after_sale_order: &Value,
) -> AppResult<()> {
    let wechat_aftersale_id = json_value_to_string(after_sale_order.get("after_sale_order_id"))
        .or_else(|| json_value_to_string(after_sale_order.get("aftersale_order_id")))
        .ok_or_else(|| AppError::Validation("售后详情缺少 after_sale_order_id".to_string()))?;
    let status = json_value_to_string(after_sale_order.get("status"))
        .unwrap_or_else(|| "unknown".to_string());
    let aftersale_type = json_value_to_string(after_sale_order.get("type"))
        .or_else(|| json_value_to_string(after_sale_order.get("after_sale_type")));
    let reason = json_value_to_string(after_sale_order.get("reason_text"))
        .or_else(|| json_value_to_string(after_sale_order.get("reason")));
    let wechat_order_id = json_value_to_string(after_sale_order.get("order_id"));
    let local_order_id = match &wechat_order_id {
        Some(wechat_order_id) => conn
            .query_row(
                "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2 LIMIT 1",
                params![shop_id, wechat_order_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let refund_amount_cents = extract_refund_amount_cents(after_sale_order);
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO aftersales
         (id, shop_id, wechat_aftersale_id, order_id, wechat_order_id, status,
          aftersale_type, reason, refund_amount_cents, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(shop_id, wechat_aftersale_id) DO UPDATE SET
           order_id = excluded.order_id,
           wechat_order_id = excluded.wechat_order_id,
           status = excluded.status,
           aftersale_type = excluded.aftersale_type,
           reason = excluded.reason,
           refund_amount_cents = excluded.refund_amount_cents,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("aftersale-{shop_id}-{wechat_aftersale_id}"),
            shop_id,
            wechat_aftersale_id,
            local_order_id,
            wechat_order_id,
            status,
            aftersale_type,
            reason,
            refund_amount_cents,
            sanitize_aftersale_payload(after_sale_order).to_string(),
            now
        ],
    )?;

    if is_active_aftersale_status(&status) {
        upsert_notification(
            conn,
            "warning",
            "aftersale",
            &wechat_aftersale_id,
            Some(shop_id),
            "微信售后待处理",
            &format!(
                "售后单 {} 当前状态为 {}，需人工判断是否同意、拒绝或补充凭证。",
                wechat_aftersale_id, status
            ),
            Some(&serde_json::json!({
                "shop_id": shop_id,
                "wechat_aftersale_id": &wechat_aftersale_id,
                "wechat_order_id": &wechat_order_id,
                "status": &status,
                "reason": &reason
            })),
        )?;
    }

    let local_order_id: Option<String> = conn
        .query_row(
            "SELECT order_id FROM aftersales WHERE shop_id = ?1 AND wechat_aftersale_id = ?2",
            params![shop_id, wechat_aftersale_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if let Some(order_id) = local_order_id {
        // 售后不再改写履约主状态（旧实现写 status='aftersale_active' 会吞掉履约位置），
        // 改为重算横向标志位，发货守卫与队列均读该标志。
        recompute_order_aftersale_flag(conn, &order_id)?;
        if is_refund_success_aftersale_status(&status) {
            if let Some(amount_cents) = refund_amount_cents.filter(|value| *value > 0) {
                conn.execute(
                    "INSERT INTO order_profit_adjustments
                     (id, order_id, kind, amount_cents, note, created_at)
                     VALUES (?1, ?2, 'refund', ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET
                       amount_cents = excluded.amount_cents,
                       note = excluded.note,
                       created_at = excluded.created_at",
                    params![
                        format!("aftersale-refund-{shop_id}-{wechat_aftersale_id}"),
                        order_id,
                        amount_cents,
                        format!("微信售后退款：{wechat_aftersale_id}"),
                        now_shanghai()
                    ],
                )?;
            }
        }
    }
    Ok(())
}

pub(in crate::commands) fn save_failed_guarantee_order(
    conn: &Connection,
    shop_id: &str,
    guarantee_order_id: &str,
    reason: &str,
) -> AppResult<()> {
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO guarantee_orders
         (id, shop_id, guarantee_order_id, status, apply_reason, raw_payload, synced_at, updated_at)
         VALUES (?1, ?2, ?3, 'sync_failed', ?4, ?5, ?6, ?6)
         ON CONFLICT(shop_id, guarantee_order_id) DO UPDATE SET
           status = excluded.status,
           apply_reason = excluded.apply_reason,
           raw_payload = excluded.raw_payload,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("guarantee-{shop_id}-{guarantee_order_id}"),
            shop_id,
            guarantee_order_id,
            reason,
            serde_json::json!({
                "guarantee_order_id": guarantee_order_id,
                "error": reason
            })
            .to_string(),
            now
        ],
    )?;
    upsert_notification(
        conn,
        "critical",
        "guarantee_sync",
        guarantee_order_id,
        Some(shop_id),
        "纠纷单详情同步失败",
        &format!("纠纷单 {guarantee_order_id} 同步失败：{reason}"),
        Some(&serde_json::json!({
            "shop_id": shop_id,
            "guarantee_order_id": guarantee_order_id,
            "reason": reason
        })),
    )?;
    Ok(())
}

pub(in crate::commands) fn save_synced_guarantee_order(
    conn: &Connection,
    shop_id: &str,
    guarantee_order: &Value,
) -> AppResult<()> {
    let guarantee_order_id = json_value_to_string(guarantee_order.get("guarantee_order_id"))
        .ok_or_else(|| AppError::Validation("纠纷单详情缺少 guarantee_order_id".to_string()))?;
    let status = json_value_to_string(guarantee_order.get("status"))
        .unwrap_or_else(|| "unknown".to_string());
    let guarantee_type = json_value_to_i64(guarantee_order.get("type"));
    let wechat_order_id = json_value_to_string(guarantee_order.get("order_id"));
    let local_order_id = match &wechat_order_id {
        Some(wechat_order_id) => conn
            .query_row(
                "SELECT id FROM orders WHERE shop_id = ?1 AND wechat_order_id = ?2 LIMIT 1",
                params![shop_id, wechat_order_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let apply_reason = json_value_to_string(guarantee_order.get("apply_reason"))
        .or_else(|| json_value_to_string(guarantee_order.get("refund_reason_text")));
    let pay_amount_cents = json_value_to_i64(guarantee_order.get("pay_amount"))
        .or_else(|| json_value_to_i64(guarantee_order.pointer("/bad_pay_info/pay_fee")))
        .or_else(|| {
            json_value_to_i64(guarantee_order.pointer("/fake_one_pay_four_info/total_pay_fee"))
        });
    let merchant_refuse_reason =
        json_value_to_string(guarantee_order.get("merchant_refuse_reason"));
    let created_time = json_value_to_i64(guarantee_order.get("create_time"));
    let updated_time = json_value_to_i64(guarantee_order.get("update_time"));
    let expire_time = json_value_to_i64(guarantee_order.get("expire_time"));
    let complete_time = json_value_to_i64(guarantee_order.get("complete_time"));
    let now = now_shanghai();
    conn.execute(
        "INSERT INTO guarantee_orders
         (id, shop_id, guarantee_order_id, order_id, wechat_order_id, guarantee_type,
          status, apply_reason, pay_amount_cents, merchant_refuse_reason, raw_payload,
          created_time, updated_time, expire_time, complete_time, synced_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)
         ON CONFLICT(shop_id, guarantee_order_id) DO UPDATE SET
           order_id = excluded.order_id,
           wechat_order_id = excluded.wechat_order_id,
           guarantee_type = excluded.guarantee_type,
           status = excluded.status,
           apply_reason = excluded.apply_reason,
           pay_amount_cents = excluded.pay_amount_cents,
           merchant_refuse_reason = excluded.merchant_refuse_reason,
           raw_payload = excluded.raw_payload,
           created_time = excluded.created_time,
           updated_time = excluded.updated_time,
           expire_time = excluded.expire_time,
           complete_time = excluded.complete_time,
           synced_at = excluded.synced_at,
           updated_at = excluded.updated_at",
        params![
            format!("guarantee-{shop_id}-{guarantee_order_id}"),
            shop_id,
            guarantee_order_id,
            local_order_id,
            wechat_order_id,
            guarantee_type,
            status,
            apply_reason,
            pay_amount_cents,
            merchant_refuse_reason,
            sanitize_aftersale_payload(guarantee_order).to_string(),
            created_time,
            updated_time,
            expire_time,
            complete_time,
            now
        ],
    )?;

    if is_active_guarantee_status(&status) {
        upsert_notification(
            conn,
            "critical",
            "guarantee_order",
            &guarantee_order_id,
            Some(shop_id),
            "微信纠纷单待处理",
            &format!(
                "纠纷单 {} 当前状态为 {}（{}），需人工核对凭证、责任方和处理时限。",
                guarantee_order_id,
                status,
                guarantee_status_text(&status)
            ),
            Some(&serde_json::json!({
                "shop_id": shop_id,
                "guarantee_order_id": &guarantee_order_id,
                "wechat_order_id": &wechat_order_id,
                "status": &status,
                "status_text": guarantee_status_text(&status),
                "apply_reason": &apply_reason,
                "pay_amount_cents": pay_amount_cents
            })),
        )?;
    }

    if let Some(order_id) = conn
        .query_row(
            "SELECT order_id FROM guarantee_orders WHERE shop_id = ?1 AND guarantee_order_id = ?2",
            params![shop_id, guarantee_order_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
    {
        // 纠纷同样只维护标志位，不吞履约主状态
        recompute_order_aftersale_flag(conn, &order_id)?;
    }
    Ok(())
}

/// 微信官方订单状态 → 本地履约状态映射。官方枚举：10 待付款 / 12 礼物待收下 / 13 待成团 /
/// 20 待发货 / 21 部分发货 / 30 待收货 / 100 完成 / 200 全部售后取消（仅 order/get 返回）/ 250 取消。
pub(in crate::commands) fn order_status_from_wechat(status: i64) -> &'static str {
    match status {
        10 | 12 | 13 => "unpaid",
        20 => "pending_shipment",
        21 => "partially_shipped",
        30 => "wechat_shipped",
        100 => "completed",
        200 | 250 => "cancelled",
        _ => "synced",
    }
}

/// 重算订单的「售后/纠纷活跃」标志位（订单履约重设计 §2.1）：售后不再吞掉履约主状态，
/// 只维护横向标志列 has_active_aftersale；售后与纠纷全部终结后自动回 0。
/// 注意：终结状态清单必须与 is_active_aftersale_status / is_active_guarantee_status 保持一致。
pub(in crate::commands) fn recompute_order_aftersale_flag(
    conn: &Connection,
    order_id: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE orders
         SET has_active_aftersale = (
           CASE WHEN EXISTS (
             SELECT 1 FROM aftersales a
             WHERE a.order_id = orders.id
               AND UPPER(a.status) NOT IN (
                 'USER_CANCELD', 'USER_CANCELLED', 'RETURN_CLOSED',
                 'MERCHANT_REFUND_SUCCESS', 'MERCHANT_RETURN_SUCCESS',
                 'MERCHANT_REFUND_RETRY_FAIL', 'MERCHANT_FAIL',
                 'MERCHANT_EXCHANGE_SUCCESS', 'SYNC_FAILED'
               )
           ) OR EXISTS (
             SELECT 1 FROM guarantee_orders g
             WHERE g.order_id = orders.id
               AND UPPER(g.status) NOT IN (
                 'STATUS_NO_NEED_PAY', 'STATUS_PAY_SUCC', 'STATUS_USER_CANCEL', 'SYNC_FAILED'
               )
           ) THEN 1 ELSE 0 END
         ),
         updated_at = ?1
         WHERE id = ?2",
        params![now_shanghai(), order_id],
    )?;
    Ok(())
}

pub(in crate::commands) fn sanitize_order_payload(order: &Value) -> Value {
    serde_json::json!({
        "order_id": json_value_to_string(order.get("order_id")),
        "status": order.get("status").and_then(Value::as_i64),
        "create_time": order.get("create_time").and_then(Value::as_i64),
        "update_time": order.get("update_time").and_then(Value::as_i64),
        "product_infos": order
            .pointer("/order_detail/product_infos")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(sanitize_order_item_payload).collect::<Vec<_>>())
            .unwrap_or_default()
    })
}

pub(in crate::commands) fn sanitize_order_item_payload(product: &Value) -> Value {
    serde_json::json!({
        "product_id": product.get("product_id").cloned().unwrap_or(Value::Null),
        "sku_id": product.get("sku_id").cloned().unwrap_or(Value::Null),
        "out_product_id": product.get("out_product_id").cloned().unwrap_or(Value::Null),
        "out_sku_id": product.get("out_sku_id").cloned().unwrap_or(Value::Null),
        "title": product.get("title").cloned().unwrap_or(Value::Null),
        "sku_cnt": product.get("sku_cnt").cloned().unwrap_or(Value::Null),
        "sale_price": product.get("sale_price").cloned().unwrap_or(Value::Null),
        "real_price": product.get("real_price").cloned().unwrap_or(Value::Null),
        "thumb_img": product.get("thumb_img").cloned().unwrap_or(Value::Null)
    })
}

pub(in crate::commands) fn sanitize_aftersale_payload(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (key, child) in map {
                if is_sensitive_json_key(key) {
                    sanitized.insert(key.clone(), Value::String("[redacted]".to_string()));
                } else {
                    sanitized.insert(key.clone(), sanitize_aftersale_payload(child));
                }
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(items.iter().map(sanitize_aftersale_payload).collect()),
        _ => value.clone(),
    }
}

pub(in crate::commands) fn is_sensitive_json_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "openid", "open_id", "unionid", "name", "tel", "mobile", "phone", "address", "receiver",
        "contact",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(in crate::commands) fn json_value_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

pub(in crate::commands) fn json_value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(value)) => value.as_i64(),
        Some(Value::String(value)) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub(in crate::commands) fn json_value_to_bool(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Number(value)) => value.as_i64().map(|value| value != 0),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(in crate::commands) fn json_value_to_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(in crate::commands) fn extract_refund_amount_cents(after_sale_order: &Value) -> Option<i64> {
    [
        "/refund_info/refund_fee",
        "/refund_info/refund_amount",
        "/refund_info/refund_price",
        "/refund_resp/refund_fee",
        "/refund_resp/refund_amount",
        "/refund_amount",
        "/refund_fee",
    ]
    .iter()
    .find_map(|path| json_value_to_i64(after_sale_order.pointer(path)))
    .or_else(|| {
        after_sale_order
            .pointer("/refund_info/refund_product_infos")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        json_value_to_i64(item.get("refund_amount"))
                            .or_else(|| json_value_to_i64(item.get("refund_fee")))
                    })
                    .sum::<i64>()
            })
            .filter(|sum| *sum > 0)
    })
}

pub(in crate::commands) fn is_active_aftersale_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_uppercase().as_str(),
        "USER_CANCELD"
            | "USER_CANCELLED"
            | "RETURN_CLOSED"
            | "MERCHANT_REFUND_SUCCESS"
            | "MERCHANT_RETURN_SUCCESS"
            | "MERCHANT_REFUND_RETRY_FAIL"
            | "MERCHANT_FAIL"
            | "MERCHANT_EXCHANGE_SUCCESS"
            | "SYNC_FAILED"
    )
}

pub(in crate::commands) fn is_refund_success_aftersale_status(status: &str) -> bool {
    matches!(
        status.to_ascii_uppercase().as_str(),
        "MERCHANT_REFUND_SUCCESS" | "MERCHANT_RETURN_SUCCESS"
    )
}

pub(in crate::commands) fn is_active_guarantee_status(status: &str) -> bool {
    !matches!(
        status.to_ascii_uppercase().as_str(),
        "STATUS_NO_NEED_PAY" | "STATUS_PAY_SUCC" | "STATUS_USER_CANCEL" | "SYNC_FAILED"
    )
}

pub(in crate::commands) fn guarantee_type_text(guarantee_type: Option<i64>) -> &'static str {
    match guarantee_type {
        Some(1) => "假一赔三/四",
        Some(2) => "坏损包退",
        Some(0) => "全部类型",
        _ => "未知类型",
    }
}

pub(in crate::commands) fn guarantee_status_text(status: &str) -> &'static str {
    match status.to_ascii_uppercase().as_str() {
        "STATUS_WAIT_MERCHANT_HANDLE" => "等待商家处理",
        "STATUS_WAIT_PLATFORM_HANDLE" => "等待平台处理",
        "STATUS_WAIT_USER_CONFIRM" => "等待用户确认",
        "STATUS_WAIT_MERCHANT_PROOF" => "等待商家举证",
        "STATUS_WAIT_USER_PROOF" => "等待用户举证",
        "STATUS_WAIT_BOTH_PROOF" => "等待双方举证",
        "STATUS_WAIT_OP_COMFIRM" => "等待 OP 确认",
        "STATUS_WAIT_PAYSCORE_DONE" => "等待支付分付款",
        "STATUS_NO_NEED_PAY" => "无需赔付",
        "STATUS_PAYING" => "赔付中",
        "STATUS_PAY_BLOCK" => "赔付金额异常待确认",
        "STATUS_PAY_SUCC" => "赔付成功",
        "STATUS_PAY_FAIL" => "赔付失败",
        "STATUS_USER_CANCEL" => "用户取消申请",
        "SYNC_FAILED" => "同步失败",
        _ => "未知状态",
    }
}

pub(in crate::commands) fn normalize_guarantee_handling_status(
    status: &str,
) -> AppResult<&'static str> {
    match status.trim() {
        "pending" => Ok("pending"),
        "in_progress" => Ok("in_progress"),
        "waiting_supplier" => Ok("waiting_supplier"),
        "evidence_ready" => Ok("evidence_ready"),
        "resolved" => Ok("resolved"),
        "ignored" => Ok("ignored"),
        _ => Err(AppError::Validation(
            "纠纷跟进状态必须是 pending/in_progress/waiting_supplier/evidence_ready/resolved/ignored"
                .to_string(),
        )),
    }
}

pub(in crate::commands) fn guarantee_handling_status_text(status: &str) -> &'static str {
    match status {
        "pending" => "待跟进",
        "in_progress" => "跟进中",
        "waiting_supplier" => "等供应商",
        "evidence_ready" => "凭证已整理",
        "resolved" => "已处理",
        "ignored" => "无需处理",
        _ => "未知跟进状态",
    }
}

pub(in crate::commands) fn aftersale_evidence_type_text(evidence_type: &str) -> &'static str {
    match evidence_type {
        "image" => "图片",
        "text" => "文字说明",
        "chat_record" => "沟通记录",
        "logistics" => "物流凭证",
        "supplier_proof" => "供应商凭证",
        "quality_check" => "质检凭证",
        "other" => "其他",
        _ => "未知凭证",
    }
}

pub(in crate::commands) fn aftersale_evidence_status_text(status: &str) -> &'static str {
    match status {
        "draft" => "草稿",
        "ready" => "已整理",
        "used" => "已使用",
        "archived" => "已归档",
        _ => "未知状态",
    }
}

pub(in crate::commands) fn supplier_aftersale_followup_type_text(
    followup_type: &str,
) -> &'static str {
    match followup_type {
        "contact" => "联系供应商",
        "evidence_request" => "索要凭证",
        "evidence_received" => "收到凭证",
        "compensation" => "赔付沟通",
        "return_refund" => "退货退款",
        "other" => "其他协同",
        _ => "未知协同",
    }
}

pub(in crate::commands) fn supplier_aftersale_followup_status_text(status: &str) -> &'static str {
    match status {
        "pending" => "待处理",
        "contacted" => "已联系",
        "waiting_supplier" => "等供应商",
        "evidence_ready" => "凭证已备",
        "compensation_pending" => "赔付待确认",
        "closed" => "已关闭",
        _ => "未知状态",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("内存库");
        // 与生产 open_connection 保持一致：外键暂关（遗留表外键指向已废弃旧表）
        conn.pragma_update(None, "foreign_keys", "OFF").expect("关闭外键");
        crate::storage::migrate(&conn).expect("迁移");
        conn
    }

    fn order_status(conn: &Connection, order_id: &str) -> String {
        conn.query_row(
            "SELECT status FROM orders WHERE id = ?1",
            params![order_id],
            |row| row.get(0),
        )
        .expect("查询订单状态")
    }

    fn seed_order(conn: &Connection) -> String {
        save_synced_order(conn, "shop-1", "wx-order-1").expect("落库");
        "order-shop-1-wx-order-1".to_string()
    }

    #[test]
    fn wechat_package_mirror_confirms_local_and_creates_external() {
        let conn = test_conn();
        let order_id = seed_order(&conn);
        let now = now_shanghai();
        // 本地已提交的包裹（local_send/submitted）+ 自己的商品映射
        conn.execute(
            "INSERT INTO order_packages
             (id, order_id, shop_id, wechat_order_id, delivery_id, waybill_id,
              deliver_type, source, status, created_at, updated_at)
             VALUES ('pkg-local', ?1, 'shop-1', 'wx-order-1', 'SF', 'SF001', 1,
                     'local_send', 'submitted', ?2, ?2)",
            params![order_id, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO order_package_items
             (id, package_id, wechat_product_id, wechat_sku_id, product_cnt, created_at)
             VALUES ('pkg-local-item', 'pkg-local', 'wxp-1', 'wxs-1', 1, ?1)",
            params![now],
        )
        .unwrap();

        let item = OrderDetailSyncItem {
            order_id: order_id.clone(),
            shop_id: "shop-1".to_string(),
            wechat_order_id: "wx-order-1".to_string(),
        };
        // 官方镜像：同运单包裹（应确认本地行）+ 外部工具发的新包裹（应建 wechat_mirror）
        let packages = vec![
            serde_json::json!({
                "delivery_id": "SF",
                "waybill_id": "SF001",
                "deliver_type": 1,
                "delivery_time": 1_770_000_000,
                "product_infos": [{ "product_id": "wxp-1", "sku_id": "wxs-1", "product_cnt": 1 }]
            }),
            serde_json::json!({
                "delivery_id": "YTO",
                "waybill_id": "YT999",
                "deliver_type": 1,
                "product_infos": [{ "product_id": "wxp-2", "sku_id": "wxs-2", "product_cnt": 3 }]
            }),
        ];
        mirror_wechat_packages(&conn, &item, &packages, &now).expect("镜像");

        // 本地包裹：状态确认 + 保留原 source 与商品映射（不重复插行）
        let (local_source, local_status, local_time): (String, String, Option<i64>) = conn
            .query_row(
                "SELECT source, status, delivery_time FROM order_packages WHERE id = 'pkg-local'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(local_source, "local_send");
        assert_eq!(local_status, "confirmed");
        assert_eq!(local_time, Some(1_770_000_000));
        let local_item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM order_package_items WHERE package_id = 'pkg-local'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(local_item_count, 1, "本地包裹商品映射不得被镜像重建");

        // 外部包裹：新建 wechat_mirror 行 + 商品行
        let (mirror_source, mirror_cnt): (String, i64) = conn
            .query_row(
                "SELECT op.source, opi.product_cnt
                 FROM order_packages op
                 JOIN order_package_items opi ON opi.package_id = op.id
                 WHERE op.waybill_id = 'YT999'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(mirror_source, "wechat_mirror");
        assert_eq!(mirror_cnt, 3);

        // 幂等：重复镜像不产生重复包裹
        mirror_wechat_packages(&conn, &item, &packages, &now).expect("重复镜像");
        let package_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM order_packages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(package_count, 2);
    }

    #[test]
    fn list_sync_keeps_business_status_and_marks_dirty() {
        let conn = test_conn();
        let order_id = seed_order(&conn);
        assert_eq!(order_status(&conn, &order_id), "synced");
        // 模拟业务侧推进到采购中后清掉脏标
        conn.execute(
            "UPDATE orders SET status = 'pending_purchase', detail_dirty = 0 WHERE id = ?1",
            params![order_id],
        )
        .unwrap();
        // 重跑列表同步：旧实现会把状态打回 pending_shipment，新实现只置脏标
        save_synced_order(&conn, "shop-1", "wx-order-1").unwrap();
        assert_eq!(order_status(&conn, &order_id), "pending_purchase");
        let dirty: i64 = conn
            .query_row(
                "SELECT detail_dirty FROM orders WHERE id = ?1",
                params![order_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dirty, 1);
    }

    #[test]
    fn guard_is_monotonic_and_allows_completed_to_cancelled() {
        let conn = test_conn();
        let order_id = seed_order(&conn);
        let apply = |status: i64| {
            apply_wechat_status(&conn, &order_id, "shop-1", "wx-order-1", status).unwrap()
        };
        apply(20);
        assert_eq!(order_status(&conn, &order_id), "pending_shipment");
        // 业务侧推进到采购中后，20（待发货）映射不得回退
        conn.execute(
            "UPDATE orders SET status = 'pending_purchase' WHERE id = ?1",
            params![order_id],
        )
        .unwrap();
        apply(20);
        assert_eq!(order_status(&conn, &order_id), "pending_purchase");
        // 微信侧已发货：外部发货回流（前进）
        apply(30);
        assert_eq!(order_status(&conn, &order_id), "wechat_shipped");
        let external_notice: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE source_type = 'order_external_shipped'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(external_notice, 1, "外部发货应发 warning 通知");
        apply(100);
        assert_eq!(order_status(&conn, &order_id), "completed");
        // 官方 200（全部售后取消）可发生在完成之后：completed→cancelled 必须放行
        apply(200);
        assert_eq!(order_status(&conn, &order_id), "cancelled");
        // 终态冻结：任何映射不得再改
        apply(20);
        assert_eq!(order_status(&conn, &order_id), "cancelled");
        // 镜像列始终无条件跟随
        let mirror: i64 = conn
            .query_row(
                "SELECT wechat_status FROM orders WHERE id = ?1",
                params![order_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mirror, 20);
    }

    #[test]
    fn cancelled_hook_cancels_open_purchase_tasks() {
        let conn = test_conn();
        let order_id = seed_order(&conn);
        conn.execute(
            "INSERT INTO order_items
             (id, order_id, shop_id, wechat_order_id, sku_count, raw_payload, created_at, updated_at)
             VALUES ('item-1', ?1, 'shop-1', 'wx-order-1', 1, '{}', '2026-06-10T00:00:00+08:00', '2026-06-10T00:00:00+08:00')",
            params![order_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO purchase_tasks
             (id, order_id, order_item_id, shop_id, status, quantity, created_at, updated_at)
             VALUES ('task-1', ?1, 'item-1', 'shop-1', 'pending_purchase', 1,
                     '2026-06-10T00:00:00+08:00', '2026-06-10T00:00:00+08:00')",
            params![order_id],
        )
        .unwrap();
        apply_wechat_status(&conn, &order_id, "shop-1", "wx-order-1", 250).unwrap();
        assert_eq!(order_status(&conn, &order_id), "cancelled");
        let task_status: String = conn
            .query_row(
                "SELECT status FROM purchase_tasks WHERE id = 'task-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(task_status, "cancelled", "采购任务应联动取消");
        let notice: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE source_type = 'order_cancelled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(notice, 1, "应发 critical 通知提醒上游拦截");
    }

    #[test]
    fn aftersale_flag_recomputes_without_eating_status() {
        let conn = test_conn();
        let order_id = seed_order(&conn);
        conn.execute(
            "UPDATE orders SET status = 'supplier_shipped' WHERE id = ?1",
            params![order_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO aftersales
             (id, shop_id, wechat_aftersale_id, order_id, status, raw_payload, synced_at, updated_at)
             VALUES ('as-1', 'shop-1', 'wx-as-1', ?1, 'WAIT_MERCHANT_HANDLE', '{}',
                     '2026-06-10T00:00:00+08:00', '2026-06-10T00:00:00+08:00')",
            params![order_id],
        )
        .unwrap();
        recompute_order_aftersale_flag(&conn, &order_id).unwrap();
        let flag: i64 = conn
            .query_row(
                "SELECT has_active_aftersale FROM orders WHERE id = ?1",
                params![order_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(flag, 1, "活跃售后应置标志位");
        assert_eq!(
            order_status(&conn, &order_id),
            "supplier_shipped",
            "履约主状态不得被售后吞掉"
        );
        conn.execute(
            "UPDATE aftersales SET status = 'MERCHANT_REFUND_SUCCESS' WHERE id = 'as-1'",
            [],
        )
        .unwrap();
        recompute_order_aftersale_flag(&conn, &order_id).unwrap();
        let flag: i64 = conn
            .query_row(
                "SELECT has_active_aftersale FROM orders WHERE id = ?1",
                params![order_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(flag, 0, "售后终结后标志位应回 0");
    }

    #[test]
    fn order_detail_reuses_item_ids_and_keeps_manual_mapping() {
        let conn = test_conn();
        let order_id = seed_order(&conn);
        let item = OrderDetailSyncItem {
            order_id: order_id.clone(),
            shop_id: "shop-1".to_string(),
            wechat_order_id: "wx-order-1".to_string(),
        };
        let order_payload = serde_json::json!({
            "order_id": "wx-order-1",
            "status": 20,
            "create_time": 1_780_000_000,
            "update_time": 1_780_000_100,
            "order_detail": {
                "price_info": {
                    "order_price": 5000,
                    "freight": 600,
                    "merchant_receieve_price": 4400,
                    "change_down_price": 0
                },
                "product_infos": [
                    {
                        "product_id": 111, "sku_id": 222,
                        "product_unique_id": "uniq-1",
                        "title": "测试商品",
                        "sku_cnt": 2, "sale_price": 2500,
                        "real_price": 5000, "estimate_price": 5000
                    }
                ],
                "delivery_info": {
                    "address_apply_time": 0,
                    "predict_delivery_time": 1_780_100_000,
                    "delivery_time_type": 1,
                    "address_info": { "use_tel_number": 1 }
                },
                "ext_info": { "merchant_notes": "备注A", "customer_notes": "买家留言" }
            }
        });
        let saved = save_order_detail(&conn, &item, &order_payload).unwrap();
        assert_eq!(saved, 1);
        let (first_id, out_before): (String, Option<String>) = conn
            .query_row(
                "SELECT id, out_product_id FROM order_items WHERE order_id = ?1",
                params![order_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(out_before.is_none());
        // 人工补映射（微信侧 payload 不含 out id）
        conn.execute(
            "UPDATE order_items SET out_product_id = 'manual-out', out_sku_id = 'manual-sku' WHERE id = ?1",
            params![first_id],
        )
        .unwrap();
        // 改价后回刷：real_price 变小，订单级金额跟随
        let mut changed = order_payload.clone();
        changed["order_detail"]["price_info"]["order_price"] = serde_json::json!(4000);
        changed["order_detail"]["price_info"]["change_down_price"] = serde_json::json!(1000);
        changed["order_detail"]["product_infos"][0]["real_price"] = serde_json::json!(4000);
        changed["order_detail"]["product_infos"][0]["is_change_price"] = serde_json::json!(true);
        changed["order_detail"]["product_infos"][0]["change_price"] = serde_json::json!(4000);
        let saved = save_order_detail(&conn, &item, &changed).unwrap();
        assert_eq!(saved, 1);
        let item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM order_items WHERE order_id = ?1",
                params![order_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(item_count, 1, "回刷必须复用既有行，不得新增重复行");
        let (id_after, out_after, real_after): (String, Option<String>, Option<i64>) = conn
            .query_row(
                "SELECT id, out_product_id, real_price FROM order_items WHERE order_id = ?1",
                params![order_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(id_after, first_id, "行 id 必须稳定（保住采购任务外键）");
        assert_eq!(out_after.as_deref(), Some("manual-out"), "人工映射不得被回刷清掉");
        assert_eq!(real_after, Some(4000));
        let (price, change_flag, dirty): (Option<i64>, Option<i64>, i64) = conn
            .query_row(
                "SELECT order_price_cents, is_change_price, detail_dirty FROM orders WHERE id = ?1",
                params![order_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(price, Some(4000), "改价后订单级金额应回流");
        assert_eq!(change_flag, Some(1));
        assert_eq!(dirty, 0, "回刷完成应清脏标");
    }

    #[test]
    fn wechat_status_mapping_covers_official_enum() {
        assert_eq!(order_status_from_wechat(10), "unpaid");
        assert_eq!(order_status_from_wechat(12), "unpaid");
        assert_eq!(order_status_from_wechat(13), "unpaid");
        assert_eq!(order_status_from_wechat(20), "pending_shipment");
        assert_eq!(order_status_from_wechat(21), "partially_shipped");
        assert_eq!(order_status_from_wechat(30), "wechat_shipped");
        assert_eq!(order_status_from_wechat(100), "completed");
        assert_eq!(order_status_from_wechat(200), "cancelled");
        assert_eq!(order_status_from_wechat(250), "cancelled");
        assert_eq!(order_status_from_wechat(999), "synced");
    }
}
