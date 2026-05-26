# 供应商 Agent 协议

本协议用于后续供应商下单 agent/skill 和 wx-xd 桌面端之间协作。首版只做安全桥接：导出非敏采购任务、接收明确结果并回填本地主控 API。

桌面端“采购任务”页已经内置同一协议，可以直接导出 JSONL/JSON/Markdown、生成结果模板、粘贴 agent 结果、干跑校验和写回。命令行版本用于外部 agent/skill 或自动化脚本。

## 边界

- 不自动登录供应商平台。
- 不自动提交供应商订单。
- 不读取或写入 SQLite。
- 不传递收件人姓名、手机号、地址、`openid`、Cookie、密码、API Key、`access_token` 或供应商平台登录凭证。
- 所有写回都必须走 `scripts/wx_xd_local_api.py` 等价的本地主控 HTTP API，继续保留 API Key 鉴权、审计和自动发货开关。

## 导出采购任务

```bash
export WX_XD_API_KEY="桌面端生成的 API Key"
python scripts/wx_xd_supplier_agent.py export --status pending_purchase --limit 100 --format jsonl
```

导出字段只包含：

- `purchase_task_id`
- `wechat_order_id`
- `shop_id`
- `shop_name`
- `status`
- `external_product_id`
- `source_url`
- `external_sku_id`
- `title`
- `quantity`
- `estimated_revenue`
- `estimated_cost`
- `estimated_profit`
- `supplier_name`
- `supplier_product_id`
- `error_summary`
- `created_at`
- `updated_at`
- `needs_mapping`
- `allowed_result_actions`

`source_url` 是外部同步商品的货源链接，用于人工采购或供应商 agent 定位商品页面；它不属于收件履约敏感信息，但不得包含登录态、Cookie、token、密钥或一次性私密参数。

## 写回结果

先生成模板：

```bash
python scripts/wx_xd_supplier_agent.py template
```

支持三类结果。

### 物流回填

```json
{"purchase_task_id":"purchase-task-id","action":"shipment","delivery_id":"SF","delivery_name":"顺丰速运","waybill_id":"SF1234567890","deliver_type":1,"estimated_cost":18.8}
```

### 采购异常

```json
{"purchase_task_id":"purchase-task-id","action":"issue","issue_type":"out_of_stock","note":"supplier reported no stock"}
```

`issue_type` 只能是：

- `out_of_stock`
- `price_changed`
- `supplier_cancelled`
- `quality_risk`
- `other`

### 映射补齐

```json
{"purchase_task_id":"purchase-task-id","action":"mapping","external_product_id":"external-product-id","external_sku_id":"external-sku-id","source_url":"https://example.com/source-product","supplier_name":"supplier","supplier_product_id":"supplier-product-id","estimated_cost":18.8,"note":"operator confirmed mapping"}
```

## 应用结果

先干跑校验：

```bash
python scripts/wx_xd_supplier_agent.py apply --json-file supplier-results.jsonl --dry-run
```

确认后写回：

```bash
python scripts/wx_xd_supplier_agent.py apply --json-file supplier-results.jsonl --continue-on-error --result-out artifacts/supplier-agent/apply-result.json
```

脚本会拒绝未知字段和敏感字段。包含收件人、手机号、地址、密钥、Cookie 或登录凭证的记录会直接失败。

## 后续升级

如果后续要做自动采购，必须作为独立供应商适配器实现，并且保持可关闭。适配器只能把最终非敏结果写回本协议，不得绕过本地主控 API。
