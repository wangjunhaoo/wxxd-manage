# wx-xd Agent Skills

这里放可复用的运营 agent skill。每个 skill 都是一个独立目录，包含 `SKILL.md` 和可选的 schema、示例或脚本。

## 当前技能

- `wx-xd-product-review`：审查采集商品，清洗标题、识别不能用于微信小店铺货的图片，并从本地微信类目候选中选择类目。
- `wx-xd-attribute-suggestion`：基于微信类目详情、商品资料和 SKU 规格，为发品必填属性生成安全候选值。

## 集成原则

- 桌面端运行时统一通过 `scripts/pi_agents/json_skill_agent.mjs` 调用 `pi-coding-agent`，不再暴露多个 provider 分支。
- `pi-coding-agent` 只作为结构化业务运行时使用，内置文件、命令和编辑工具必须关闭。
- Rust 侧只传入脱敏业务 JSON，API Key 加密存储并仅传给本地子进程，不写入日志或数据库明文。
- AI 只生成审查建议；最终是否进入铺货仍由本地任务状态机、微信类目缓存和人工确认控制。
- skill 不允许保存密钥、Cookie、收件人信息或完整敏感请求体。
