# 微信小店铺货中台

面向两人内部使用的 Tauri 桌面端运营中台，用来管理微信小店多店铺、外部商品批量铺货、订单同步、采购履约、发货、售后和经营分析。

当前首版实现重点是把项目骨架和最小铺货闭环跑通：

- `Tauri v2 + React 18 + TypeScript + Vite + Rust + SQLite`（前端 UI 为「Soft」设计方向，自研零依赖响应式 store，见 `src/runtime/`）
- 本地 SQLite 初始化、WAL、基础表结构和默认店铺组
- 店铺组创建、店铺录入、`app_secret` 加密保存和稳定版 token 验证入口
- 微信统一 client 骨架、加密 token 缓存和脱敏 API 调用日志
- 店铺基础信息同步入口和 API 额度快照查询入口
- 本地主控 HTTP API：默认监听 `127.0.0.1:17890`，支持外部系统查店铺组、创建铺货/改价任务、查询任务、触发 runner，以及查询采购任务、回填供应商物流、标记供应商异常、查询售后/纠纷、记录售后责任、记录纠纷本地跟进、记录/更新/导出本地凭证、记录供应商售后协同、受控提交售后同意/拒绝和记录利润调整
- 桌面端生成/重置本地 HTTP API Key，后端只保存摘要，不保存明文 Key
- 外部 HTTP API 审计：记录方法、路径、状态码、耗时和脱敏请求/响应摘要
- 外部铺货任务创建命令：`create_external_publish_job`
- 铺货任务查询命令：`get_publish_job`
- 任务中心命令：`list_task_runs`、`run_publish_pipeline_once`
- 铺货任务本地 runner：对外只暴露一键铺货推进，内部自动处理店铺校验、类目、属性、素材、提交、审核同步和上架确认
- 微信发品参数草稿：外部可传 `metadata.wechat_category_ids`、`wechat_attrs`、定价策略、运费和服务配置，由系统生成 `metadata.wechat_add_product_payload`
- 类目规则缓存：按店铺同步微信类目树、单类目详情、商品发布规则、发货方式规则和运费模板 ID
- 微信类目预检：由铺货推进内部自动调用 `categoryprecheck`，并用本地类目详情缓存检查必填商品/销售属性
- 必填属性补齐：优先自动使用采集审查结果、AI 建议、类目默认值、SKU 规格同义词和标题规则，高置信时直接写回草稿
- AI Agent 设置页：默认关闭，统一使用 `pi-coding-agent` 调用 OpenAI-compatible 模型网关，API Key 加密保存且可选，采集审查和铺货自动补齐复用同一套技能配置
- 铺货结果页：只显示待铺货、铺货中、已上架和异常；异常原因按人能处理的问题展示
- 图片素材表 `publish_assets`，按店铺缓存微信 `mmecimage.cn/p/` 图片链接并记录失败原因
- 外部图片素材预处理：下载图片、检查 301/302、校验格式/大小/宽高，必要时压缩/转 JPEG 后走微信二进制上传
- 微信发品提交：由铺货推进内部调用 `addproduct`，成功后保存微信 `product_id` 并自动同步审核/上架状态
- 批量改价任务创建、查询、本地前置校验、微信 `updateproduct` 提交和线上价格确认
- 售后同步和异常处理台：拉取售后列表/详情、同步官方拒绝原因、脱敏保存详情、展示处理状态/失败原因、关联订单、回写退款调整项，并支持人工责任归因、供应商赔付回款和人工触发同意/拒绝
- 纠纷/保障单同步：调用微信 `searchguaranteeorder/getguaranteeorder`，脱敏缓存纠纷详情，在售后异常页展示举证状态、赔付金额、过期时间和关联订单，并支持记录本地跟进状态、责任方、备注和供应商赔付回款
- 本地凭证资料包：售后异常页可为售后单/纠纷单记录凭证标题、说明、本地文件路径和来源链接，并标记草稿、已整理、已使用或已归档；列表展示每单本地凭证数量，可按单查看凭证，并支持按筛选或按单据导出 `md/json/jsonl` 元数据清单，只整理本地资料，不读取文件内容、不上传微信、不提交平台处理
- 供应商售后协同：售后异常页和本地主控 API 可按售后单/纠纷单记录脱敏沟通摘要、索证状态、供应商名称和可选采购任务引用；不登录供应商平台、不保存收件信息、不自动处理纠纷，赔付入账仍走利润调整链路
- 通知中心：统一展示铺货失败、采购缺映射、同单多物流、发货失败、售后待处理和同步失败，可筛选未读/级别并定位业务页面
- 采购异常处理：支持人工标记供应商缺货、涨价、取消、质量风险或其他异常，只通知人工处理，不自动换供应商或取消订单
- 采购任务本地 HTTP API：面向后续供应商下单 agent/skill 暴露非敏采购任务、物流回填和异常标记入口，审计摘要不记录收件人敏感信息或完整请求体
- 本地 API 调用器：[scripts/wx_xd_local_api.py](/Users/wangjunhao/Code/project/wx-xd/scripts/wx_xd_local_api.py)，支持店铺组查询、铺货/改价任务创建与查询、队列 runner 触发、采购任务处理、售后列表/责任归因/受控动作/本地凭证记录、状态更新和资料包导出、供应商售后协同、纠纷单同步/本地跟进、订单利润/调整、库存风险和动销分析，供外部系统或后续 agent/skill 复用
- 供应商 agent 安全桥：桌面端“采购任务”页和 [scripts/wx_xd_supplier_agent.py](/Users/wangjunhao/Code/project/wx-xd/scripts/wx_xd_supplier_agent.py) 都支持按 [docs/supplier-agent-protocol.md](/Users/wangjunhao/Code/project/wx-xd/docs/supplier-agent-protocol.md) 导出含货源链接的非敏采购清单、校验外部结果并回填物流/异常/映射
- 订单利润核算：按订单汇总成交额、采购成本、采购运费、退款、售后赔付、供应商赔付回款、其他成本和毛利状态
- 库存风控：基于外部商品 SKU 库存、采购占用、供应商异常和已铺店铺数生成低库存、断货和库存压力提醒
- 商品动销分析：基于真实订单、采购成本、售后关联、铺货覆盖和库存风险生成继续铺货、调价、补货或观察建议
- 数据备份：启动时每日自动备份、本地备份列表、一键备份、SHA-256、SQLite 完整性校验和恢复前回滚备份
- 快递公司同步：履约发货页可按店铺调用微信 `getdeliverycompanylistnew` 同步快递公司编码，发货录单优先使用同步结果，静态常用编码作为兜底
- 前端总览、通知中心、店铺组、外部铺货 API、批量改价、任务中心、履约发货、采购任务、售后异常、利润核算、铺货任务、数据备份主视图
- 浏览器预览模式，便于不开 Tauri 窗口时验证界面

## 本地运行

安装依赖：

```bash
npm install --include=dev
```

安装 Python 采集依赖：

```bash
bash scripts/setup_python_env.sh
```

脚本会创建项目内虚拟环境 `.venv`，并安装淘宝采集运行时依赖，
避免 macOS/Homebrew Python 的 `externally-managed-environment` 限制。桌面端会优先使用
这个虚拟环境里的 Python。

AI Agent 统一使用 `@mariozechner/pi-coding-agent`。设置页的 Base URL 按
OpenAI-compatible `/chat/completions` 网关填写，例如 `http://127.0.0.1:8080/v1`；
API Key 可留空用于无鉴权本地网关，保存后仍会加密存储。

启动前端预览：

```bash
npm run dev -- --host 127.0.0.1
```

启动 Tauri 桌面端：

```bash
npm run tauri dev
```

调用本地主控 API：

```bash
export WX_XD_API_KEY="桌面端生成的 API Key"
python scripts/wx_xd_local_api.py shop-groups
python scripts/wx_xd_local_api.py publish-create --json-file publish_payload.json
python scripts/wx_xd_local_api.py publish-get publish-task-id
python scripts/wx_xd_local_api.py price-create --json-file price_payload.json
python scripts/wx_xd_local_api.py price-get price-task-id
python scripts/wx_xd_local_api.py runner order-sync
python scripts/wx_xd_local_api.py runner order-detail-sync
python scripts/wx_xd_local_api.py runner purchase-task-generation
python scripts/wx_xd_local_api.py runner delivery-submit
python scripts/wx_xd_local_api.py runner publish-pipeline
python scripts/wx_xd_local_api.py runner price-precheck
python scripts/wx_xd_local_api.py runner price-submit
python scripts/wx_xd_local_api.py runner price-confirm
python scripts/wx_xd_local_api.py runner aftersale-sync
python scripts/wx_xd_local_api.py runner operations
python scripts/wx_xd_local_api.py delivery-settings
python scripts/wx_xd_local_api.py delivery-auto-send --enabled
python scripts/wx_xd_local_api.py delivery-companies --shop-id shop-id
python scripts/wx_xd_local_api.py delivery-companies-sync shop-id
python scripts/wx_xd_local_api.py delivery-shipments --status send_failed --limit 100
python scripts/wx_xd_local_api.py delivery-record --order-id order-id --delivery-id SF --waybill-id SF1234567890 --deliver-type 1
python scripts/wx_xd_local_api.py delivery-retry shipment-id
python scripts/wx_xd_local_api.py purchase-tasks --status pending_purchase --limit 100
python scripts/wx_xd_local_api.py purchase-mapping purchase-task-id --external-product-id demo-1688-10001 --external-sku-id black-m --supplier-name "supplier" --supplier-product-id "10001"
python scripts/wx_xd_local_api.py purchase-shipment purchase-task-id --delivery-id SF --waybill-id SF1234567890 --deliver-type 1 --estimated-cost 18.8
python scripts/wx_xd_local_api.py purchase-issue purchase-task-id --issue-type out_of_stock --note "supplier reported no stock"
python scripts/wx_xd_supplier_agent.py export --status pending_purchase --limit 100 --format jsonl
python scripts/wx_xd_supplier_agent.py template
python scripts/wx_xd_supplier_agent.py apply --json-file supplier-results.jsonl --dry-run
python scripts/wx_xd_supplier_agent.py apply --json-file supplier-results.jsonl --continue-on-error --result-out artifacts/supplier-agent/apply-result.json
python scripts/wx_xd_local_api.py aftersale-reject-reasons-sync shop-id
python scripts/wx_xd_local_api.py aftersale-reject-reasons --shop-id shop-id --reject-scene 1
python scripts/wx_xd_local_api.py aftersales --status active --limit 100
python scripts/wx_xd_local_api.py aftersale-responsibility aftersale-id --party supplier --supplier-compensation-cents 1200 --note "supplier confirmed compensation"
python scripts/wx_xd_local_api.py aftersale-evidence-record --target-type guarantee --target-id guarantee-id --evidence-type supplier_proof --title "supplier proof" --content-text "metadata only"
python scripts/wx_xd_local_api.py aftersale-evidence --target-type guarantee --status draft
python scripts/wx_xd_local_api.py aftersale-evidence-status evidence-id --status ready
python scripts/wx_xd_local_api.py aftersale-evidence-export --target-type guarantee --status ready --format md
python scripts/wx_xd_local_api.py aftersale-accept aftersale-id --accept-type 2 --note "manual approval"
python scripts/wx_xd_local_api.py aftersale-reject aftersale-id --reject-reason-type 1 --reject-reason "evidence mismatch"
python scripts/wx_xd_local_api.py guarantee-sync
python scripts/wx_xd_local_api.py guarantee-orders --status active --limit 100
python scripts/wx_xd_local_api.py order-profits --status pending_cost --limit 100
python scripts/wx_xd_local_api.py order-profit-adjustment order-id --kind other_cost --amount-cents 300 --note "manual cost adjustment"
python scripts/wx_xd_local_api.py inventory-risks --status low_stock --limit 100
python scripts/wx_xd_local_api.py product-sales --status scale_candidate --limit 100
python scripts/wx_xd_local_api.py inventory-scan
```

构建检查：

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

## 打包安装

生成 macOS 安装包：

```bash
npm run tauri build
```

打包前会自动执行：

- `npm run build`：构建前端资源。
- `npm run bundle:runtime`：把淘宝采集 Python 依赖和 AI Agent Node 依赖准备到 `runtime/`。
- Tauri 打包：把前端、Rust 后端、Python 脚本、`python-vendor`、Node 可执行文件、`node_modules` 和 `requirements.txt` 一并写入 `.app` 资源目录。

产物位置：

- `.app`：`src-tauri/target/release/bundle/macos/微信小店铺货中台.app`
- `.dmg`：`src-tauri/target/release/bundle/dmg/微信小店铺货中台_0.1.0_aarch64.dmg`

安装方式：双击 `.dmg`，把 `微信小店铺货中台.app` 拖到 `Applications`。

注意：当前安装包已包含 Python 业务依赖、Node 可执行文件和 Node 业务依赖；
淘宝采集脚本仍会调用系统里的 `/usr/bin/python3`。如果要发给完全没有
Python/Command Line Tools 的机器，后续应把 Python 也改成 Tauri sidecar 随包分发。

## 文档入口

- 微信小店官方 API 缓存：[docs/wechat-shop-api/index.md](/Users/wangjunhao/Code/project/wx-xd/docs/wechat-shop-api/index.md)
- API 接入注意事项：[docs/wechat-shop-api/implementation-notes.md](/Users/wangjunhao/Code/project/wx-xd/docs/wechat-shop-api/implementation-notes.md)
- 系统设计：[docs/system-design.md](/Users/wangjunhao/Code/project/wx-xd/docs/system-design.md)
- Tauri 架构：[docs/desktop-architecture.md](/Users/wangjunhao/Code/project/wx-xd/docs/desktop-architecture.md)
- 功能清单：[docs/feature-checklist.md](/Users/wangjunhao/Code/project/wx-xd/docs/feature-checklist.md)

## 开发规则

开发前先读 [AGENTS.md](/Users/wangjunhao/Code/project/wx-xd/AGENTS.md)。关键约束包括：

- 默认中文沟通和文档，业务时间使用 `Asia/Shanghai`。
- 逐店 `appid/app_secret` 直连，不走服务商授权。
- 店铺密钥必须加密存储，不能写入前端、日志、测试快照或文档样例。
- AI Provider 默认关闭，API Key 必须加密保存，不得写入前端、日志、测试快照或文档样例。
- 微信 API 统一走 client，集中处理 token、限流、重试、错误码和日志。
- 长耗时任务必须进入本地任务队列，不在页面请求中阻塞。
- 铺货页面只暴露一键推进和异常处理，不能把微信 `categoryprecheck`、素材上传、`addproduct` 或上架确认拆成用户操作。
- 自动推进只能串联现有 runner，单步失败要可见，并且必须尊重自动发货开关。
- 自寄快递发货前优先同步微信快递公司列表，`delivery_id` 必须来自官方列表或明确兜底为 `OTHER`。
- 首版采购为人工处理；供应商 agent 桥可在桌面端或脚本中导出非敏任务并写回物流、异常或映射结果，不自动调用供应商平台下单。
- 自动推进里的铺货默认作为一个整体执行；底层前置校验、必填属性补齐、类目预检、素材上传、`addproduct` 提交、状态同步和上架提交不再拆成页面开关。
- 售后拒绝原因优先使用店铺级官方缓存；同意/拒绝只允许人工或受控本地主控 API 明确触发，必须写入最近动作、微信 API 日志和通知中心；不得进入自动推进 runner。
- 库存风控只能基于真实外部商品库存、采购占用、供应商异常和店铺商品映射生成提醒，不得虚构库存或用库存提醒引导虚假动销。
- 动销分析只使用真实订单、真实售后、真实库存、采购成本和店铺商品映射生成建议，不把铺货建议当成自动刷单或虚假动销动作。
- 系统只做合规外部供应商代发与运营中台，不做刷单、虚假动销、绕审核或规避规则能力。

## 下一步

优先继续补齐：

1. 更深的供应商协同处理、售后凭证上传和受控纠纷平台处理动作。
2. 外部图片 AI 补齐、去重和生产化素材规则。
3. 铺货异常的自动归因、自动修复和批量重试体验。
