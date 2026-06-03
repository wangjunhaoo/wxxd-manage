use crate::storage::{AppError, AppResult};
use reqwest::{multipart, Client, Url};
use serde::{Deserialize, Serialize};

const STABLE_TOKEN_URL: &str = "https://api.weixin.qq.com/cgi-bin/stable_token";
const SHOP_BASIC_INFO_URL: &str = "https://api.weixin.qq.com/channels/ec/basics/info/get";
const API_QUOTA_URL: &str = "https://api.weixin.qq.com/cgi-bin/openapi/quota/get";
const IMAGE_UPLOAD_URL: &str = "https://api.weixin.qq.com/shop/ec/basics/img/upload";
const PRODUCT_ADD_URL: &str = "https://api.weixin.qq.com/channels/ec/product/add";
const PRODUCT_UPDATE_URL: &str = "https://api.weixin.qq.com/channels/ec/product/update";
const PRODUCT_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/product/get";
const PRODUCT_LISTING_URL: &str = "https://api.weixin.qq.com/channels/ec/product/listing";
const CATEGORY_PRECHECK_URL: &str =
    "https://api.weixin.qq.com/channels/ec/product/categoryprecheck";
const ORDER_LIST_URL: &str = "https://api.weixin.qq.com/channels/ec/order/list/get";
const ORDER_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/order/get";
const ORDER_PRICE_UPDATE_URL: &str = "https://api.weixin.qq.com/channels/ec/order/price/update";
const SEND_DELIVERY_URL: &str = "https://api.weixin.qq.com/channels/ec/order/delivery/send";
const DELIVERY_COMPANY_LIST_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/deliverycompanylist/new/get";
const AFTERSALE_LIST_URL: &str = "https://api.weixin.qq.com/channels/ec/aftersale/getaftersalelist";
const AFTERSALE_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/aftersale/getaftersaleorder";
const AFTERSALE_ACCEPT_URL: &str = "https://api.weixin.qq.com/channels/ec/aftersale/acceptapply";
const AFTERSALE_REJECT_URL: &str = "https://api.weixin.qq.com/channels/ec/aftersale/rejectapply";
const AFTERSALE_REJECT_REASON_URL: &str =
    "https://api.weixin.qq.com/channels/ec/aftersale/rejectreason/get";
const GUARANTEE_SEARCH_URL: &str =
    "https://api.weixin.qq.com/channels/ec/aftersale/searchguaranteeorder";
const GUARANTEE_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/aftersale/getguaranteeorder";
const CATEGORY_RELATION_LIST_URL: &str =
    "https://api.weixin.qq.com/shop/ec/category/get_category_relation_list";
const CATEGORY_RELATION_DETAIL_URL: &str =
    "https://api.weixin.qq.com/shop/ec/category/get_category_relation_detail";
const CATEGORY_ALL_URL: &str = "https://api.weixin.qq.com/shop/ec/category/all";
const CATEGORY_DETAIL_URL: &str = "https://api.weixin.qq.com/shop/ec/category/detail";
const CATEGORY_PRODUCT_RULE_URL: &str =
    "https://api.weixin.qq.com/shop/ec/category/getcategoryproductrule";
const CATEGORY_DELIVERY_RULE_URL: &str =
    "https://api.weixin.qq.com/shop/ec/category/getcategoryrule";
const FREIGHT_TEMPLATE_LIST_URL: &str =
    "https://api.weixin.qq.com/channels/ec/merchant/getfreighttemplatelist";
const MERCHANT_ADDRESS_LIST_URL: &str =
    "https://api.weixin.qq.com/channels/ec/merchant/address/list";
const MERCHANT_ADDRESS_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/merchant/address/get";

#[derive(Clone)]
pub struct WechatShopClient {
    http: Client,
}

#[derive(Debug, Serialize)]
pub struct StableAccessToken {
    pub access_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize)]
pub struct WechatApiError {
    pub errcode: i64,
    pub errmsg: String,
}

#[derive(Debug, Serialize)]
pub enum WechatCallResult<T> {
    Success(T),
    ApiError(WechatApiError),
}

#[derive(Debug, Serialize)]
pub struct WechatCallMeta {
    pub endpoint: &'static str,
    pub method: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StableTokenCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<StableAccessToken>,
}

#[derive(Debug, Serialize)]
pub struct ShopBasicInfoCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ShopBasicInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ShopBasicInfo {
    pub nickname: Option<String>,
    pub headimg_url: Option<String>,
    pub subject_type: Option<String>,
    pub status: Option<String>,
    pub username: Option<String>,
    pub is_local_life: Option<i64>,
    pub open_timestamp: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ApiQuotaCall {
    pub meta: WechatCallMeta,
    pub cgi_path: String,
    pub result: WechatCallResult<ApiQuotaInfo>,
}

#[derive(Debug, Serialize)]
pub struct ImageUploadCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ImageUploadResult>,
}

#[derive(Debug, Serialize)]
pub struct ProductAddCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ProductAddResult>,
}

#[derive(Debug, Serialize)]
pub struct ProductUpdateCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ProductUpdateResult>,
}

#[derive(Debug, Serialize)]
pub struct ProductGetCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ProductGetInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProductListingCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ProductListingResult>,
}

#[derive(Debug, Serialize)]
pub struct CategoryPrecheckCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<CategoryPrecheckResult>,
}

#[derive(Debug, Serialize)]
pub struct OrderListCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<OrderListResult>,
}

#[derive(Debug, Serialize)]
pub struct OrderGetCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<OrderGetResult>,
}

#[derive(Debug, Serialize)]
pub struct OrderPriceUpdateCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<OrderPriceUpdateResult>,
}

#[derive(Debug, Serialize)]
pub struct SendDeliveryCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<SendDeliveryResult>,
}

#[derive(Debug, Serialize)]
pub struct AftersaleListCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<AftersaleListResult>,
}

#[derive(Debug, Serialize)]
pub struct AftersaleGetCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<AftersaleGetResult>,
}

#[derive(Debug, Serialize)]
pub struct WechatRawCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<WechatRawResult>,
}

#[derive(Debug, Serialize)]
pub struct MerchantAddressListCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<MerchantAddressListResult>,
}

#[derive(Debug, Serialize)]
pub struct MerchantAddressDetailCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<MerchantAddressDetailSummary>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WechatRawResult {
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MerchantAddressListResult {
    pub address_ids: Vec<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MerchantAddressDetailSummary {
    pub address_id: i64,
    pub send_addr: bool,
    pub default_send: bool,
    pub recv_addr: bool,
    pub default_recv: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageUploadResult {
    pub media_id: Option<String>,
    pub pay_media_id: Option<String>,
    pub img_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProductAddResult {
    pub product_id: String,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProductUpdateResult {
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProductGetInfo {
    pub product: Option<WechatProductSnapshot>,
    pub edit_product: Option<WechatProductSnapshot>,
    pub audit_info: Option<serde_json::Value>,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProductListingResult {
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CategoryPrecheckResult {
    pub all_pass: bool,
    pub fail_reasons: Vec<String>,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrderListResult {
    pub order_id_list: Vec<String>,
    pub next_key: Option<String>,
    pub has_more: bool,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrderGetResult {
    pub order: serde_json::Value,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrderPriceUpdateResult {
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SendDeliveryResult {
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AftersaleListResult {
    pub after_sale_order_id_list: Vec<String>,
    pub next_key: Option<String>,
    pub has_more: bool,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AftersaleGetResult {
    pub after_sale_order: serde_json::Value,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WechatProductSnapshot {
    pub product_id: Option<serde_json::Value>,
    pub out_product_id: Option<String>,
    pub status: Option<i64>,
    pub edit_status: Option<i64>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiQuotaInfo {
    pub quota: Option<ApiQuotaCounter>,
    pub rate_limit: Option<ApiRateLimit>,
    pub component_rate_limit: Option<ApiRateLimit>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiQuotaCounter {
    pub daily_limit: Option<i64>,
    pub used: Option<i64>,
    pub remain: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiRateLimit {
    pub call_count: Option<i64>,
    pub refresh_second: Option<i64>,
}

#[derive(Debug, Serialize)]
struct StableTokenRequest<'a> {
    grant_type: &'static str,
    appid: &'a str,
    secret: &'a str,
    force_refresh: bool,
}

#[derive(Debug, Deserialize)]
struct StableTokenResponse {
    access_token: Option<String>,
    expires_in: Option<i64>,
    errcode: Option<i64>,
    errmsg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ShopBasicInfoResponse {
    errcode: i64,
    errmsg: String,
    info: Option<ShopBasicInfo>,
}

#[derive(Debug, Serialize)]
struct ApiQuotaRequest<'a> {
    cgi_path: &'a str,
}

#[derive(Debug, Deserialize)]
struct ApiQuotaResponse {
    errcode: i64,
    errmsg: String,
    quota: Option<ApiQuotaCounter>,
    rate_limit: Option<ApiRateLimit>,
    component_rate_limit: Option<ApiRateLimit>,
}

#[derive(Debug, Deserialize)]
struct ImageUploadResponse {
    errcode: i64,
    errmsg: String,
    pic_file: Option<ImageUploadPicFile>,
}

#[derive(Debug, Deserialize)]
struct ImageUploadPicFile {
    media_id: Option<String>,
    pay_media_id: Option<String>,
    img_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProductAddResponse {
    errcode: i64,
    errmsg: String,
    product_id: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ProductUpdateResponse {
    errcode: i64,
    errmsg: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ProductGetRequest<'a> {
    product_id: &'a str,
    data_type: i64,
}

#[derive(Debug, Deserialize)]
struct ProductGetResponse {
    errcode: i64,
    errmsg: String,
    product: Option<WechatProductSnapshot>,
    edit_product: Option<WechatProductSnapshot>,
    audit_info: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ProductListingRequest<'a> {
    product_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct ProductListingResponse {
    errcode: i64,
    errmsg: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct CategoryPrecheckRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    cat_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CategoryPrecheckResponse {
    errcode: i64,
    errmsg: String,
    all_pass: Option<bool>,
    fail_reasons: Option<Vec<serde_json::Value>>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct OrderListRequest<'a> {
    create_time_range: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_time_range: Option<TimeRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<i64>,
    page_size: i64,
    next_key: &'a str,
}

#[derive(Debug, Serialize)]
struct TimeRange {
    start_time: i64,
    end_time: i64,
}

#[derive(Debug, Deserialize)]
struct OrderListResponse {
    errcode: i64,
    errmsg: String,
    order_id_list: Option<Vec<serde_json::Value>>,
    next_key: Option<String>,
    has_more: Option<bool>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct OrderGetRequest<'a> {
    order_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct OrderGetResponse {
    errcode: i64,
    errmsg: String,
    order: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct OrderPriceUpdateRequest<'a> {
    order_id: &'a str,
    change_order_infos: &'a [OrderPriceUpdateInfo],
    change_express: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    express_fee: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OrderPriceUpdateInfo {
    pub product_id: String,
    pub sku_id: String,
    pub change_price: i64,
}

#[derive(Debug, Deserialize)]
struct OrderPriceUpdateResponse {
    errcode: i64,
    errmsg: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SendDeliveryResponse {
    errcode: i64,
    errmsg: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct DeliveryCompanyListRequest {
    ewaybill_only: bool,
}

#[derive(Debug, Serialize)]
struct AftersaleListRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    begin_create_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_create_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    begin_update_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_update_time: Option<i64>,
    next_key: &'a str,
}

#[derive(Debug, Deserialize)]
struct AftersaleListResponse {
    errcode: i64,
    errmsg: String,
    after_sale_order_id_list: Option<Vec<serde_json::Value>>,
    next_key: Option<String>,
    has_more: Option<bool>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct AftersaleGetRequest<'a> {
    after_sale_order_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct AftersaleGetResponse {
    errcode: i64,
    errmsg: String,
    after_sale_order: Option<serde_json::Value>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct AftersaleAcceptRequest<'a> {
    after_sale_order_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accept_type: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AftersaleRejectRequest<'a> {
    after_sale_order_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reject_reason: Option<&'a str>,
    reject_reason_type: i64,
}

#[derive(Debug, Serialize)]
struct GuaranteeSearchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    begin_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<i64>,
    offset: i64,
    limit: i64,
}

#[derive(Debug, Serialize)]
struct GuaranteeGetRequest<'a> {
    guarantee_order_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct RawWechatResponse {
    errcode: i64,
    errmsg: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct CategoryDetailRequest {
    cat_id: i64,
}

#[derive(Debug, Serialize)]
struct CategoryRelationListRequest {
    is_filter_status: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CategoryRelationDetailRequest {
    category_id: i64,
}

#[derive(Debug, Serialize)]
struct CategoryProductRuleRequest {
    cat_id: i64,
    release_mode: i64,
}

#[derive(Debug, Serialize)]
struct CategoryDeliveryRuleRequest {
    category_id: i64,
}

#[derive(Debug, Serialize)]
struct FreightTemplateListRequest {
    offset: i64,
    limit: i64,
}

#[derive(Debug, Serialize)]
struct MerchantAddressListRequest {
    offset: i64,
    limit: i64,
}

#[derive(Debug, Deserialize)]
struct MerchantAddressListResponse {
    errcode: i64,
    errmsg: String,
    #[serde(default)]
    address_id_list: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct MerchantAddressGetRequest {
    address_id: i64,
}

#[derive(Debug, Deserialize)]
struct MerchantAddressGetResponse {
    errcode: i64,
    errmsg: String,
    address_detail: Option<serde_json::Value>,
}

impl Default for WechatShopClient {
    fn default() -> Self {
        Self {
            http: Client::new(),
        }
    }
}

impl WechatShopClient {
    pub async fn get_stable_access_token(
        &self,
        appid: &str,
        secret: &str,
        force_refresh: bool,
    ) -> AppResult<StableTokenCall> {
        let payload = StableTokenRequest {
            grant_type: "client_credential",
            appid,
            secret,
            force_refresh,
        };
        let response = self
            .http
            .post(STABLE_TOKEN_URL)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json::<StableTokenResponse>()
            .await?;

        let result = match (response.access_token, response.expires_in) {
            (Some(access_token), Some(expires_in)) if !access_token.is_empty() => {
                WechatCallResult::Success(StableAccessToken {
                    access_token,
                    expires_in,
                })
            }
            _ => WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode.unwrap_or(-999_999),
                errmsg: response
                    .errmsg
                    .unwrap_or_else(|| "微信返回缺少 access_token".to_string()),
            }),
        };

        Ok(StableTokenCall {
            meta: WechatCallMeta {
                endpoint: STABLE_TOKEN_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_shop_basic_info(&self, access_token: &str) -> AppResult<ShopBasicInfoCall> {
        let mut url = Url::parse(SHOP_BASIC_INFO_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<ShopBasicInfoResponse>()
            .await?;
        let result = if response.errcode == 0 {
            WechatCallResult::Success(response.info.unwrap_or(ShopBasicInfo {
                nickname: None,
                headimg_url: None,
                subject_type: None,
                status: None,
                username: None,
                is_local_life: None,
                open_timestamp: None,
            }))
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ShopBasicInfoCall {
            meta: WechatCallMeta {
                endpoint: SHOP_BASIC_INFO_URL,
                method: "GET",
            },
            result,
        })
    }

    pub async fn get_api_quota(
        &self,
        access_token: &str,
        cgi_path: &str,
    ) -> AppResult<ApiQuotaCall> {
        let mut url = Url::parse(API_QUOTA_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&ApiQuotaRequest { cgi_path })
            .send()
            .await?
            .error_for_status()?
            .json::<ApiQuotaResponse>()
            .await?;
        let result = if response.errcode == 0 {
            WechatCallResult::Success(ApiQuotaInfo {
                quota: response.quota,
                rate_limit: response.rate_limit,
                component_rate_limit: response.component_rate_limit,
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ApiQuotaCall {
            meta: WechatCallMeta {
                endpoint: API_QUOTA_URL,
                method: "POST",
            },
            cgi_path: cgi_path.to_string(),
            result,
        })
    }

    pub async fn upload_image_bytes(
        &self,
        access_token: &str,
        bytes: Vec<u8>,
        file_name: &str,
        mime_type: &str,
        width: u32,
        height: u32,
    ) -> AppResult<ImageUploadCall> {
        let mut url = Url::parse(IMAGE_UPLOAD_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token)
            .append_pair("upload_type", "0")
            .append_pair("resp_type", "1")
            .append_pair("height", &height.to_string())
            .append_pair("width", &width.to_string());

        let part = multipart::Part::bytes(bytes)
            .file_name(file_name.to_string())
            .mime_str(mime_type)
            .map_err(|error| AppError::Validation(format!("图片 MIME 类型不合法: {error}")))?;
        let form = multipart::Form::new().part("media", part);
        let response = self
            .http
            .post(url)
            .multipart(form)
            .send()
            .await?
            .error_for_status()?
            .json::<ImageUploadResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let pic_file = response.pic_file.unwrap_or(ImageUploadPicFile {
                media_id: None,
                pay_media_id: None,
                img_url: None,
            });
            match pic_file.img_url {
                Some(img_url) if !img_url.trim().is_empty() => {
                    WechatCallResult::Success(ImageUploadResult {
                        media_id: pic_file.media_id,
                        pay_media_id: pic_file.pay_media_id,
                        img_url,
                    })
                }
                _ => WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_998,
                    errmsg: "微信图片上传成功响应缺少 pic_file.img_url".to_string(),
                }),
            }
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ImageUploadCall {
            meta: WechatCallMeta {
                endpoint: IMAGE_UPLOAD_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn add_product(
        &self,
        access_token: &str,
        payload: &serde_json::Value,
    ) -> AppResult<ProductAddCall> {
        let mut url = Url::parse(PRODUCT_ADD_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(payload)
            .send()
            .await?
            .error_for_status()?
            .json::<ProductAddResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let product_id = product_add_response_product_id(&response);
            match product_id {
                Some(product_id) if !product_id.trim().is_empty() => {
                    let mut raw_payload = serde_json::Map::new();
                    raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
                    raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
                    if let Some(value) = response.product_id {
                        raw_payload.insert("product_id".to_string(), value);
                    }
                    raw_payload.extend(response.extra);
                    WechatCallResult::Success(ProductAddResult {
                        product_id,
                        raw_payload: serde_json::Value::Object(raw_payload),
                    })
                }
                _ => WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_997,
                    errmsg: "微信添加商品成功响应缺少 product_id".to_string(),
                }),
            }
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ProductAddCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_ADD_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn update_product(
        &self,
        access_token: &str,
        payload: &serde_json::Value,
    ) -> AppResult<ProductUpdateCall> {
        let mut url = Url::parse(PRODUCT_UPDATE_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(payload)
            .send()
            .await?
            .error_for_status()?
            .json::<ProductUpdateResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.extend(response.extra);
            WechatCallResult::Success(ProductUpdateResult {
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ProductUpdateCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_UPDATE_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_product(
        &self,
        access_token: &str,
        product_id: &str,
        data_type: i64,
    ) -> AppResult<ProductGetCall> {
        let mut url = Url::parse(PRODUCT_GET_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&ProductGetRequest {
                product_id,
                data_type,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<ProductGetResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            if let Some(product) = &response.product {
                raw_payload.insert(
                    "product".to_string(),
                    serde_json::to_value(product).unwrap_or(serde_json::Value::Null),
                );
            }
            if let Some(edit_product) = &response.edit_product {
                raw_payload.insert(
                    "edit_product".to_string(),
                    serde_json::to_value(edit_product).unwrap_or(serde_json::Value::Null),
                );
            }
            if let Some(audit_info) = &response.audit_info {
                raw_payload.insert("audit_info".to_string(), audit_info.clone());
            }
            raw_payload.extend(response.extra);
            WechatCallResult::Success(ProductGetInfo {
                product: response.product,
                edit_product: response.edit_product,
                audit_info: response.audit_info,
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ProductGetCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_GET_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn listing_product(
        &self,
        access_token: &str,
        product_id: &str,
    ) -> AppResult<ProductListingCall> {
        let mut url = Url::parse(PRODUCT_LISTING_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&ProductListingRequest { product_id })
            .send()
            .await?
            .error_for_status()?
            .json::<ProductListingResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.extend(response.extra);
            WechatCallResult::Success(ProductListingResult {
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ProductListingCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_LISTING_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn category_precheck(
        &self,
        access_token: &str,
        cat_id: Option<i64>,
    ) -> AppResult<CategoryPrecheckCall> {
        let mut url = Url::parse(CATEGORY_PRECHECK_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&CategoryPrecheckRequest { cat_id })
            .send()
            .await?
            .error_for_status()?
            .json::<CategoryPrecheckResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let fail_reason_values = response.fail_reasons.unwrap_or_default();
            let fail_reasons = fail_reason_values
                .iter()
                .map(wechat_fail_reason_to_string)
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>();
            let all_pass = response.all_pass.unwrap_or_else(|| fail_reasons.is_empty());
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.insert("all_pass".to_string(), serde_json::json!(all_pass));
            raw_payload.insert(
                "fail_reasons".to_string(),
                serde_json::Value::Array(fail_reason_values),
            );
            raw_payload.extend(response.extra);
            WechatCallResult::Success(CategoryPrecheckResult {
                all_pass,
                fail_reasons,
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(CategoryPrecheckCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_PRECHECK_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_order_list(
        &self,
        access_token: &str,
        start_time: i64,
        end_time: i64,
        status: Option<i64>,
        page_size: i64,
        next_key: &str,
    ) -> AppResult<OrderListCall> {
        let mut url = Url::parse(ORDER_LIST_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&OrderListRequest {
                create_time_range: TimeRange {
                    start_time,
                    end_time,
                },
                update_time_range: None,
                status,
                page_size,
                next_key,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<OrderListResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let order_id_values = response.order_id_list.unwrap_or_default();
            let order_id_list = order_id_values
                .iter()
                .filter_map(json_value_to_string)
                .collect::<Vec<_>>();
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.insert(
                "order_id_list".to_string(),
                serde_json::Value::Array(order_id_values),
            );
            if let Some(next_key) = &response.next_key {
                raw_payload.insert("next_key".to_string(), serde_json::json!(next_key));
            }
            raw_payload.insert(
                "has_more".to_string(),
                serde_json::json!(response.has_more.unwrap_or(false)),
            );
            raw_payload.extend(response.extra);
            WechatCallResult::Success(OrderListResult {
                order_id_list,
                next_key: response.next_key,
                has_more: response.has_more.unwrap_or(false),
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(OrderListCall {
            meta: WechatCallMeta {
                endpoint: ORDER_LIST_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_order(&self, access_token: &str, order_id: &str) -> AppResult<OrderGetCall> {
        let mut url = Url::parse(ORDER_GET_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&OrderGetRequest { order_id })
            .send()
            .await?
            .error_for_status()?
            .json::<OrderGetResponse>()
            .await?;

        let result = if response.errcode == 0 {
            match response.order {
                Some(order) => {
                    let mut raw_payload = serde_json::Map::new();
                    raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
                    raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
                    raw_payload.insert("order".to_string(), order.clone());
                    raw_payload.extend(response.extra);
                    WechatCallResult::Success(OrderGetResult {
                        order,
                        raw_payload: serde_json::Value::Object(raw_payload),
                    })
                }
                None => WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_996,
                    errmsg: "微信订单详情成功响应缺少 order".to_string(),
                }),
            }
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(OrderGetCall {
            meta: WechatCallMeta {
                endpoint: ORDER_GET_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn change_order_price(
        &self,
        access_token: &str,
        order_id: &str,
        change_order_infos: &[OrderPriceUpdateInfo],
        change_express: bool,
        express_fee: Option<i64>,
    ) -> AppResult<OrderPriceUpdateCall> {
        let mut url = Url::parse(ORDER_PRICE_UPDATE_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&OrderPriceUpdateRequest {
                order_id,
                change_order_infos,
                change_express,
                express_fee,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<OrderPriceUpdateResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.extend(response.extra);
            WechatCallResult::Success(OrderPriceUpdateResult {
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(OrderPriceUpdateCall {
            meta: WechatCallMeta {
                endpoint: ORDER_PRICE_UPDATE_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn send_delivery(
        &self,
        access_token: &str,
        payload: &serde_json::Value,
    ) -> AppResult<SendDeliveryCall> {
        let mut url = Url::parse(SEND_DELIVERY_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(payload)
            .send()
            .await?
            .error_for_status()?
            .json::<SendDeliveryResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.extend(response.extra);
            WechatCallResult::Success(SendDeliveryResult {
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(SendDeliveryCall {
            meta: WechatCallMeta {
                endpoint: SEND_DELIVERY_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_delivery_company_list(
        &self,
        access_token: &str,
        ewaybill_only: bool,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(DELIVERY_COMPANY_LIST_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&DeliveryCompanyListRequest { ewaybill_only })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: DELIVERY_COMPANY_LIST_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_aftersale_list(
        &self,
        access_token: &str,
        begin_update_time: i64,
        end_update_time: i64,
        next_key: &str,
    ) -> AppResult<AftersaleListCall> {
        let mut url = Url::parse(AFTERSALE_LIST_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&AftersaleListRequest {
                begin_create_time: None,
                end_create_time: None,
                begin_update_time: Some(begin_update_time),
                end_update_time: Some(end_update_time),
                next_key,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<AftersaleListResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let aftersale_id_values = response.after_sale_order_id_list.unwrap_or_default();
            let after_sale_order_id_list = aftersale_id_values
                .iter()
                .filter_map(json_value_to_string)
                .collect::<Vec<_>>();
            let mut raw_payload = serde_json::Map::new();
            raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
            raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
            raw_payload.insert(
                "after_sale_order_id_list".to_string(),
                serde_json::Value::Array(aftersale_id_values),
            );
            if let Some(next_key) = &response.next_key {
                raw_payload.insert("next_key".to_string(), serde_json::json!(next_key));
            }
            raw_payload.insert(
                "has_more".to_string(),
                serde_json::json!(response.has_more.unwrap_or(false)),
            );
            raw_payload.extend(response.extra);
            WechatCallResult::Success(AftersaleListResult {
                after_sale_order_id_list,
                next_key: response.next_key,
                has_more: response.has_more.unwrap_or(false),
                raw_payload: serde_json::Value::Object(raw_payload),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(AftersaleListCall {
            meta: WechatCallMeta {
                endpoint: AFTERSALE_LIST_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_aftersale_order(
        &self,
        access_token: &str,
        after_sale_order_id: &str,
    ) -> AppResult<AftersaleGetCall> {
        let mut url = Url::parse(AFTERSALE_GET_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&AftersaleGetRequest {
                after_sale_order_id,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<AftersaleGetResponse>()
            .await?;

        let result = if response.errcode == 0 {
            match response.after_sale_order {
                Some(after_sale_order) => {
                    let mut raw_payload = serde_json::Map::new();
                    raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
                    raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
                    raw_payload.insert("after_sale_order".to_string(), after_sale_order.clone());
                    raw_payload.extend(response.extra);
                    WechatCallResult::Success(AftersaleGetResult {
                        after_sale_order,
                        raw_payload: serde_json::Value::Object(raw_payload),
                    })
                }
                None => WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_995,
                    errmsg: "微信售后单详情成功响应缺少 after_sale_order".to_string(),
                }),
            }
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(AftersaleGetCall {
            meta: WechatCallMeta {
                endpoint: AFTERSALE_GET_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn accept_aftersale(
        &self,
        access_token: &str,
        after_sale_order_id: &str,
        address_id: Option<&str>,
        accept_type: Option<i64>,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(AFTERSALE_ACCEPT_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&AftersaleAcceptRequest {
                after_sale_order_id,
                address_id,
                accept_type,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: AFTERSALE_ACCEPT_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn reject_aftersale(
        &self,
        access_token: &str,
        after_sale_order_id: &str,
        reject_reason_type: i64,
        reject_reason: Option<&str>,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(AFTERSALE_REJECT_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&AftersaleRejectRequest {
                after_sale_order_id,
                reject_reason,
                reject_reason_type,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: AFTERSALE_REJECT_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_aftersale_reject_reasons(
        &self,
        access_token: &str,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(AFTERSALE_REJECT_REASON_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&serde_json::json!({}))
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: AFTERSALE_REJECT_REASON_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn search_guarantee_orders(
        &self,
        access_token: &str,
        begin_time: i64,
        end_time: i64,
        offset: i64,
        limit: i64,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(GUARANTEE_SEARCH_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&GuaranteeSearchRequest {
                begin_time: Some(begin_time),
                end_time: Some(end_time),
                r#type: Some(0),
                offset,
                limit,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: GUARANTEE_SEARCH_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_guarantee_order(
        &self,
        access_token: &str,
        guarantee_order_id: &str,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(GUARANTEE_GET_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&GuaranteeGetRequest { guarantee_order_id })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: GUARANTEE_GET_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_category_relation_list(
        &self,
        access_token: &str,
        status: Option<i64>,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(CATEGORY_RELATION_LIST_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&CategoryRelationListRequest {
                is_filter_status: status.is_some(),
                status,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_RELATION_LIST_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_category_relation_detail(
        &self,
        access_token: &str,
        category_id: i64,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(CATEGORY_RELATION_DETAIL_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&CategoryRelationDetailRequest { category_id })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_RELATION_DETAIL_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_all_categories(&self, access_token: &str) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(CATEGORY_ALL_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_ALL_URL,
                method: "GET",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_category_detail(
        &self,
        access_token: &str,
        cat_id: i64,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(CATEGORY_DETAIL_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&CategoryDetailRequest { cat_id })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_DETAIL_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_category_product_rule(
        &self,
        access_token: &str,
        cat_id: i64,
        release_mode: i64,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(CATEGORY_PRODUCT_RULE_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&CategoryProductRuleRequest {
                cat_id,
                release_mode,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_PRODUCT_RULE_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_delivery_method_category_rule(
        &self,
        access_token: &str,
        category_id: i64,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(CATEGORY_DELIVERY_RULE_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token)
            .append_pair("rule_id", "2");

        let response = self
            .http
            .post(url)
            .json(&CategoryDeliveryRuleRequest { category_id })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: CATEGORY_DELIVERY_RULE_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn get_freight_template_list(
        &self,
        access_token: &str,
        offset: i64,
        limit: i64,
    ) -> AppResult<WechatRawCall> {
        let mut url = Url::parse(FREIGHT_TEMPLATE_LIST_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&FreightTemplateListRequest { offset, limit })
            .send()
            .await?
            .error_for_status()?
            .json::<RawWechatResponse>()
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: FREIGHT_TEMPLATE_LIST_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    pub async fn list_merchant_addresses(
        &self,
        access_token: &str,
        offset: i64,
        limit: i64,
    ) -> AppResult<MerchantAddressListCall> {
        let mut url = Url::parse(MERCHANT_ADDRESS_LIST_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&MerchantAddressListRequest { offset, limit })
            .send()
            .await?
            .error_for_status()?
            .json::<MerchantAddressListResponse>()
            .await?;

        let result = if response.errcode == 0 {
            let mut address_ids = response
                .address_id_list
                .iter()
                .filter_map(|value| wechat_json_value_to_i64(Some(value)))
                .filter(|address_id| *address_id > 0)
                .collect::<Vec<_>>();
            address_ids.sort_unstable();
            address_ids.dedup();
            WechatCallResult::Success(MerchantAddressListResult { address_ids })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(MerchantAddressListCall {
            meta: WechatCallMeta {
                endpoint: MERCHANT_ADDRESS_LIST_URL,
                method: "POST",
            },
            result,
        })
    }

    pub async fn get_merchant_address(
        &self,
        access_token: &str,
        address_id: i64,
    ) -> AppResult<MerchantAddressDetailCall> {
        let mut url = Url::parse(MERCHANT_ADDRESS_GET_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);

        let response = self
            .http
            .post(url)
            .json(&MerchantAddressGetRequest { address_id })
            .send()
            .await?
            .error_for_status()?
            .json::<MerchantAddressGetResponse>()
            .await?;

        let result = if response.errcode == 0 {
            match merchant_address_detail_summary(response.address_detail.as_ref(), address_id) {
                Ok(detail) => WechatCallResult::Success(detail),
                Err(errmsg) => WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_999,
                    errmsg,
                }),
            }
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(MerchantAddressDetailCall {
            meta: WechatCallMeta {
                endpoint: MERCHANT_ADDRESS_GET_URL,
                method: "POST",
            },
            result,
        })
    }
}

fn raw_wechat_result(mut response: RawWechatResponse) -> WechatCallResult<WechatRawResult> {
    if response.errcode == 0 {
        let mut raw_payload = serde_json::Map::new();
        raw_payload.insert("errcode".to_string(), serde_json::json!(response.errcode));
        raw_payload.insert("errmsg".to_string(), serde_json::json!(response.errmsg));
        raw_payload.extend(std::mem::take(&mut response.extra));
        WechatCallResult::Success(WechatRawResult {
            raw_payload: serde_json::Value::Object(raw_payload),
        })
    } else {
        WechatCallResult::ApiError(WechatApiError {
            errcode: response.errcode,
            errmsg: response.errmsg,
        })
    }
}

fn product_add_response_product_id(response: &ProductAddResponse) -> Option<String> {
    response
        .product_id
        .as_ref()
        .or_else(|| response.extra.get("product_id"))
        .or_else(|| {
            response
                .extra
                .get("data")
                .and_then(|data| data.get("product_id"))
        })
        .or_else(|| {
            response
                .extra
                .get("data")
                .and_then(|data| data.get("product"))
                .and_then(|product| product.get("product_id"))
        })
        .or_else(|| {
            response
                .extra
                .get("product")
                .and_then(|product| product.get("product_id"))
        })
        .or_else(|| {
            response
                .extra
                .get("product_info")
                .and_then(|product| product.get("product_id"))
        })
        .or_else(|| {
            response
                .extra
                .get("result")
                .and_then(|result| result.get("product_id"))
        })
        .and_then(json_value_to_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn merchant_address_detail_summary(
    value: Option<&serde_json::Value>,
    fallback_address_id: i64,
) -> Result<MerchantAddressDetailSummary, String> {
    let detail = value
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "微信地址详情缺少 address_detail".to_string())?;
    let address_id =
        wechat_json_value_to_i64(detail.get("address_id")).unwrap_or(fallback_address_id);
    if address_id <= 0 {
        return Err("微信地址详情缺少有效 address_id".to_string());
    }
    Ok(MerchantAddressDetailSummary {
        address_id,
        send_addr: wechat_json_value_to_bool(detail.get("send_addr")),
        default_send: wechat_json_value_to_bool(detail.get("default_send")),
        recv_addr: wechat_json_value_to_bool(detail.get("recv_addr")),
        default_recv: wechat_json_value_to_bool(detail.get("default_recv")),
    })
}

fn wechat_json_value_to_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok())),
        serde_json::Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn wechat_json_value_to_bool(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::Number(value)) => value.as_i64().unwrap_or_default() != 0,
        Some(serde_json::Value::String(value)) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        _ => false,
    }
}

fn json_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn wechat_fail_reason_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Object(object) => {
            for key in ["reason", "desc", "message", "errmsg", "name"] {
                if let Some(text) = object.get(key).and_then(json_value_to_string) {
                    return text;
                }
            }
            value.to_string()
        }
        _ => value.to_string(),
    }
}

impl From<&WechatApiError> for AppError {
    fn from(value: &WechatApiError) -> Self {
        AppError::WechatApi {
            errcode: value.errcode,
            errmsg: value.errmsg.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_add_response_product_id_reads_nested_data() {
        let response: ProductAddResponse = serde_json::from_value(serde_json::json!({
            "errcode": 0,
            "errmsg": "ok",
            "data": {
                "product_id": "123456"
            }
        }))
        .expect("响应应可解析");

        assert_eq!(
            product_add_response_product_id(&response).as_deref(),
            Some("123456")
        );
    }
}
