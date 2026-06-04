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
#[derive(Debug, Clone, Copy)]
pub struct ErrorClassification {
    pub category: ErrorCategory,
    /// 最终失败时展示给用户的关注级别（Transient 在重试耗尽后按此展示）。
    pub attention: Attention,
    /// driver 是否应在退避后自动重试。
    pub retriable: bool,
    /// 给“能处理问题的人”看的中文原因。
    pub human_reason: &'static str,
    /// 推荐的一键动作提示。
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
            ErrorCode::SyncFailed => {
                (Transient, Error, true, "同步微信状态失败", "稍后自动重试")
            }

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
            ErrorCode::MissingAfterSaleAddress => (
                Recoverable,
                NeedConfirm,
                false,
                "店铺缺少可用售后地址",
                "请在店铺设置中添加售后退货地址",
            ),
            ErrorCode::AmbiguousAfterSaleAddress => (
                Recoverable,
                NeedConfirm,
                false,
                "存在多个售后地址，无法自动选择",
                "请选择一个售后退货地址",
            ),
            ErrorCode::MissingFreightTemplate => (
                Recoverable,
                NeedConfirm,
                false,
                "店铺缺少可用运费模板",
                "请在店铺设置中配置运费模板",
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
            ErrorCode::ShopNotFound => {
                (Fatal, Error, false, "目标店铺不存在", "请检查店铺配置")
            }
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
    human_reason: "出现未归类的错误",
    suggested_action: "请查看详情中的错误信息",
};

/// 归一化任意错误码字符串：已知码精确判定，未知码兜底为异常。
pub fn classify_error_code(code: &str) -> ErrorClassification {
    match ErrorCode::parse(code) {
        Some(c) => c.classify(),
        None => UNKNOWN_CLASSIFICATION,
    }
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
            assert!(seen.insert(code.as_str()), "错误码字符串重复: {}", code.as_str());
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
                    assert_eq!(c.attention, Attention::NeedConfirm, "{}", code.as_str());
                    assert!(!c.retriable, "{} 可补救项不应自动重试", code.as_str());
                }
                ErrorCategory::Fatal => {
                    assert_eq!(c.attention, Attention::Error, "{}", code.as_str());
                    assert!(!c.retriable, "{} 需修项不应自动重试", code.as_str());
                }
                ErrorCategory::Transient => {
                    assert_eq!(c.attention, Attention::Error, "{}", code.as_str());
                    assert!(c.retriable, "{} 瞬时项应可自动重试", code.as_str());
                }
            }
        }
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
}
