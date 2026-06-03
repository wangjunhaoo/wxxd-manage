#!/usr/bin/env node

import {
  AuthStorage,
  createAgentSession,
  createExtensionRuntime,
  ModelRegistry,
  SessionManager,
  SettingsManager,
} from "@mariozechner/pi-coding-agent";
import { createAgentTools } from "./agent_tools.mjs";

const CUSTOM_PROVIDER_TYPE = "custom";
const DEFAULT_CUSTOM_PROVIDER_ID = "wx-xd-custom";
const DEFAULT_API = "openai-completions";
const SUPPORTED_APIS = new Set([
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
]);
const FALLBACK_API_KEY = "wx-xd-local-openai-compatible";

function fail(message, code = 1) {
  console.error(message);
  process.exit(code);
}

function readStdin() {
  return new Promise((resolve, reject) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      data += chunk;
    });
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", reject);
  });
}

function normalizeOptionalBaseUrl(value) {
  const baseUrl = String(value || "").trim().replace(/\/+$/, "");
  if (!baseUrl) return "";
  new URL(baseUrl);
  return baseUrl;
}

function normalizeProviderId(value, fallback) {
  const providerId = String(value || fallback || "").trim();
  if (!/^[a-z0-9][a-z0-9._-]{0,79}$/.test(providerId)) {
    throw new Error("provider id 只能包含小写字母、数字、点、横线和下划线");
  }
  return providerId;
}

function normalizeProviderType(value) {
  const providerType = String(value || CUSTOM_PROVIDER_TYPE).trim();
  if (!providerType || providerType === "pi_coding_agent") {
    return CUSTOM_PROVIDER_TYPE;
  }
  return normalizeProviderId(providerType, CUSTOM_PROVIDER_TYPE);
}

function normalizeApi(value) {
  const api = String(value || DEFAULT_API).trim();
  if (!SUPPORTED_APIS.has(api)) {
    throw new Error(`Custom API 不受支持：${api}`);
  }
  return api;
}

function normalizeModel(value) {
  const model = String(value || "").trim();
  if (!model) {
    throw new Error("model 不能为空");
  }
  return model;
}

function normalizePositiveInteger(value, fallback) {
  const parsed = Number(value || fallback);
  if (!Number.isInteger(parsed) || parsed <= 0 || parsed > 4_000_000) {
    return fallback;
  }
  return parsed;
}

function openAiCompat(api) {
  if (api !== "openai-completions") {
    return undefined;
  }
  return {
    supportsDeveloperRole: false,
    supportsReasoningEffort: false,
    supportsStore: false,
    maxTokensField: "max_tokens",
  };
}

function buildCustomModel({ modelId, api, contextWindow, maxTokens }) {
  return {
    id: modelId,
    name: modelId,
    reasoning: false,
    input: ["text"],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    contextWindow,
    maxTokens,
    compat: openAiCompat(api),
  };
}

function configureModelRegistry(authStorage, modelRegistry, request) {
  const providerType = normalizeProviderType(request.provider_type);
  const modelId = normalizeModel(request.model);
  const baseUrl = normalizeOptionalBaseUrl(request.base_url);
  const api = normalizeApi(request.api);
  const apiKey = String(request.api_key || "").trim();
  const contextWindow = normalizePositiveInteger(request.context_window, 128000);
  const maxTokens = normalizePositiveInteger(request.max_tokens, 4096);
  const providerId =
    providerType === CUSTOM_PROVIDER_TYPE
      ? normalizeProviderId(request.custom_provider_id, DEFAULT_CUSTOM_PROVIDER_ID)
      : providerType;

  if (apiKey) {
    authStorage.setRuntimeApiKey(providerId, apiKey);
  }

  if (providerType === CUSTOM_PROVIDER_TYPE) {
    if (!baseUrl) {
      throw new Error("Custom provider 必须填写 base_url");
    }
    const providerApiKey = apiKey || FALLBACK_API_KEY;
    authStorage.setRuntimeApiKey(providerId, providerApiKey);
    modelRegistry.registerProvider(providerId, {
      name: `wx-xd Custom (${providerId})`,
      baseUrl,
      apiKey: providerApiKey,
      api,
      models: [buildCustomModel({ modelId, api, contextWindow, maxTokens })],
    });
  } else if (baseUrl) {
    const existing = modelRegistry.find(providerId, modelId);
    if (existing) {
      modelRegistry.registerProvider(providerId, {
        name: providerId,
        baseUrl,
        apiKey: apiKey || undefined,
      });
    } else {
      const providerApiKey = apiKey || FALLBACK_API_KEY;
      authStorage.setRuntimeApiKey(providerId, providerApiKey);
      modelRegistry.registerProvider(providerId, {
        name: providerId,
        baseUrl,
        apiKey: providerApiKey,
        api,
        models: [buildCustomModel({ modelId, api, contextWindow, maxTokens })],
      });
    }
  }

  const model = modelRegistry.find(providerId, modelId);
  if (!model) {
    throw new Error(`pi 未找到模型：provider=${providerId}, model=${modelId}`);
  }
  return model;
}

function extractJson(text) {
  const value = String(text || "").trim();
  if (!value) {
    throw new Error("模型未返回内容");
  }
  try {
    return JSON.parse(value);
  } catch {
    const fenced = value.match(/```(?:json)?\s*([\s\S]*?)```/i);
    if (fenced?.[1]) {
      return JSON.parse(fenced[1].trim());
    }
    const firstObject = value.indexOf("{");
    const lastObject = value.lastIndexOf("}");
    if (firstObject >= 0 && lastObject > firstObject) {
      return JSON.parse(value.slice(firstObject, lastObject + 1));
    }
    const firstArray = value.indexOf("[");
    const lastArray = value.lastIndexOf("]");
    if (firstArray >= 0 && lastArray > firstArray) {
      return JSON.parse(value.slice(firstArray, lastArray + 1));
    }
    throw new Error(`模型返回不是 JSON：${value.slice(0, 240)}`);
  }
}

function buildPrompt(request) {
  const imageUrls = Array.isArray(request.image_urls)
    ? request.image_urls.filter((url) => typeof url === "string" && url.trim()).slice(0, 12)
    : [];
  return [
    "# 任务",
    `技能：${request.skill_name || "wx-xd-skill"}@${request.skill_version || "1.0.0"}`,
    "",
    "# 可用工具",
    "你有以下查询工具可用，主动使用它们获取所需数据：",
    "",
    "【商品】",
    "- get_product_detail(task_id)：查询采集商品完整数据（标题、图片、SKU、元数据、已有属性建议）",
    "",
    "【类目】",
    "- search_categories(shop_id, query)：搜索微信类目，返回候选类目路径和 cat_id",
    "- get_category_detail(shop_id, cat_id)：查询类目必填属性和 allowed_values",
    "- get_active_categories(shop_id)：查询店铺已开通的叶子类目列表",
    "- get_category_tree(shop_id, parent_cat_id?)：查询类目树结构",
    "",
    "【店铺】",
    "- get_shop_info(shop_id)：查询店铺状态和权限",
    "- get_freight_templates(shop_id)：查询运费模板列表",
    "- get_after_sale_addresses(shop_id)：查询售后/退货地址",
    "- get_delivery_companies(shop_id)：查询可用快递公司",
    "",
    "【文档】",
    "- search_wechat_docs(query)：查询微信小店文档（属性定义、枚举说明）",
    "",
    "【订单】",
    "- get_order(order_id)：查询订单完整详情（商品行、采购任务、发货记录）",
    "- list_orders(shop_id?, status?, limit?)：查询订单列表",
    "",
    "【售后】",
    "- get_aftersale(aftersale_id)：查询售后单详情",
    "- list_aftersales(shop_id?, status?, limit?)：查询售后列表",
    "- get_reject_reasons(shop_id)：查询拒绝原因枚举",
    "",
    "【采购】",
    "- get_purchase_task(task_id)：查询采购任务详情（供应商、物流、成本）",
    "",
    "【商品跟踪】",
    "- get_shop_product(shop_id, external_product_id)：查询商品在店铺的铺货状态",
    "- list_collection_tasks(status?, limit?)：查询采集任务列表",
    "",
    "【分析】",
    "- get_product_sales(external_product_id)：查询商品销售统计",
    "- get_inventory_risk(external_product_id?)：查询库存风险",
    "- get_profit_summary(order_id?, limit?)：查询利润汇总",
    "",
    "# 输出约束",
    "只返回一个合法 JSON 对象，不要 Markdown，不要代码块，不要解释文字。",
    "如果无法高置信判断，也必须按 schema 返回空值、低 confidence 和原因。",
    "在输出前，自检所有属性值是否在 allowed_values 中；不在的必须修正。",
    "",
    "# 技能说明",
    String(request.instructions || "").trim(),
    "",
    "# 输出 Schema",
    String(request.output_schema || "").trim(),
    "",
    "# 输入 JSON",
    JSON.stringify(request.input ?? {}, null, 2),
    "",
    "# 图片链接",
    imageUrls.length ? imageUrls.map((url, index) => `${index + 1}. ${url}`).join("\n") : "无",
  ].join("\n");
}

function createResourceLoader(request) {
  const agentState = {
    apiBaseUrl: request.agent_api_base_url || "http://127.0.0.1:17890",
    apiKey: request.agent_api_key || "",
  };
  const customTools = createAgentTools(agentState);

  return {
    getExtensions: () => ({ extensions: [], errors: [], runtime: createExtensionRuntime() }),
    getSkills: () => ({ skills: [], diagnostics: [] }),
    getPrompts: () => ({ prompts: [], diagnostics: [] }),
    getThemes: () => ({ themes: [], diagnostics: [] }),
    getAgentsFiles: () => ({ agentsFiles: [] }),
    getSystemPrompt: () =>
      [
        "你是 wx-xd 桌面端的结构化业务 Agent，负责审查采集商品并补齐微信小店发品属性。",
        "你可以使用查询工具获取所需数据：商品(get_product_detail)、类目(search_categories/get_category_detail/get_active_categories/get_category_tree)、店铺(get_shop_info/get_freight_templates/get_after_sale_addresses/get_delivery_companies)、文档(search_wechat_docs)。",
        "主动使用工具获取信息，不要猜测。不确定时查文档、查类目详情、查已有数据。",
        "所有业务时间按 Asia/Shanghai / UTC+08:00 理解。",
        "最终输出必须是机器可解析 JSON，严格遵循 output_schema。",
      ].join("\n"),
    getAppendSystemPrompt: () => [],
    extendResources: () => {},
    reload: async () => {},
    // 暴露自定义工具
    getCustomTools: () => customTools,
  };
}

async function run(request) {
  const authStorage = AuthStorage.inMemory();
  const modelRegistry = ModelRegistry.inMemory(authStorage);
  const model = configureModelRegistry(authStorage, modelRegistry, request);

  const cwd = process.cwd();
  const resourceLoader = createResourceLoader(request);
  const customTools = resourceLoader.getCustomTools();

  const { session } = await createAgentSession({
    cwd,
    agentDir: cwd,
    model,
    thinkingLevel: "off",
    authStorage,
    modelRegistry,
    resourceLoader,
    noTools: "builtin",
    customTools,
    sessionManager: SessionManager.inMemory(cwd),
    settingsManager: SettingsManager.inMemory({
      compaction: { enabled: false },
      retry: { enabled: false },
    }),
  });

  try {
    await session.prompt(buildPrompt(request), {
      expandPromptTemplates: false,
      source: "rpc",
    });
    const text = session.getLastAssistantText();
    if (!text) {
      const lastAssistant = [...session.messages].reverse().find((message) => message.role === "assistant");
      if (lastAssistant?.errorMessage) {
        throw new Error(lastAssistant.errorMessage);
      }
    }
    return extractJson(text);
  } finally {
    session.dispose();
  }
}

if (process.argv.includes("--check")) {
  console.log("ready");
  process.exit(0);
}

try {
  const raw = await readStdin();
  if (!raw.trim()) {
    fail("stdin 不能为空", 2);
  }
  const request = JSON.parse(raw);
  const response = await run(request);
  process.stdout.write(`${JSON.stringify(response)}\n`);
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
