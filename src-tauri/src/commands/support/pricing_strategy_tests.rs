use super::*;

fn create_settings_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("打开内存数据库");
    conn.execute(
        "CREATE TABLE app_settings (
              key TEXT PRIMARY KEY,
              value_json TEXT NOT NULL,
              updated_at TEXT NOT NULL
            )",
        [],
    )
    .expect("创建配置表");
    conn
}

fn create_category_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("打开内存数据库");
    conn.execute(
        "CREATE TABLE wechat_categories (
              shop_id TEXT NOT NULL,
              cat_id INTEGER NOT NULL,
              parent_cat_id INTEGER,
              level INTEGER,
              name TEXT NOT NULL,
              PRIMARY KEY(shop_id, cat_id)
            )",
        [],
    )
    .expect("创建微信类目表");
    conn.execute(
        "CREATE TABLE wechat_category_relations (
              shop_id TEXT NOT NULL,
              cat_id INTEGER NOT NULL,
              status INTEGER NOT NULL,
              PRIMARY KEY(shop_id, cat_id)
            )",
        [],
    )
    .expect("创建微信类目权限表");
    conn
}

fn insert_category(
    conn: &Connection,
    cat_id: i64,
    parent_cat_id: Option<i64>,
    level: i64,
    name: &str,
) {
    conn.execute(
        "INSERT INTO wechat_categories (shop_id, cat_id, parent_cat_id, level, name)
             VALUES ('shop-1', ?1, ?2, ?3, ?4)",
        params![cat_id, parent_cat_id, level, name],
    )
    .expect("写入微信类目");
    conn.execute(
        "INSERT INTO wechat_category_relations (shop_id, cat_id, status)
             VALUES ('shop-1', ?1, 1)",
        params![cat_id],
    )
    .expect("写入微信类目权限");
}

fn product_with_category_hint(title: &str, category_hint: &str) -> ExternalProductInput {
    ExternalProductInput {
        external_product_id: "demo".to_string(),
        title: title.to_string(),
        source_url: "https://example.com/item".to_string(),
        images: vec![],
        detail_images: vec![],
        skus: vec![],
        supplier_name: None,
        supplier_product_id: None,
        category_hint: Some(category_hint.to_string()),
        brand_hint: None,
        weight_gram: None,
        metadata: serde_json::json!({}),
    }
}

fn product_without_category_hint(title: &str) -> ExternalProductInput {
    ExternalProductInput {
        external_product_id: "demo".to_string(),
        title: title.to_string(),
        source_url: "https://example.com/item".to_string(),
        images: vec![],
        detail_images: vec![],
        skus: vec![],
        supplier_name: None,
        supplier_product_id: None,
        category_hint: None,
        brand_hint: None,
        weight_gram: None,
        metadata: serde_json::json!({}),
    }
}

#[test]
fn infer_wechat_category_from_cache_matches_children_traditional_clothing() {
    let conn = create_category_conn();
    insert_category(&conn, 10000116, None, 1, "母婴");
    insert_category(&conn, 10000123, Some(10000116), 2, "童装");
    insert_category(&conn, 6225, Some(10000123), 3, "旗袍唐装/民族服装");
    insert_category(&conn, 10000111, None, 1, "服饰内衣");
    insert_category(&conn, 10000113, Some(10000111), 2, "女装");
    insert_category(&conn, 10000663, Some(10000113), 3, "唐装/中式服装/民族服装");
    insert_category(&conn, 547661, Some(10000663), 4, "唐装");

    let product = product_with_category_hint(
        "2026 新款儿童唐装旗袍",
        "童装/婴儿装/亲子装/儿童旗袍/唐装/民族服装/唐装",
    );
    let inferred =
        infer_wechat_category_from_cache(&conn, "shop-1", &product).expect("推断微信类目");

    let inferred = inferred.expect("应命中童装传统服饰类目");
    assert_eq!(inferred.category_ids, vec![10000116, 10000123, 6225]);
    assert_eq!(inferred.category_path, "母婴 > 童装 > 旗袍唐装/民族服装");
}

#[test]
fn infer_wechat_category_from_cache_skips_single_broad_adult_term() {
    let conn = create_category_conn();
    insert_category(&conn, 10000111, None, 1, "服饰内衣");
    insert_category(&conn, 10000113, Some(10000111), 2, "女装");
    insert_category(&conn, 10000663, Some(10000113), 3, "唐装/中式服装/民族服装");
    insert_category(&conn, 547661, Some(10000663), 4, "唐装");

    let product = product_with_category_hint("成人唐装外套", "唐装");
    let inferred =
        infer_wechat_category_from_cache(&conn, "shop-1", &product).expect("推断微信类目");

    assert!(inferred.is_none());
}

#[test]
fn infer_wechat_category_from_cache_matches_leaf_name_in_title_without_hint() {
    let conn = create_category_conn();
    insert_category(&conn, 10000116, None, 1, "母婴");
    insert_category(&conn, 10000123, Some(10000116), 2, "童装");
    insert_category(&conn, 6215, Some(10000123), 3, "T恤");

    let product = product_without_category_hint("2026新款儿童短袖纯棉T恤男童女童夏季婴幼儿上衣");
    let inferred =
        infer_wechat_category_from_cache(&conn, "shop-1", &product).expect("推断微信类目");

    let inferred = inferred.expect("应从标题命中微信 T 恤类目");
    assert_eq!(inferred.category_ids, vec![10000116, 10000123, 6215]);
    assert_eq!(inferred.category_path, "母婴 > 童装 > T恤");
}

#[test]
fn broad_category_candidates_include_active_leaf_when_strict_match_missing() {
    let conn = create_category_conn();
    insert_category(&conn, 10000116, None, 1, "母婴");
    insert_category(&conn, 10000123, Some(10000116), 2, "童装");
    insert_category(&conn, 6236, Some(10000123), 3, "裤子");

    let product =
        product_with_category_hint("2026春秋夏季新品女小童薄款打底裤长裤", "女装/女士精品>卫裤");

    let strict = suggest_wechat_category_candidates_from_cache(&conn, "shop-1", &product, 5)
        .expect("读取精确类目候选");
    let broad = suggest_wechat_category_broad_candidates_from_cache(&conn, "shop-1", &product, 20)
        .expect("读取宽类目候选");

    assert!(strict.is_empty());
    assert!(broad.iter().any(|candidate| {
        candidate.category_ids == vec![10000116, 10000123, 6236]
            && candidate.category_path == "母婴 > 童装 > 裤子"
            && candidate.source == "local_category_cache_broad"
    }));
}

#[test]
fn load_publish_pricing_strategy_uses_default_when_missing() {
    let conn = create_settings_conn();
    let strategy = load_publish_pricing_strategy(&conn).expect("读取默认价格策略");

    assert_eq!(strategy.sale_price_markup_rate, 1.6);
    assert_eq!(strategy.sale_price_fixed_cents, 0);
    assert_eq!(strategy.sale_price_floor_cents, 100);
}

#[test]
fn save_and_load_publish_pricing_strategy_round_trips() {
    let conn = create_settings_conn();
    let strategy = PublishPricingStrategy {
        sale_price_markup_rate: 1.8,
        sale_price_fixed_cents: 500,
        sale_price_floor_cents: 990,
    };

    save_publish_pricing_strategy_to_db(&conn, &strategy).expect("保存价格策略");
    let loaded = load_publish_pricing_strategy(&conn).expect("读取价格策略");

    assert_eq!(loaded.sale_price_markup_rate, 1.8);
    assert_eq!(loaded.sale_price_fixed_cents, 500);
    assert_eq!(loaded.sale_price_floor_cents, 990);
}

#[test]
fn normalize_ai_base_url_strips_endpoint_paths_for_agents_sdk() {
    assert_eq!(
        normalize_ai_base_url_for_provider(AI_PROVIDER_CUSTOM, "https://api.openai.com")
            .expect("规范化官方根路径"),
        "https://api.openai.com/v1"
    );
    assert_eq!(
        normalize_ai_base_url_for_provider(
            AI_PROVIDER_CUSTOM,
            "https://proxy.example.com/v1/chat/completions"
        )
        .expect("规范化 chat completions 路径"),
        "https://proxy.example.com/v1"
    );
    assert_eq!(
        normalize_ai_base_url_for_provider(
            AI_PROVIDER_CUSTOM,
            "https://proxy.example.com/v1/responses"
        )
        .expect("规范化 responses 路径"),
        "https://proxy.example.com/v1"
    );
}

#[test]
fn summarize_agent_stderr_explains_openresty_404() {
    let message = summarize_agent_stderr(
        r#"{"error":"Error getting response: <html><head><title>404 Not Found</title></head><body><center><h1>404 Not Found</h1></center><hr><center>openresty</center></body></html>"}"#,
    );

    assert!(message.contains("Custom provider"));
    assert!(message.contains("/chat/completions"));
}

#[test]
fn explicit_sku_price_keeps_priority_over_markup_strategy() {
    let sku = crate::models::ExternalSkuInput {
        external_sku_id: "sku-1".to_string(),
        specs: serde_json::json!({}),
        cost_price: 12.5,
        stock: 10,
    };
    let metadata = serde_json::json!({
        "sku_prices": { "sku-1": 1999 },
        "sale_price_markup_rate": 8.0,
        "sale_price_fixed_cents": 1000,
        "sale_price_floor_cents": 100
    });
    let metadata = metadata.as_object();

    let price = resolve_sku_sale_price_cents(&sku, metadata).expect("计算 SKU 售价");

    assert_eq!(price, 1999);
}

#[test]
fn apply_publish_pricing_strategy_writes_metadata_fields() {
    let mut product = ExternalProductInput {
        external_product_id: "demo".to_string(),
        title: "示例商品".to_string(),
        source_url: "https://example.com/item".to_string(),
        images: vec![],
        detail_images: vec![],
        skus: vec![],
        supplier_name: None,
        supplier_product_id: None,
        category_hint: None,
        brand_hint: None,
        weight_gram: None,
        metadata: serde_json::json!({ "collection_source": "test" }),
    };
    let strategy = PublishPricingStrategy {
        sale_price_markup_rate: 1.7,
        sale_price_fixed_cents: 300,
        sale_price_floor_cents: 880,
    };

    apply_publish_pricing_strategy(&mut product, &strategy);

    assert_eq!(product.metadata["sale_price_markup_rate"], 1.7);
    assert_eq!(product.metadata["sale_price_fixed_cents"], 300);
    assert_eq!(product.metadata["sale_price_floor_cents"], 880);
    assert_eq!(product.metadata["collection_source"], "test");
}
