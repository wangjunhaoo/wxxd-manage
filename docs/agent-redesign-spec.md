# wx-xd Agent 体系重构规格

## 1. 结论

当前项目里的 agent 能力已经有雏形，但它不是一个完整的 agent 体系。

真实代码里现在存在三类能力：

- 采集商品审查：`run_collection_review_once` 调用 AI 审查标题、图片和类目。
- 发品属性建议：`run_publish_ai_attribute_suggestions_once` 对缺失类目属性生成候选值。
- 供应商 Agent 桥：导出非敏采购任务，再接收外部结果写回物流、异常或映射。

问题不是“模型不够强”，而是系统缺少统一规格：

- 没有统一的 agent 定义、运行记录、输入输出校验和版本管理。
- agent 执行结果散落在 `task_logs`、`notifications`、`review_result_json`、`publish_attribute_suggestions` 和 `agent_skill_settings`。
- 只有商品审查是正式 skill，属性建议仍是内联 prompt/schema。
- 前端只能开关和试跑，看不到真实运行历史、失败分类、输入摘要、输出和可回放证据。
- 供应商 agent 桥是安全的协议桥，但还不是可观测、可审计、可评测的 agent 运行链路。

重构目标是把 agent 做成“可治理的业务执行层”，而不是让 AI 直接绕过本地状态机。

## 2. 现状调研

### 2.1 AI provider 配置

后端入口：

- `src-tauri/src/commands/ai.rs`
  - `get_ai_provider_settings`
  - `save_ai_provider_settings`
  - `test_ai_provider`
  - `list_agent_skills`
  - `test_agent_skill`
- `src-tauri/src/commands/support.rs`
  - `load_optional_ai_provider_config`
  - `request_ai_skill_json`
  - `request_pi_agent_json`

当前只保留一个业务运行时，但 provider 可按 Pi 文档选择：

- `pi-coding-agent`：通过 Node 子进程运行 `scripts/pi_agents/json_skill_agent.mjs`。
- 内置 provider 使用 Pi provider id，例如 `xiaomi`、`openai`、`anthropic`、`deepseek`、`openrouter`。
- `custom` 会在内存里按 Pi `models.json` 结构注册 provider，支持 `openai-completions`、`openai-responses`、`anthropic-messages` 和 `google-generative-ai`。

已有优点：

- AI provider 默认关闭。
- API Key 加密存储到 `ai_provider_credentials`。
- Key 只传给本地子进程，不写入日志或数据库明文。
- `pi-coding-agent` 的文件、命令和编辑工具在业务运行时中关闭。

主要问题：

- Rust 只解析 JSON，没有基于 JSON Schema 做统一校验和错误归类。
- 运行超时、模型错误、schema 错误、业务拒绝都被压成 `Validation` 文本，后续很难运营排障。

### 2.2 Agent skill 管理

当前正式 skill：

- `agent-skills/wx-xd-product-review/SKILL.md`
- `agent-skills/wx-xd-product-review/output_schema.json`

Rust 内置定义：

- `PRODUCT_REVIEW_SKILL`
- `builtin_agent_skills() -> vec![&PRODUCT_REVIEW_SKILL]`
- `agent_skill_settings` 只保存启停、模型覆盖、temperature 和最近试跑摘要。

已有优点：

- 商品审查 skill 有独立 `SKILL.md` 和输出 schema。
- 前端有 `AgentSkillsSection.vue` 可查看文件状态、运行时状态、试跑状态。

主要问题：

- skill registry 是硬编码单例，新增 skill 必须改 Rust。
- 没有 input schema、样例集、工具权限、业务场景、自动应用阈值。
- `file_status` 只判断内置文件是否存在或 checksum，不支持版本迁移和兼容性声明。
- `last_test_summary` 只能看最近一次，不能查历史。

### 2.3 采集审查 agent

核心链路：

1. `run_collection_review_once`
2. `review_collection_task`
3. 本地规则先清洗标题、过滤明显不可用图片、匹配类目候选
4. `request_ai_collection_review_json`
5. `apply_ai_collection_review`
6. `apply_ai_review_category`
7. 结果写回 `collection_tasks.review_*`

已有优点：

- AI 只在本地规则之后补充判断。
- 类目只能从本地候选中选择，不能编造微信类目 ID。
- 图片移除有置信度阈值。
- 低置信或 AI 不可用会进入人工确认。

主要问题：

- AI 输入和输出没有落入统一运行记录。
- `notes` 等模型输出没有被系统化展示。
- 图片审查失败只形成 summary，缺少逐图可解释证据。
- 没有固定评测样例，无法判断 prompt 改动后是否变好。
- 商品审查和后续铺货属性补齐没有共享一个“商品质量档案”。

### 2.4 发品属性建议 agent

核心链路：

1. `run_publish_attribute_fill_once`：本地规则补齐。
2. `run_publish_ai_attribute_suggestions_once`：AI 介入缺失属性。
3. `build_attribute_fill_plan`
4. `fill_attribute_plan_with_ai`
5. `request_ai_attribute_suggestion`
6. `upsert_publish_attribute_suggestion`
7. `apply_publish_attribute_suggestions`

已有优点：

- 高置信建议才能自动写回。
- 人工采纳后只回到 `ready_to_publish`，不会跳过微信类目预检。
- 输出写入 `publish_attribute_suggestions`，可以按任务和商品查看。

主要问题：

- 属性建议不是正式 skill，只是内联 `ATTRIBUTE_SUGGESTION_OUTPUT_SCHEMA` 和 instruction。
- `request_ai_chat_json` 和 `request_ai_skill_json` 是两套模型调用协议。
- 单个属性逐次调用模型，缺少批量上下文、缓存和成本控制。
- prompt_json 同时塞 request/response，不利于稳定展示、回放和脱敏审计。

### 2.5 供应商 Agent 桥

核心入口：

- 桌面端：
  - `export_supplier_agent_tasks`
  - `get_supplier_agent_result_template`
  - `apply_supplier_agent_results`
- 脚本：
  - `scripts/wx_xd_supplier_agent.py`
  - `scripts/wx_xd_local_api.py`
- 文档：
  - `docs/supplier-agent-protocol.md`

已有优点：

- 只导出非敏采购字段。
- 写回只允许 `shipment`、`issue`、`mapping` 三类结果。
- 拒绝未知字段和敏感字段。
- 所有写回继续走本地主控 API 或同等后端命令，不直接读写 SQLite。

主要问题：

- 它是“文件/文本桥”，没有 agent run 概念。
- 干跑和写回结果不沉淀为可检索的历史批次。
- 外部 agent 如何完成采购、在哪里运行、用了哪些工具，系统无法感知。
- 不能按采购任务看到 agent 上次导出时间、结果、失败原因和人工确认记录。

## 3. 目标边界

### 3.1 必须做到

- Agent 能力必须可配置、可观测、可回放、可评测。
- 每次 agent 执行都有统一 `agent_runs` 记录。
- 每个 agent 输出都必须过 schema 校验和业务校验。
- 自动动作必须走现有 Rust 命令、队列 runner 或本地主控 HTTP API。
- 人工确认必须是一等状态，不能藏在错误摘要里。
- 所有时间字段使用 `Asia/Shanghai` / `+08:00`。
- 敏感信息不得进入 agent 输入、日志、快照、错误栈或导出文件。

### 3.2 继续禁止

- 禁止 AI 直接写 SQLite。
- 禁止 AI 绕过 API Key 鉴权和审计。
- 禁止 AI 跳过微信 `categoryprecheck`、素材上传、`addproduct`、审核轮询和上架确认。
- 禁止 AI 臆造品牌授权、资质、类目 ID、属性值或库存。
- 禁止自动登录供应商平台并下单，除非后续单独做供应商适配器、显式开关和人工确认。
- 禁止把收件人姓名、手机号、地址、Cookie、`access_token`、`app_secret`、API Key 放进 agent 输入或输出。

## 4. 目标架构

### 4.1 分层

```text
前端 Agent 工作台
  -> Rust Agent Service
    -> Agent Registry
    -> Agent Run Store
    -> Runtime Adapter
       -> pi-coding-agent JSON skill runtime
    -> Tool Gateway
       -> 现有 Tauri commands / local HTTP API / runner
    -> Human Gate
       -> 人工确认、批量采纳、拒绝、重试
```

### 4.2 统一概念

Agent Skill：

- 描述一个业务能力。
- 包含 instruction、input schema、output schema、工具权限、自动应用规则和评测样例。

Agent Run：

- 描述一次 agent 执行。
- 记录来源对象、输入摘要、模型、输出、状态、错误、耗时和人工处理结果。

Tool Call：

- 描述 agent 建议或执行的一个受控动作。
- 只能来自白名单。
- 高风险动作必须人工确认。

Human Gate：

- 决定输出是自动应用、待确认、拒绝、重跑还是转人工。
- 任何影响微信平台或供应商履约的动作都要在这里留下记录。

## 5. 数据模型

### 5.1 `agent_skills`

替代或增强当前 `agent_skill_settings`。

字段建议：

- `name TEXT PRIMARY KEY`
- `version TEXT NOT NULL`
- `description TEXT NOT NULL`
- `scene TEXT NOT NULL`
- `enabled INTEGER NOT NULL`
- `runtime TEXT NOT NULL`
- `provider_override TEXT`
- `model_override TEXT`
- `temperature REAL`
- `skill_path TEXT NOT NULL`
- `input_schema_path TEXT`
- `output_schema_path TEXT NOT NULL`
- `tool_policy_json TEXT NOT NULL`
- `auto_apply_policy_json TEXT NOT NULL`
- `checksum TEXT NOT NULL`
- `status TEXT NOT NULL`
- `last_test_status TEXT`
- `last_test_run_id TEXT`
- `updated_at TEXT NOT NULL`

说明：

- `scene` 可取 `collection_review`、`publish_attribute`、`supplier_bridge`、`operation_planner`。
- `runtime` 可取 `model_skill`、`deterministic_bridge`、`planner`。
- `tool_policy_json` 明确工具白名单和风险级别。
- `auto_apply_policy_json` 明确置信度阈值和业务校验条件。

### 5.2 `agent_runs`

新增核心表。

字段建议：

- `id TEXT PRIMARY KEY`
- `skill_name TEXT NOT NULL`
- `skill_version TEXT NOT NULL`
- `scene TEXT NOT NULL`
- `source_type TEXT NOT NULL`
- `source_id TEXT NOT NULL`
- `shop_id TEXT`
- `status TEXT NOT NULL`
- `provider_type TEXT`
- `model TEXT`
- `temperature REAL`
- `input_summary TEXT NOT NULL`
- `input_snapshot_json TEXT`
- `output_json TEXT`
- `validated_output_json TEXT`
- `tool_calls_json TEXT`
- `decision TEXT`
- `error_code TEXT`
- `error_summary TEXT`
- `started_at TEXT`
- `finished_at TEXT`
- `duration_ms INTEGER`
- `created_at TEXT NOT NULL`

状态：

- `queued`
- `running`
- `succeeded`
- `needs_review`
- `blocked`
- `failed`
- `cancelled`

要求：

- `input_snapshot_json` 必须是脱敏后的业务快照。
- `output_json` 只保存模型结构化输出，不保存完整 prompt。
- 错误要拆成 `error_code` 和 `error_summary`，不能只放整段文本。

### 5.3 `agent_run_events`

记录执行过程。

字段建议：

- `id TEXT PRIMARY KEY`
- `run_id TEXT NOT NULL`
- `event_type TEXT NOT NULL`
- `level TEXT NOT NULL`
- `message TEXT NOT NULL`
- `data_json TEXT`
- `created_at TEXT NOT NULL`

典型事件：

- `input_prepared`
- `runtime_started`
- `model_response_received`
- `schema_validated`
- `business_validated`
- `tool_planned`
- `human_gate_required`
- `applied`
- `failed`

### 5.4 `agent_tool_calls`

当后续做 planner 或更复杂工具调用时新增。

字段建议：

- `id TEXT PRIMARY KEY`
- `run_id TEXT NOT NULL`
- `tool_name TEXT NOT NULL`
- `risk_level TEXT NOT NULL`
- `status TEXT NOT NULL`
- `request_summary TEXT NOT NULL`
- `response_summary TEXT`
- `requires_confirmation INTEGER NOT NULL`
- `confirmed_by TEXT`
- `confirmed_at TEXT`
- `created_at TEXT NOT NULL`

`risk_level`：

- `read`
- `suggest`
- `write_low`
- `write_high`
- `external_platform`

首版只允许 `read`、`suggest`、`write_low`。

## 6. Runtime 规格

### 6.1 统一输入

Rust 调 runtime 时统一传：

```json
{
  "run_id": "agent-run-id",
  "skill": {
    "name": "wx-xd-product-review",
    "version": "1.0.0"
  },
  "runtime": {
    "provider_type": "xiaomi",
    "model": "mimo-v2.5-pro",
    "temperature": 0.1
  },
  "schemas": {
    "input_schema": {},
    "output_schema": {}
  },
  "input": {},
  "tool_policy": {},
  "output_mode": "json_schema"
}
```

### 6.2 统一输出

所有 runtime 必须返回：

```json
{
  "status": "succeeded",
  "output": {},
  "usage": {
    "input_tokens": null,
    "output_tokens": null
  },
  "runtime_meta": {
    "provider_type": "xiaomi",
    "model": "mimo-v2.5-pro"
  },
  "warnings": []
}
```

失败时返回：

```json
{
  "status": "failed",
  "error_code": "SCHEMA_VALIDATION_FAILED",
  "error_summary": "输出缺少 required 字段 category",
  "runtime_meta": {}
}
```

### 6.3 错误分类

后端必须归一化为：

- `PROVIDER_NOT_CONFIGURED`
- `RUNTIME_MISSING`
- `RUNTIME_TIMEOUT`
- `PROVIDER_HTTP_FAILED`
- `PROVIDER_AUTH_FAILED`
- `MODEL_EMPTY_OUTPUT`
- `MODEL_NON_JSON_OUTPUT`
- `SCHEMA_VALIDATION_FAILED`
- `BUSINESS_VALIDATION_FAILED`
- `SENSITIVE_DATA_BLOCKED`
- `TOOL_POLICY_DENIED`
- `UNKNOWN_AGENT_ERROR`

## 7. Skill 规格

### 7.1 `wx-xd-product-review`

用途：

- 审查采集商品能否进入铺货。
- 清洗标题。
- 判断图片风险。
- 从本地候选类目中选择类目。

输入来源：

- `collection_tasks.collected_data`
- 本地类目候选
- 规则过滤后的图片 URL

输出：

- `title`
- `remove_image_urls`
- `category`
- `needs_human_review`
- `notes`
- `confidence`

自动应用条件：

- 总置信度 `>= 75` 才允许采用标题。
- 图片移除单项置信度 `>= 80`。
- 类目置信度 `>= 75`，且 `category_ids` 必须完全匹配候选。
- 任一 `needs_human_review=true` 进入人工确认。

写回：

- `collection_tasks.review_status`
- `collection_tasks.review_summary`
- `collection_tasks.reviewed_data`
- `collection_tasks.review_result_json`
- `agent_runs`

增强点：

- 每张图片记录 `kept/removed/review_needed`。
- 前端详情页展示模型 notes、图片原因和类目选择原因。
- 增加样例集覆盖水印、二维码、店铺招牌、低清图、错误类目。

### 7.2 `wx-xd-attribute-suggestion`

用途：

- 对 `CATEGORY_ATTRS_NEED_AI_FILL` 失败项生成必填属性候选值。

输入来源：

- `publish_job_items.raw_payload`
- 生成或外部传入的微信发品草稿
- `wechat_category_details.raw_payload`
- 缺失属性列表
- SKU 规格和标题

输出：

- `attr_kind`
- `attr_key`
- `value`
- `sku_values`
- `confidence`
- `reason`
- `evidence`

自动应用条件：

- 置信度 `>= 85`。
- 值必须在官方允许值内，或能被本地规则归一到允许值。
- 销售属性必须覆盖全部 SKU，或者明确单值适用于全部 SKU。
- 不得生成资质、品牌授权、执行标准等高风险事实。

写回：

- `publish_attribute_suggestions`
- `agent_runs`
- 高置信时写回发品草稿并回到 `ready_to_publish`
- 低置信时保持 `CATEGORY_ATTRS_NEED_AI_FILL`

增强点：

- 从内联 schema 升级为正式 skill 目录。
- 支持同一商品多个缺失属性一次请求，减少模型调用次数。
- prompt_json 拆成 `input_snapshot_json`、`output_json`、`reason`。

### 7.3 `wx-xd-operation-planner`

用途：

- 读取当前任务、通知、失败项，给出下一轮运营推进计划。
- 只能规划和触发现有 runner，不直接改业务表。

允许工具：

- `list_task_runs`
- `list_notifications`
- `run_publish_tasks_once`
- `run_publish_attribute_fill_once`
- `run_publish_ai_attribute_suggestions_once`
- `run_publish_category_prechecks_once`
- `run_publish_asset_uploads_once`
- `run_publish_submits_once`
- `run_publish_status_sync_once`
- `run_publish_listing_once`
- `run_delivery_submission_once`
- `run_inventory_risk_scan_once`

高风险限制：

- 不允许调用微信售后同意/拒绝。
- 不允许打开自动发货开关。
- 不允许创建供应商平台订单。
- 不允许批量清空、删除、恢复数据库。

输出：

```json
{
  "summary": "建议先处理类目属性，再跑类目预检",
  "steps": [
    {
      "tool": "run_publish_attribute_fill_once",
      "reason": "存在 CATEGORY_ATTRS_NEED_AI_FILL 失败项",
      "risk_level": "write_low",
      "requires_confirmation": false
    }
  ],
  "blocked_items": [],
  "needs_human_review": []
}
```

执行方式：

- 首版只展示计划，不自动执行。
- 第二阶段允许执行 `write_low` 且明确白名单的 runner。
- 高风险步骤只生成待确认动作。

### 7.4 `wx-xd-supplier-bridge`

用途：

- 将供应商 agent 桥纳入统一可观测体系。

性质：

- `runtime = deterministic_bridge`
- 不是模型 skill。
- 负责导出、导入、校验、写回和记录批次。

输入：

- 采购任务筛选条件。
- 外部结果 JSON/JSONL。

输出：

- 导出批次记录。
- 干跑校验结果。
- 写回结果。

写回：

- `agent_runs`
- `agent_run_events`
- 现有采购任务、物流、异常、映射表。

增强点：

- 每次导出生成 `run_id` 或 `batch_id`。
- 结果文件必须带 `source_run_id`，方便追溯。
- 桌面采购页按任务展示最近导出和最近写回状态。

## 8. 后端改造清单

### 8.1 Rust 模块

新增建议：

- `src-tauri/src/agent/mod.rs`
- `src-tauri/src/agent/registry.rs`
- `src-tauri/src/agent/run_store.rs`
- `src-tauri/src/agent/runtime.rs`
- `src-tauri/src/agent/schema.rs`
- `src-tauri/src/agent/tool_policy.rs`
- `src-tauri/src/commands/agent.rs`

迁移方向：

- `commands/ai.rs` 只保留 Tauri command 门面。
- `support.rs` 中 AI provider、runtime、schema、属性建议 helper 拆出到 agent 和 publish 子模块。
- `PRODUCT_REVIEW_SKILL` 从硬编码常量升级为 registry seed。

### 8.2 Tauri commands

新增命令：

- `list_agent_skills`
- `get_agent_skill`
- `save_agent_skill_settings`
- `test_agent_skill`
- `list_agent_runs`
- `get_agent_run`
- `retry_agent_run`
- `cancel_agent_run`
- `list_agent_run_events`
- `run_agent_skill_once`
- `run_operation_planner_once`

保留兼容：

- `get_ai_provider_settings`
- `save_ai_provider_settings`
- `test_ai_provider`
- `run_publish_ai_attribute_suggestions_once`
- `export_supplier_agent_tasks`
- `apply_supplier_agent_results`

### 8.3 本地主控 HTTP API

新增只读接口：

- `GET /api/agent-skills`
- `GET /api/agent-runs`
- `GET /api/agent-runs/{run_id}`
- `GET /api/agent-runs/{run_id}/events`

新增受控执行接口：

- `POST /api/runners/agent-operation-plan`
- `POST /api/agent-runs/{run_id}/retry`

审计要求：

- request summary 只记录 skill、source_type、source_id、shop_id。
- response summary 只记录状态、错误码、是否需要人工确认。
- 不记录完整 input/output。

## 9. 前端工作台规格

### 9.1 Agent 总览

展示：

- AI provider 状态。
- skill 数量、启用数、可运行数。
- 今日运行次数、失败数、待人工确认数。
- 最近失败错误分类。

### 9.2 Skill 详情

展示：

- 版本、描述、场景。
- runtime、模型、temperature。
- input schema、output schema、checksum。
- 工具权限。
- 自动应用阈值。
- 最近 20 次运行。
- 固定样例试跑入口。

### 9.3 Run 详情

展示：

- 来源对象：采集任务、铺货项、采购任务或 runner。
- 输入摘要。
- 脱敏输入快照。
- 模型输出。
- schema 校验结果。
- 业务校验结果。
- 写回结果。
- 错误分类和重试建议。
- 上海时间开始/结束/耗时。

### 9.4 人工确认队列

统一展示：

- 采集审查待确认。
- 属性建议待采纳。
- 供应商结果待写回。
- 运营 planner 高风险动作。

操作：

- 采纳。
- 拒绝。
- 修改后采纳。
- 重跑。
- 标记无需处理。

## 10. 评测规格

### 10.1 样例目录

建议新增：

```text
agent-skills/
  wx-xd-product-review/
    eval_cases/
      clean-product.json
      taobao-watermark.json
      wrong-category.json
      low-confidence-image.json
  wx-xd-attribute-suggestion/
    eval_cases/
      single-option-product-attr.json
      sku-sale-attr.json
      no-safe-answer.json
```

### 10.2 评测维度

商品审查：

- 标题清洗准确率。
- 不可用图片召回率。
- 类目必须来自候选。
- 低置信时必须进入人工确认。

属性建议：

- 值必须在允许值内。
- SKU 映射完整。
- 不能编造高风险属性。
- 高置信自动应用比例。

供应商桥：

- 敏感字段拦截。
- 未知字段拦截。
- 三类 action 路由正确。
- 干跑不产生写回。

### 10.3 验收命令

后续实现时建议提供：

```bash
npm run test:agent
cargo test agent_
node scripts/pi_agents/json_skill_agent.mjs < agent-skills/wx-xd-product-review/eval_cases/clean-product.json
```

后台测试单次不得超过 60 秒。

## 11. 分阶段计划

### P0：规格落地和可观测性

目标：

- 新增 `agent_runs` 和 `agent_run_events`。
- 商品审查和属性建议执行都写 run。
- 供应商桥导出/应用也写 run。
- 错误归一化。

不做：

- 不引入自动规划。
- 不新增高风险工具调用。

验收：

- 任意一次采集审查都能在 Agent 工作台看到 run。
- 任意一次 AI 属性建议都能看到输入摘要、输出和应用结果。
- 任意一次供应商结果写回都能看到干跑/写回结果。

### P1：Skill 正规化

目标：

- `wx-xd-attribute-suggestion` 独立为正式 skill。
- registry 支持多个 skill。
- pi-coding-agent runtime 返回统一 JSON 结构。
- JSON Schema 校验统一。

验收：

- 前端可看到两个 model skill。
- 两个 skill 都能试跑样例。
- runtime 不再固定商品审查 Pydantic 类型。

### P2：Agent 工作台

目标：

- 新增 run 列表、run 详情、事件时间线。
- 人工确认队列统一入口。
- 采集审查、属性建议、供应商桥不再只靠各自页面分散处理。

验收：

- 运营能按失败类型筛选 agent run。
- 运营能从 run 跳回原始业务对象。
- 运营能看到为什么自动应用或为什么需要人工确认。

### P3：运营规划 agent

目标：

- 实现 `wx-xd-operation-planner`。
- 首版只生成计划。
- 后续允许执行低风险 runner。

验收：

- planner 输出的步骤只来自白名单。
- 高风险动作只进入确认队列。
- 单步失败不会影响后续可执行步骤。

### P4：供应商适配器

目标：

- 在供应商桥基础上引入独立供应商适配器。
- 只在显式开关下运行。
- 仍然通过本地主控 API 写回结果。

验收：

- 不暴露收件信息。
- 不绕过 API Key 和审计。
- 不自动向供应商平台提交订单，除非用户在对应适配器中显式确认。

## 12. 近期建议的第一批代码改造

第一批不要做大而全，建议只改三件事：

1. 新增 `agent_runs` / `agent_run_events` 表和基础读写 helper。
2. 给 `review_collection_task`、`run_publish_ai_attribute_suggestions_once`、`apply_supplier_agent_results` 接入 run 记录。
3. 把 `wx-xd-attribute-suggestion` 从内联 schema 拆到 `agent-skills/`，并让现有属性建议链路通过统一 skill runtime。

这样能先把“agent 做得好不好”变成可观察事实，再继续做规划和自动化。

## 13. 完成定义

本轮规格真正落地后，Agent 体系应满足：

- 运营知道哪个 agent 在什么时候处理了哪个业务对象。
- 开发能从 run 追到输入、输出、校验、写回和错误。
- Prompt 或 schema 改动能通过样例集回归。
- AI 只给建议或触发受控动作，不越过业务状态机。
- 所有敏感字段在进入 agent 之前被拦截或脱敏。
- 所有业务时间都以 `Asia/Shanghai` / `+08:00` 记录和展示。
