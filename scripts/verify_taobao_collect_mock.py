#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""本地验证淘宝采集链路。

该脚本不访问真实淘宝。它用 CloakBrowser 拦截一个假淘宝商品页：
1. 首次打开商品页返回 NC 滑块验证页；
2. 滑块通过后写入 x5sec；
3. 再次打开商品页返回商品页，并通过 script 标签触发 mtop detail/desc；
4. 复用 taobao_collector 的验证码处理、响应捕获和商品解析函数。
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
from typing import Any, Dict

from taobao_collector import (
    _capture_product_data,
    _check_access_limit_cooldown,
    _check_captcha_failure_cooldown,
    _clear_access_limit_cooldown,
    _clear_captcha_failure_cooldown,
    _handle_possible_captcha,
    _handle_login_confirm,
    _extract_product_from_dom,
    _launch_cloakbrowser_once,
    _merge_product_with_dom,
    _needs_dom_enrichment,
    _parse_mtop_response,
    _browser_slot_lock,
    _profile_lock,
    _read_access_limit_cooldown,
    _read_captcha_failure_cooldown,
    _record_access_limit_cooldown,
    _trigger_product_requests,
    close_browser_context,
)
from taobao_captcha import (
    get_taobao_access_limited_reason,
    get_taobao_manual_verification_reason,
    is_taobao_captcha_page,
)
from taobao_captcha.solver import CaptchaAttemptHistory


SOURCE_URL = "https://item.taobao.com/item.htm?id=mock-collect"
DETAIL_URL = "https://h5api.m.taobao.com/h5/mtop.taobao.detail.getdetail/1.0/?callback=mtopjsonp1"
DESC_URL = "https://h5api.m.taobao.com/h5/mtop.taobao.detail.getdesc/1.0/?callback=mtopjsonp2"
DOM_FALLBACK_URL = "https://detail.tmall.com/item.htm?id=897770375203"
LOGIN_CONFIRM_URL = "https://login.taobao.com/havanaone/login/login.htm?bizName=taobao&redirectURL=https%3A%2F%2Fitem.taobao.com%2Fitem.htm%3Fid%3Dmock-collect"
CAPTCHA_HISTORY_FILE = "taobao_captcha_history.jsonl"
Page = Any
Request = Any
Route = Any


def _slider_html() -> str:
    return """<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <style>
    #nocaptcha {
      width: 360px;
      margin: 80px;
      padding: 20px;
      border: 1px solid #ddd;
      background: #fff;
    }
    #nc_1_n1t {
      width: 300px;
      height: 38px;
      background: #eee;
      position: relative;
    }
    #nc_1_n1z {
      width: 42px;
      height: 38px;
      background: #1677ff;
      position: absolute;
      left: 0;
      top: 0;
      cursor: pointer;
    }
  </style>
</head>
<body>
  <div id="nocaptcha" class="nc-container">
    <p>请按住滑块，拖动到最右边完成验证码</p>
    <div id="nc_1_n1t" class="nc_scale">
      <div id="nc_1_n1z" class="nc_iconfont"></div>
    </div>
  </div>
  <script>
    document.addEventListener('mouseup', () => {
      document.cookie = 'x5sec=mock_x5sec_collect; path=/; SameSite=None; Secure';
      const node = document.querySelector('#nocaptcha');
      if (node) node.remove();
    });
  </script>
</body>
</html>"""


def _stuck_slider_html() -> str:
    return _slider_html().replace(
        """
  <script>
    document.addEventListener('mouseup', () => {
      document.cookie = 'x5sec=mock_x5sec_collect; path=/; SameSite=None; Secure';
      const node = document.querySelector('#nocaptcha');
      if (node) node.remove();
    });
  </script>""",
        """
  <script>
    document.addEventListener('mouseup', () => {
      const node = document.querySelector('#nocaptcha');
      if (node) node.setAttribute('data-still-blocked', '1');
    });
  </script>""",
    )


def _product_html() -> str:
    return f"""<!doctype html>
<html>
<head><meta charset="utf-8" /></head>
<body>
  <h1 class="tb-main-title">Mock 淘宝商品</h1>
  <p>普通商品文案：适合柜门滑块、抽屉滑轨等配件，扫码看安装说明，不应被误判为验证码或人工验证。</p>
  <div class="interface-card faceplate-surface sms-info">普通商品参数区</div>
  <img id="J_ImgBooth" src="//img.alicdn.com/mock-main.jpg" />
  <div class="tb-sku">颜色分类</div>
  <div class="tm-price">19.90</div>
  <script src="{DETAIL_URL}"></script>
  <script src="{DESC_URL}"></script>
</body>
</html>"""


def _access_denied_html() -> str:
    return """<!doctype html>
<html>
<head><meta charset="utf-8" /><title>访问被拒绝</title></head>
<body>
  <h1>淘宝网 Taobao.com</h1>
  <p>
    您的账户近期访问行为存在异常，涉嫌不当获取使用平台商业信息，
    系统将限制该账号的部分访问功能（不影响商家店铺正常经营），
    预计2099-05-30 11时后恢复正常。
  </p>
  <a>点我反馈</a>
</body>
</html>"""


def _manual_verification_html() -> str:
    return """<!doctype html>
<html>
<head><meta charset="utf-8" /><title>淘宝安全验证</title></head>
<body>
  <h1>淘宝安全验证</h1>
  <p>当前账号需要进行人脸验证，请使用手机淘宝扫码后拍摄脸部完成验证。</p>
  <iframe
    id="alibaba-login-box"
    src="https://passport.taobao.com/mini_login.htm?mock=face"
    style="width: 360px; height: 240px; border: 0;"
  ></iframe>
</body>
</html>"""


def _login_confirm_html() -> str:
    return """<!doctype html>
<html>
<head><meta charset="utf-8" /></head>
<body>
  <main>
    <h2>手机扫码登录</h2>
    <section>
      <h2>确认登录</h2>
      <div>我**豪</div>
      <button id="quick-login" type="button">快速进入</button>
    </section>
  </main>
  <script>
    document.querySelector('#quick-login').addEventListener('click', () => {
      document.cookie = 'unb=mock_login_user; path=/; SameSite=None; Secure';
      location.href = 'https://item.taobao.com/item.htm?id=mock-collect';
    });
  </script>
</body>
</html>"""


def _new_tmall_dom_html() -> str:
    return """<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <title>女童裙子夏款2026新款洋气童装女孩夏季网纱公主裙儿童夏天连衣裙-tmall.com天猫</title>
  <style>
    img.product { width: 320px; height: 320px; display: block; }
  </style>
</head>
<body>
  <a class="shopName" href="https://shop.example.test/">神奇童年旗舰店</a>
  <h1 class="ItemTitle--mainTitle">女童裙子夏款2026新款洋气童装女孩夏季网纱公主裙儿童夏天连衣裙</h1>
  <div class="Price--priceText">平台加补后 ￥ 87.71 起</div>
  <img class="product" src="//img.alicdn.com/imgextra/i3/2200571728892/O1CN01AZn06T2FYarCmauzK_!!4611686018427384828-0-item_pic.jpg_q50.jpg_.webp" />
  <img class="product" src="//img.alicdn.com/imgextra/i3/2200571728892/O1CN01AZn06T2FYarCmauzK_!!4611686018427384828-0-item_pic.jpg_.webp" />
  <img class="product" src="//img.alicdn.com/imgextra/i4/2200571728892/O1CN012LjZAT2FYarDaO2IG_!!2200571728892.jpg_q50.jpg_.webp" />
  <section class="sku-panel">
    <span>颜色分类</span>
    <button><img src="//img.alicdn.com/O1CN01XM0J4z2FYawQIZ6rZ_!!2200571728892.jpg_.webp" />米白色</button>
    <button><img src="//img.alicdn.com/O1CN01WGzPDo2FYawydUBxd_!!2200571728892.jpg_.webp" />绿色</button>
    <span>身高</span>
    <button>110cm</button>
    <button>120cm</button>
    <span>数量</span>
    <span>有货</span>
  </section>
  <section class="params">
    <span>植物花卉</span><span>图案</span>
    <span>薄</span><span>厚薄</span>
    <span>连衣裙</span><span>裙型</span>
    <span>品牌</span><span>神奇童年</span>
    <span>货号</span><span>Q13209</span>
    <span>风格</span><span>清新</span>
    <span>材质成分</span><span>其他材质100%</span>
    <span>适用季节</span><span>夏季</span>
    <span>安全等级</span><span>B类</span>
  </section>
  <section class="detail-content">
    <h2>图文详情</h2>
    <img class="detail" src="//img.alicdn.com/imgextra/i1/2200571728892/O1CN01DETAIL2FYarCmauzK_!!2200571728892.jpg_790x10000q75.jpg_.webp" />
  </section>
  <section class="rate-list">
    <h2>用户评价</h2>
    <img class="rate" src="//gw.alicdn.com/bao/uploaded/i2/O1CN01QdzD7x1LJphyJh4fL_!!4611686018427382815-0-rate.jpg_960x960.jpg_.webp" />
  </section>
</body>
</html>"""


def _detail_jsonp() -> str:
    payload = {
        "data": {
            "item": {
                "title": "Mock 淘宝商品",
                "images": [
                    "//img.alicdn.com/mock-main-1.jpg",
                    "https://img.alicdn.com/mock-main-2.jpg",
                ],
            },
            "sku": {
                "valItemInfo": {
                    "props": [
                        {
                            "name": "颜色",
                            "values": [
                                {"valueId": "11", "name": "红色"},
                                {"valueId": "12", "name": "蓝色"},
                            ],
                        },
                        {
                            "name": "尺码",
                            "values": [
                                {"valueId": "21", "name": "M"},
                            ],
                        },
                    ],
                    "skuList": [
                        {"skuId": "sku-red-m", "pPath": "1:11;2:21"},
                        {"skuId": "sku-blue-m", "pPath": "1:12;2:21"},
                    ],
                }
            },
            "skuCore": {
                "sku2info": {
                    "sku-red-m": {
                        "price": {"priceMoney": 1990},
                        "quantity": 8,
                    },
                    "sku-blue-m": {
                        "promotion": {"priceMoney": 1890},
                        "quantity": 6,
                    },
                }
            },
        }
    }
    return "mtopjsonp1(" + json.dumps(payload, ensure_ascii=False) + ");"


def _desc_jsonp() -> str:
    payload = {
        "data": {
            "desc": "<p><img src='//img.alicdn.com/mock-detail-1.jpg'></p>"
        }
    }
    return "mtopjsonp2(" + json.dumps(payload, ensure_ascii=False) + ");"


def _has_x5sec(request: Request) -> bool:
    cookie = request.headers.get("cookie", "")
    return "x5sec=mock_x5sec_collect" in cookie


def _route_source(route: Route, state: Dict[str, int]) -> None:
    state["source_hits"] += 1
    route.fulfill(
        status=200,
        content_type="text/html; charset=utf-8",
        body=_product_html() if _has_x5sec(route.request) or state["source_hits"] > 1 else _slider_html(),
    )


def _route_detail(route: Route) -> None:
    route.fulfill(
        status=200,
        content_type="application/javascript; charset=utf-8",
        body=_detail_jsonp(),
    )


def _route_desc(route: Route) -> None:
    route.fulfill(
        status=200,
        content_type="application/javascript; charset=utf-8",
        body=_desc_jsonp(),
    )


def _route_fake_pages(page: Page) -> None:
    state = {"source_hits": 0}
    page.route(SOURCE_URL, lambda route: _route_source(route, state))
    page.route("**/mtop.taobao.detail.getdetail/**", _route_detail)
    page.route("**/mtop.taobao.detail.getdesc/**", _route_desc)


def _route_access_denied_page(page: Page) -> None:
    page.route(
        SOURCE_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_access_denied_html(),
        ),
    )


def _route_stuck_slider_page(page: Page) -> None:
    page.route(
        SOURCE_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_stuck_slider_html(),
        ),
    )


def _route_manual_verification_page(page: Page) -> None:
    page.route(
        SOURCE_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_manual_verification_html(),
        ),
    )


def _route_login_confirm_page(page: Page) -> None:
    page.route(
        "https://login.taobao.com/havanaone/login/login.htm**",
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_login_confirm_html(),
        ),
    )
    page.route(
        "https://item.taobao.com/item.htm**",
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_product_html(),
        ),
    )
    page.route("**/mtop.taobao.detail.getdetail/**", _route_detail)
    page.route("**/mtop.taobao.detail.getdesc/**", _route_desc)


def _read_captcha_history(profile_dir: str):
    path = os.path.join(profile_dir, CAPTCHA_HISTORY_FILE)
    records = []
    if not os.path.exists(path):
        return records, ""
    with open(path, "r", encoding="utf-8") as f:
        raw = f.read()
    for line in raw.splitlines():
        if line.strip():
            records.append(json.loads(line))
    return records, raw


def _run_collect_flow(headed: bool) -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        context = None
        try:
            context, page = _launch_cloakbrowser_once(profile_dir, headless=not headed)
            _route_fake_pages(page)

            page.goto(SOURCE_URL, wait_until="domcontentloaded", timeout=10_000)
            _handle_possible_captcha(context, page, SOURCE_URL, "mock 首次打开商品页")
            captured = _capture_product_data(
                page,
                timeout_sec=8,
                trigger=lambda: _trigger_product_requests(page, SOURCE_URL),
            )
            captcha_false_positive = is_taobao_captcha_page(page)
            parsed = _parse_mtop_response(captured, SOURCE_URL)
            cookies = {cookie["name"]: cookie["value"] for cookie in context.cookies()}
        finally:
            close_browser_context(context)

        history_records, history_raw = _read_captcha_history(profile_dir)

    checks: Dict[str, bool] = {
        "has_x5sec": cookies.get("x5sec") == "mock_x5sec_collect",
        "captured_detail": bool(captured.get("detail")),
        "captured_desc": bool(captured.get("desc")),
        "no_false_positive": not captcha_false_positive,
        "parsed_title": parsed.get("title") == "Mock 淘宝商品",
        "parsed_skus": len(parsed.get("skus") or []) == 2,
        "parsed_detail_image": parsed.get("detail_images") == [
            "https://img.alicdn.com/mock-detail-1.jpg"
        ],
        "history_recorded": any(record.get("success") and record.get("strategy") for record in history_records),
        "history_has_shanghai_time": any("+08:00" in str(record.get("created_at")) for record in history_records),
        "history_no_cookie_value": "mock_x5sec_collect" not in history_raw,
    }
    return {"checks": checks, "history": history_records, "parsed": parsed}


def _run_dom_fallback_flow(headed: bool) -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        context = None
        try:
            context, page = _launch_cloakbrowser_once(profile_dir, headless=not headed)
            page.route(
                DOM_FALLBACK_URL,
                lambda route: route.fulfill(
                    status=200,
                    content_type="text/html; charset=utf-8",
                    body=_new_tmall_dom_html(),
                ),
            )
            page.goto(DOM_FALLBACK_URL, wait_until="domcontentloaded", timeout=10_000)
            parsed = _extract_product_from_dom(page, DOM_FALLBACK_URL)
        finally:
            close_browser_context(context)

    container_product = _parse_mtop_response(
        {
            "detail": None,
            "desc": None,
            "container": [{
                "data": {
                    "itemTitle": "女童裙子夏款2026新款洋气童装女孩夏季网纱公主裙儿童夏天连衣裙",
                    "picUrl": "//img.alicdn.com/O1CN01AZn06T2FYarCmauzK_!!4611686018427384828-0-item_pic.jpg_q50.jpg_.webp",
                }
            }],
        },
        DOM_FALLBACK_URL,
    )
    merged = _merge_product_with_dom(container_product, parsed)

    checks = {
        "dom_title": parsed.get("title") == "女童裙子夏款2026新款洋气童装女孩夏季网纱公主裙儿童夏天连衣裙",
        "dom_images": len(parsed.get("images") or []) >= 2,
        "dom_images_no_review": not any("rate" in image for image in parsed.get("images") or []),
        "dom_images_deduped": len(parsed.get("images") or []) == len(set(parsed.get("images") or [])),
        "dom_images_normalized": not any("_q50" in image or "_.webp" in image for image in parsed.get("images") or []),
        "dom_detail_images": parsed.get("detail_images") == [
            "https://img.alicdn.com/imgextra/i1/2200571728892/O1CN01DETAIL2FYarCmauzK_!!2200571728892.jpg"
        ],
        "dom_sku_options": parsed.get("metadata", {}).get("taobao_sku_options", {}).get("颜色分类") == ["米白色", "绿色"],
        "dom_sku_cross_product": len(parsed.get("skus") or []) == 4,
        "dom_param_brand": parsed.get("brand_hint") == "神奇童年",
        "dom_param_item_no": parsed.get("metadata", {}).get("taobao_item_params", {}).get("货号") == "Q13209",
        "dom_param_material": parsed.get("metadata", {}).get("taobao_item_params", {}).get("材质成分") == "其他材质100%",
        "dom_category_hint": parsed.get("category_hint") == "童装/女童连衣裙",
        "dom_price": (parsed.get("skus") or [{}])[0].get("cost_price") == 87.71,
        "container_needs_dom_enrichment": _needs_dom_enrichment(container_product),
        "container_merged_skus": len(merged.get("skus") or []) == 4,
        "container_merged_brand": merged.get("brand_hint") == "神奇童年",
    }
    return {"checks": checks, "parsed": parsed, "container_product": container_product, "merged": merged}


def _run_access_denied_flow(headed: bool) -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        context = None
        try:
            context, page = _launch_cloakbrowser_once(profile_dir, headless=not headed)
            _route_access_denied_page(page)

            page.goto(SOURCE_URL, wait_until="domcontentloaded", timeout=10_000)
            reason = get_taobao_access_limited_reason(page)
            captcha_detected = is_taobao_captcha_page(page)
            error_message = ""
            try:
                _handle_possible_captcha(context, page, SOURCE_URL, "mock 访问限制")
            except RuntimeError as exc:
                error_message = str(exc)
            cookies = {cookie["name"]: cookie["value"] for cookie in context.cookies()}
        finally:
            close_browser_context(context)

        cooldown = _read_access_limit_cooldown(profile_dir) or {}
        preflight_error = ""
        try:
            _check_access_limit_cooldown(profile_dir)
        except RuntimeError as exc:
            preflight_error = str(exc)
        _clear_access_limit_cooldown(profile_dir)
        cooldown_after_clear = _read_access_limit_cooldown(profile_dir)

    checks: Dict[str, bool] = {
        "access_limited_detected": bool(reason),
        "not_captcha": not captcha_detected,
        "hard_stop_error": "淘宝拒绝访问" in error_message,
        "no_x5sec_written": "x5sec" not in cookies,
        "cooldown_recorded": cooldown.get("resume_at") == "2099-05-30 11:00:00+08:00",
        "preflight_blocks_without_browser": "本次未打开浏览器访问淘宝" in preflight_error,
        "cooldown_clearable": cooldown_after_clear is None,
    }
    return {
        "checks": checks,
        "reason": reason,
        "error": error_message,
        "cooldown": cooldown,
        "preflight_error": preflight_error,
    }


def _run_captcha_failure_cooldown_flow(headed: bool) -> Dict[str, object]:
    old_retries = os.environ.get("WX_XD_TAOBAO_CAPTCHA_MAX_RETRIES")
    old_rounds = os.environ.get("WX_XD_TAOBAO_CAPTCHA_MAX_ROUNDS")
    os.environ["WX_XD_TAOBAO_CAPTCHA_MAX_RETRIES"] = "1"
    os.environ["WX_XD_TAOBAO_CAPTCHA_MAX_ROUNDS"] = "1"
    try:
        with tempfile.TemporaryDirectory() as profile_dir:
            context = None
            try:
                context, page = _launch_cloakbrowser_once(profile_dir, headless=not headed)
                _route_stuck_slider_page(page)

                page.goto(SOURCE_URL, wait_until="domcontentloaded", timeout=10_000)
                error_message = ""
                try:
                    _handle_possible_captcha(context, page, SOURCE_URL, "mock 验证失败")
                except RuntimeError as exc:
                    error_message = str(exc)
                cookies = {cookie["name"]: cookie["value"] for cookie in context.cookies()}
            finally:
                close_browser_context(context)

            cooldown = _read_captcha_failure_cooldown(profile_dir) or {}
            preflight_error = ""
            try:
                _check_captcha_failure_cooldown(profile_dir)
            except RuntimeError as exc:
                preflight_error = str(exc)
            _clear_captcha_failure_cooldown(profile_dir)
            cooldown_after_clear = _read_captcha_failure_cooldown(profile_dir)
    finally:
        if old_retries is None:
            os.environ.pop("WX_XD_TAOBAO_CAPTCHA_MAX_RETRIES", None)
        else:
            os.environ["WX_XD_TAOBAO_CAPTCHA_MAX_RETRIES"] = old_retries
        if old_rounds is None:
            os.environ.pop("WX_XD_TAOBAO_CAPTCHA_MAX_ROUNDS", None)
        else:
            os.environ["WX_XD_TAOBAO_CAPTCHA_MAX_ROUNDS"] = old_rounds

    checks: Dict[str, bool] = {
        "failure_error_returned": "自动处理失败" in error_message,
        "no_x5sec_written": "x5sec" not in cookies,
        "failure_cooldown_recorded": cooldown.get("resume_at", "").endswith("+08:00"),
        "preflight_blocks_without_browser": "本次未打开浏览器访问淘宝" in preflight_error,
        "failure_cooldown_clearable": cooldown_after_clear is None,
    }
    return {
        "checks": checks,
        "error": error_message,
        "cooldown": cooldown,
        "preflight_error": preflight_error,
    }


def _run_manual_verification_flow(headed: bool) -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        context = None
        try:
            context, page = _launch_cloakbrowser_once(profile_dir, headless=not headed)
            _route_manual_verification_page(page)

            page.goto(SOURCE_URL, wait_until="domcontentloaded", timeout=10_000)
            manual_reason = get_taobao_manual_verification_reason(page)
            captcha_detected = is_taobao_captcha_page(page)
            error_message = ""
            try:
                _handle_possible_captcha(context, page, SOURCE_URL, "mock 人工验证")
            except RuntimeError as exc:
                error_message = str(exc)
            cookies = {cookie["name"]: cookie["value"] for cookie in context.cookies()}
        finally:
            close_browser_context(context)

        cooldown = _read_captcha_failure_cooldown(profile_dir) or {}
        preflight_error = ""
        try:
            _check_captcha_failure_cooldown(profile_dir)
        except RuntimeError as exc:
            preflight_error = str(exc)
        _clear_captcha_failure_cooldown(profile_dir)
        cooldown_after_clear = _read_captcha_failure_cooldown(profile_dir)

    checks: Dict[str, bool] = {
        "manual_verification_detected": "人脸验证" in (manual_reason or ""),
        "reported_as_blocked": captcha_detected,
        "manual_error_returned": "人工完成验证" in error_message,
        "no_x5sec_written": "x5sec" not in cookies,
        "manual_cooldown_recorded": "人脸验证" in str(cooldown.get("reason", "")),
        "preflight_blocks_without_browser": "本次未打开浏览器访问淘宝" in preflight_error,
        "manual_cooldown_clearable": cooldown_after_clear is None,
    }
    return {
        "checks": checks,
        "reason": manual_reason,
        "error": error_message,
        "cooldown": cooldown,
        "preflight_error": preflight_error,
    }


def _run_login_confirm_flow(headed: bool) -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        context = None
        try:
            context, page = _launch_cloakbrowser_once(profile_dir, headless=not headed)
            _route_login_confirm_page(page)
            page.goto(LOGIN_CONFIRM_URL, wait_until="domcontentloaded", timeout=10_000)
            handled = _handle_login_confirm(page)
            cookies = {cookie["name"]: cookie["value"] for cookie in context.cookies()}
            body_text = page.evaluate("() => document.body?.innerText || ''")
            current_url = page.url
        finally:
            close_browser_context(context)

    checks = {
        "login_confirm_handled": handled is True,
        "quick_login_cookie_written": cookies.get("unb") == "mock_login_user",
        "redirected_to_product": "item.taobao.com/item.htm?id=mock-collect" in current_url,
        "confirm_text_gone": "快速进入" not in body_text,
    }
    return {
        "checks": checks,
        "url": current_url,
        "cookies": sorted(cookies.keys()),
    }


def _run_profile_lock_flow() -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        lock_error = ""
        with _profile_lock(profile_dir, timeout_sec=1):
            try:
                with _profile_lock(profile_dir, timeout_sec=0.1):
                    pass
            except RuntimeError as exc:
                lock_error = str(exc)

        reacquired = False
        with _profile_lock(profile_dir, timeout_sec=1):
            reacquired = True

    checks = {
        "nested_lock_rejected": "正在被另一个采集/登录进程使用" in lock_error,
        "lock_released_and_reacquired": reacquired,
    }
    return {"checks": checks, "error": lock_error}


def _run_browser_slot_lock_flow() -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as base_dir:
        profile_dir = os.path.join(base_dir, "profile")
        os.makedirs(profile_dir, exist_ok=True)
        lock_error = ""
        with _browser_slot_lock(profile_dir, timeout_sec=1):
            try:
                with _browser_slot_lock(profile_dir, timeout_sec=0.1):
                    pass
            except RuntimeError as exc:
                lock_error = str(exc)

        reacquired = False
        with _browser_slot_lock(profile_dir, timeout_sec=1):
            reacquired = True

        cross_profile_error = ""
        explicit_root = os.path.join(base_dir, "slot-root")
        previous_root = os.environ.get("WX_XD_TAOBAO_BROWSER_SLOT_ROOT")
        os.environ["WX_XD_TAOBAO_BROWSER_SLOT_ROOT"] = explicit_root
        try:
            profile_a = os.path.join(base_dir, "a", "profile")
            profile_b = os.path.join(base_dir, "b", "profile")
            os.makedirs(profile_a, exist_ok=True)
            os.makedirs(profile_b, exist_ok=True)
            with _browser_slot_lock(profile_a, timeout_sec=1):
                try:
                    with _browser_slot_lock(profile_b, timeout_sec=0.1):
                        pass
                except RuntimeError as exc:
                    cross_profile_error = str(exc)
        finally:
            if previous_root is None:
                os.environ.pop("WX_XD_TAOBAO_BROWSER_SLOT_ROOT", None)
            else:
                os.environ["WX_XD_TAOBAO_BROWSER_SLOT_ROOT"] = previous_root

    checks = {
        "nested_slot_rejected": "浏览器槽位已满" in lock_error,
        "slot_released_and_reacquired": reacquired,
        "explicit_root_cross_profile_rejected": "浏览器槽位已满" in cross_profile_error,
    }
    return {
        "checks": checks,
        "error": lock_error,
        "cross_profile_error": cross_profile_error,
    }


def _run_login_preflight_flow() -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        _record_access_limit_cooldown(
            profile_dir,
            "淘宝拒绝访问：账号近期访问行为异常，平台已限制部分访问功能。平台提示预计2099-05-30 11时后恢复正常。",
            "mock 登录预检",
        )
        script_path = os.path.join(os.path.dirname(__file__), "taobao_collector.py")
        proc = subprocess.run(
            [
                sys.executable,
                "-B",
                script_path,
                "login",
                "--profile-dir",
                profile_dir,
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )

    try:
        payload = json.loads(proc.stdout.strip() or "{}")
    except json.JSONDecodeError:
        payload = {}
    error = str(payload.get("error") or "")
    checks = {
        "login_preflight_exits_nonzero": proc.returncode != 0,
        "login_preflight_json_error": "本次未打开浏览器访问淘宝" in error,
    }
    return {
        "checks": checks,
        "returncode": proc.returncode,
        "stdout": proc.stdout.strip(),
        "stderr": proc.stderr.strip(),
    }


def _run_check_login_preflight_flow() -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        _record_access_limit_cooldown(
            profile_dir,
            "淘宝拒绝访问：账号近期访问行为异常，平台已限制部分访问功能。平台提示预计2099-05-30 11时后恢复正常。",
            "mock 登录态检测预检",
        )
        script_path = os.path.join(os.path.dirname(__file__), "taobao_collector.py")
        proc = subprocess.run(
            [
                sys.executable,
                "-B",
                script_path,
                "check-login",
                "--profile-dir",
                profile_dir,
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )

    try:
        payload = json.loads(proc.stdout.strip() or "{}")
    except json.JSONDecodeError:
        payload = {}
    stderr = proc.stderr.strip()
    checks = {
        "check_login_preflight_ok": proc.returncode == 0,
        "check_login_reports_cooldown": payload.get("access_limited") is True,
        "check_login_no_browser_start": "正在安装必要依赖 cloakbrowser" not in stderr,
    }
    return {
        "checks": checks,
        "returncode": proc.returncode,
        "stdout": proc.stdout.strip(),
        "stderr": stderr,
    }


def _run_strategy_history_flow() -> Dict[str, object]:
    with tempfile.TemporaryDirectory() as profile_dir:
        path = os.path.join(profile_dir, CAPTCHA_HISTORY_FILE)
        records = [
            {
                "created_at": "2026-05-27 12:00:00+08:00",
                "attempt": 1,
                "strategy": "legacy",
                "success": True,
                "duration_sec": 2.4,
            },
            {
                "created_at": "2026-05-27 12:01:00+08:00",
                "attempt": 1,
                "strategy": "smooth",
                "success": True,
                "duration_sec": 1.8,
            },
            {
                "created_at": "2026-05-27 12:02:00+08:00",
                "attempt": 2,
                "strategy": "smooth",
                "success": True,
                "duration_sec": 1.6,
            },
            {
                "created_at": "2026-05-27 12:03:00+08:00",
                "attempt": 3,
                "strategy": "two_phase",
                "success": False,
                "duration_sec": 3.0,
            },
        ]
        with open(path, "w", encoding="utf-8") as f:
            for record in records:
                f.write(json.dumps(record, ensure_ascii=False) + "\n")

        order = CaptchaAttemptHistory(path).strategy_order()
        raw = open(path, "r", encoding="utf-8").read()

    checks = {
        "history_prefers_most_successful_strategy": order[0] == "smooth",
        "history_keeps_all_strategies": set(order) == {"legacy", "smooth", "two_phase"},
        "history_fixture_has_no_cookie_value": "x5sec" not in raw,
    }
    return {"checks": checks, "strategy_order": order}


class _FakeMtopResponse:
    def __init__(self, url: str, payload: Dict[str, object]):
        self.url = url
        self._payload = payload

    def json(self) -> Dict[str, object]:
        return self._payload


class _FakeMtopPage:
    def __init__(self):
        self.url = SOURCE_URL
        self._listeners: Dict[str, list] = {}

    def on(self, event: str, callback):
        self._listeners.setdefault(event, []).append(callback)

    def remove_listener(self, event: str, callback):
        callbacks = self._listeners.get(event, [])
        if callback in callbacks:
            callbacks.remove(callback)

    def emit_response(self, response: _FakeMtopResponse):
        for callback in list(self._listeners.get("response", [])):
            callback(response)


def _run_capture_detail_grace_flow() -> Dict[str, object]:
    old_grace = os.environ.get("WX_XD_TAOBAO_CAPTURE_DETAIL_GRACE_SEC")
    old_poll = os.environ.get("WX_XD_TAOBAO_CAPTURE_POLL_INTERVAL_SEC")
    os.environ["WX_XD_TAOBAO_CAPTURE_DETAIL_GRACE_SEC"] = "0.2"
    os.environ["WX_XD_TAOBAO_CAPTURE_POLL_INTERVAL_SEC"] = "0.05"
    page = _FakeMtopPage()
    payload = {
        "data": {
            "item": {
                "title": "Mock 快速返回商品",
                "images": ["//img.alicdn.com/mock-fast.jpg"],
            },
            "sku": {"valItemInfo": {"skuList": []}},
            "skuCore": {"sku2info": {}},
        }
    }
    started_at = time.monotonic()
    try:
        captured = _capture_product_data(
            page,
            timeout_sec=5,
            trigger=lambda: page.emit_response(_FakeMtopResponse(DETAIL_URL, payload)),
        )
    finally:
        if old_grace is None:
            os.environ.pop("WX_XD_TAOBAO_CAPTURE_DETAIL_GRACE_SEC", None)
        else:
            os.environ["WX_XD_TAOBAO_CAPTURE_DETAIL_GRACE_SEC"] = old_grace
        if old_poll is None:
            os.environ.pop("WX_XD_TAOBAO_CAPTURE_POLL_INTERVAL_SEC", None)
        else:
            os.environ["WX_XD_TAOBAO_CAPTURE_POLL_INTERVAL_SEC"] = old_poll
    elapsed_sec = time.monotonic() - started_at

    checks = {
        "capture_detail_without_desc": bool(captured.get("detail")),
        "capture_missing_desc_allowed": captured.get("desc") is None,
        "capture_detail_grace_fast_exit": elapsed_sec < 2.0,
        "capture_listeners_removed": all(not callbacks for callbacks in page._listeners.values()),
    }
    return {"checks": checks, "elapsed_sec": elapsed_sec}


def main() -> int:
    parser = argparse.ArgumentParser(description="验证本地淘宝采集 mock 链路")
    parser.add_argument("--headed", action="store_true", help="使用可见浏览器")
    args = parser.parse_args()

    collect_result = _run_collect_flow(args.headed)
    dom_fallback_result = _run_dom_fallback_flow(args.headed)
    access_denied_result = _run_access_denied_flow(args.headed)
    captcha_failure_result = _run_captcha_failure_cooldown_flow(args.headed)
    manual_verification_result = _run_manual_verification_flow(args.headed)
    login_confirm_result = _run_login_confirm_flow(args.headed)
    profile_lock_result = _run_profile_lock_flow()
    browser_slot_lock_result = _run_browser_slot_lock_flow()
    login_preflight_result = _run_login_preflight_flow()
    check_login_preflight_result = _run_check_login_preflight_flow()
    strategy_history_result = _run_strategy_history_flow()
    capture_detail_grace_result = _run_capture_detail_grace_flow()
    result = {
        "collect": collect_result,
        "dom_fallback": dom_fallback_result,
        "access_denied": access_denied_result,
        "captcha_failure": captcha_failure_result,
        "manual_verification": manual_verification_result,
        "login_confirm": login_confirm_result,
        "profile_lock": profile_lock_result,
        "browser_slot_lock": browser_slot_lock_result,
        "login_preflight": login_preflight_result,
        "check_login_preflight": check_login_preflight_result,
        "strategy_history": strategy_history_result,
        "capture_detail_grace": capture_detail_grace_result,
    }

    all_checks = [
        *collect_result["checks"].values(),
        *dom_fallback_result["checks"].values(),
        *access_denied_result["checks"].values(),
        *captcha_failure_result["checks"].values(),
        *manual_verification_result["checks"].values(),
        *login_confirm_result["checks"].values(),
        *profile_lock_result["checks"].values(),
        *browser_slot_lock_result["checks"].values(),
        *login_preflight_result["checks"].values(),
        *check_login_preflight_result["checks"].values(),
        *strategy_history_result["checks"].values(),
        *capture_detail_grace_result["checks"].values(),
    ]
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if all(all_checks) else 1


if __name__ == "__main__":
    raise SystemExit(main())
