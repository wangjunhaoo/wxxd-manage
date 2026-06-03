// Agent 自定义工具 — 供 json_skill_agent.mjs 通过 customTools 注册。
// 每个工具通过 HTTP 调用本地 Tauri API（http://127.0.0.1:17890）。

import { defineTool } from "@mariozechner/pi-coding-agent";

// TypeBox 兼容：运行时用简单 schema 对象（pi-coding-agent 内部会处理）
function objectSchema(props) {
  return {
    type: "object",
    properties: props,
    additionalProperties: false,
  };
}

function stringSchema(description) {
  return { type: "string", description };
}

function intSchema(description) {
  return { type: "integer", description, minimum: 1 };
}

// ── 通用 HTTP 调用 ──────────────────────────────────────

function safeEncode(value) {
  return encodeURIComponent(String(value ?? ""));
}

async function agentFetch(state, path) {
  const baseUrl = state.apiBaseUrl || "http://127.0.0.1:17890";
  const apiKey = state.apiKey || "";
  const url = `${baseUrl}${path}`;
  const headers = {};
  if (apiKey) {
    headers["x-wx-xd-api-key"] = apiKey;
  }
  const res = await fetch(url, { headers, signal: AbortSignal.timeout(15000) });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text.slice(0, 300)}`);
  }
  return res.json();
}

function textContent(data) {
  return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
}

// ── 工具定义 ──────────────────────────────────────────────

export function createAgentTools(state) {
  return [
    defineTool({
      name: "get_product_detail",
      label: "查询商品详情",
      description:
        "获取采集商品的完整数据，包含标题、来源链接、图片URL、SKU规格（含价格库存）、外部元数据、类目线索、已有的AI属性建议",
      parameters: stringSchema("采集任务ID（如 col_xxx）"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(state, `/api/agent/products/${safeEncode(params)}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "search_categories",
      label: "搜索微信类目",
      description:
        "根据关键词搜索店铺可用的微信小店类目，返回匹配类目的完整路径、cat_id、是否叶子类目、是否已开通",
      parameters: objectSchema({
        shop_id: stringSchema("店铺ID"),
        query: stringSchema("搜索关键词（如 童装、T恤、食品）"),
      }),
      execute: async (_toolCallId, params) => {
        const q = encodeURIComponent(params.query || "");
        const data = await agentFetch(
          state,
          `/api/agent/shops/${encodeURIComponent(params.shop_id)}/categories/search?q=${q}&limit=20`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_category_detail",
      label: "查询类目详情",
      description:
        "获取微信类目的完整属性要求，包括：必填商品属性（product_attrs）和必填销售属性（sale_attrs），每个属性包含 key、类型(select_one/select_many/string)、allowed_values（允许值列表）、是否必填",
      parameters: objectSchema({
        shop_id: stringSchema("店铺ID"),
        cat_id: intSchema("类目cat_id"),
      }),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${encodeURIComponent(params.shop_id)}/categories/${params.cat_id}`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_shop_info",
      label: "查询店铺信息",
      description:
        "获取店铺基本信息和状态，包括：店铺名称、状态、微信认证状态、主体类型、可用类目数量、运费模板数量",
      parameters: stringSchema("店铺ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(state, `/api/agent/shops/${safeEncode(params)}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "search_wechat_docs",
      label: "查询微信文档",
      description:
        "查询微信小店开发文档和属性说明，包括：属性定义、枚举值含义、商品发布规则。适用于需要理解特定属性含义或确认可选值范围的场景",
      parameters: stringSchema("查询关键词（如 适用年龄、安全等级、面料材质）"),
      execute: async (_toolCallId, params) => {
        const q = encodeURIComponent(params || "");
        const data = await agentFetch(state, `/api/agent/docs/search?q=${q}`);
        return textContent(data);
      },
    }),

    // ── P1 工具 ──────────────────────────────────────────

    defineTool({
      name: "get_active_categories",
      label: "查询已开通类目",
      description:
        "获取店铺所有已开通的微信叶子类目列表，每个类目包含完整路径、cat_id、是否可用",
      parameters: stringSchema("店铺ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${safeEncode(params)}/categories/active`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_category_tree",
      label: "查询类目树",
      description:
        "获取微信类目树结构，可按父类目筛选子类目。不传 parent_cat_id 时返回一级类目",
      parameters: objectSchema({
        shop_id: stringSchema("店铺ID"),
        parent_cat_id: intSchema("父类目 cat_id，不传则返回一级类目"),
      }),
      execute: async (_toolCallId, params) => {
        const query = params.parent_cat_id ? `?parent_cat_id=${params.parent_cat_id}` : "";
        const data = await agentFetch(
          state,
          `/api/agent/shops/${encodeURIComponent(params.shop_id)}/categories/tree${query}`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_freight_templates",
      label: "查询运费模板",
      description: "获取店铺的运费模板列表，包含模板ID和名称",
      parameters: stringSchema("店铺ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${safeEncode(params)}/freight-templates`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_after_sale_addresses",
      label: "查询售后地址",
      description: "获取店铺的售后/退货地址信息",
      parameters: stringSchema("店铺ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${safeEncode(params)}/after-sale-addresses`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_delivery_companies",
      label: "查询快递公司",
      description: "获取店铺可用的快递公司列表，包含 delivery_id 和名称",
      parameters: stringSchema("店铺ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${safeEncode(params)}/delivery-companies`,
        );
        return textContent(data);
      },
    }),

    // ── P2 工具 ──────────────────────────────────────────

    defineTool({
      name: "get_order",
      label: "查询订单",
      description: "获取订单完整详情，包括商品行、采购任务、发货记录",
      parameters: stringSchema("订单ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(state, `/api/agent/orders/${safeEncode(params)}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "list_orders",
      label: "查询订单列表",
      description: "获取订单列表，可按店铺、状态筛选",
      parameters: objectSchema({
        shop_id: stringSchema("店铺ID（可选）"),
        status: stringSchema("状态筛选（可选）"),
        limit: intSchema("返回数量，默认20，最大100"),
      }),
      execute: async (_toolCallId, params) => {
        const query = new URLSearchParams();
        if (params.shop_id) query.set("shop_id", params.shop_id);
        if (params.status) query.set("status", params.status);
        if (params.limit) query.set("limit", String(params.limit));
        const qs = query.toString();
        const data = await agentFetch(state, `/api/agent/orders${qs ? `?${qs}` : ""}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_aftersale",
      label: "查询售后单",
      description: "获取售后单详情，包括售后类型、原因、退款金额、责任归属、处理状态",
      parameters: stringSchema("售后单ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(state, `/api/agent/aftersales/${safeEncode(params)}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "list_aftersales",
      label: "查询售后列表",
      description: "获取售后单列表，可按店铺、状态筛选",
      parameters: objectSchema({
        shop_id: stringSchema("店铺ID（可选）"),
        status: stringSchema("状态筛选（可选）"),
        limit: intSchema("返回数量，默认20，最大100"),
      }),
      execute: async (_toolCallId, params) => {
        const query = new URLSearchParams();
        if (params.shop_id) query.set("shop_id", params.shop_id);
        if (params.status) query.set("status", params.status);
        if (params.limit) query.set("limit", String(params.limit));
        const qs = query.toString();
        const data = await agentFetch(state, `/api/agent/aftersales${qs ? `?${qs}` : ""}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_reject_reasons",
      label: "查询拒绝原因",
      description: "获取店铺的售后拒绝原因枚举列表，用于拒绝售后时选择原因",
      parameters: stringSchema("店铺ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${safeEncode(params)}/reject-reasons`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_purchase_task",
      label: "查询采购任务",
      description: "获取采购任务详情，包括供应商信息、物流单号、成本估算",
      parameters: stringSchema("采购任务ID"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/purchase-tasks/${safeEncode(params)}`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_shop_product",
      label: "查询铺货商品状态",
      description: "查询某个商品在指定店铺的铺货状态，包括微信商品ID、审核状态、当前价格",
      parameters: objectSchema({
        shop_id: stringSchema("店铺ID"),
        external_product_id: stringSchema("外部商品ID（淘宝链接等）"),
      }),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/shops/${encodeURIComponent(params.shop_id)}/products/${encodeURIComponent(params.external_product_id)}`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "list_collection_tasks",
      label: "查询采集任务列表",
      description: "获取采集任务列表，可按状态筛选",
      parameters: objectSchema({
        status: stringSchema("状态筛选（可选），如 success/failed/reviewing"),
        limit: intSchema("返回数量，默认20，最大50"),
      }),
      execute: async (_toolCallId, params) => {
        const query = new URLSearchParams();
        if (params.status) query.set("status", params.status);
        if (params.limit) query.set("limit", String(params.limit));
        const qs = query.toString();
        const data = await agentFetch(state, `/api/agent/collection-tasks${qs ? `?${qs}` : ""}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_product_sales",
      label: "查询商品销售数据",
      description: "获取商品的销售统计数据，包括销量、销售额、活跃店铺数",
      parameters: stringSchema("外部商品ID（淘宝链接）"),
      execute: async (_toolCallId, params) => {
        const data = await agentFetch(
          state,
          `/api/agent/product-sales/${safeEncode(params)}`,
        );
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_inventory_risk",
      label: "查询库存风险",
      description: "获取库存风险分析，包括缺货预警、低库存商品列表",
      parameters: stringSchema("外部商品ID（可选，不传则返回全部风险商品）"),
      execute: async (_toolCallId, params) => {
        const query = params ? `?external_product_id=${safeEncode(params)}` : "";
        const data = await agentFetch(state, `/api/agent/inventory-risks${query}`);
        return textContent(data);
      },
    }),

    defineTool({
      name: "get_profit_summary",
      label: "查询利润汇总",
      description: "获取订单利润汇总，包括收入、采购成本、退款、预估利润",
      parameters: objectSchema({
        order_id: stringSchema("订单ID（可选，不传则返回全部）"),
        limit: intSchema("返回数量，默认20，最大50"),
      }),
      execute: async (_toolCallId, params) => {
        const query = new URLSearchParams();
        if (params.order_id) query.set("order_id", params.order_id);
        if (params.limit) query.set("limit", String(params.limit));
        const qs = query.toString();
        const data = await agentFetch(state, `/api/agent/profit-summary${qs ? `?${qs}` : ""}`);
        return textContent(data);
      },
    }),
  ];
}
