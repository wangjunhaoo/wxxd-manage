//! 流水线错误码归一化（pipeline error classification）。
//!
//! 背景：旧实现把错误码字符串散落在 7 个 runner 里，既无法穷举，也没有统一的
//! “待确认 vs 异常 / 是否自动重试 / 人话原因” 判定。本模块把这些字符串收敛成一个
//! [`ErrorCode`] 枚举，并提供纯函数 [`classify_error_code`]，作为 driver 推进决策
//! 和前端展示的唯一事实来源。
//!
//! 本模块只含纯函数与静态数据，无副作用、无 IO，便于穷举单测。

use serde::Serialize;

/// 用户可见的关注级别，决定前端是否给商品行打标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attention {
    /// 无需关注：自动处理中或已完成。
    None,
    /// 待确认：人工补一下（类目/属性/图片/地址/运费）就能继续。
    NeedConfirm,
    /// 异常：需要改配置、换号或排查后才能继续。
    Error,
}

/// 错误码的处置类别，决定 driver 如何推进。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// 瞬时故障（网络/令牌/接口抖动）：driver 应退避自动重试，耗尽后才升级为异常。
    Transient,
    /// 可补救：缺类目/属性/图片/地址/运费等，等人工补齐。
    Recoverable,
    /// 需修：配置缺失、数据非法或重复，等人工改配置或排查 bug。
    Fatal,
}

/// 一条错误码的归一化判定结果。全部字段均为 `'static`，可 `Copy`。
///
/// `category`（三类法）由一致性单测校验、保持归一表自洽，目前仅被单测读取，
/// 故标注 `allow(dead_code)`——它是归一表自洽的单一事实来源，非废弃代码。
#[derive(Debug, Clone, Copy)]
pub struct ErrorClassification {
    #[allow(dead_code)]
    pub category: ErrorCategory,
    /// 最终失败时展示给用户的关注级别（Transient 在重试耗尽后按此展示）。
    pub attention: Attention,
    /// driver 是否应在退避后自动重试（无限次，按指数退避节奏）。
    pub retriable: bool,
    /// 「需人工确认」类错误的有限次自动重试上限：Some(n) 表示虽 retriable=false、
    /// attention=NeedConfirm，driver 仍可在退避后自动重试 n 轮（如 AI 补属性偶发失败
    /// 冷却后再试常能通过），超限后保持 blocked 等人工。None 表示不做有限次自动重试。
    pub auto_retry_limit: Option<i64>,
    /// 给“能处理问题的人”看的中文原因。
    pub human_reason: &'static str,
    /// 推荐的一键动作提示，随流水线视图透出给前端异常行展示。
    pub suggested_action: &'static str,
}

/// 归一后的流水线错误码。覆盖采集、审查与铺货 9 阶段链路上真实出现过的错误码字符串。
///
/// 非流水线链路（采购/售后/价格/订单）的错误码不在此枚举内，会被
/// [`classify_error_code`] 当作未知码兜底为 [`ErrorCategory::Fatal`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // —— 瞬时故障（Transient，自动退避重试） ——
    AccessTokenFailed,
    ProviderHttpFailed,
    WechatRequestFailed,
    WechatAddproductHttpFailed,
    WechatGetproductHttpFailed,
    WechatUpdateproductHttpFailed,
    WechatListingproductHttpFailed,
    WechatCategoryPrecheckHttpFailed,
    WechatImageUploadHttpFailed,
    WechatAddressListHttpFailed,
    WechatAddressDetailHttpFailed,
    WechatFreightTemplateSyncFailed,
    SyncFailed,

    // —— 可补救（Recoverable，待确认） ——
    MissingWechatLeafCategoryId,
    CategoryAttrsNeedAiFill,
    CategoryNeedsAiFill,
    WechatPayloadNeedsAiFill,
    SkuSpecValueMalformed,
    MissingAfterSaleAddress,
    AmbiguousAfterSaleAddress,
    MissingFreightTemplate,
    InsufficientHeadImages,
    InsufficientDetailImages,
    InvalidImageSourceUrl,
    ImagePreprocessFailed,
    ProductAssetsEmpty,
    ReviewNeedsConfirm,
    ReviewBlocked,

    // —— 需修（Fatal，异常） ——
    ShopSecretMissing,
    ShopNotActive,
    ShopNotFound,
    ProviderNotConfigured,
    ProviderAuthFailed,
    SkillDisabled,
    InvalidProductPayload,
    DuplicateExternalProductInShop,
    MissingWechatProductId,
    MissingWechatProductPayload,
    PrecheckUnexpectedError,
    CategoryDetailCacheMissing,
    CategoryAttrFillFailed,
    WechatCategoryPrecheckFailed,
    WechatApiError,
    UnknownAgentError,
    ValidationError,
    CollectFailed,
    CollectAccessLimited,
}

impl ErrorCode {
    /// 所有已知错误码，供穷举单测与归类一致性校验使用。
    pub const ALL: &'static [ErrorCode] = &[
        ErrorCode::AccessTokenFailed,
        ErrorCode::ProviderHttpFailed,
        ErrorCode::WechatRequestFailed,
        ErrorCode::WechatAddproductHttpFailed,
        ErrorCode::WechatGetproductHttpFailed,
        ErrorCode::WechatUpdateproductHttpFailed,
        ErrorCode::WechatListingproductHttpFailed,
        ErrorCode::WechatCategoryPrecheckHttpFailed,
        ErrorCode::WechatImageUploadHttpFailed,
        ErrorCode::WechatAddressListHttpFailed,
        ErrorCode::WechatAddressDetailHttpFailed,
        ErrorCode::WechatFreightTemplateSyncFailed,
        ErrorCode::SyncFailed,
        ErrorCode::MissingWechatLeafCategoryId,
        ErrorCode::CategoryAttrsNeedAiFill,
        ErrorCode::CategoryNeedsAiFill,
        ErrorCode::WechatPayloadNeedsAiFill,
        ErrorCode::SkuSpecValueMalformed,
        ErrorCode::MissingAfterSaleAddress,
        ErrorCode::AmbiguousAfterSaleAddress,
        ErrorCode::MissingFreightTemplate,
        ErrorCode::InsufficientHeadImages,
        ErrorCode::InsufficientDetailImages,
        ErrorCode::InvalidImageSourceUrl,
        ErrorCode::ImagePreprocessFailed,
        ErrorCode::ProductAssetsEmpty,
        ErrorCode::ReviewNeedsConfirm,
        ErrorCode::ReviewBlocked,
        ErrorCode::ShopSecretMissing,
        ErrorCode::ShopNotActive,
        ErrorCode::ShopNotFound,
        ErrorCode::ProviderNotConfigured,
        ErrorCode::ProviderAuthFailed,
        ErrorCode::SkillDisabled,
        ErrorCode::InvalidProductPayload,
        ErrorCode::DuplicateExternalProductInShop,
        ErrorCode::MissingWechatProductId,
        ErrorCode::MissingWechatProductPayload,
        ErrorCode::PrecheckUnexpectedError,
        ErrorCode::CategoryDetailCacheMissing,
        ErrorCode::CategoryAttrFillFailed,
        ErrorCode::WechatCategoryPrecheckFailed,
        ErrorCode::WechatApiError,
        ErrorCode::UnknownAgentError,
        ErrorCode::ValidationError,
        ErrorCode::CollectFailed,
        ErrorCode::CollectAccessLimited,
    ];

    /// 枚举对应的错误码字符串（与现有代码里散落的字面量保持一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::AccessTokenFailed => "ACCESS_TOKEN_FAILED",
            ErrorCode::ProviderHttpFailed => "PROVIDER_HTTP_FAILED",
            ErrorCode::WechatRequestFailed => "WECHAT_REQUEST_FAILED",
            ErrorCode::WechatAddproductHttpFailed => "WECHAT_ADDPRODUCT_HTTP_FAILED",
            ErrorCode::WechatGetproductHttpFailed => "WECHAT_GETPRODUCT_HTTP_FAILED",
            ErrorCode::WechatUpdateproductHttpFailed => "WECHAT_UPDATEPRODUCT_HTTP_FAILED",
            ErrorCode::WechatListingproductHttpFailed => "WECHAT_LISTINGPRODUCT_HTTP_FAILED",
            ErrorCode::WechatCategoryPrecheckHttpFailed => "WECHAT_CATEGORY_PRECHECK_HTTP_FAILED",
            ErrorCode::WechatImageUploadHttpFailed => "WECHAT_IMAGE_UPLOAD_HTTP_FAILED",
            ErrorCode::WechatAddressListHttpFailed => "WECHAT_ADDRESS_LIST_HTTP_FAILED",
            ErrorCode::WechatAddressDetailHttpFailed => "WECHAT_ADDRESS_DETAIL_HTTP_FAILED",
            ErrorCode::WechatFreightTemplateSyncFailed => "WECHAT_FREIGHT_TEMPLATE_SYNC_FAILED",
            ErrorCode::SyncFailed => "SYNC_FAILED",
            ErrorCode::MissingWechatLeafCategoryId => "MISSING_WECHAT_LEAF_CATEGORY_ID",
            ErrorCode::CategoryAttrsNeedAiFill => "CATEGORY_ATTRS_NEED_AI_FILL",
            ErrorCode::CategoryNeedsAiFill => "CATEGORY_NEEDS_AI_FILL",
            ErrorCode::WechatPayloadNeedsAiFill => "WECHAT_PAYLOAD_NEEDS_AI_FILL",
            ErrorCode::SkuSpecValueMalformed => "SKU_SPEC_VALUE_MALFORMED",
            ErrorCode::MissingAfterSaleAddress => "MISSING_AFTER_SALE_ADDRESS",
            ErrorCode::AmbiguousAfterSaleAddress => "AMBIGUOUS_AFTER_SALE_ADDRESS",
            ErrorCode::MissingFreightTemplate => "MISSING_FREIGHT_TEMPLATE",
            ErrorCode::InsufficientHeadImages => "INSUFFICIENT_HEAD_IMAGES",
            ErrorCode::InsufficientDetailImages => "INSUFFICIENT_DETAIL_IMAGES",
            ErrorCode::InvalidImageSourceUrl => "INVALID_IMAGE_SOURCE_URL",
            ErrorCode::ImagePreprocessFailed => "IMAGE_PREPROCESS_FAILED",
            ErrorCode::ProductAssetsEmpty => "PRODUCT_ASSETS_EMPTY",
            ErrorCode::ReviewNeedsConfirm => "REVIEW_NEEDS_CONFIRM",
            ErrorCode::ReviewBlocked => "REVIEW_BLOCKED",
            ErrorCode::ShopSecretMissing => "SHOP_SECRET_MISSING",
            ErrorCode::ShopNotActive => "SHOP_NOT_ACTIVE",
            ErrorCode::ShopNotFound => "SHOP_NOT_FOUND",
            ErrorCode::ProviderNotConfigured => "PROVIDER_NOT_CONFIGURED",
            ErrorCode::ProviderAuthFailed => "PROVIDER_AUTH_FAILED",
            ErrorCode::SkillDisabled => "SKILL_DISABLED",
            ErrorCode::InvalidProductPayload => "INVALID_PRODUCT_PAYLOAD",
            ErrorCode::DuplicateExternalProductInShop => "DUPLICATE_EXTERNAL_PRODUCT_IN_SHOP",
            ErrorCode::MissingWechatProductId => "MISSING_WECHAT_PRODUCT_ID",
            ErrorCode::MissingWechatProductPayload => "MISSING_WECHAT_PRODUCT_PAYLOAD",
            ErrorCode::PrecheckUnexpectedError => "PRECHECK_UNEXPECTED_ERROR",
            ErrorCode::CategoryDetailCacheMissing => "CATEGORY_DETAIL_CACHE_MISSING",
            ErrorCode::CategoryAttrFillFailed => "CATEGORY_ATTR_FILL_FAILED",
            ErrorCode::WechatCategoryPrecheckFailed => "WECHAT_CATEGORY_PRECHECK_FAILED",
            ErrorCode::WechatApiError => "WECHAT_API_ERROR",
            ErrorCode::UnknownAgentError => "UNKNOWN_AGENT_ERROR",
            ErrorCode::ValidationError => "VALIDATION_ERROR",
            ErrorCode::CollectFailed => "COLLECT_FAILED",
            ErrorCode::CollectAccessLimited => "COLLECT_ACCESS_LIMITED",
        }
    }

    /// 把错误码字符串解析为枚举；未知字符串返回 `None`。
    pub fn parse(code: &str) -> Option<ErrorCode> {
        ErrorCode::ALL.iter().copied().find(|c| c.as_str() == code)
    }

    /// 「需人工确认」类错误的有限次自动重试上限（详见 [`ErrorClassification::auto_retry_limit`]）。
    /// 纯技术性缺属性类错误先让 driver 自动重试（回退到 category_precheck 的在线详情 AI 兜底），
    /// 超限才转人工，减少无谓的人工点击。
    fn auto_retry_limit(self) -> Option<i64> {
        match self {
            // AI 补属性偶发失败/限流：冷却后再试常能通过，给 3 次机会
            ErrorCode::CategoryAttrsNeedAiFill | ErrorCode::WechatPayloadNeedsAiFill => Some(3),
            // 类目推断对 AI 限流敏感，给 2 次机会；仍失败大概率真歧义，转人工
            ErrorCode::CategoryNeedsAiFill => Some(2),
            _ => None,
        }
    }

    /// 该错误码的归一化判定。
    pub fn classify(self) -> ErrorClassification {
        use Attention::*;
        use ErrorCategory::*;
        let (category, attention, retriable, human_reason, suggested_action) = match self {
            // —— 瞬时故障 —— attention 为耗尽后归宿，retriable=true 表示先自动重试。
            ErrorCode::AccessTokenFailed => (
                Transient,
                Error,
                true,
                "获取微信接口令牌失败",
                "稍后自动重试；多次失败请检查店铺密钥与网络",
            ),
            ErrorCode::ProviderHttpFailed => (
                Transient,
                Error,
                true,
                "AI 服务网络请求失败",
                "稍后自动重试；持续失败请检查 AI 网关地址",
            ),
            ErrorCode::WechatRequestFailed => {
                (Transient, Error, true, "调用微信接口失败", "稍后自动重试")
            }
            ErrorCode::WechatAddproductHttpFailed => (
                Transient,
                Error,
                true,
                "提交商品到微信时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatGetproductHttpFailed => (
                Transient,
                Error,
                true,
                "查询微信商品状态时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatUpdateproductHttpFailed => (
                Transient,
                Error,
                true,
                "更新微信商品时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatListingproductHttpFailed => (
                Transient,
                Error,
                true,
                "上架微信商品时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatCategoryPrecheckHttpFailed => {
                (Transient, Error, true, "类目预检网络失败", "稍后自动重试")
            }
            ErrorCode::WechatImageUploadHttpFailed => (
                Transient,
                Error,
                true,
                "上传图片到微信时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatAddressListHttpFailed => (
                Transient,
                Error,
                true,
                "查询售后地址时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatAddressDetailHttpFailed => (
                Transient,
                Error,
                true,
                "查询售后地址详情时网络失败",
                "稍后自动重试",
            ),
            ErrorCode::WechatFreightTemplateSyncFailed => {
                (Transient, Error, true, "同步运费模板失败", "稍后自动重试")
            }
            ErrorCode::SyncFailed => (Transient, Error, true, "同步微信状态失败", "稍后自动重试"),

            // —— 可补救 —— 等人工补齐后即可继续。
            ErrorCode::MissingWechatLeafCategoryId => (
                Recoverable,
                NeedConfirm,
                false,
                "无法自动确定微信叶子类目",
                "请为该商品手动选择微信类目",
            ),
            ErrorCode::CategoryAttrsNeedAiFill => (
                Recoverable,
                NeedConfirm,
                false,
                "类目必填属性需要补齐",
                "请确认或补全商品属性",
            ),
            ErrorCode::CategoryNeedsAiFill => (
                Recoverable,
                NeedConfirm,
                false,
                "类目或属性需要人工确认",
                "请确认类目并补全属性",
            ),
            ErrorCode::WechatPayloadNeedsAiFill => (
                Recoverable,
                NeedConfirm,
                false,
                "发品资料缺少必填属性",
                "请补全商品必填属性",
            ),
            ErrorCode::SkuSpecValueMalformed => (
                Recoverable,
                NeedConfirm,
                false,
                "SKU 规格值异常（疑似采集错位，如多个尺码挤在一格）",
                "请重新采集，或在商品编辑中把该规格拆成多个 SKU",
            ),
            // —— 店铺配置缺失三码：retriable=true 自愈。runner 每次都会在线拉取微信
            //    地址/运费模板列表，用户在微信后台补配后 ≤30 分钟（退避封顶）自动恢复推进，
            //    零人工点击；attention 保持 NeedConfirm 提示用户该去补配置。
            ErrorCode::MissingAfterSaleAddress => (
                Recoverable,
                NeedConfirm,
                true,
                "店铺缺少可用售后地址",
                "请在店铺设置中添加售后退货地址，配好后系统自动继续",
            ),
            ErrorCode::AmbiguousAfterSaleAddress => (
                Recoverable,
                NeedConfirm,
                true,
                "存在多个售后地址，无法自动选择",
                "请设置默认退货地址，设好后系统自动继续",
            ),
            ErrorCode::MissingFreightTemplate => (
                Recoverable,
                NeedConfirm,
                true,
                "店铺缺少可用运费模板",
                "请在店铺设置中配置运费模板，配好后系统自动继续",
            ),
            ErrorCode::InsufficientHeadImages => (
                Recoverable,
                NeedConfirm,
                false,
                "主图数量不足",
                "请补充或更换商品主图",
            ),
            ErrorCode::InsufficientDetailImages => (
                Recoverable,
                NeedConfirm,
                false,
                "详情图数量不足",
                "请补充商品详情图",
            ),
            ErrorCode::InvalidImageSourceUrl => (
                Recoverable,
                NeedConfirm,
                false,
                "图片链接无效",
                "请更换商品图片",
            ),
            ErrorCode::ImagePreprocessFailed => (
                Recoverable,
                NeedConfirm,
                false,
                "图片处理失败（格式/大小/尺寸不符）",
                "请更换符合要求的图片",
            ),
            ErrorCode::ProductAssetsEmpty => (
                Recoverable,
                NeedConfirm,
                false,
                "商品缺少可用图片素材",
                "请补充商品图片",
            ),
            ErrorCode::ReviewNeedsConfirm => (
                Recoverable,
                NeedConfirm,
                false,
                "AI 审查置信不足，需人工确认",
                "请确认标题、类目与属性",
            ),
            ErrorCode::ReviewBlocked => (
                Recoverable,
                NeedConfirm,
                false,
                "审查发现需人工处理的问题（如图片违规）",
                "请处理后重新提交",
            ),

            // —— 需修 —— 需改配置、换号或排查后才能继续。
            ErrorCode::ShopSecretMissing => (
                Fatal,
                Error,
                false,
                "店铺未配置接口密钥",
                "请在店铺设置中填写并保存 app_secret",
            ),
            ErrorCode::ShopNotActive => (
                Fatal,
                Error,
                false,
                "店铺未启用或未通过验证",
                "请先在店铺设置中完成验证并启用",
            ),
            ErrorCode::ShopNotFound => (Fatal, Error, false, "目标店铺不存在", "请检查店铺配置"),
            ErrorCode::ProviderNotConfigured => (
                Fatal,
                Error,
                false,
                "AI 服务未配置",
                "请在 AI 设置中配置服务地址与密钥",
            ),
            ErrorCode::ProviderAuthFailed => (
                Fatal,
                Error,
                false,
                "AI 服务鉴权失败",
                "请在 AI 设置中检查 API Key",
            ),
            ErrorCode::SkillDisabled => (
                Fatal,
                Error,
                false,
                "所需 AI 技能未启用",
                "请在 AI 设置中启用对应技能",
            ),
            ErrorCode::InvalidProductPayload => (
                Fatal,
                Error,
                false,
                "商品发布资料校验不通过",
                "请检查商品标题、SKU、价格等基础信息",
            ),
            ErrorCode::DuplicateExternalProductInShop => (
                Fatal,
                Error,
                false,
                "该商品已在此店铺铺过货",
                "无需重复铺货；如需更新请用改价或编辑",
            ),
            ErrorCode::MissingWechatProductId => (
                Fatal,
                Error,
                false,
                "缺少微信商品 ID，无法继续",
                "请重新提交该商品",
            ),
            ErrorCode::MissingWechatProductPayload => (
                Fatal,
                Error,
                false,
                "发品资料缺失，无法提交",
                "请点击「重新铺货」重建发品资料",
            ),
            ErrorCode::PrecheckUnexpectedError => (
                Fatal,
                Error,
                false,
                "发品前校验出现意外错误",
                "请点击「重新铺货」重试；持续失败请查看任务日志",
            ),
            // 类目详情缓存缺失是系统自身缓存问题：Transient 自动重试，
            // AI 补属性 runner 的缓存缺失分支会推进到 category_precheck 在线拉取详情自愈。
            ErrorCode::CategoryDetailCacheMissing => (
                Transient,
                Error,
                true,
                "类目详情缓存缺失，等待自动拉取",
                "无需操作，系统将自动拉取类目详情后继续",
            ),
            ErrorCode::CategoryAttrFillFailed => (
                Fatal,
                Error,
                false,
                "AI 补属性失败",
                "请点击「重新铺货」重试，或在确认面板手动补全属性",
            ),
            ErrorCode::WechatCategoryPrecheckFailed => (
                Fatal,
                Error,
                false,
                "微信类目预检不通过",
                "请检查类目资质或更换类目",
            ),
            ErrorCode::WechatApiError => (
                Fatal,
                Error,
                false,
                "微信接口返回错误",
                "请查看详情中的微信错误信息",
            ),
            ErrorCode::UnknownAgentError => (
                Fatal,
                Error,
                false,
                "AI 处理出现未知错误",
                "请重试；持续失败请检查 AI 设置",
            ),
            ErrorCode::ValidationError => {
                (Fatal, Error, false, "数据校验失败", "请检查并修正相关信息")
            }
            ErrorCode::CollectFailed => (
                Fatal,
                Error,
                false,
                "采集淘宝商品失败",
                "请检查商品链接或稍后手动重试",
            ),
            ErrorCode::CollectAccessLimited => (
                Fatal,
                Error,
                false,
                "淘宝访问受限（风控冷却中）",
                "请稍后再试或更换淘宝账号，勿频繁重试",
            ),
        };
        ErrorClassification {
            category,
            attention,
            retriable,
            auto_retry_limit: self.auto_retry_limit(),
            human_reason,
            suggested_action,
        }
    }
}

/// 对未知错误码的兜底判定：当作需人工排查的异常。
const UNKNOWN_CLASSIFICATION: ErrorClassification = ErrorClassification {
    category: ErrorCategory::Fatal,
    attention: Attention::Error,
    retriable: false,
    auto_retry_limit: None,
    human_reason: "出现未归类的错误",
    suggested_action: "请查看详情中的错误信息",
};

/// 归一化任意错误码字符串：已知码精确判定，未知码兜底为异常。
pub fn classify_error_code(code: &str) -> ErrorClassification {
    if let Some(c) = ErrorCode::parse(code) {
        return c.classify();
    }
    // 微信动态状态码(WECHAT_PRODUCT_STATUS_3/8/13/... 由 product_status.rs 按真实 status 拼接、
    // 不在固定枚举里)统一兜底归类，避免落 UNKNOWN_CLASSIFICATION 在前端显示「出现未归类的错误」。
    // retriable 保持 false：审核驳回须人工改商品后重提，自动重试只会再次驳回并触发微信限频。
    if code.starts_with("WECHAT_PRODUCT_STATUS_") {
        return ErrorClassification {
            category: ErrorCategory::Fatal,
            attention: Attention::Error,
            retriable: false,
            auto_retry_limit: None,
            human_reason: "微信审核未通过",
            suggested_action: "请按微信驳回原因修改商品后点「重新铺货」重新提交",
        };
    }
    // 微信 addproduct 偶发吞 SKU（errcode=0 但草稿 skus=[]）：submit 阶段已自动「getproduct 校验
    // + 删空草稿 + 重新 addproduct」重试多次仍未入库，判定为微信端接口偶发故障。retriable=false——
    // 自动重试已用尽，继续自动重试只会再撞偶发故障并消耗发品配额，留待人工择机重新提交（requeue）。
    // 微信 addproduct 偶发吞 SKU 的单次失败（errcode=0 但草稿 skus=[]）：retriable=true，交 driver
    // 指数退避(1→2→4→8→16→30分钟)隔开时间重排重新 addproduct，避开微信对「同 spu 短时重复 addproduct」
    // 持续吞 SKU 的陷阱（亚秒级连续重试基本必吞，隔分钟级重试约 44%/轮恢复）。超重排上限升级为下方 fatal 码。
    if code == "WECHAT_ADDPRODUCT_SKU_SWALLOWED" {
        return ErrorClassification {
            category: ErrorCategory::Transient,
            attention: Attention::Error,
            retriable: true,
            auto_retry_limit: None,
            human_reason: "微信偶发吞 SKU，正在自动退避重排重试",
            suggested_action: "无需操作，系统将隔开时间自动重新提交",
        };
    }
    if code == "WECHAT_ADDPRODUCT_DRAFT_SKU_ZERO" {
        return ErrorClassification {
            category: ErrorCategory::Fatal,
            attention: Attention::Error,
            retriable: false,
            auto_retry_limit: None,
            human_reason: "微信偶发吞 SKU，自动退避重排多轮仍未入库",
            suggested_action: "微信 addproduct 接口偶发故障，请点「重新铺货」择机重新提交",
        };
    }
    // 店铺发品配额受限（未缴足保证金的店铺上架上限 100 个）：addproduct 报 10020110、
    // listingproduct 报内层 6600133。比吞 SKU 更上层的硬天花板，非代码能解，给出明确指引。
    if code == "WECHAT_ADDPRODUCT_10020110" || code == "WECHAT_LISTINGPRODUCT_6600133" {
        return ErrorClassification {
            category: ErrorCategory::Fatal,
            attention: Attention::Error,
            retriable: false,
            auto_retry_limit: None,
            human_reason: "店铺发品配额受限（未缴足保证金限 100 个上架）",
            suggested_action: "请缴纳店铺保证金解除配额，或先下架闲置商品再重试",
        };
    }
    // 微信接口动态错误码前缀兜底（WECHAT_ADDPRODUCT_{errcode} 等由 runner 按真实 errcode
    // 拼接，无法穷举）：归为 Fatal 不自动重试（真实 errcode 各异，盲目重试不安全），
    // 人话文案点明是哪个接口报错，详细 errmsg 经 error_summary 透出。
    if code.starts_with("WECHAT_ADDPRODUCT_") {
        return ErrorClassification {
            category: ErrorCategory::Fatal,
            attention: Attention::Error,
            retriable: false,
            auto_retry_limit: None,
            human_reason: "微信发品接口返回错误",
            suggested_action: "请查看店铺详情中的微信报错信息，处理后点「重新铺货」",
        };
    }
    if code.starts_with("WECHAT_LISTINGPRODUCT_") {
        return ErrorClassification {
            category: ErrorCategory::Fatal,
            attention: Attention::Error,
            retriable: false,
            auto_retry_limit: None,
            human_reason: "微信上架接口返回错误",
            suggested_action: "请查看店铺详情中的微信报错信息，处理后点「重新铺货」",
        };
    }
    if code.starts_with("WECHAT_CATEGORY_PRECHECK_") {
        return ErrorClassification {
            category: ErrorCategory::Fatal,
            attention: Attention::Error,
            retriable: false,
            auto_retry_limit: None,
            human_reason: "微信类目预检返回错误",
            suggested_action: "请检查类目资质或更换类目后点「重新铺货」",
        };
    }
    UNKNOWN_CLASSIFICATION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip_for_all_known_codes() {
        for code in ErrorCode::ALL {
            let s = code.as_str();
            assert!(!s.is_empty(), "错误码字符串不能为空");
            assert_eq!(ErrorCode::parse(s), Some(*code), "parse 往返失败: {s}");
        }
    }

    #[test]
    fn no_duplicate_code_strings() {
        let mut seen = std::collections::HashSet::new();
        for code in ErrorCode::ALL {
            assert!(
                seen.insert(code.as_str()),
                "错误码字符串重复: {}",
                code.as_str()
            );
        }
    }

    #[test]
    fn classification_is_consistent_with_category() {
        for code in ErrorCode::ALL {
            let c = code.classify();
            assert!(!c.human_reason.is_empty(), "{} 缺人话原因", code.as_str());
            assert!(
                !c.suggested_action.is_empty(),
                "{} 缺动作提示",
                code.as_str()
            );
            match c.category {
                ErrorCategory::Recoverable => {
                    // 可补救项一律提示用户关注（NeedConfirm）；retriable 允许为 true
                    //（店铺配置缺失类：用户补配后自动自愈）。
                    assert_eq!(c.attention, Attention::NeedConfirm, "{}", code.as_str());
                }
                ErrorCategory::Fatal => {
                    assert_eq!(c.attention, Attention::Error, "{}", code.as_str());
                    assert!(!c.retriable, "{} 需修项不应自动重试", code.as_str());
                    assert!(
                        c.auto_retry_limit.is_none(),
                        "{} 需修项不应有限次自动重试",
                        code.as_str()
                    );
                }
                ErrorCategory::Transient => {
                    assert_eq!(c.attention, Attention::Error, "{}", code.as_str());
                    assert!(c.retriable, "{} 瞬时项应可自动重试", code.as_str());
                }
            }
            // auto_retry_limit 只该出现在 retriable=false 的可补救项上（有限次兜底重试）
            if c.auto_retry_limit.is_some() {
                assert!(
                    !c.retriable && c.category == ErrorCategory::Recoverable,
                    "{} auto_retry_limit 仅用于不自动重试的可补救项",
                    code.as_str()
                );
            }
        }
    }

    #[test]
    fn auto_retry_policy_table() {
        // 纯技术性缺属性类：有限次自动重试（回退 category_precheck AI 兜底），超限转人工
        assert_eq!(
            classify_error_code("CATEGORY_ATTRS_NEED_AI_FILL").auto_retry_limit,
            Some(3)
        );
        assert_eq!(
            classify_error_code("WECHAT_PAYLOAD_NEEDS_AI_FILL").auto_retry_limit,
            Some(3)
        );
        assert_eq!(
            classify_error_code("CATEGORY_NEEDS_AI_FILL").auto_retry_limit,
            Some(2)
        );
        // 店铺配置缺失类：retriable 自愈（用户补配后 ≤30 分钟自动恢复）
        for code in [
            "MISSING_FREIGHT_TEMPLATE",
            "MISSING_AFTER_SALE_ADDRESS",
            "AMBIGUOUS_AFTER_SALE_ADDRESS",
        ] {
            let c = classify_error_code(code);
            assert!(c.retriable, "{code} 应自动自愈重试");
            assert_eq!(c.attention, Attention::NeedConfirm, "{code} 仍应提示用户");
        }
        // 缺图类/类目歧义/审查类：保持纯人工
        for code in [
            "INSUFFICIENT_HEAD_IMAGES",
            "INSUFFICIENT_DETAIL_IMAGES",
            "MISSING_WECHAT_LEAF_CATEGORY_ID",
            "REVIEW_NEEDS_CONFIRM",
        ] {
            let c = classify_error_code(code);
            assert!(!c.retriable, "{code} 不应自动重试");
            assert_eq!(c.auto_retry_limit, None, "{code} 不应有限次重试");
        }
    }

    #[test]
    fn dynamic_wechat_codes_classified() {
        // 配额受限精确码
        let quota = classify_error_code("WECHAT_ADDPRODUCT_10020110");
        assert!(quota.human_reason.contains("配额"));
        assert!(!quota.retriable);
        let quota2 = classify_error_code("WECHAT_LISTINGPRODUCT_6600133");
        assert!(quota2.human_reason.contains("配额"));
        // 动态 errcode 前缀兜底（不再落「未归类」）
        let add = classify_error_code("WECHAT_ADDPRODUCT_47001");
        assert_eq!(add.human_reason, "微信发品接口返回错误");
        let listing = classify_error_code("WECHAT_LISTINGPRODUCT_12345");
        assert_eq!(listing.human_reason, "微信上架接口返回错误");
        let precheck = classify_error_code("WECHAT_CATEGORY_PRECHECK_999");
        assert_eq!(precheck.human_reason, "微信类目预检返回错误");
        // 本地动态码已入枚举
        assert!(classify_error_code("CATEGORY_DETAIL_CACHE_MISSING").retriable);
        assert_eq!(
            classify_error_code("PRECHECK_UNEXPECTED_ERROR").category,
            ErrorCategory::Fatal
        );
        assert_eq!(
            classify_error_code("MISSING_WECHAT_PRODUCT_PAYLOAD").category,
            ErrorCategory::Fatal
        );
        assert_eq!(
            classify_error_code("CATEGORY_ATTR_FILL_FAILED").category,
            ErrorCategory::Fatal
        );
    }

    #[test]
    fn unknown_code_falls_back_to_fatal() {
        let c = classify_error_code("SOME_TOTALLY_UNKNOWN_CODE");
        assert_eq!(c.category, ErrorCategory::Fatal);
        assert_eq!(c.attention, Attention::Error);
        assert!(!c.retriable);
        assert!(!c.human_reason.is_empty());
    }

    #[test]
    fn known_samples_classify_as_expected() {
        assert_eq!(
            classify_error_code("MISSING_WECHAT_LEAF_CATEGORY_ID").attention,
            Attention::NeedConfirm
        );
        assert_eq!(
            classify_error_code("SHOP_SECRET_MISSING").attention,
            Attention::Error
        );
        assert!(classify_error_code("ACCESS_TOKEN_FAILED").retriable);
        assert_eq!(
            classify_error_code("REVIEW_BLOCKED").category,
            ErrorCategory::Recoverable
        );
        assert_eq!(
            classify_error_code("COLLECT_ACCESS_LIMITED").category,
            ErrorCategory::Fatal
        );
    }

    #[test]
    fn addproduct_sku_swallow_codes_classified() {
        // 退避重排码：Transient/可自动重试（driver 隔开时间重排，避开微信短时重复吞 SKU 陷阱）。
        let retry = classify_error_code("WECHAT_ADDPRODUCT_SKU_SWALLOWED");
        assert_eq!(retry.category, ErrorCategory::Transient);
        assert_eq!(retry.attention, Attention::Error);
        assert!(retry.retriable, "退避重排码应可自动重试");
        assert!(!retry.human_reason.is_empty());
        assert!(!retry.suggested_action.is_empty());
        // 升级真失败码：Fatal/不自动重试（多轮退避仍被吞，留待人工 requeue）。
        let dead = classify_error_code("WECHAT_ADDPRODUCT_DRAFT_SKU_ZERO");
        assert_eq!(dead.category, ErrorCategory::Fatal);
        assert_eq!(dead.attention, Attention::Error);
        assert!(!dead.retriable, "退避重排用尽，不应再自动重试");
    }
}
