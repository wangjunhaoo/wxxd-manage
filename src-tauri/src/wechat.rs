use crate::storage::{AppError, AppResult};
use reqwest::{multipart, Client, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const STABLE_TOKEN_URL: &str = "https://api.weixin.qq.com/cgi-bin/stable_token";
const SHOP_BASIC_INFO_URL: &str = "https://api.weixin.qq.com/channels/ec/basics/info/get";
const API_QUOTA_URL: &str = "https://api.weixin.qq.com/cgi-bin/openapi/quota/get";
const IMAGE_UPLOAD_URL: &str = "https://api.weixin.qq.com/shop/ec/basics/img/upload";
const VIDEO_INIT_UPLOAD_URL: &str = "https://api.weixin.qq.com/shop/ec/basics/video/initupload";
const VIDEO_UPLOAD_PART_URL: &str = "https://api.weixin.qq.com/shop/ec/basics/video/uploadpart";
const VIDEO_FINISH_UPLOAD_URL: &str = "https://api.weixin.qq.com/shop/ec/basics/video/finishupload";
const VIDEO_GET_PLAY_INFO_URL: &str = "https://api.weixin.qq.com/shop/ec/basics/video/getplayinfo";
const PRODUCT_ADD_URL: &str = "https://api.weixin.qq.com/channels/ec/product/add";
const PRODUCT_UPDATE_URL: &str = "https://api.weixin.qq.com/channels/ec/product/update";
const PRODUCT_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/product/get";
const PRODUCT_LISTING_URL: &str = "https://api.weixin.qq.com/channels/ec/product/listing";
const CATEGORY_PRECHECK_URL: &str =
    "https://api.weixin.qq.com/channels/ec/product/categoryprecheck";
const ORDER_LIST_URL: &str = "https://api.weixin.qq.com/channels/ec/order/list/get";
const ORDER_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/order/get";
const ORDER_PRICE_UPDATE_URL: &str = "https://api.weixin.qq.com/channels/ec/order/price/update";
const ORDER_SEARCH_URL: &str = "https://api.weixin.qq.com/channels/ec/order/search";
const ORDER_SENSITIVE_DECODE_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/sensitiveinfo/decode";
const ORDER_CHANGESKU_GET_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/preshipmentchangesku/get";
const ORDER_CHANGESKU_APPROVE_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/preshipmentchangesku/approve";
const ORDER_CHANGESKU_REJECT_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/preshipmentchangesku/reject";
const ORDER_ADDRESS_MODIFY_ACCEPT_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/addressmodify/accept";
const ORDER_ADDRESS_MODIFY_REJECT_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/addressmodify/reject";
const ORDER_DELIVERY_COMPENSATION_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/delivery/compensation";
const ORDER_DELIVERY_INFO_UPDATE_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/deliveryinfo/update";
const ORDER_MERCHANT_NOTES_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/merchantnotes/update";
const ORDER_VIRTUAL_TEL_DELAY_URL: &str =
    "https://api.weixin.qq.com/channels/ec/order/virtualnumber/delay";
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
const FREIGHT_TEMPLATE_DETAIL_URL: &str =
    "https://api.weixin.qq.com/channels/ec/merchant/getfreighttemplatedetail";
const MERCHANT_ADDRESS_LIST_URL: &str =
    "https://api.weixin.qq.com/channels/ec/merchant/address/list";
const MERCHANT_ADDRESS_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/merchant/address/get";
const PRODUCT_LIST_URL: &str = "https://api.weixin.qq.com/channels/ec/product/list/get";
const PRODUCT_DELISTING_URL: &str = "https://api.weixin.qq.com/channels/ec/product/delisting";
const PRODUCT_DELETE_URL: &str = "https://api.weixin.qq.com/channels/ec/product/delete";
const STOCK_GET_URL: &str = "https://api.weixin.qq.com/channels/ec/product/stock/get";
const STOCK_BATCHGET_URL: &str = "https://api.weixin.qq.com/channels/ec/product/stock/batchget";
const STOCK_UPDATE_URL: &str = "https://api.weixin.qq.com/channels/ec/product/stock/update";

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
struct VideoInitUploadResponse {
    errcode: i64,
    errmsg: String,
    data: Option<VideoInitUploadData>,
}

#[derive(Debug, Deserialize)]
struct VideoInitUploadData {
    video_upload_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VideoUploadPartResponse {
    errcode: i64,
    errmsg: String,
    data: Option<VideoUploadPartData>,
}

#[derive(Debug, Deserialize)]
struct VideoUploadPartData {
    part_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VideoFinishUploadResponse {
    errcode: i64,
    errmsg: String,
}

#[derive(Debug, Deserialize)]
struct VideoGetPlayInfoResponse {
    errcode: i64,
    errmsg: String,
    data: Option<VideoGetPlayInfoData>,
}

#[derive(Debug, Deserialize)]
struct VideoGetPlayInfoData {
    url: Option<String>,
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

/// 订单列表的时间过滤轴。官方文档：create_time_range / update_time_range 二选一至少传一个，
/// 单窗跨度 ≤7 天。增量同步走 Update（状态变化/改价/取消/完成都体现在 update_time 上），
/// 每日补漏兜底走 Create（对冲「不传 status 行为未明」的风险）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderListTimeField {
    Create,
    Update,
}

#[derive(Debug, Serialize)]
struct OrderListRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    create_time_range: Option<TimeRange>,
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
struct FreightTemplateDetailRequest<'a> {
    template_id: &'a str,
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

// ===== 商品管理（列表 / 下架 / 删除）与库存（查询 / 批量 / 更新）相关结构 =====

#[derive(Debug, Serialize)]
pub struct ProductListCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ProductListResult>,
}

/// 下架 / 删除 / 改库存这类只回 errcode/errmsg 的写操作统一调用结果。
#[derive(Debug, Serialize)]
pub struct ProductMutationCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<ProductMutationResult>,
}

#[derive(Debug, Serialize)]
pub struct StockGetCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<StockInfo>,
}

#[derive(Debug, Serialize)]
pub struct StockBatchGetCall {
    pub meta: WechatCallMeta,
    pub result: WechatCallResult<StockBatchInfo>,
}

/// 获取商品列表：游标分页，仅返回商品 id 列表（详情需再调 get_product）。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProductListResult {
    pub product_ids: Vec<String>,
    pub next_key: Option<String>,
    pub total_num: i64,
}

/// 下架 / 删除 / 改库存这类只回 errcode/errmsg 的写操作统一结果。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProductMutationResult {
    pub raw_payload: serde_json::Value,
}

/// 单 SKU 库存查询结果（normal=通用库存，total=通用+区域总量）。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockInfo {
    pub normal_stock_num: i64,
    pub total_stock_num: i64,
    pub raw_payload: serde_json::Value,
}

/// 批量库存查询结果（spu→sku→warehouse 三层，原样透传给上层解析）。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockBatchInfo {
    pub spu_stock_list: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ProductListRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<i64>,
    page_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_key: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct ProductIdRequest<'a> {
    product_id: &'a str,
}

#[derive(Debug, Serialize)]
struct StockGetRequest<'a> {
    product_id: &'a str,
    sku_id: &'a str,
}

#[derive(Debug, Serialize)]
struct StockBatchGetRequest<'a> {
    product_id: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    stock_type: Option<i64>,
}

#[derive(Debug, Serialize)]
struct StockUpdateRequest<'a> {
    product_id: &'a str,
    sku_id: &'a str,
    diff_type: i64,
    num: i64,
}

#[derive(Debug, Deserialize)]
struct ProductListResponse {
    errcode: i64,
    errmsg: String,
    #[serde(default)]
    product_ids: Vec<serde_json::Value>,
    #[serde(default)]
    next_key: Option<String>,
    #[serde(default)]
    total_num: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ProductMutationResponse {
    errcode: i64,
    errmsg: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StockGetResponse {
    errcode: i64,
    errmsg: String,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StockBatchGetResponse {
    errcode: i64,
    errmsg: String,
    data: Option<StockBatchData>,
}

#[derive(Debug, Deserialize)]
struct StockBatchData {
    #[serde(default)]
    spu_stock_list: Vec<serde_json::Value>,
}

impl Default for WechatShopClient {
    fn default() -> Self {
        // 审查修复：无超时的 Client 在 TCP 黑洞（丢包不 RST）时会无限 hang，
        // driver tick 超时只 detach 不取消 → 僵尸任务无限存活并与新 tick 并发。
        // 120s 总超时取「覆盖最慢的素材上传」与「限制僵尸存活时间」的折中。
        Self {
            http: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }
}

impl WechatShopClient {
    /// 统一「endpoint + access_token 挂 query + POST JSON + HTTP 状态校验 + 反序列化」样板。
    /// 错误语义与原各调用点逐字一致：URL 不合法 → AppError::Validation("微信接口 URL 不合法: …")，
    /// 网络错误 / 非 2xx 状态 / 响应体解析失败 → reqwest 错误经 `?` 透传包装为 AppError。
    async fn post_json<Req: Serialize + ?Sized, Resp: DeserializeOwned>(
        &self,
        endpoint: &str,
        access_token: &str,
        body: &Req,
    ) -> AppResult<Resp> {
        let mut url = Url::parse(endpoint)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token);
        Ok(self
            .http
            .post(url)
            .json(body)
            .send()
            .await?
            .error_for_status()?
            .json::<Resp>()
            .await?)
    }

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
        let response: ApiQuotaResponse = self
            .post_json(API_QUOTA_URL, access_token, &ApiQuotaRequest { cgi_path })
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

    /// 视频上传第①步：申请上传，返回 video_upload_key（关联整个上传流程）。
    /// scene_type=162 为商品视频（限制 500MB/180秒），file_type 固定 mp4。
    pub async fn video_init_upload(
        &self,
        access_token: &str,
        file_size: usize,
    ) -> AppResult<WechatCallResult<String>> {
        let body = serde_json::json!({
            "scene_type": 162,
            "file_type": "mp4",
            "file_size": file_size,
        });
        let response: VideoInitUploadResponse = self
            .post_json(VIDEO_INIT_UPLOAD_URL, access_token, &body)
            .await?;
        if response.errcode == 0 {
            match response.data.and_then(|data| data.video_upload_key) {
                Some(key) if !key.trim().is_empty() => Ok(WechatCallResult::Success(key)),
                _ => Ok(WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_997,
                    errmsg: "微信视频 initupload 成功但缺少 video_upload_key".to_string(),
                })),
            }
        } else {
            Ok(WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            }))
        }
    }

    /// 视频上传第②步：上传单个分块，返回服务端计算的 part_sha（finishupload 时回传校验）。
    /// partnum 从 1 起；除最后一块外每块须 ≥1MB，否则 finishupload 会返回 10020342。
    pub async fn video_upload_part(
        &self,
        access_token: &str,
        video_upload_key: &str,
        partnum: u32,
        bytes: Vec<u8>,
    ) -> AppResult<WechatCallResult<String>> {
        let mut url = Url::parse(VIDEO_UPLOAD_PART_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token)
            .append_pair("video_upload_key", video_upload_key)
            .append_pair("partnum", &partnum.to_string());
        let part = multipart::Part::bytes(bytes)
            .file_name("part")
            .mime_str("application/octet-stream")
            .map_err(|error| AppError::Validation(format!("视频分块 MIME 不合法: {error}")))?;
        let form = multipart::Form::new().part("media", part);
        let response = self
            .http
            .post(url)
            .multipart(form)
            .send()
            .await?
            .error_for_status()?
            .json::<VideoUploadPartResponse>()
            .await?;
        if response.errcode == 0 {
            match response.data.and_then(|data| data.part_sha) {
                Some(sha) if !sha.trim().is_empty() => Ok(WechatCallResult::Success(sha)),
                _ => Ok(WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_996,
                    errmsg: "微信视频 uploadpart 成功但缺少 part_sha".to_string(),
                })),
            }
        } else {
            Ok(WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            }))
        }
    }

    /// 视频上传第③步：完成上传，提交全部分片的 (partnum, part_sha) 供服务端校验连续性与 SHA。
    pub async fn video_finish_upload(
        &self,
        access_token: &str,
        video_upload_key: &str,
        finish_parts: &[(u32, String)],
    ) -> AppResult<WechatCallResult<()>> {
        let parts: Vec<serde_json::Value> = finish_parts
            .iter()
            .map(|(partnum, part_sha)| {
                serde_json::json!({ "partnum": partnum, "part_sha": part_sha })
            })
            .collect();
        let body = serde_json::json!({
            "video_upload_key": video_upload_key,
            "finish_parts": parts,
        });
        let response: VideoFinishUploadResponse = self
            .post_json(VIDEO_FINISH_UPLOAD_URL, access_token, &body)
            .await?;
        if response.errcode == 0 {
            Ok(WechatCallResult::Success(()))
        } else {
            Ok(WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            }))
        }
    }

    /// 视频上传第④步：获取临时播放 URL（GET）。10020347 表示视频仍在转码，需由上层轮询重试。
    pub async fn video_get_play_info(
        &self,
        access_token: &str,
        video_upload_key: &str,
    ) -> AppResult<WechatCallResult<String>> {
        let mut url = Url::parse(VIDEO_GET_PLAY_INFO_URL)
            .map_err(|error| AppError::Validation(format!("微信接口 URL 不合法: {error}")))?;
        url.query_pairs_mut()
            .append_pair("access_token", access_token)
            .append_pair("video_upload_key", video_upload_key);
        let response = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<VideoGetPlayInfoResponse>()
            .await?;
        if response.errcode == 0 {
            match response.data.and_then(|data| data.url) {
                Some(play_url) if !play_url.trim().is_empty() => {
                    Ok(WechatCallResult::Success(play_url))
                }
                _ => Ok(WechatCallResult::ApiError(WechatApiError {
                    errcode: -999_995,
                    errmsg: "微信视频 getplayinfo 成功但缺少 url".to_string(),
                })),
            }
        } else {
            Ok(WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            }))
        }
    }

    /// 4 步分块上传视频到微信，返回临时播放 URL（填 add_product 的 head_videos.video_url）。
    /// 流程：initupload(scene_type=162) → 逐块 uploadpart 收集 part_sha → finishupload 校验 →
    /// 轮询 getplayinfo（10020347 转码中则每 2 秒重试，最多 30 次）。任一步 API 失败返回 ApiError，
    /// 由调用方降级处理（视频为可选增强，失败不阻断商品上架）。
    pub async fn upload_video_bytes(
        &self,
        access_token: &str,
        bytes: Vec<u8>,
    ) -> AppResult<WechatCallResult<String>> {
        // 分块大小取 1.5MB，落在微信要求的 1~2MB 区间内（除末块外每块须 ≥1MB）。
        const PART_SIZE: usize = 1_500_000;
        const TRANSCODING_ERRCODE: i64 = 10020347;
        const MAX_POLLS: u32 = 30;

        if bytes.is_empty() {
            return Ok(WechatCallResult::ApiError(WechatApiError {
                errcode: -999_999,
                errmsg: "视频内容为空".to_string(),
            }));
        }
        let file_size = bytes.len();

        // ① 申请上传
        let video_upload_key = match self.video_init_upload(access_token, file_size).await? {
            WechatCallResult::Success(key) => key,
            WechatCallResult::ApiError(error) => return Ok(WechatCallResult::ApiError(error)),
        };

        // ② 分块上传，partnum 从 1 起，收集每块的 part_sha
        let mut finish_parts: Vec<(u32, String)> = Vec::new();
        for (index, chunk) in bytes.chunks(PART_SIZE).enumerate() {
            let partnum = (index + 1) as u32;
            match self
                .video_upload_part(access_token, &video_upload_key, partnum, chunk.to_vec())
                .await?
            {
                WechatCallResult::Success(part_sha) => finish_parts.push((partnum, part_sha)),
                WechatCallResult::ApiError(error) => return Ok(WechatCallResult::ApiError(error)),
            }
        }

        // ③ 完成上传（服务端校验分片连续 + SHA）
        if let WechatCallResult::ApiError(error) = self
            .video_finish_upload(access_token, &video_upload_key, &finish_parts)
            .await?
        {
            return Ok(WechatCallResult::ApiError(error));
        }

        // ④ 轮询取临时播放 URL（10020347 = 转码中，每 2 秒重试）
        for attempt in 0..MAX_POLLS {
            match self
                .video_get_play_info(access_token, &video_upload_key)
                .await?
            {
                WechatCallResult::Success(play_url) => {
                    return Ok(WechatCallResult::Success(play_url))
                }
                WechatCallResult::ApiError(error) if error.errcode == TRANSCODING_ERRCODE => {
                    if attempt + 1 >= MAX_POLLS {
                        return Ok(WechatCallResult::ApiError(error));
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
                WechatCallResult::ApiError(error) => return Ok(WechatCallResult::ApiError(error)),
            }
        }
        Ok(WechatCallResult::ApiError(WechatApiError {
            errcode: TRANSCODING_ERRCODE,
            errmsg: "微信视频转码超时（轮询约 60 秒仍未就绪）".to_string(),
        }))
    }

    pub async fn add_product(
        &self,
        access_token: &str,
        payload: &serde_json::Value,
    ) -> AppResult<ProductAddCall> {
        let response: ProductAddResponse = self
            .post_json(PRODUCT_ADD_URL, access_token, payload)
            .await?;

        let result = if response.errcode == 0 {
            let product_id = product_add_response_product_id(&response);
            match product_id {
                Some(product_id) if !product_id.trim().is_empty() => {
                    let raw_payload = make_raw_payload(
                        response.errcode,
                        &response.errmsg,
                        vec![("product_id", response.product_id)],
                        response.extra,
                    );
                    WechatCallResult::Success(ProductAddResult {
                        product_id,
                        raw_payload,
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
        let response: ProductUpdateResponse = self
            .post_json(PRODUCT_UPDATE_URL, access_token, payload)
            .await?;

        let result = if response.errcode == 0 {
            WechatCallResult::Success(ProductUpdateResult {
                raw_payload: make_raw_payload(
                    response.errcode,
                    &response.errmsg,
                    vec![],
                    response.extra,
                ),
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
        let response: ProductGetResponse = self
            .post_json(
                PRODUCT_GET_URL,
                access_token,
                &ProductGetRequest {
                    product_id,
                    data_type,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            let raw_payload = make_raw_payload(
                response.errcode,
                &response.errmsg,
                vec![
                    (
                        "product",
                        response.product.as_ref().map(|product| {
                            serde_json::to_value(product).unwrap_or(serde_json::Value::Null)
                        }),
                    ),
                    (
                        "edit_product",
                        response.edit_product.as_ref().map(|edit_product| {
                            serde_json::to_value(edit_product).unwrap_or(serde_json::Value::Null)
                        }),
                    ),
                    ("audit_info", response.audit_info.clone()),
                ],
                response.extra,
            );
            WechatCallResult::Success(ProductGetInfo {
                product: response.product,
                edit_product: response.edit_product,
                audit_info: response.audit_info,
                raw_payload,
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
        let response: ProductListingResponse = self
            .post_json(
                PRODUCT_LISTING_URL,
                access_token,
                &ProductListingRequest { product_id },
            )
            .await?;

        let result = if response.errcode == 0 {
            WechatCallResult::Success(ProductListingResult {
                raw_payload: make_raw_payload(
                    response.errcode,
                    &response.errmsg,
                    vec![],
                    response.extra,
                ),
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
        let response: CategoryPrecheckResponse = self
            .post_json(
                CATEGORY_PRECHECK_URL,
                access_token,
                &CategoryPrecheckRequest { cat_id },
            )
            .await?;

        let result = if response.errcode == 0 {
            let fail_reason_values = response.fail_reasons.unwrap_or_default();
            let fail_reasons = fail_reason_values
                .iter()
                .map(wechat_fail_reason_to_string)
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>();
            let all_pass = response.all_pass.unwrap_or_else(|| fail_reasons.is_empty());
            let raw_payload = make_raw_payload(
                response.errcode,
                &response.errmsg,
                vec![
                    ("all_pass", Some(serde_json::json!(all_pass))),
                    (
                        "fail_reasons",
                        Some(serde_json::Value::Array(fail_reason_values)),
                    ),
                ],
                response.extra,
            );
            WechatCallResult::Success(CategoryPrecheckResult {
                all_pass,
                fail_reasons,
                raw_payload,
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
        time_field: OrderListTimeField,
        start_time: i64,
        end_time: i64,
        status: Option<i64>,
        page_size: i64,
        next_key: &str,
    ) -> AppResult<OrderListCall> {
        let range = TimeRange {
            start_time,
            end_time,
        };
        let (create_time_range, update_time_range) = match time_field {
            OrderListTimeField::Create => (Some(range), None),
            OrderListTimeField::Update => (None, Some(range)),
        };
        let response: OrderListResponse = self
            .post_json(
                ORDER_LIST_URL,
                access_token,
                &OrderListRequest {
                    create_time_range,
                    update_time_range,
                    status,
                    page_size,
                    next_key,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            let order_id_values = response.order_id_list.unwrap_or_default();
            let order_id_list = order_id_values
                .iter()
                .filter_map(json_value_to_string)
                .collect::<Vec<_>>();
            let raw_payload = make_raw_payload(
                response.errcode,
                &response.errmsg,
                vec![
                    (
                        "order_id_list",
                        Some(serde_json::Value::Array(order_id_values)),
                    ),
                    (
                        "next_key",
                        response
                            .next_key
                            .as_ref()
                            .map(|next_key| serde_json::json!(next_key)),
                    ),
                    (
                        "has_more",
                        Some(serde_json::json!(response.has_more.unwrap_or(false))),
                    ),
                ],
                response.extra,
            );
            WechatCallResult::Success(OrderListResult {
                order_id_list,
                next_key: response.next_key,
                has_more: response.has_more.unwrap_or(false),
                raw_payload,
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
        let response: OrderGetResponse = self
            .post_json(ORDER_GET_URL, access_token, &OrderGetRequest { order_id })
            .await?;

        let result = if response.errcode == 0 {
            match response.order {
                Some(order) => {
                    let raw_payload = make_raw_payload(
                        response.errcode,
                        &response.errmsg,
                        vec![("order", Some(order.clone()))],
                        response.extra,
                    );
                    WechatCallResult::Success(OrderGetResult { order, raw_payload })
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
        let response: OrderPriceUpdateResponse = self
            .post_json(
                ORDER_PRICE_UPDATE_URL,
                access_token,
                &OrderPriceUpdateRequest {
                    order_id,
                    change_order_infos,
                    change_express,
                    express_fee,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            WechatCallResult::Success(OrderPriceUpdateResult {
                raw_payload: make_raw_payload(
                    response.errcode,
                    &response.errmsg,
                    vec![],
                    response.extra,
                ),
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
        let response: SendDeliveryResponse = self
            .post_json(SEND_DELIVERY_URL, access_token, payload)
            .await?;

        let result = if response.errcode == 0 {
            WechatCallResult::Success(SendDeliveryResult {
                raw_payload: make_raw_payload(
                    response.errcode,
                    &response.errmsg,
                    vec![],
                    response.extra,
                ),
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
        let response: RawWechatResponse = self
            .post_json(
                DELIVERY_COMPANY_LIST_URL,
                access_token,
                &DeliveryCompanyListRequest { ewaybill_only },
            )
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: DELIVERY_COMPANY_LIST_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    /// 通用原始订单系调用：POST 任意端点、返回原始 payload。
    /// 解密/搜索/换SKU/改址裁决等轻接口共用（返回结构由调用方按文档解析）。
    async fn post_order_raw(
        &self,
        endpoint: &'static str,
        access_token: &str,
        payload: &serde_json::Value,
    ) -> AppResult<WechatRawCall> {
        let response: RawWechatResponse = self.post_json(endpoint, access_token, payload).await?;
        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    /// 解密订单收货信息（sensitiveinfo/decode）。注意官方约束：有每日/每月解密额度、
    /// 同一订单多次调用仅计一次额度、10020198 为限速错误码（调用方需退避）。
    pub async fn decode_order_sensitive_info(
        &self,
        access_token: &str,
        order_id: &str,
    ) -> AppResult<WechatRawCall> {
        self.post_order_raw(
            ORDER_SENSITIVE_DECODE_URL,
            access_token,
            &serde_json::json!({ "order_id": order_id }),
        )
        .await
    }

    /// 订单搜索（order/search）。search_condition 至少含一个字段；
    /// 本系统主要用 {address_under_review: true} 批量发现改址待审单（官方唯一批量入口）。
    pub async fn search_orders(
        &self,
        access_token: &str,
        search_condition: &serde_json::Value,
        status: Option<i64>,
        page_size: i64,
        next_key: &str,
    ) -> AppResult<WechatRawCall> {
        let mut payload = serde_json::json!({
            "search_condition": search_condition,
            "page_size": page_size,
            "next_key": next_key
        });
        if let Some(status) = status {
            payload["status"] = serde_json::json!(status);
        }
        self.post_order_raw(ORDER_SEARCH_URL, access_token, &payload)
            .await
    }

    /// 获取待处理的发货前换SKU申请（仅返回订单号列表，详情须回刷 order/get 取 change_sku_info）。
    pub async fn preshipment_changesku_get(
        &self,
        access_token: &str,
        page_size: i64,
        next_key: &str,
    ) -> AppResult<WechatRawCall> {
        self.post_order_raw(
            ORDER_CHANGESKU_GET_URL,
            access_token,
            &serde_json::json!({ "page_size": page_size, "next_key": next_key }),
        )
        .await
    }

    /// 同意/拒绝发货前换SKU申请（整单粒度）。超时默认=拒绝。
    pub async fn preshipment_changesku_decide(
        &self,
        access_token: &str,
        order_id: &str,
        approve: bool,
    ) -> AppResult<WechatRawCall> {
        let endpoint = if approve {
            ORDER_CHANGESKU_APPROVE_URL
        } else {
            ORDER_CHANGESKU_REJECT_URL
        };
        self.post_order_raw(
            endpoint,
            access_token,
            &serde_json::json!({ "order_id": order_id }),
        )
        .await
    }

    /// 同意/拒绝买家修改收货地址申请。注意：12 小时不处理=系统自动同意（对商家不利方向）。
    pub async fn address_modify_decide(
        &self,
        access_token: &str,
        order_id: &str,
        approve: bool,
    ) -> AppResult<WechatRawCall> {
        let endpoint = if approve {
            ORDER_ADDRESS_MODIFY_ACCEPT_URL
        } else {
            ORDER_ADDRESS_MODIFY_REJECT_URL
        };
        self.post_order_raw(
            endpoint,
            access_token,
            &serde_json::json!({ "order_id": order_id }),
        )
        .await
    }

    /// 补发包裹（delivery/compensation）。官方约束：order_id 为数字类型；
    /// deliver_type 枚举为 1=快递/6=无需物流（与 send 的 1/3 不同）；
    /// 前置要求 SKU 已全部发货且在售后期内；一单 ≤10 个补发包裹。
    pub async fn compensate_delivery(
        &self,
        access_token: &str,
        order_id: i64,
        delivery_list: &serde_json::Value,
        reason: i64,
    ) -> AppResult<WechatRawCall> {
        self.post_order_raw(
            ORDER_DELIVERY_COMPENSATION_URL,
            access_token,
            &serde_json::json!({
                "order_id": order_id,
                "delivery_list": delivery_list,
                "reason": reason
            }),
        )
        .await
    }

    /// 修改运单信息（deliveryinfo/update）。双模式互斥：
    /// delivery_list=整单重报（拆单发货的单不支持）；change_infos=包裹级 old→new 替换。
    /// 官方限制未完成单 ≤3 次（超限报 606041），调用方需本地计数提前禁用。
    pub async fn update_delivery_info(
        &self,
        access_token: &str,
        order_id: i64,
        delivery_list: Option<&serde_json::Value>,
        change_infos: Option<&serde_json::Value>,
    ) -> AppResult<WechatRawCall> {
        let mut payload = serde_json::json!({ "order_id": order_id });
        if let Some(list) = delivery_list {
            payload["delivery_list"] = list.clone();
        }
        if let Some(infos) = change_infos {
            payload["change_infos"] = infos.clone();
        }
        self.post_order_raw(ORDER_DELIVERY_INFO_UPDATE_URL, access_token, &payload)
            .await
    }

    /// 更新订单商家备注（merchantnotes/update）。发货成功后用于把采购单号+运单摘要
    /// 镜像进微信侧备注；失败不影响主流程（调用方 best-effort）。
    pub async fn update_merchant_notes(
        &self,
        access_token: &str,
        order_id: &str,
        merchant_notes: &str,
    ) -> AppResult<WechatRawCall> {
        self.post_order_raw(
            ORDER_MERCHANT_NOTES_URL,
            access_token,
            &serde_json::json!({ "order_id": order_id, "merchant_notes": merchant_notes }),
        )
        .await
    }

    /// 延长虚拟号有效期（virtualnumber/delay）。注意官方语义：order/get 的
    /// tel_number_ext_info.has_delay_times 是「已延期次数」（非剩余次数），
    /// 剩余可延次数以本接口返回的 available_extend_num 为准；每单可延次数有限。
    pub async fn delay_virtual_tel_number(
        &self,
        access_token: &str,
        order_id: &str,
    ) -> AppResult<WechatRawCall> {
        self.post_order_raw(
            ORDER_VIRTUAL_TEL_DELAY_URL,
            access_token,
            &serde_json::json!({ "order_id": order_id }),
        )
        .await
    }

    pub async fn get_aftersale_list(
        &self,
        access_token: &str,
        begin_update_time: i64,
        end_update_time: i64,
        next_key: &str,
    ) -> AppResult<AftersaleListCall> {
        let response: AftersaleListResponse = self
            .post_json(
                AFTERSALE_LIST_URL,
                access_token,
                &AftersaleListRequest {
                    begin_create_time: None,
                    end_create_time: None,
                    begin_update_time: Some(begin_update_time),
                    end_update_time: Some(end_update_time),
                    next_key,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            let aftersale_id_values = response.after_sale_order_id_list.unwrap_or_default();
            let after_sale_order_id_list = aftersale_id_values
                .iter()
                .filter_map(json_value_to_string)
                .collect::<Vec<_>>();
            let raw_payload = make_raw_payload(
                response.errcode,
                &response.errmsg,
                vec![
                    (
                        "after_sale_order_id_list",
                        Some(serde_json::Value::Array(aftersale_id_values)),
                    ),
                    (
                        "next_key",
                        response
                            .next_key
                            .as_ref()
                            .map(|next_key| serde_json::json!(next_key)),
                    ),
                    (
                        "has_more",
                        Some(serde_json::json!(response.has_more.unwrap_or(false))),
                    ),
                ],
                response.extra,
            );
            WechatCallResult::Success(AftersaleListResult {
                after_sale_order_id_list,
                next_key: response.next_key,
                has_more: response.has_more.unwrap_or(false),
                raw_payload,
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
        let response: AftersaleGetResponse = self
            .post_json(
                AFTERSALE_GET_URL,
                access_token,
                &AftersaleGetRequest {
                    after_sale_order_id,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            match response.after_sale_order {
                Some(after_sale_order) => {
                    let raw_payload = make_raw_payload(
                        response.errcode,
                        &response.errmsg,
                        vec![("after_sale_order", Some(after_sale_order.clone()))],
                        response.extra,
                    );
                    WechatCallResult::Success(AftersaleGetResult {
                        after_sale_order,
                        raw_payload,
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
        let response: RawWechatResponse = self
            .post_json(
                AFTERSALE_ACCEPT_URL,
                access_token,
                &AftersaleAcceptRequest {
                    after_sale_order_id,
                    address_id,
                    accept_type,
                },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                AFTERSALE_REJECT_URL,
                access_token,
                &AftersaleRejectRequest {
                    after_sale_order_id,
                    reject_reason,
                    reject_reason_type,
                },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                AFTERSALE_REJECT_REASON_URL,
                access_token,
                &serde_json::json!({}),
            )
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
        let response: RawWechatResponse = self
            .post_json(
                GUARANTEE_SEARCH_URL,
                access_token,
                &GuaranteeSearchRequest {
                    begin_time: Some(begin_time),
                    end_time: Some(end_time),
                    r#type: Some(0),
                    offset,
                    limit,
                },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                GUARANTEE_GET_URL,
                access_token,
                &GuaranteeGetRequest { guarantee_order_id },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                CATEGORY_RELATION_LIST_URL,
                access_token,
                &CategoryRelationListRequest {
                    is_filter_status: status.is_some(),
                    status,
                },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                CATEGORY_RELATION_DETAIL_URL,
                access_token,
                &CategoryRelationDetailRequest { category_id },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                CATEGORY_DETAIL_URL,
                access_token,
                &CategoryDetailRequest { cat_id },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                CATEGORY_PRODUCT_RULE_URL,
                access_token,
                &CategoryProductRuleRequest {
                    cat_id,
                    release_mode,
                },
            )
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
        let response: RawWechatResponse = self
            .post_json(
                FREIGHT_TEMPLATE_LIST_URL,
                access_token,
                &FreightTemplateListRequest { offset, limit },
            )
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: FREIGHT_TEMPLATE_LIST_URL,
                method: "POST",
            },
            result: raw_wechat_result(response),
        })
    }

    /// 查询单个运费模板详情（getfreighttemplatedetail）。
    /// 列表接口仅返回 template_id，模板名称与计费方式等需逐个调本接口获取。
    pub async fn get_freight_template_detail(
        &self,
        access_token: &str,
        template_id: &str,
    ) -> AppResult<WechatRawCall> {
        let response: RawWechatResponse = self
            .post_json(
                FREIGHT_TEMPLATE_DETAIL_URL,
                access_token,
                &FreightTemplateDetailRequest { template_id },
            )
            .await?;

        Ok(WechatRawCall {
            meta: WechatCallMeta {
                endpoint: FREIGHT_TEMPLATE_DETAIL_URL,
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
        let response: MerchantAddressListResponse = self
            .post_json(
                MERCHANT_ADDRESS_LIST_URL,
                access_token,
                &MerchantAddressListRequest { offset, limit },
            )
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
        let response: MerchantAddressGetResponse = self
            .post_json(
                MERCHANT_ADDRESS_GET_URL,
                access_token,
                &MerchantAddressGetRequest { address_id },
            )
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

    /// 获取商品列表（游标分页）。仅返回商品 id 列表，详情需逐个再调 get_product。
    pub async fn get_product_list(
        &self,
        access_token: &str,
        status: Option<i64>,
        page_size: i64,
        next_key: Option<&str>,
    ) -> AppResult<ProductListCall> {
        let response: ProductListResponse = self
            .post_json(
                PRODUCT_LIST_URL,
                access_token,
                &ProductListRequest {
                    status,
                    page_size,
                    next_key,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            let product_ids = response
                .product_ids
                .iter()
                .filter_map(json_value_to_string)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            WechatCallResult::Success(ProductListResult {
                product_ids,
                next_key: response.next_key.filter(|value| !value.is_empty()),
                total_num: response.total_num.unwrap_or_default(),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(ProductListCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_LIST_URL,
                method: "POST",
            },
            result,
        })
    }

    /// 商品下架。只对非卖赠品生效；审核中的商品需先撤回审核。
    pub async fn delisting_product(
        &self,
        access_token: &str,
        product_id: &str,
    ) -> AppResult<ProductMutationCall> {
        let response: ProductMutationResponse = self
            .post_json(
                PRODUCT_DELISTING_URL,
                access_token,
                &ProductIdRequest { product_id },
            )
            .await?;

        Ok(ProductMutationCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_DELISTING_URL,
                method: "POST",
            },
            result: mutation_result(response),
        })
    }

    /// 删除商品。审核中的商品无法删除。
    pub async fn delete_product(
        &self,
        access_token: &str,
        product_id: &str,
    ) -> AppResult<ProductMutationCall> {
        let response: ProductMutationResponse = self
            .post_json(
                PRODUCT_DELETE_URL,
                access_token,
                &ProductIdRequest { product_id },
            )
            .await?;

        Ok(ProductMutationCall {
            meta: WechatCallMeta {
                endpoint: PRODUCT_DELETE_URL,
                method: "POST",
            },
            result: mutation_result(response),
        })
    }

    /// 获取单 SKU 库存（normal=通用库存，total=通用+区域总量）。
    pub async fn get_stock(
        &self,
        access_token: &str,
        product_id: &str,
        sku_id: &str,
    ) -> AppResult<StockGetCall> {
        let response: StockGetResponse = self
            .post_json(
                STOCK_GET_URL,
                access_token,
                &StockGetRequest { product_id, sku_id },
            )
            .await?;

        let result = if response.errcode == 0 {
            let data = response.data.unwrap_or(serde_json::Value::Null);
            let normal = wechat_json_value_to_i64(data.get("normal_stock_num")).unwrap_or_default();
            let total = wechat_json_value_to_i64(data.get("total_stock_num")).unwrap_or(normal);
            WechatCallResult::Success(StockInfo {
                normal_stock_num: normal,
                total_stock_num: total,
                raw_payload: data,
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(StockGetCall {
            meta: WechatCallMeta {
                endpoint: STOCK_GET_URL,
                method: "POST",
            },
            result,
        })
    }

    /// 批量获取库存（单次 product_id 上限 50）。spu→sku→warehouse 三层原样透传。
    pub async fn batch_get_stock(
        &self,
        access_token: &str,
        product_ids: &[String],
        stock_type: Option<i64>,
    ) -> AppResult<StockBatchGetCall> {
        let response: StockBatchGetResponse = self
            .post_json(
                STOCK_BATCHGET_URL,
                access_token,
                &StockBatchGetRequest {
                    product_id: product_ids,
                    stock_type,
                },
            )
            .await?;

        let result = if response.errcode == 0 {
            WechatCallResult::Success(StockBatchInfo {
                spu_stock_list: response
                    .data
                    .map(|data| data.spu_stock_list)
                    .unwrap_or_default(),
            })
        } else {
            WechatCallResult::ApiError(WechatApiError {
                errcode: response.errcode,
                errmsg: response.errmsg,
            })
        };

        Ok(StockBatchGetCall {
            meta: WechatCallMeta {
                endpoint: STOCK_BATCHGET_URL,
                method: "POST",
            },
            result,
        })
    }

    /// 快速更新库存。diff_type：1 增 / 2 减 / 3 设置（3 在高并发下有覆盖风险，优先 1/2）。
    pub async fn update_stock(
        &self,
        access_token: &str,
        product_id: &str,
        sku_id: &str,
        diff_type: i64,
        num: i64,
    ) -> AppResult<ProductMutationCall> {
        let response: ProductMutationResponse = self
            .post_json(
                STOCK_UPDATE_URL,
                access_token,
                &StockUpdateRequest {
                    product_id,
                    sku_id,
                    diff_type,
                    num,
                },
            )
            .await?;

        Ok(ProductMutationCall {
            meta: WechatCallMeta {
                endpoint: STOCK_UPDATE_URL,
                method: "POST",
            },
            result: mutation_result(response),
        })
    }
}

/// 构造微信响应 raw_payload 的统一形态：errcode + errmsg → 业务字段（值为 None 的键不写入）→
/// extend(extra) 透传剩余字段。写入顺序与各调用点原手写逻辑逐一一致，产物字节级不变。
fn make_raw_payload(
    errcode: i64,
    errmsg: &str,
    fields: Vec<(&'static str, Option<serde_json::Value>)>,
    extra: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut raw_payload = serde_json::Map::new();
    raw_payload.insert("errcode".to_string(), serde_json::json!(errcode));
    raw_payload.insert("errmsg".to_string(), serde_json::json!(errmsg));
    for (key, value) in fields {
        if let Some(value) = value {
            raw_payload.insert(key.to_string(), value);
        }
    }
    raw_payload.extend(extra);
    serde_json::Value::Object(raw_payload)
}

/// 下架 / 删除 / 改库存等只回 errcode/errmsg 的写操作统一构造结果。
fn mutation_result(response: ProductMutationResponse) -> WechatCallResult<ProductMutationResult> {
    if response.errcode == 0 {
        WechatCallResult::Success(ProductMutationResult {
            raw_payload: make_raw_payload(
                response.errcode,
                &response.errmsg,
                vec![],
                response.extra,
            ),
        })
    } else {
        WechatCallResult::ApiError(WechatApiError {
            errcode: response.errcode,
            errmsg: response.errmsg,
        })
    }
}

fn raw_wechat_result(response: RawWechatResponse) -> WechatCallResult<WechatRawResult> {
    if response.errcode == 0 {
        WechatCallResult::Success(WechatRawResult {
            raw_payload: make_raw_payload(
                response.errcode,
                &response.errmsg,
                vec![],
                response.extra,
            ),
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
