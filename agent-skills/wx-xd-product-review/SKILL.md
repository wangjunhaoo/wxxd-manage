---
name: wx-xd-product-review
description: 审查无货源商品采集结果，清洗标题、识别不可用于微信小店铺货的图片，并基于候选类目给出可复用的运营审查建议。
version: 1.0.0
---

# 微信小店无货源商品审查

当需要判断一个外部采集商品是否可以进入微信小店铺货流程时，使用本技能。

## 目标

- 清洗标题，移除品牌、供应商店名、淘宝/天猫等来源平台词、旗舰店/专卖店等店铺词。
- 审查主图和详情图，识别不能进入微信小店发品素材的图片。
- 在调用方提供的微信类目候选中选择最合适的类目；不能凭空编造微信类目 ID。
- 输出结构化 JSON，供本地状态机决定通过、人工确认或拦截。

## 输入

调用方会提供一个 JSON 对象，通常包含：

- `product`：采集商品信息，包括标题、来源链接、供应商、主图、详情图、SKU。
- `category_candidates`：调用方从本地微信类目缓存中检索到的候选类目。
- `review_rules`：本次审查需要重点执行的规则。
- `image_urls`：随请求附带给模型观察的主图和详情图 URL。

## 审查规则

1. 标题不能包含品牌名、授权暗示、来源平台词、供应商店名或店铺类型词。
2. 主图和详情图不能包含明显淘宝/天猫/店铺标识、二维码、联系方式、旺旺/客服水印、促销贴片、大面积平台水印、外部平台引流信息。
3. 商品图需要体现真实商品本身；只有平台 Logo、店铺招牌、纯广告图、无法识别商品主体的图片应建议移除。
4. 类目只能从 `category_candidates` 中选择。候选不足或不确定时，返回 `needs_human_review: true`。
5. 不得伪造品牌授权、质检、材质、适用年龄、执行标准、微信类目 ID 或商品事实。
6. 低置信判断必须标记为人工确认，不能为了自动通过而臆断。

## 输出

只返回 JSON，不输出 Markdown。字段：

```json
{
  "title": "建议使用的清洗标题，无法确定时为 null",
  "remove_image_urls": [
    {
      "url": "建议移除的图片 URL",
      "reason": "移除原因",
      "confidence": 0
    }
  ],
  "category": {
    "category_ids": [1, 2, 3],
    "category_path": "一级 / 二级 / 三级",
    "confidence": 0,
    "reason": "选择原因"
  },
  "needs_human_review": true,
  "notes": ["给运营看的简短说明"],
  "confidence": 0
}
```

## 输出约束

- `confidence` 是 0 到 100 的整数。
- `remove_image_urls[].url` 必须来自输入图片，不得新增 URL。
- `category.category_ids` 必须完全等于某个输入候选的 `category_ids`。
- 如果没有可靠类目，`category` 返回 `null`，并设置 `needs_human_review: true`。
- 如果图片或标题存在不确定风险，设置 `needs_human_review: true`。
