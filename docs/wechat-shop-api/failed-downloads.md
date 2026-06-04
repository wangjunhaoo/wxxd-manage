# 微信小店 API 文档下载失败清单

- 生成时间：2026-06-04T14:51:31+08:00
- 失败数量：3

| 模块 | 接口 | 优先级 | 官方链接 | 错误 |
| --- | --- | --- | --- | --- |
| 基础调用与素材 | 查询接口调用额度 | 核心候选 | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/common/api_getapiquota.html) | `TimeoutError('The read operation timed out')` |
| 店铺管理 | 获取店铺基本信息 | 核心必用 | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/storemanage/api_mmecapi_basicinfo.html) | `TimeoutError('The read operation timed out')` |
| 类目、品牌、发品规则 | 获取品牌详情 | 核心候选 | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/brand/api_getbrandlogic.html) | `TimeoutError('The read operation timed out')` |
