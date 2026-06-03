---
name: wx-xd-attribute-suggestion
description: 补齐单个微信小店发品必填属性的候选值。当铺货流程中仍有属性未补齐时使用。
version: 1.0.0
---

# 微信小店必填属性补齐

当商品审查后仍有必填属性缺失时，使用本技能。**你必须使用查询工具获取数据，不依赖预填信息。**

## 可用工具

| 工具 | 用途 |
|------|------|
| `get_product_detail(task_id)` | 获取商品的标题、SKU、已有的属性建议 |
| `get_category_detail(shop_id, cat_id)` | 获取类目的必填属性定义和 allowed_values |
| `search_wechat_docs(query)` | 查询属性定义和填写规范 |

## 工作流程

### 1. 获取类目要求

```
get_category_detail(shop_id, cat_id)
```

找到当前缺失的属性，确认其类型（select_one/select_many/string）和 allowed_values。

### 2. 获取商品上下文

```
get_product_detail(task_id)
```

查看标题、SKU 规格、外部元数据（taobao_item_params）、已有的 ai_attr_suggestions。

### 3. 推断属性值

按优先级：

1. **ai_attr_suggestions 已有值** → 直接使用（置信度 96）
2. **allowed_values 中仅有一个选项** → 直接使用（置信度 90）
3. **从商品标题匹配** → 检查 allowed_values 中哪个值在标题中出现。例如标题含「纯棉」，allowed_values 中有「纯棉」→ 选中
4. **从 SKU 规格推断**（销售属性）→ 优先返回 `sku_values`，每个 SKU 对应一个值
5. **从 taobao_item_params 匹配** → 如果外部字段名与属性名相关，使用外部值
6. **关联推断** → 已有「面料材质=纯棉」→ 可推断「面料材质成分含量=棉100%」

**不确定时：**
```
search_wechat_docs(query)
```
查文档确认属性含义。

### 4. 输出

按 output_schema 输出，**value 必须从 allowed_values 中原样选取**（select_one/select_many 类型），不允许缩写或近义词。

- 能确定 → confidence ≥ 85，reason 说明依据
- 有线索但不确定 → confidence 60-84，reason 说明不确定原因
- 完全无法判断 → confidence < 60，value 返回 null
