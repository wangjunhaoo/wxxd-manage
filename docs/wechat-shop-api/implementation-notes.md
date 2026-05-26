# 微信小店 API 实现注意事项

## 接入与凭证

- 本项目默认逐店 `appid/app_secret` 直连接入微信小店，不走服务商授权。
- `app_secret` 必须加密存储，禁止出现在前端、日志、测试快照、文档样例和异常堆栈中。
- `app_secret` 和 `access_token` 在 SQLite 中只保存密文；主密钥优先保存到系统 Keychain，Keychain 不可用时才落本地加密兜底。
- `access_token` 按店铺缓存，记录上海时区到期时间，并在过期前刷新；刷新失败不能影响其他店铺。
- token 获取优先使用稳定版接口 `POST https://api.weixin.qq.com/cgi-bin/stable_token`，请求体为 `grant_type/appid/secret/force_refresh`；默认 `force_refresh=false`，强制刷新必须谨慎使用。
- 所有微信 API 调用统一走 `WeChatShopClient`，页面和业务代码不得直接拼接官方 URL。
- 店铺凭证验证成功后，下一步必须调用 `GET /channels/ec/basics/info/get` 同步店铺昵称、头像、主体类型、开店状态和原始 ID，作为店铺健康状态的依据。
- API 额度查询使用 `POST /cgi-bin/openapi/quota/get`，请求体 `cgi_path` 只保存接口路径，例如 `/channels/ec/basics/info/get`，不要带域名。

## 限流、重试与排障

- 每个店铺、每类接口分别做限流，批量铺货、订单同步、库存同步必须队列化。
- 网络失败、限流、临时服务错误允许有限重试；参数错误、权限错误、审核错误不应盲目重试。
- 调用失败要保存店铺、接口、任务 ID、微信错误码、`rid`、响应摘要、重试次数和下一次重试时间。
- token、店铺资料、额度查询都要写入脱敏调用日志；日志中不得出现 `secret`、`access_token` 或完整请求体。
- 遇到微信返回 `rid` 时，优先使用本地调用日志和 `api_getridinfo` 排查。

## 商品与铺货

- 发品前必须校验类目、品牌、属性、资质、运费模板和图片素材。
- 商品模型按“供应商商品 -> 标准商品 -> 店铺刊登商品”映射，不要把微信 `product_id` 当作内部主键。
- 类目树、类目详情、商品发布规则、发货方式规则和运费模板 ID 必须按店铺缓存，后续 AI 补齐和发品前校验优先使用本地缓存。
- 类目详情同步使用 `POST /shop/ec/category/detail`；商品发布规则使用 `POST /shop/ec/category/getcategoryproductrule`；发货方式规则使用 `POST /shop/ec/category/getcategoryrule?rule_id=2`；运费模板列表使用 `POST /channels/ec/merchant/getfreighttemplatelist`。
- 同一商品批量铺到多店时，必须记录每个店铺的 `product_id`、`sku_id`、审核状态和失败原因，支持单项重试。
- 铺货失败不要整批回滚已成功店铺，应该按任务项补偿。
- 本地前置校验通过只代表任务项进入 `ready_to_publish`，不代表微信已发布成功；只有 `img/upload`、`addproduct` 和后续审核状态确认后才能进入成功态。
- 当前本地 runner 必须先校验店铺激活、密钥存在、重复铺货、主图不少于 3 张、详情图存在、类目线索存在、SKU 有可售库存，并生成或验证微信 `addproduct` 参数草稿。
- 微信 `addproduct` 的商品头图和详情图必须使用 `img/upload` 返回的 `mmecimage.cn/p/` 链接；外部原图 URL 只能进入素材上传阶段，不能直接进入发品参数。
- `img/upload` 当前优先使用 `upload_type=0`、`resp_type=1` 二进制上传：先由本地下载外部图片、禁止 301/302 跳转、校验格式和大小，必要时压缩/转 JPEG 后再上传，避免微信 URL 模式的跳转、Content-Length 和 Content-Type 问题。
- 如果源 URL 已经是 `mmecimage.cn/p/`，本系统只记录复用，不再次上传；同店铺同源图已上传成功时也复用历史微信图片链接。
- 素材上传阶段成功后任务项进入 `assets_ready`，仍不代表商品已发布；下一阶段必须构造 `addproduct` 参数并保存微信返回的商品 ID。
- `addproduct` 接口路径为 `POST /channels/ec/product/add`，成功后必须保存微信返回的 `product_id` 到任务项和店铺商品映射。
- 如果外部已提供 `metadata.wechat_add_product_payload`，提交阶段直接使用并覆盖注入 `head_imgs` 和 `desc_info.imgs`。
- 如果外部没有完整 payload，前置校验会基于 `metadata.wechat_category_ids`/`cats_v2`、`wechat_attrs`、SKU 规格、成本价、定价策略、运费模板和 `extra_service` 生成 `metadata.wechat_add_product_payload` 草稿；缺少真实微信类目 ID 时必须失败，不能用 `category_hint` 臆造类目。
- 草稿生成阶段如果没有 `metadata.wechat_attrs` 会写入任务日志警告；后续应基于已缓存的 `api_getcategorydetail` 结果和 AI/规则补齐必填属性。
- 类目预检阶段使用 `POST /channels/ec/product/categoryprecheck`，任务项通过后进入 `category_prechecked`；如果微信返回 `all_pass=false`，必须把 `fail_reasons` 写入任务项失败原因。
- 本地已缓存类目详情时，素材上传前要用 `product_attr_list` 和 `sale_attr_list` 的必填项校验 `metadata.wechat_attrs` 与 `skus[].specs`/`sku_attrs`；缺失时任务项失败并提示 AI/外部系统补齐。
- `CATEGORY_ATTRS_NEED_AI_FILL` 失败项先走 `run_publish_attribute_fill_once`：外部 AI 可写入 `metadata.ai_attr_suggestions`，系统也会生成 `publish_attribute_suggestions.prompt_json`；只有高置信补齐才写回 `metadata.wechat_add_product_payload` 并重新进入类目预检。
- 可选 AI provider 只用于生成属性候选值：默认关闭，桌面端配置 OpenAI-compatible `base_url/model/API Key` 后，`run_publish_ai_attribute_suggestions_once` 会读取 `publish_attribute_suggestions.prompt_json`、调用模型返回 JSON，再复用本地允许值校验和高置信应用规则。
- AI provider API Key 按店铺密钥同等级加密保存；日志、外部 API 审计、通知、任务错误和文档样例不得出现 API Key、完整 prompt 或完整模型响应。
- AI 输出不能替代微信官方类目详情、发品规则和 `categoryprecheck`，只能把可验证候选值写入 `publish_attribute_suggestions` 或高置信写回发品草稿；写回后必须重新跑类目预检。
- 人工采纳属性建议使用 `apply_publish_attribute_suggestions`，支持 SKU 级销售属性映射；采纳后只更新 `metadata.wechat_add_product_payload` 并重新校验缺失属性，不能直接进入素材上传或发布成功。
- 本地未缓存类目详情时不能臆造必填属性，只能记录告警并建议先同步类目规则；官方 `categoryprecheck` 仍需要继续执行，用于店铺和类目资质检查。
- `addproduct` 成功后进入 `submitted`，表示已提交微信，仍需后续轮询/同步审核状态后才能进入最终成功。
- `getproduct` 接口路径为 `POST /channels/ec/product/get`；提交后的任务项使用 `product_id` 和 `data_type=3` 同步线上与草稿状态。
- `status=5` 才表示商品已上架成功；`status=4` 或 `edit_status=4` 只表示审核通过，任务项进入 `audit_passed`，后续仍需上架动作。
- `submitted`、`audit_pending` 都是未完成状态；任务中心不能把这两类状态记为最终成功。
- 审核失败、异步失败、quota 不足、限频、冻结、风控下架、封禁、商品不存在等微信状态必须写入 `error_code` 和 `error_summary`，方便按店铺/商品定位失败原因。
- `listingproduct` 接口路径为 `POST /channels/ec/product/listing`，请求体只需要 `product_id`；只允许对 `audit_passed` 任务项调用。
- `listingproduct` 返回 ok 只能说明上架请求已提交，任务项应回到 `audit_pending`，后续仍需 `getproduct` 确认 `status=5`。
- 官方提示频繁调用上架接口可能被封禁；若商品处于审核中、上传中或异步提审中，不能重复调用上架。

## 库存与价格

- 库存更新优先使用增减量语义；谨慎使用直接设置库存，避免高并发下覆盖微信侧库存。
- 供应商 SKU、标准 SKU、店铺 SKU 必须建立稳定映射。
- 断货、低于安全库存、供应商下架时，应触发店铺 SKU 库存同步、下架或人工审核任务。
- 价格变更建议进入审核队列，避免供应商成本波动直接影响所有店铺售价。
- 当前库存风控先基于外部商品 SKU 库存、采购占用、供应商异常和店铺商品映射生成断货/低库存/库存压力提醒；未接入微信 `updatestock` 前不得改写微信侧库存。
- 当前商品动销分析先基于本地真实订单、订单项、采购成本、售后关联、铺货店铺数和库存风险生成建议；未接入微信罗盘、收藏和资金流水前，不得臆造曝光、点击、收藏或结算数据。
- 批量改价任务先做本地前置校验，校验通过进入 `ready_to_update`；真实提交必须调用微信 `updateproduct`。
- 提交 `updateproduct` 前必须先调用 `getproduct(data_type=3)` 获取完整商品线上/草稿数据，在完整商品参数基础上只修改 `skus[].sale_price`，避免只传价格片段导致微信拒绝或覆盖商品字段。
- `updateproduct` 返回成功后只能进入 `submitted`，表示微信已接收改价请求；后续仍需同步商品状态和 SKU 价格后才能进入最终成功。
- 改价确认使用 `getproduct(data_type=3)`；只有线上 `product.skus[].sale_price` 全部等于目标价时才进入 `success`，如果只有 `edit_product` 草稿价格匹配则保持 `audit_pending` 继续轮询。
- 改价任务必须依赖本地 `shop_products` 的 `wechat_product_id`；缺少已铺货商品或微信商品 ID 时要失败可见，不能猜测商品。

## 订单、发货与售后

- 订单同步、敏感信息解密、采购单生成、物流回填、微信发货必须走后台任务。
- `getorderlist` 接口路径为 `POST /channels/ec/order/list/get`；首版默认用最近 1 天创建时间范围和订单状态 `20` 同步待发货订单。
- `getorderlist` 单次时间范围不能超过 7 天，每页数量不能超过 100；本地 runner 单次每店最多翻 5 页，避免页面交互长时间阻塞。
- 订单列表接口只落订单号、店铺、微信状态、内部状态和同步时间；不保存收件人姓名、手机号、地址等敏感信息。
- `getorder` 接口路径为 `POST /channels/ec/order/get`；订单详情同步必须作为订单列表之后的独立队列阶段。
- 订单详情只能白名单保存订单 ID、状态、商品、金额、备注等履约必要字段；收件人姓名、手机号、地址等字段不得进入原始详情摘要、日志或测试快照。
- 订单项必须落本地 `order_items`，保存微信商品 ID、微信 SKU ID、外部商品 ID、外部 SKU ID、数量和金额，作为后续采购任务的来源。
- 敏感信息解密、采购任务生成和发货回填必须作为独立队列阶段，不要混进订单列表同步。
- 采购任务从订单项生成；有外部商品映射的进入 `pending_purchase`，缺少映射的进入 `needs_mapping` 并通知人工处理。
- 采购任务列表和采购表导出只使用非敏字段：订单号、店铺、外部商品/SKU、标题、数量、金额、成本/毛利和处理原因；不要把收件人姓名、手机号、地址放入 CSV。
- 供应商缺货、涨价、取消和质量风险只能标记采购任务异常并写入通知中心；当前系统不得自动换供应商、自动取消采购或自动取消微信订单。
- 面向供应商下单 agent/skill 的本地 HTTP API 只开放非敏采购任务查询、供应商物流回填和供应商异常标记；接口审计只保存任务 ID、异常类型、是否有物流单号/备注等摘要，不保存收件人姓名、手机号、地址或完整请求体。
- 自动采购 agent/skill 只能把供应商平台处理结果回填到本地主控 API，优先复用 `scripts/wx_xd_local_api.py`；不得直接读写 SQLite、不得保存供应商平台登录凭证、不得绕过自动发货开关。
- 订单利润核算只使用订单、订单项、采购任务和订单级调整项；退款、售后赔付、采购运费和其他成本必须落 `order_profit_adjustments`，不能写入日志或临时页面状态后丢失。
- 实际毛利需要采购任务存在且采购成本全部补齐；缺采购任务或缺成本时只能作为预估/待补齐状态展示。
- 首版采购为人工处理，不自动向供应商平台下单；后续自动采购 agent/skill 必须独立开关、独立日志和独立供应商适配器。
- 采购任务物流回填后进入 `supplier_shipped`；只有同一微信订单下所有采购任务都已回填且物流信息一致，才自动创建订单级发货单。
- 同一微信订单存在多个供应商物流时，当前 MVP 不自动调用 `senddelivery`；后续需要按微信多包裹/拆单规则单独设计。
- 物流单号首版人工录入；调用微信 `senddelivery` 必须受自动发货配置开关控制，开关关闭时只能进入待确认发货。
- 任务中心自动推进只能按顺序触发现有 runner；不得跳过 `submitted`、`audit_pending`、`audit_passed`、`ready_to_send` 等中间状态直接写最终成功。
- 自动推进中的铺货链路必须按 `ready_to_publish` -> `category_prechecked` -> `assets_ready` -> `submitted` -> `audit_pending/audit_passed/success` 推进；本地 HTTP API 触发 runner 时也必须遵守同一状态机。
- 自动推进失败要返回 `step/error`，让运营能定位是订单同步、发货、铺货状态同步、上架还是改价确认失败。
- `senddelivery` 接口路径为 `POST /channels/ec/order/delivery/send`，请求体必须包含 `order_id` 和 `delivery_list`。
- 自寄快递发货使用 `deliver_type=1`，必须提供 `delivery_id` 和 `waybill_id`；`delivery_id` 来自 `getdeliverycompanylistnew`，非主流快递可填 `OTHER`。
- `getdeliverycompanylistnew` 接口路径为 `POST /channels/ec/order/deliverycompanylist/new/get`，请求体包含 `ewaybill_only`；本地按店铺缓存 `delivery_id`/`delivery_name`，发货表单优先使用缓存结果。
- 发货商品列表必须来自本地 `order_items` 的微信 `product_id`、`sku_id` 和数量，缺少任一字段时不得调用微信发货。
- 虚拟商品使用 `deliver_type=3` 时必须整单发货；当前 MVP 不自动判断订单是否虚拟，后续需要从订单详情规则补齐。
- 微信发货失败必须保留 `errcode`、`errmsg`、物流单、订单和店铺，常见失败包括订单状态不允许发货、售后未完成、快递公司不合法、单号不合法、风险订单和地址修改待处理。
- 发货失败重试只允许把物流单重新放回本地队列；是否提交微信仍由自动发货开关和 `delivery.submit_wechat_shipment` 队列任务决定。
- `getaftersalelist` 接口路径为 `POST /channels/ec/aftersale/getaftersalelist`；必须成对传入创建时间或更新时间范围，单次范围不能超过 24 小时，本地默认用最近 24 小时更新时间窗口。
- `getaftersaleorder` 接口路径为 `POST /channels/ec/aftersale/getaftersaleorder`；列表只返回售后单号，详情同步必须作为独立请求并记录单个售后失败原因。
- 售后详情入库前必须递归脱敏 `openid`、姓名、电话、地址、联系人等字段；日志中只保存售后单号、状态、错误码和摘要。
- 当前售后只做同步、展示、关联订单、利润退款调整、人工责任归因，以及人工或受控 API 触发的同意/拒绝；纠纷/保障单只做同步、展示和本地跟进记录。本地凭证资料包只保存凭证元数据，不读取文件内容、不上传微信。上传凭证、差额退款和微信纠纷处理动作暂不实现，后续必须独立开关和审计。
- `acceptapply` 接口路径为 `POST /channels/ec/aftersale/acceptapply`，请求体必须包含 `after_sale_order_id`；`address_id` 在同意退货等场景可能必填，缺失时微信可能返回 `10021060`，地址无效可能返回 `10021062`。
- `acceptapply.accept_type=1` 表示同意退货，适用于退货退款或换货场景；`accept_type=2` 表示同意退款，适用于已收到货的退款或仅退款场景；不传时由平台按当前售后状态和类型判断。
- 不要在 1 分钟内重复同意同一订单下多个售后退款，避免触发平台处理中或重复退款风险；微信返回平台处理中时要记录失败原因，等待后续同步或人工复核。
- `rejectapply` 接口路径为 `POST /channels/ec/aftersale/rejectapply`，请求体必须包含 `after_sale_order_id` 和 `reject_reason_type`；`reject_reason` 可填自定义原因，但不能把敏感信息或完整沟通记录写入日志。
- `getaftersalerejectreason` 接口路径为 `POST /channels/ec/aftersale/rejectreason/get`，请求体传空 JSON；返回 `reason_list`，包含 `reject_reason_type`、`reject_reason_type_text`、`reject_reason` 和 `reject_scene`。
- `reject_scene=1/4/5/6/7` 分别表示拒绝仅退款、拒绝退货退款、拒绝换货、拒绝换货发新商品、极速换货收货处理；本地按店铺缓存官方拒绝原因，拒绝售后时优先选择缓存原因。
- `searchguaranteeorder` 接口路径为 `POST /channels/ec/aftersale/searchguaranteeorder`；请求体可按 `begin_time/end_time/type/offset/limit` 搜索保障/纠纷单，当前本地默认同步最近 24 小时并逐单拉详情。
- `getguaranteeorder` 接口路径为 `POST /channels/ec/aftersale/getguaranteeorder`；请求体必须包含 `guarantee_order_id`。详情包含 `openid/unionid` 等买家标识和凭证 media_id，入库前必须递归脱敏，日志不得保存完整买家信息。
- 纠纷单状态中 `STATUS_WAIT_MERCHANT_HANDLE`、`STATUS_WAIT_MERCHANT_PROOF`、`STATUS_WAIT_BOTH_PROOF` 等需要人工处理或举证，必须写入通知中心；当前不自动提交微信纠纷处理动作、不上传凭证。人工记录本地跟进和供应商赔付时，只能写入本地 `guarantee_orders` 和订单级 `other_income` 调整项。
- 售后同意/拒绝调用成功只表示处理动作已提交；本地只记录 `last_action*` 和通知，最终状态必须靠后续 `getaftersaleorder` 同步确认。
- 微信退款成功售后可以写入订单级 `refund` 调整项，但必须按售后单号幂等，避免重复扣减利润。
- 供应商责任归因属于本地人工处理结果；供应商赔付回款只能在售后已关联本地订单时写入订单级 `other_income` 调整项，未关联订单时只能先保留在售后单上。
- 纠纷本地跟进属于本地人工处理结果；`record_guarantee_followup` 不能调用微信纠纷处理接口，供应商赔付回款只有在纠纷已关联本地订单时才能写入订单级 `other_income` 调整项。
- 售后/纠纷本地凭证资料属于本地人工整理结果；`record_aftersale_evidence`、`update_aftersale_evidence_status` 和 `export_aftersale_evidence` 不能调用微信素材、投诉举证或保障单举证接口，`used` 只表示内部资料已被人工使用过，不代表微信举证已提交；导出文件不得包含文件内容、收件信息、微信凭据或平台处理结果，日志和通知中也不能保存完整说明、本地路径、来源链接或图片内容。
- 敏感信息只在履约所需范围内解密和展示，日志中必须脱敏。
- 外部供应商代发场景下，内部采购单是履约主线；微信订单、供应商订单和物流单需要三方关联。
- 售后单和纠纷单要关联原订单、采购单、供应商责任和物流信息，避免只处理微信侧状态。

## 时间与数据口径

- 所有业务时间默认使用 `Asia/Shanghai` / `UTC+08:00`。
- 任务调度、订单同步窗口、动销日统计、月统计必须按上海时区计算边界。
- 官方接口如返回 Unix 秒级时间戳，入库时记录原始值和转换后的带时区时间。

## 合规边界

- 系统只做合规外部供应商代发、铺货、履约和运营分析。
- 不实现刷单、虚假动销、绕审核、绕平台规则、批量规避风控等能力。
