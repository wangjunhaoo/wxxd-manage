# 微信小店 API 文档索引

- 生成时间：2026-05-22T15:31:36+08:00
- 官方源：https://developers.weixin.qq.com/doc/store/shop/API/
- 原始 HTML 目录：`docs/wechat-shop-api/raw/`
- 接口数量：80
- 优先级说明：`核心必用` 是系统主链路必须实现；`核心候选` 是高概率使用；`可选增强` 是运营增强；`参考` 是官方模式参考。

## 模块清单

### 基础调用与素材

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 获取接口调用凭据 | 逐店直连获取 access_token | 是 | `docs/wechat-shop-api/raw/apimgnt/common/api_getaccesstoken.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/common/api_getaccesstoken.html) | unchanged |
| 核心候选 | 获取稳定版接口调用凭据 | token 刷新和稳定调用备选方案 | 是 | `docs/wechat-shop-api/raw/apimgnt/common/api_getstableaccesstoken.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/common/api_getstableaccesstoken.html) | unchanged |
| 核心候选 | 查询接口调用额度 | 监控接口额度和限流策略 | 是 | `docs/wechat-shop-api/raw/apimgnt/common/api_getapiquota.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/common/api_getapiquota.html) | unchanged |
| 核心候选 | 查询 rid 信息 | 按微信返回 rid 排查失败请求 | 是 | `docs/wechat-shop-api/raw/apimgnt/common/api_getridinfo.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/common/api_getridinfo.html) | unchanged |
| 核心必用 | 图片上传 | 商品主图、详情图、售后凭证上传 | 是 | `docs/wechat-shop-api/raw/apimgnt/resource/api_img_upload.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/resource/api_img_upload.html) | unchanged |
| 核心候选 | 资质图片上传 | 品牌、类目、特殊资质上传 | 是 | `docs/wechat-shop-api/raw/apimgnt/resource/api_qualificationupload.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/resource/api_qualificationupload.html) | unchanged |
| 可选增强 | 视频初始化上传 | 商品视频素材上传 | 是 | `docs/wechat-shop-api/raw/apimgnt/resource/api_video_initupload.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/resource/api_video_initupload.html) | unchanged |
| 可选增强 | 视频分片上传 | 商品视频素材上传 | 是 | `docs/wechat-shop-api/raw/apimgnt/resource/api_video_uploadpart.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/resource/api_video_uploadpart.html) | unchanged |
| 可选增强 | 视频完成上传 | 商品视频素材上传 | 是 | `docs/wechat-shop-api/raw/apimgnt/resource/api_video_finishupload.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/resource/api_video_finishupload.html) | unchanged |
| 可选增强 | 获取视频播放信息 | 校验商品视频素材 | 是 | `docs/wechat-shop-api/raw/apimgnt/resource/api_video_getplayinfo.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/apimgnt/resource/api_video_getplayinfo.html) | unchanged |

### 店铺管理

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 获取店铺基本信息 | 店铺接入校验、店铺资料同步 | 是 | `docs/wechat-shop-api/raw/storemanage/api_mmecapi_basicinfo.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/storemanage/api_mmecapi_basicinfo.html) | unchanged |
| 可选增强 | 获取店铺二维码 | 店铺推广物料 | 是 | `docs/wechat-shop-api/raw/storemanage/api_getshopqrcode.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/storemanage/api_getshopqrcode.html) | unchanged |
| 可选增强 | 获取店铺 H5 URL | 店铺外链和运营入口 | 是 | `docs/wechat-shop-api/raw/storemanage/api_getshoph5url.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/storemanage/api_getshoph5url.html) | unchanged |
| 可选增强 | 获取店铺推广链接 | 渠道推广和投放链接 | 是 | `docs/wechat-shop-api/raw/storemanage/api_getshoptaglink.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/storemanage/api_getshoptaglink.html) | unchanged |

### 类目、品牌、发品规则

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 获取所有类目 | 建立本地类目树 | 是 | `docs/wechat-shop-api/raw/channels-shop-category/api_getallcategory.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-category/api_getallcategory.html) | unchanged |
| 核心必用 | 获取类目详情 | 获取类目属性、资质和发品约束 | 是 | `docs/wechat-shop-api/raw/channels-shop-category/api_getcategorydetail.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-category/api_getcategorydetail.html) | unchanged |
| 核心必用 | 获取类目发品规则 | 铺货前规则校验 | 是 | `docs/wechat-shop-api/raw/category-rule/api_getcategoryproductrule.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/category-rule/api_getcategoryproductrule.html) | unchanged |
| 核心候选 | 获取类目配送方式规则 | 运费模板和发货方式校验 | 是 | `docs/wechat-shop-api/raw/category-rule/api_get_delivery_method_category_rule.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/category-rule/api_get_delivery_method_category_rule.html) | unchanged |
| 核心候选 | 获取有效品牌列表 | 品牌匹配和资质校验 | 是 | `docs/wechat-shop-api/raw/brand/api_getvalidbrandlistlogic.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/brand/api_getvalidbrandlistlogic.html) | unchanged |
| 核心候选 | 获取品牌详情 | 品牌资质和发品限制确认 | 是 | `docs/wechat-shop-api/raw/brand/api_getbrandlogic.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/brand/api_getbrandlogic.html) | unchanged |

### 商品与铺货

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 新增商品 | 批量铺货创建店铺商品 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_addproduct.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_addproduct.html) | unchanged |
| 核心必用 | 更新商品 | 同步标题、图片、价格、SKU 和属性 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_updateproduct.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_updateproduct.html) | unchanged |
| 核心必用 | 获取商品 | 同步店铺商品详情和审核结果 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproduct.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproduct.html) | unchanged |
| 核心必用 | 获取商品列表 | 店铺商品盘点和增量同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproductlist.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproductlist.html) | unchanged |
| 核心必用 | 商品上架 | 铺货后上架 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_listingproduct.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_listingproduct.html) | unchanged |
| 核心必用 | 商品下架 | 断货、违规、滞销下架 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_delistingproduct.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_delistingproduct.html) | unchanged |
| 核心候选 | 删除商品 | 清理无效或重复铺货商品 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_deleteproduct.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_deleteproduct.html) | unchanged |
| 核心必用 | 发品前校验 | 铺货前校验类目和参数 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_categoryprecheck.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_categoryprecheck.html) | unchanged |
| 核心候选 | 商品类目预测 | 根据标题和主图推荐类目 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_product_classify.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_product_classify.html) | unchanged |
| 核心候选 | 获取商品审核额度 | 铺货节奏和审核额度控制 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproductauditquota.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproductauditquota.html) | unchanged |
| 可选增强 | 获取免审策略 | 识别免审或审核策略 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproductauditstrategy.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproductauditstrategy.html) | unchanged |
| 可选增强 | 设置免审策略 | 策略配置参考 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_setproductauditstrategy.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_setproductauditstrategy.html) | unchanged |
| 可选增强 | 获取商品二维码 | 商品推广物料 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproductqrcode.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproductqrcode.html) | unchanged |
| 可选增强 | 获取商品 H5 URL | 商品链接和运营验证 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproducth5url.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproducth5url.html) | unchanged |
| 可选增强 | 获取商品 scheme | 跳转链路和私域投放 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_getproductscheme.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_getproductscheme.html) | unchanged |
| 核心候选 | 外部商品映射 | 供应商商品和微信商品映射参考 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_externalproductmapping.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_externalproductmapping.html) | unchanged |
| 核心候选 | 新版外部商品映射 | 供应商商品和微信商品映射参考 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/shop/api_externalproductmappingnew.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/shop/api_externalproductmappingnew.html) | unchanged |

### 库存与促销

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 获取库存 | 单商品库存同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/stock/api_getstock.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/stock/api_getstock.html) | unchanged |
| 核心必用 | 批量获取库存 | 多商品库存盘点 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/stock/api_batchgetstock.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/stock/api_batchgetstock.html) | unchanged |
| 核心必用 | 更新库存 | 供应商库存变更同步到店铺 SKU | 是 | `docs/wechat-shop-api/raw/channels-shop-product/stock/api_updatestock.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/stock/api_updatestock.html) | unchanged |
| 核心候选 | 获取库存流水 | 库存异常追踪 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/stock/api_getstockflow.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/stock/api_getstockflow.html) | unchanged |
| 可选增强 | 新增限时抢购任务 | 活动促销参考 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/limiteddiscounttask/api_addlimiteddiscounttask.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/limiteddiscounttask/api_addlimiteddiscounttask.html) | unchanged |
| 可选增强 | 获取限时抢购任务列表 | 活动状态同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/limiteddiscounttask/api_getlimiteddiscounttasklist.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/limiteddiscounttask/api_getlimiteddiscounttasklist.html) | unchanged |
| 可选增强 | 更新限时抢购任务 | 活动促销维护 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/limiteddiscounttask/api_updatelimiteddiscounttask.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/limiteddiscounttask/api_updatelimiteddiscounttask.html) | unchanged |
| 可选增强 | 停止限时抢购任务 | 活动停止和风控 | 是 | `docs/wechat-shop-api/raw/channels-shop-product/limiteddiscounttask/api_stoplimiteddiscounttask.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-product/limiteddiscounttask/api_stoplimiteddiscounttask.html) | unchanged |

### 订单、发货、物流

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 获取订单详情 | 订单详情同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-order/api_getorder.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-order/api_getorder.html) | unchanged |
| 核心必用 | 获取订单列表 | 订单增量同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-order/api_getorderlist.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-order/api_getorderlist.html) | unchanged |
| 核心候选 | 搜索订单 | 异常订单补偿查询 | 是 | `docs/wechat-shop-api/raw/channels-shop-order/api_searchorder.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-order/api_searchorder.html) | unchanged |
| 核心必用 | 解密敏感信息 | 收件人信息解密和采购履约 | 是 | `docs/wechat-shop-api/raw/channels-shop-order/api_decodesensitiveinfo.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-order/api_decodesensitiveinfo.html) | unchanged |
| 可选增强 | 修改商家备注 | 订单运营标记 | 是 | `docs/wechat-shop-api/raw/channels-shop-order/api_changemerchantnotes.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-order/api_changemerchantnotes.html) | unchanged |
| 可选增强 | 修改订单地址 | 买家改址场景处理 | 是 | `docs/wechat-shop-api/raw/channels-shop-order/api_changeorderaddress.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-order/api_changeorderaddress.html) | unchanged |
| 核心必用 | 订单发货 | 供应商物流回填后发货 | 是 | `docs/wechat-shop-api/raw/channels-shop-delivery/delivery/api_senddelivery.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-delivery/delivery/api_senddelivery.html) | unchanged |
| 核心必用 | 获取快递公司列表新版 | 物流公司编码映射 | 是 | `docs/wechat-shop-api/raw/channels-shop-delivery/delivery/api_getdeliverycompanylistnew.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-delivery/delivery/api_getdeliverycompanylistnew.html) | unchanged |
| 核心候选 | 新增运费模板 | 店铺运费模板管理 | 是 | `docs/wechat-shop-api/raw/channels-shop-delivery/merchant/api_addfreighttemplate.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-delivery/merchant/api_addfreighttemplate.html) | unchanged |
| 核心候选 | 获取运费模板列表 | 铺货选择运费模板 | 是 | `docs/wechat-shop-api/raw/channels-shop-delivery/merchant/api_getfreighttemplatelist.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-delivery/merchant/api_getfreighttemplatelist.html) | unchanged |
| 核心候选 | 获取运费模板详情 | 校验运费模板规则 | 是 | `docs/wechat-shop-api/raw/channels-shop-delivery/merchant/api_getfreighttemplatedetail.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-delivery/merchant/api_getfreighttemplatedetail.html) | unchanged |

### 售后与纠纷

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心必用 | 获取售后单列表 | 售后增量同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/aftersale/api_getaftersalelist.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/aftersale/api_getaftersalelist.html) | unchanged |
| 核心必用 | 获取售后单详情 | 售后详情同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/aftersale/api_getaftersaleorder.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/aftersale/api_getaftersaleorder.html) | unchanged |
| 核心候选 | 同意售后申请 | 售后处理 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/aftersale/api_acceptapply.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/aftersale/api_acceptapply.html) | unchanged |
| 核心候选 | 拒绝售后申请 | 售后处理 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/aftersale/api_rejectapply.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/aftersale/api_rejectapply.html) | unchanged |
| 核心候选 | 获取售后原因 | 售后原因字典 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/aftersale/api_getaftersalereason.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/aftersale/api_getaftersalereason.html) | unchanged |
| 核心候选 | 获取售后拒绝原因 | 售后拒绝原因字典 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/aftersale/api_getaftersalerejectreason.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/aftersale/api_getaftersalerejectreason.html) | unchanged |
| 核心候选 | 搜索纠纷单 | 纠纷单同步 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/guarantee/api_searchguaranteeorder.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/guarantee/api_searchguaranteeorder.html) | unchanged |
| 核心候选 | 获取纠纷单详情 | 纠纷单处理 | 是 | `docs/wechat-shop-api/raw/channels-shop-aftersale/guarantee/api_getguaranteeorder.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/channels-shop-aftersale/guarantee/api_getguaranteeorder.html) | unchanged |

### 供应商与代发参考

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 参考 | 获取供应商列表 | 官方供应商关系参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/relation/api_get_supplier_list.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/relation/api_get_supplier_list.html) | unchanged |
| 参考 | 邀请供应商 | 官方供应商关系参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/relation/api_invite_supplier.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/relation/api_invite_supplier.html) | unchanged |
| 参考 | 代发订单列表 | 官方代发订单模型参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/order/api_dropship_list.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/order/api_dropship_list.html) | unchanged |
| 参考 | 代发订单详情 | 官方代发订单模型参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/order/api_dropship_get.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/order/api_dropship_get.html) | unchanged |
| 参考 | 搜索代发订单 | 官方代发订单检索参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/order/api_dropship_search.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/order/api_dropship_search.html) | unchanged |
| 参考 | 分配代发订单 | 官方代发履约参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/order/api_dropship_assign.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/order/api_dropship_assign.html) | unchanged |
| 参考 | 取消代发订单 | 官方代发履约参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/order/api_dropship_cancel.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/order/api_dropship_cancel.html) | unchanged |
| 参考 | 获取供应商商品列表 | 官方供应商商品参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/auto/api_get_product_list.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/auto/api_get_product_list.html) | unchanged |
| 参考 | 获取分销设置 | 官方分销设置参考 | 待确认 | `docs/wechat-shop-api/raw/supplier/auto/api_get_distribution.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/supplier/auto/api_get_distribution.html) | unchanged |

### 动销与经营分析

| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 核心候选 | 获取店铺整体数据 | 店铺经营看板 | 是 | `docs/wechat-shop-api/raw/compass/api_getshopoverall.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/compass/api_getshopoverall.html) | unchanged |
| 核心候选 | 获取店铺商品列表数据 | 商品动销排行 | 是 | `docs/wechat-shop-api/raw/compass/api_getshopproductlist.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/compass/api_getshopproductlist.html) | unchanged |
| 核心候选 | 获取店铺商品详情数据 | 商品动销诊断 | 是 | `docs/wechat-shop-api/raw/compass/api_getshopproductdata.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/compass/api_getshopproductdata.html) | unchanged |
| 可选增强 | 获取成交画像数据 | 成交用户与销售画像 | 是 | `docs/wechat-shop-api/raw/compass/api_getshopsaleprofiledata.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/compass/api_getshopsaleprofiledata.html) | unchanged |
| 可选增强 | 获取店铺收藏数 | 店铺关注和收藏指标 | 是 | `docs/wechat-shop-api/raw/favorite/shopfavorite/api_getfavoritescount.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/favorite/shopfavorite/api_getfavoritescount.html) | unchanged |
| 可选增强 | 获取订单资金流水 | 毛利和结算核对 | 是 | `docs/wechat-shop-api/raw/funds/funds/api_listorderflow.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/funds/funds/api_listorderflow.html) | unchanged |
| 可选增强 | 获取账户余额 | 财务看板参考 | 是 | `docs/wechat-shop-api/raw/funds/funds/api_getbalance.html` | [官方文档](https://developers.weixin.qq.com/doc/store/shop/API/funds/funds/api_getbalance.html) | unchanged |

## 使用约定

- 实现微信接口前，先阅读本索引、`implementation-notes.md` 和对应原始 HTML。
- 原始 HTML 只作为官方快照缓存，不在其中手工改动内容。
- 如果官方文档更新，重新运行 `python3 scripts/download_wechat_shop_docs.py`。
- 下载失败的接口查看 `failed-downloads.md`，优先确认官方路径是否调整。
