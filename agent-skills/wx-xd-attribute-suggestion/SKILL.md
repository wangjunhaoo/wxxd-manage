---
name: wx-xd-attribute-suggestion
description: 基于微信类目详情、商品资料和 SKU 规格，为发品必填属性生成安全候选值。
version: 1.0.0
---

# 微信小店必填属性建议

当铺货任务因为 `CATEGORY_ATTRS_NEED_AI_FILL` 缺少微信类目必填属性时，使用本技能。

## 目标

- 只基于输入里的商品标题、SKU 规格、外部元数据、微信类目详情和允许值生成建议。
- 商品属性返回 `value`；销售属性如果不同 SKU 需要不同值，优先返回 `sku_values`。
- 不能确定时返回 `value: null`、空 `sku_values` 和低置信度。
- 不得伪造品牌授权、资质、执行标准、材质、适用年龄、库存或任何商品事实。

## 输入

调用方会提供单个缺失属性的 JSON 对象，通常包含：

- `product`：商品标题、类目提示、供应商字段和 SKU 摘要。
- `attr_kind`：`product` 或 `sale`。
- `attr_key`：缺失属性名称。
- `allowed_values`：微信类目详情中的允许值，可能为空。
- `sku_specs`：外部 SKU 规格。
- `existing_payload`：当前微信发品草稿摘要。

## 输出

只返回 JSON，不输出 Markdown。字段：

```json
{
  "value": "属性值或 null",
  "sku_values": [
    {
      "sku_index": 0,
      "value": "SKU 级属性值"
    }
  ],
  "confidence": 0,
  "reason": "一句话说明依据"
}
```

## 输出约束

- `confidence` 是 0 到 100 的整数。
- 如果 `allowed_values` 非空，返回值必须能匹配其中一个允许值。
- 销售属性如果每个 SKU 不同，`sku_values` 必须覆盖可判断的 SKU。
- 低置信或无依据时不要猜测，返回低置信并说明需要人工确认。
