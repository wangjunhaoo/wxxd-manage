import type {
  OperationalAutomationSettings,
  PublishPricingStrategy,
} from "../../types/app";
export const defaultAutomationSettings = (): OperationalAutomationSettings => ({
  order_sync_enabled: true,
  order_detail_sync_enabled: true,
  aftersale_sync_enabled: true,
  purchase_task_enabled: true,
  delivery_submission_enabled: true,
  publish_enabled: true,
  price_confirm_enabled: true,
  // 订单自动化 L1 总开关默认关闭（灰度），打开后 30s 订单 driver 自动推进全链路
  order_automation_enabled: false,
  negotiation_scan_enabled: true,
  address_decode_enabled: true,
});

/** 申请收件箱：类型与状态中文标签 */
export const orderRequestKindLabels: Record<string, string> = {
  address_change: "买家改址",
  sku_change: "换SKU",
  delivery_negotiation: "发货协商",
  real_number: "真实号申请",
};

export const orderRequestStateLabels: Record<string, string> = {
  pending: "待处理",
  accepted: "已同意",
  rejected: "已拒绝",
  resolved_external: "已被外部定性",
  user_cancelled: "买家已取消",
  auto_timeout: "已超时",
};

export const defaultPublishPricingStrategy = (): PublishPricingStrategy => ({
  sale_price_markup_rate: 1.6,
  sale_price_fixed_cents: 0,
  sale_price_floor_cents: 100,
});

export const fallbackDeliveryCompanyOptions = [
  { value: "SF", label: "顺丰" },
  { value: "ZTO", label: "中通" },
  { value: "YTO", label: "圆通" },
  { value: "YUNDA", label: "韵达" },
  { value: "JTSD", label: "极兔" },
  { value: "STO", label: "申通" },
  { value: "JD", label: "京东" },
  { value: "EMS", label: "中国邮政" },
  { value: "OTHER", label: "其他" },
];
export const purchaseIssueTypeOptions = [
  { value: "out_of_stock", label: "供应商缺货" },
  { value: "price_changed", label: "供应商涨价" },
  { value: "supplier_cancelled", label: "供应商取消" },
  { value: "quality_risk", label: "质量风险" },
  { value: "other", label: "其他异常" },
];
export const inventoryRiskStatusOptions = [
  { value: "all", label: "全部状态" },
  { value: "out_of_stock", label: "断货" },
  { value: "supplier_issue", label: "供应商异常" },
  { value: "stock_pressure", label: "库存压力" },
  { value: "low_stock", label: "低库存" },
  { value: "not_listed", label: "未铺货" },
  { value: "healthy", label: "正常" },
];
export const productSalesStatusOptions = [
  { value: "all", label: "全部状态" },
  { value: "scale_candidate", label: "可放量" },
  { value: "stock_risk", label: "库存风险" },
  { value: "margin_risk", label: "毛利风险" },
  { value: "aftersale_watch", label: "售后观察" },
  { value: "no_sales", label: "未动销" },
  { value: "not_listed", label: "未铺货" },
  { value: "steady", label: "稳定观察" },
  { value: "observe", label: "数据不足" },
];
export const productManagementStatusOptions = [
  { value: "all", label: "全部商品" },
  { value: "publish_failed", label: "铺货失败" },
  { value: "inventory_risk", label: "库存风险" },
  { value: "not_listed", label: "未铺货" },
  { value: "listed_sold", label: "已动销" },
  { value: "listed_unsold", label: "已铺未动销" },
];
export const orderManagementStatusOptions = [
  { value: "all", label: "全部订单" },
  { value: "detail_failed", label: "详情失败" },
  { value: "needs_detail", label: "待同步详情" },
  { value: "needs_purchase", label: "待采购处理" },
  { value: "needs_shipment", label: "待发货" },
  { value: "aftersale_active", label: "售后处理中" },
  { value: "completed", label: "已完成" },
  { value: "cancelled", label: "已取消" },
];
export const profitAdjustmentKindOptions = [
  { value: "purchase_freight", label: "采购运费" },
  { value: "refund", label: "退款" },
  { value: "aftersale_compensation", label: "售后赔付" },
  { value: "other_cost", label: "其他成本" },
  { value: "other_income", label: "其他收入" },
];
export const aftersaleResponsibilityOptions = [
  { value: "supplier", label: "供应商责任" },
  { value: "merchant", label: "本店责任" },
  { value: "customer", label: "买家原因" },
  { value: "platform", label: "平台原因" },
  { value: "unknown", label: "待确认" },
];
export const guaranteeHandlingStatusOptions = [
  { value: "pending", label: "待跟进" },
  { value: "in_progress", label: "跟进中" },
  { value: "waiting_supplier", label: "等供应商" },
  { value: "evidence_ready", label: "凭证已整理" },
  { value: "resolved", label: "已处理" },
  { value: "ignored", label: "无需处理" },
];
export const evidenceTargetTypeOptions = [
  { value: "aftersale", label: "售后单" },
  { value: "guarantee", label: "纠纷单" },
];
export const evidenceTypeOptions = [
  { value: "image", label: "图片" },
  { value: "text", label: "文字说明" },
  { value: "chat_record", label: "沟通记录" },
  { value: "logistics", label: "物流凭证" },
  { value: "supplier_proof", label: "供应商凭证" },
  { value: "quality_check", label: "质检凭证" },
  { value: "other", label: "其他" },
];
export const evidenceStatusOptions = [
  { value: "draft", label: "草稿" },
  { value: "ready", label: "已整理" },
  { value: "used", label: "已使用" },
  { value: "archived", label: "已归档" },
];
export const supplierFollowupTypeOptions = [
  { value: "contact", label: "联系供应商" },
  { value: "evidence_request", label: "索要凭证" },
  { value: "evidence_received", label: "收到凭证" },
  { value: "compensation", label: "赔付沟通" },
  { value: "return_refund", label: "退货退款" },
  { value: "other", label: "其他协同" },
];
export const supplierFollowupStatusOptions = [
  { value: "pending", label: "待处理" },
  { value: "contacted", label: "已联系" },
  { value: "waiting_supplier", label: "等供应商" },
  { value: "evidence_ready", label: "凭证已备" },
  { value: "compensation_pending", label: "赔付待确认" },
  { value: "closed", label: "已关闭" },
];
export const aftersaleTerminalStatuses = [
  "MERCHANT_REFUND_SUCCESS",
  "MERCHANT_RETURN_SUCCESS",
  "USER_CANCELD",
  "USER_CANCELLED",
  "RETURN_CLOSED",
  "sync_failed",
];
export const statusTone: Record<string, string> = {
  active: "success",
  not_verified: "warning",
  missing_secret: "warning",
  auth_failed: "danger",
  api_failed: "danger",
  queued: "info",
  pending: "info",
  running: "primary",
  prechecking: "primary",
  category_prechecking: "primary",
  asset_uploading: "primary",
  listing: "primary",
  audit_pending: "primary",
  audit_passed: "success",
  assets_ready: "success",
  publishing: "primary",
  submitted: "primary",
  ready_to_publish: "success",
  category_prechecked: "success",
  ready_to_update: "success",
  success: "success",
  failed: "danger",
  partial_success: "warning",
  pending_purchase: "warning",
  needs_mapping: "warning",
  supplier_shipped: "warning",
  supplier_out_of_stock: "danger",
  supplier_price_changed: "warning",
  supplier_cancelled: "danger",
  supplier_quality_risk: "danger",
  supplier_exception: "danger",
  waiting_confirmation: "warning",
  ready_to_send: "primary",
  wechat_shipped: "success",
  send_failed: "danger",
  blocked: "danger",
  submitting: "primary",
  partially_shipped: "warning",
  shipping_submitted: "primary",
  aftersale_active: "danger",
  MERCHANT_PROCESSING: "warning",
  MERCHANT_REFUND_SUCCESS: "success",
  MERCHANT_RETURN_SUCCESS: "success",
  MERCHANT_FAIL: "danger",
  USER_CANCELD: "info",
  USER_CANCELLED: "info",
  RETURN_CLOSED: "info",
  draft: "info",
  ready: "success",
  listed: "success",
  exception: "danger",
  used: "primary",
  archived: "info",
  contacted: "primary",
  waiting_supplier: "warning",
  evidence_ready: "success",
  compensation_pending: "warning",
  closed: "info",
  STATUS_WAIT_MERCHANT_HANDLE: "danger",
  STATUS_WAIT_MERCHANT_PROOF: "danger",
  STATUS_WAIT_BOTH_PROOF: "danger",
  STATUS_WAIT_PLATFORM_HANDLE: "warning",
  STATUS_WAIT_USER_CONFIRM: "warning",
  STATUS_PAY_BLOCK: "danger",
  STATUS_PAY_FAIL: "danger",
  STATUS_PAYING: "primary",
  STATUS_PAY_SUCC: "success",
  STATUS_NO_NEED_PAY: "success",
  STATUS_USER_CANCEL: "info",
  sync_failed: "danger",
  missing_purchase_task: "warning",
  missing_cost: "warning",
  loss: "danger",
  profitable: "success",
  out_of_stock: "danger",
  supplier_issue: "danger",
  stock_pressure: "warning",
  low_stock: "warning",
  not_listed: "info",
  healthy: "success",
  scale_candidate: "success",
  stock_risk: "danger",
  margin_risk: "danger",
  aftersale_watch: "warning",
  no_sales: "warning",
  steady: "success",
  observe: "info",
  publish_failed: "danger",
  inventory_risk: "warning",
  listed_sold: "success",
  listed_unsold: "info",
  detail_failed: "danger",
  needs_detail: "warning",
  needs_purchase: "warning",
  needs_shipment: "warning",
  purchase_issue: "danger",
  none: "info",
  unread: "warning",
  read: "info",
};
export const createDefaultPriceUpdatePayload = () =>
  JSON.stringify(
    {
      request_id: `price-${Date.now()}`,
      target_shop_group_ids: ["group-default"],
      products: [
        {
          external_product_id: "demo-1688-10001",
          target_price_cents: 3290,
          reason: "测试商品售价调整",
        },
      ],
    },
    null,
    2,
  );

export const createDefaultOrderPriceAdjustmentPayload = () =>
  JSON.stringify(
    {
      request_id: `order-price-${Date.now()}`,
      orders: [
        {
          shop_id: "replace-with-shop-id",
          wechat_order_id: "replace-with-unpaid-order-id",
          change_express: false,
          express_fee_cents: null,
          note: "未付款订单人工让利",
          lines: [
            {
              product_id: "replace-with-wechat-product-id",
              sku_id: "replace-with-wechat-sku-id",
              change_price_cents: 300,
            },
          ],
        },
      ],
    },
    null,
    2,
  );
