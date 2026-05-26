#!/usr/bin/env python3
"""Download WeChat Shop API documentation into docs/wechat-shop-api.

The script is intentionally self-contained so it can run in a fresh project.
It keeps an explicit API list, preserves official HTML under raw/, and writes
an index, implementation notes, a manifest, and a failure report.
"""

from __future__ import annotations

import hashlib
import json
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
from zoneinfo import ZoneInfo


BASE_URL = "https://developers.weixin.qq.com/doc/store/shop/API/"
PROJECT_ROOT = Path(__file__).resolve().parents[1]
DOC_ROOT = PROJECT_ROOT / "docs" / "wechat-shop-api"
RAW_ROOT = DOC_ROOT / "raw"
SHANGHAI_TZ = ZoneInfo("Asia/Shanghai")


@dataclass(frozen=True)
class ApiDoc:
    module: str
    title: str
    path: str
    purpose: str
    priority: str
    permission: str = "待确认"

    @property
    def url(self) -> str:
        return BASE_URL + self.path

    @property
    def local_path(self) -> Path:
        return RAW_ROOT / self.path


API_DOCS: list[ApiDoc] = [
    # 基础调用与素材
    ApiDoc("基础调用与素材", "获取接口调用凭据", "apimgnt/common/api_getaccesstoken.html", "逐店直连获取 access_token", "核心必用", "是"),
    ApiDoc("基础调用与素材", "获取稳定版接口调用凭据", "apimgnt/common/api_getstableaccesstoken.html", "token 刷新和稳定调用备选方案", "核心候选", "是"),
    ApiDoc("基础调用与素材", "查询接口调用额度", "apimgnt/common/api_getapiquota.html", "监控接口额度和限流策略", "核心候选", "是"),
    ApiDoc("基础调用与素材", "查询 rid 信息", "apimgnt/common/api_getridinfo.html", "按微信返回 rid 排查失败请求", "核心候选", "是"),
    ApiDoc("基础调用与素材", "图片上传", "apimgnt/resource/api_img_upload.html", "商品主图、详情图、售后凭证上传", "核心必用", "是"),
    ApiDoc("基础调用与素材", "资质图片上传", "apimgnt/resource/api_qualificationupload.html", "品牌、类目、特殊资质上传", "核心候选", "是"),
    ApiDoc("基础调用与素材", "视频初始化上传", "apimgnt/resource/api_video_initupload.html", "商品视频素材上传", "可选增强", "是"),
    ApiDoc("基础调用与素材", "视频分片上传", "apimgnt/resource/api_video_uploadpart.html", "商品视频素材上传", "可选增强", "是"),
    ApiDoc("基础调用与素材", "视频完成上传", "apimgnt/resource/api_video_finishupload.html", "商品视频素材上传", "可选增强", "是"),
    ApiDoc("基础调用与素材", "获取视频播放信息", "apimgnt/resource/api_video_getplayinfo.html", "校验商品视频素材", "可选增强", "是"),
    # 店铺管理
    ApiDoc("店铺管理", "获取店铺基本信息", "storemanage/api_mmecapi_basicinfo.html", "店铺接入校验、店铺资料同步", "核心必用", "是"),
    ApiDoc("店铺管理", "获取店铺二维码", "storemanage/api_getshopqrcode.html", "店铺推广物料", "可选增强", "是"),
    ApiDoc("店铺管理", "获取店铺 H5 URL", "storemanage/api_getshoph5url.html", "店铺外链和运营入口", "可选增强", "是"),
    ApiDoc("店铺管理", "获取店铺推广链接", "storemanage/api_getshoptaglink.html", "渠道推广和投放链接", "可选增强", "是"),
    # 类目、品牌、发品规则
    ApiDoc("类目、品牌、发品规则", "获取所有类目", "channels-shop-category/api_getallcategory.html", "建立本地类目树", "核心必用", "是"),
    ApiDoc("类目、品牌、发品规则", "获取类目详情", "channels-shop-category/api_getcategorydetail.html", "获取类目属性、资质和发品约束", "核心必用", "是"),
    ApiDoc("类目、品牌、发品规则", "获取类目发品规则", "category-rule/api_getcategoryproductrule.html", "铺货前规则校验", "核心必用", "是"),
    ApiDoc("类目、品牌、发品规则", "获取类目配送方式规则", "category-rule/api_get_delivery_method_category_rule.html", "运费模板和发货方式校验", "核心候选", "是"),
    ApiDoc("类目、品牌、发品规则", "获取有效品牌列表", "brand/api_getvalidbrandlistlogic.html", "品牌匹配和资质校验", "核心候选", "是"),
    ApiDoc("类目、品牌、发品规则", "获取品牌详情", "brand/api_getbrandlogic.html", "品牌资质和发品限制确认", "核心候选", "是"),
    # 商品与铺货
    ApiDoc("商品与铺货", "新增商品", "channels-shop-product/shop/api_addproduct.html", "批量铺货创建店铺商品", "核心必用", "是"),
    ApiDoc("商品与铺货", "更新商品", "channels-shop-product/shop/api_updateproduct.html", "同步标题、图片、价格、SKU 和属性", "核心必用", "是"),
    ApiDoc("商品与铺货", "获取商品", "channels-shop-product/shop/api_getproduct.html", "同步店铺商品详情和审核结果", "核心必用", "是"),
    ApiDoc("商品与铺货", "获取商品列表", "channels-shop-product/shop/api_getproductlist.html", "店铺商品盘点和增量同步", "核心必用", "是"),
    ApiDoc("商品与铺货", "商品上架", "channels-shop-product/shop/api_listingproduct.html", "铺货后上架", "核心必用", "是"),
    ApiDoc("商品与铺货", "商品下架", "channels-shop-product/shop/api_delistingproduct.html", "断货、违规、滞销下架", "核心必用", "是"),
    ApiDoc("商品与铺货", "删除商品", "channels-shop-product/shop/api_deleteproduct.html", "清理无效或重复铺货商品", "核心候选", "是"),
    ApiDoc("商品与铺货", "发品前校验", "channels-shop-product/shop/api_categoryprecheck.html", "铺货前校验类目和参数", "核心必用", "是"),
    ApiDoc("商品与铺货", "商品类目预测", "channels-shop-product/shop/api_product_classify.html", "根据标题和主图推荐类目", "核心候选", "是"),
    ApiDoc("商品与铺货", "获取商品审核额度", "channels-shop-product/shop/api_getproductauditquota.html", "铺货节奏和审核额度控制", "核心候选", "是"),
    ApiDoc("商品与铺货", "获取免审策略", "channels-shop-product/shop/api_getproductauditstrategy.html", "识别免审或审核策略", "可选增强", "是"),
    ApiDoc("商品与铺货", "设置免审策略", "channels-shop-product/shop/api_setproductauditstrategy.html", "策略配置参考", "可选增强", "是"),
    ApiDoc("商品与铺货", "获取商品二维码", "channels-shop-product/shop/api_getproductqrcode.html", "商品推广物料", "可选增强", "是"),
    ApiDoc("商品与铺货", "获取商品 H5 URL", "channels-shop-product/shop/api_getproducth5url.html", "商品链接和运营验证", "可选增强", "是"),
    ApiDoc("商品与铺货", "获取商品 scheme", "channels-shop-product/shop/api_getproductscheme.html", "跳转链路和私域投放", "可选增强", "是"),
    ApiDoc("商品与铺货", "外部商品映射", "channels-shop-product/shop/api_externalproductmapping.html", "供应商商品和微信商品映射参考", "核心候选", "是"),
    ApiDoc("商品与铺货", "新版外部商品映射", "channels-shop-product/shop/api_externalproductmappingnew.html", "供应商商品和微信商品映射参考", "核心候选", "是"),
    # 库存与促销
    ApiDoc("库存与促销", "获取库存", "channels-shop-product/stock/api_getstock.html", "单商品库存同步", "核心必用", "是"),
    ApiDoc("库存与促销", "批量获取库存", "channels-shop-product/stock/api_batchgetstock.html", "多商品库存盘点", "核心必用", "是"),
    ApiDoc("库存与促销", "更新库存", "channels-shop-product/stock/api_updatestock.html", "供应商库存变更同步到店铺 SKU", "核心必用", "是"),
    ApiDoc("库存与促销", "获取库存流水", "channels-shop-product/stock/api_getstockflow.html", "库存异常追踪", "核心候选", "是"),
    ApiDoc("库存与促销", "新增限时抢购任务", "channels-shop-product/limiteddiscounttask/api_addlimiteddiscounttask.html", "活动促销参考", "可选增强", "是"),
    ApiDoc("库存与促销", "获取限时抢购任务列表", "channels-shop-product/limiteddiscounttask/api_getlimiteddiscounttasklist.html", "活动状态同步", "可选增强", "是"),
    ApiDoc("库存与促销", "更新限时抢购任务", "channels-shop-product/limiteddiscounttask/api_updatelimiteddiscounttask.html", "活动促销维护", "可选增强", "是"),
    ApiDoc("库存与促销", "停止限时抢购任务", "channels-shop-product/limiteddiscounttask/api_stoplimiteddiscounttask.html", "活动停止和风控", "可选增强", "是"),
    # 订单、发货、物流
    ApiDoc("订单、发货、物流", "获取订单详情", "channels-shop-order/api_getorder.html", "订单详情同步", "核心必用", "是"),
    ApiDoc("订单、发货、物流", "获取订单列表", "channels-shop-order/api_getorderlist.html", "订单增量同步", "核心必用", "是"),
    ApiDoc("订单、发货、物流", "搜索订单", "channels-shop-order/api_searchorder.html", "异常订单补偿查询", "核心候选", "是"),
    ApiDoc("订单、发货、物流", "解密敏感信息", "channels-shop-order/api_decodesensitiveinfo.html", "收件人信息解密和采购履约", "核心必用", "是"),
    ApiDoc("订单、发货、物流", "修改商家备注", "channels-shop-order/api_changemerchantnotes.html", "订单运营标记", "可选增强", "是"),
    ApiDoc("订单、发货、物流", "修改订单地址", "channels-shop-order/api_changeorderaddress.html", "买家改址场景处理", "可选增强", "是"),
    ApiDoc("订单、发货、物流", "订单发货", "channels-shop-delivery/delivery/api_senddelivery.html", "供应商物流回填后发货", "核心必用", "是"),
    ApiDoc("订单、发货、物流", "获取快递公司列表新版", "channels-shop-delivery/delivery/api_getdeliverycompanylistnew.html", "物流公司编码映射", "核心必用", "是"),
    ApiDoc("订单、发货、物流", "新增运费模板", "channels-shop-delivery/merchant/api_addfreighttemplate.html", "店铺运费模板管理", "核心候选", "是"),
    ApiDoc("订单、发货、物流", "获取运费模板列表", "channels-shop-delivery/merchant/api_getfreighttemplatelist.html", "铺货选择运费模板", "核心候选", "是"),
    ApiDoc("订单、发货、物流", "获取运费模板详情", "channels-shop-delivery/merchant/api_getfreighttemplatedetail.html", "校验运费模板规则", "核心候选", "是"),
    # 售后与纠纷
    ApiDoc("售后与纠纷", "获取售后单列表", "channels-shop-aftersale/aftersale/api_getaftersalelist.html", "售后增量同步", "核心必用", "是"),
    ApiDoc("售后与纠纷", "获取售后单详情", "channels-shop-aftersale/aftersale/api_getaftersaleorder.html", "售后详情同步", "核心必用", "是"),
    ApiDoc("售后与纠纷", "同意售后申请", "channels-shop-aftersale/aftersale/api_acceptapply.html", "售后处理", "核心候选", "是"),
    ApiDoc("售后与纠纷", "拒绝售后申请", "channels-shop-aftersale/aftersale/api_rejectapply.html", "售后处理", "核心候选", "是"),
    ApiDoc("售后与纠纷", "获取售后原因", "channels-shop-aftersale/aftersale/api_getaftersalereason.html", "售后原因字典", "核心候选", "是"),
    ApiDoc("售后与纠纷", "获取售后拒绝原因", "channels-shop-aftersale/aftersale/api_getaftersalerejectreason.html", "售后拒绝原因字典", "核心候选", "是"),
    ApiDoc("售后与纠纷", "搜索纠纷单", "channels-shop-aftersale/guarantee/api_searchguaranteeorder.html", "纠纷单同步", "核心候选", "是"),
    ApiDoc("售后与纠纷", "获取纠纷单详情", "channels-shop-aftersale/guarantee/api_getguaranteeorder.html", "纠纷单处理", "核心候选", "是"),
    # 供应商与代发参考
    ApiDoc("供应商与代发参考", "获取供应商列表", "supplier/relation/api_get_supplier_list.html", "官方供应商关系参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "邀请供应商", "supplier/relation/api_invite_supplier.html", "官方供应商关系参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "代发订单列表", "supplier/order/api_dropship_list.html", "官方代发订单模型参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "代发订单详情", "supplier/order/api_dropship_get.html", "官方代发订单模型参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "搜索代发订单", "supplier/order/api_dropship_search.html", "官方代发订单检索参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "分配代发订单", "supplier/order/api_dropship_assign.html", "官方代发履约参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "取消代发订单", "supplier/order/api_dropship_cancel.html", "官方代发履约参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "获取供应商商品列表", "supplier/auto/api_get_product_list.html", "官方供应商商品参考", "参考", "待确认"),
    ApiDoc("供应商与代发参考", "获取分销设置", "supplier/auto/api_get_distribution.html", "官方分销设置参考", "参考", "待确认"),
    # 动销与经营分析
    ApiDoc("动销与经营分析", "获取店铺整体数据", "compass/api_getshopoverall.html", "店铺经营看板", "核心候选", "是"),
    ApiDoc("动销与经营分析", "获取店铺商品列表数据", "compass/api_getshopproductlist.html", "商品动销排行", "核心候选", "是"),
    ApiDoc("动销与经营分析", "获取店铺商品详情数据", "compass/api_getshopproductdata.html", "商品动销诊断", "核心候选", "是"),
    ApiDoc("动销与经营分析", "获取成交画像数据", "compass/api_getshopsaleprofiledata.html", "成交用户与销售画像", "可选增强", "是"),
    ApiDoc("动销与经营分析", "获取店铺收藏数", "favorite/shopfavorite/api_getfavoritescount.html", "店铺关注和收藏指标", "可选增强", "是"),
    ApiDoc("动销与经营分析", "获取订单资金流水", "funds/funds/api_listorderflow.html", "毛利和结算核对", "可选增强", "是"),
    ApiDoc("动销与经营分析", "获取账户余额", "funds/funds/api_getbalance.html", "财务看板参考", "可选增强", "是"),
]


IMPLEMENTATION_NOTES = """# 微信小店 API 实现注意事项

## 接入与凭证

- 本项目默认逐店 `appid/app_secret` 直连接入微信小店，不走服务商授权。
- `app_secret` 必须加密存储，禁止出现在前端、日志、测试快照、文档样例和异常堆栈中。
- `access_token` 按店铺缓存，记录过期时间，并在过期前刷新；刷新失败不能影响其他店铺。
- 所有微信 API 调用统一走 `WeChatShopClient`，页面和业务代码不得直接拼接官方 URL。

## 限流、重试与排障

- 每个店铺、每类接口分别做限流，批量铺货、订单同步、库存同步必须队列化。
- 网络失败、限流、临时服务错误允许有限重试；参数错误、权限错误、审核错误不应盲目重试。
- 调用失败要保存店铺、接口、任务 ID、微信错误码、`rid`、响应摘要、重试次数和下一次重试时间。
- 遇到微信返回 `rid` 时，优先使用本地调用日志和 `api_getridinfo` 排查。

## 商品与铺货

- 发品前必须校验类目、品牌、属性、资质、运费模板和图片素材。
- 商品模型按“供应商商品 -> 标准商品 -> 店铺刊登商品”映射，不要把微信 `product_id` 当作内部主键。
- 同一商品批量铺到多店时，必须记录每个店铺的 `product_id`、`sku_id`、审核状态和失败原因，支持单项重试。
- 铺货失败不要整批回滚已成功店铺，应该按任务项补偿。

## 库存与价格

- 库存更新优先使用增减量语义；谨慎使用直接设置库存，避免高并发下覆盖微信侧库存。
- 供应商 SKU、标准 SKU、店铺 SKU 必须建立稳定映射。
- 断货、低于安全库存、供应商下架时，应触发店铺 SKU 库存同步、下架或人工审核任务。
- 价格变更建议进入审核队列，避免供应商成本波动直接影响所有店铺售价。

## 订单、发货与售后

- 订单同步、敏感信息解密、采购单生成、物流回填、微信发货必须走后台任务。
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
"""


def now_shanghai() -> str:
    return datetime.now(SHANGHAI_TZ).isoformat(timespec="seconds")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_bytes_if_exists(path: Path) -> bytes | None:
    if not path.exists():
        return None
    return path.read_bytes()


def download_one(doc: ApiDoc) -> dict[str, object]:
    request = Request(
        doc.url,
        headers={
            "User-Agent": "Mozilla/5.0 wx-xd-doc-archiver/1.0",
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        },
    )
    started = time.time()
    try:
        with urlopen(request, timeout=30) as response:
            status = getattr(response, "status", 200)
            content = response.read()
            content_type = response.headers.get("content-type", "")
        if status >= 400:
            raise HTTPError(doc.url, status, f"HTTP {status}", hdrs=None, fp=None)
        if b"<html" not in content[:5000].lower() and b"<!doctype html" not in content[:5000].lower():
            raise ValueError("response does not look like HTML")

        doc.local_path.parent.mkdir(parents=True, exist_ok=True)
        old_content = read_bytes_if_exists(doc.local_path)
        changed = old_content != content
        if changed:
            doc.local_path.write_bytes(content)
        return {
            "module": doc.module,
            "title": doc.title,
            "priority": doc.priority,
            "permission": doc.permission,
            "purpose": doc.purpose,
            "source_url": doc.url,
            "local_path": str(doc.local_path.relative_to(PROJECT_ROOT)),
            "status": "downloaded" if changed else "unchanged",
            "bytes": len(content),
            "sha256": sha256_bytes(content),
            "content_type": content_type,
            "duration_ms": round((time.time() - started) * 1000),
            "fetched_at": now_shanghai(),
        }
    except (HTTPError, URLError, TimeoutError, ValueError) as exc:
        return {
            "module": doc.module,
            "title": doc.title,
            "priority": doc.priority,
            "permission": doc.permission,
            "purpose": doc.purpose,
            "source_url": doc.url,
            "local_path": str(doc.local_path.relative_to(PROJECT_ROOT)),
            "status": "failed",
            "error": repr(exc),
            "duration_ms": round((time.time() - started) * 1000),
            "fetched_at": now_shanghai(),
        }


def grouped_docs(docs: Iterable[ApiDoc]) -> dict[str, list[ApiDoc]]:
    groups: dict[str, list[ApiDoc]] = {}
    for doc in docs:
        groups.setdefault(doc.module, []).append(doc)
    return groups


def write_index(results: list[dict[str, object]]) -> None:
    result_by_url = {str(item["source_url"]): item for item in results}
    lines: list[str] = [
        "# 微信小店 API 文档索引",
        "",
        f"- 生成时间：{now_shanghai()}",
        f"- 官方源：{BASE_URL}",
        f"- 原始 HTML 目录：`docs/wechat-shop-api/raw/`",
        f"- 接口数量：{len(API_DOCS)}",
        "- 优先级说明：`核心必用` 是系统主链路必须实现；`核心候选` 是高概率使用；`可选增强` 是运营增强；`参考` 是官方模式参考。",
        "",
        "## 模块清单",
        "",
    ]
    for module, docs in grouped_docs(API_DOCS).items():
        lines.append(f"### {module}")
        lines.append("")
        lines.append("| 优先级 | 接口 | 本系统用途 | 权限确认 | 本地文档 | 官方链接 | 下载状态 |")
        lines.append("| --- | --- | --- | --- | --- | --- | --- |")
        for doc in docs:
            result = result_by_url.get(doc.url, {})
            status = str(result.get("status", "not-run"))
            local = str(doc.local_path.relative_to(PROJECT_ROOT))
            lines.append(
                f"| {doc.priority} | {doc.title} | {doc.purpose} | {doc.permission} | "
                f"`{local}` | [官方文档]({doc.url}) | {status} |"
            )
        lines.append("")
    lines.extend(
        [
            "## 使用约定",
            "",
            "- 实现微信接口前，先阅读本索引、`implementation-notes.md` 和对应原始 HTML。",
            "- 原始 HTML 只作为官方快照缓存，不在其中手工改动内容。",
            "- 如果官方文档更新，重新运行 `python3 scripts/download_wechat_shop_docs.py`。",
            "- 下载失败的接口查看 `failed-downloads.md`，优先确认官方路径是否调整。",
            "",
        ]
    )
    (DOC_ROOT / "index.md").write_text("\n".join(lines), encoding="utf-8")


def write_manifest(results: list[dict[str, object]]) -> None:
    manifest = {
        "generated_at": now_shanghai(),
        "timezone": "Asia/Shanghai",
        "source_base_url": BASE_URL,
        "total": len(results),
        "downloaded": sum(1 for item in results if item["status"] == "downloaded"),
        "unchanged": sum(1 for item in results if item["status"] == "unchanged"),
        "failed": sum(1 for item in results if item["status"] == "failed"),
        "items": results,
    }
    (DOC_ROOT / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def write_failures(results: list[dict[str, object]]) -> None:
    failed = [item for item in results if item["status"] == "failed"]
    lines = [
        "# 微信小店 API 文档下载失败清单",
        "",
        f"- 生成时间：{now_shanghai()}",
        f"- 失败数量：{len(failed)}",
        "",
    ]
    if failed:
        lines.append("| 模块 | 接口 | 优先级 | 官方链接 | 错误 |")
        lines.append("| --- | --- | --- | --- | --- |")
        for item in failed:
            error = str(item.get("error", "")).replace("|", "\\|")
            lines.append(
                f"| {item['module']} | {item['title']} | {item['priority']} | "
                f"[官方文档]({item['source_url']}) | `{error}` |"
            )
    else:
        lines.append("本次没有下载失败的文档。")
    lines.append("")
    (DOC_ROOT / "failed-downloads.md").write_text("\n".join(lines), encoding="utf-8")


def write_notes() -> None:
    (DOC_ROOT / "implementation-notes.md").write_text(IMPLEMENTATION_NOTES, encoding="utf-8")


def main() -> int:
    DOC_ROOT.mkdir(parents=True, exist_ok=True)
    RAW_ROOT.mkdir(parents=True, exist_ok=True)

    results: list[dict[str, object]] = []
    with ThreadPoolExecutor(max_workers=8) as executor:
        futures = {executor.submit(download_one, doc): doc for doc in API_DOCS}
        for future in as_completed(futures):
            results.append(future.result())

    priority_order = {doc.url: index for index, doc in enumerate(API_DOCS)}
    results.sort(key=lambda item: priority_order[str(item["source_url"])])

    write_index(results)
    write_notes()
    write_manifest(results)
    write_failures(results)

    failed_count = sum(1 for item in results if item["status"] == "failed")
    downloaded_count = sum(1 for item in results if item["status"] == "downloaded")
    unchanged_count = sum(1 for item in results if item["status"] == "unchanged")
    print(
        f"done: total={len(results)} downloaded={downloaded_count} "
        f"unchanged={unchanged_count} failed={failed_count}"
    )
    if failed_count:
        print(f"failure report: {DOC_ROOT / 'failed-downloads.md'}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

