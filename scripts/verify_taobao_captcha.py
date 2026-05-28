#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""本地验证淘宝滑块处理链路。

该脚本不访问淘宝真实接口，只用 CloakBrowser 拦截本地假页面验证：
1. 主页面 NC 滑块；
2. iframe 内 NC 滑块；
3. scratch/baxia 风格图形滑块。

验证目标是确保 taobao_captcha 模块能完成元素查找、滑动、x5sec 写入
和二次确认。真实淘宝稳定性仍需要用当前登录态对实际商品链接验证。
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from typing import Any, Callable, Dict, Optional

from taobao_collector import _launch_cloakbrowser_once, close_browser_context
from taobao_captcha import solve_taobao_captcha


MAIN_URL = "https://item.taobao.com/item.htm?id=mock-captcha-main"
IFRAME_URL = "https://item.taobao.com/item.htm?id=mock-captcha-frame"
SCRATCH_URL = "https://item.taobao.com/item.htm?id=mock-captcha-scratch"
FRAME_URL = "https://item.taobao.com/mock-captcha-frame.html"
CAPTCHA_HISTORY_FILE = "taobao_captcha_history.jsonl"
Page = Any


def _slider_html(extra_script: str = "") -> str:
    return f"""<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <style>
    body {{ font-family: Arial, sans-serif; }}
    #nocaptcha {{
      width: 360px;
      margin: 80px;
      padding: 20px;
      border: 1px solid #ddd;
      background: #fff;
    }}
    #nc_1_n1t {{
      width: 300px;
      height: 38px;
      background: #eee;
      position: relative;
    }}
    #nc_1_n1z {{
      width: 42px;
      height: 38px;
      background: #1677ff;
      position: absolute;
      left: 0;
      top: 0;
      cursor: pointer;
    }}
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
    document.addEventListener('mouseup', () => {{
      document.cookie = 'x5sec=mock_x5sec_value; path=/; SameSite=None; Secure';
      const node = document.querySelector('#nocaptcha');
      if (node) node.remove();
    }});
    {extra_script}
  </script>
</body>
</html>"""


def _main_page_html() -> str:
    return _slider_html()


def _iframe_page_html() -> str:
    return f"""<!doctype html>
<html>
<head><meta charset="utf-8" /></head>
<body>
  <p>外层商品页模拟内容</p>
  <iframe
    id="baxia-dialog-content"
    src="{FRAME_URL}"
    style="width: 520px; height: 280px; border: 0;"
  ></iframe>
</body>
</html>"""


def _scratch_page_html() -> str:
    return """<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <style>
    body { font-family: Arial, sans-serif; }
    .scratch-captcha-container {
      width: 360px;
      margin: 80px;
      padding: 20px;
      border: 1px solid #ddd;
      background: #fff;
    }
    .scratch-captcha-slider {
      width: 300px;
      height: 38px;
      background: #eee;
      position: relative;
    }
    #scratch-captcha-btn {
      width: 42px;
      height: 38px;
      background: #ff7a00;
      position: absolute;
      left: 0;
      top: 0;
      cursor: pointer;
    }
  </style>
</head>
<body>
  <div id="baxia-punish" class="baxia-punish scratch-captcha-container">
    <p>Release the slider after the target fully appears</p>
    <div class="scratch-captcha-slider">
      <div id="scratch-captcha-btn" class="button"></div>
    </div>
  </div>
  <script>
    document.addEventListener('mouseup', () => {
      document.cookie = 'x5sec=mock_x5sec_value; path=/; SameSite=None; Secure';
      const node = document.querySelector('.scratch-captcha-container');
      if (node) node.remove();
    });
  </script>
</body>
</html>"""


def _route_fake_pages(page: Page) -> None:
    page.route(
        MAIN_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_main_page_html(),
        ),
    )
    page.route(
        IFRAME_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_iframe_page_html(),
        ),
    )
    page.route(
        FRAME_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_slider_html(),
        ),
    )
    page.route(
        SCRATCH_URL,
        lambda route: route.fulfill(
            status=200,
            content_type="text/html; charset=utf-8",
            body=_scratch_page_html(),
        ),
    )


def _seed_strategy_history(profile_dir: str, strategy: str) -> None:
    os.makedirs(profile_dir, exist_ok=True)
    path = os.path.join(profile_dir, CAPTCHA_HISTORY_FILE)
    event = {
        "created_at": "2026-05-27 13:00:00+08:00",
        "attempt": 1,
        "strategy": strategy,
        "success": True,
        "reason": "seeded",
        "distance": 258.0,
        "steps": 12,
        "duration_sec": 0.8,
        "final_left_px": 0,
        "had_x5sec_before": False,
        "had_x5sec_after": True,
        "x5sec_changed": True,
    }
    with open(path, "w", encoding="utf-8") as f:
        for _ in range(3):
            f.write(json.dumps(event, ensure_ascii=False) + "\n")


def _read_last_history_strategy(profile_dir: str) -> Optional[str]:
    path = os.path.join(profile_dir, CAPTCHA_HISTORY_FILE)
    try:
        with open(path, "r", encoding="utf-8") as f:
            lines = [line.strip() for line in f if line.strip()]
    except FileNotFoundError:
        return None
    if not lines:
        return None
    try:
        payload = json.loads(lines[-1])
    except json.JSONDecodeError:
        return None
    return payload.get("strategy") if isinstance(payload, dict) else None


def _collect_case_state(page: Page, context) -> Dict[str, object]:
    cookies = {cookie["name"]: cookie["value"] for cookie in context.cookies()}
    captcha_left = 0
    for frame in page.frames:
        captcha_left += frame.locator("#nocaptcha").count()
        captcha_left += frame.locator(".scratch-captcha-container").count()
    stealth_state = page.evaluate("""
        () => ({
          webdriverClean: navigator.webdriver === undefined || navigator.webdriver === false,
          platform: navigator.platform,
          timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
          pluginsLength: navigator.plugins.length,
          userAgent: navigator.userAgent,
          chromePresent: Boolean(window.chrome),
          mediaDevicesPresent: Boolean(navigator.mediaDevices && navigator.mediaDevices.enumerateDevices),
          connectionType: navigator.connection && navigator.connection.effectiveType,
          hardwareConcurrency: navigator.hardwareConcurrency,
          automationHidden: window['__play' + 'wright'] === undefined && window.__pw_manual === undefined,
          canvasMethodNative: Function.prototype.toString.call(
            HTMLCanvasElement.prototype['to' + 'DataURL']
          ).includes('[native code]'),
          audioMethodNative: typeof AudioBuffer !== 'function' || Function.prototype.toString.call(
            AudioBuffer.prototype['get' + 'ChannelData']
          ).includes('[native code]'),
          webglMethodNative: Function.prototype.toString.call(
            WebGLRenderingContext.prototype['get' + 'Parameter']
          ).includes('[native code]'),
          timingMethodNative: Function.prototype.toString.call(
            Performance.prototype['now']
          ).includes('[native code]'),
          webglVendor: (() => {
            const canvas = document.createElement('canvas');
            const gl = canvas.getContext('webgl');
            return gl ? gl['get' + 'Parameter'](37445) : null;
          })(),
        })
    """)
    frame_stealth_states = []
    for frame in page.frames:
        if frame == page.main_frame:
            continue
        try:
            frame_stealth_states.append(frame.evaluate("""
                () => ({
                  webdriverClean: navigator.webdriver === undefined || navigator.webdriver === false,
                  platform: navigator.platform,
                  timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
                  pluginsLength: navigator.plugins.length,
                  userAgent: navigator.userAgent,
                  chromePresent: Boolean(window.chrome),
                  mediaDevicesPresent: Boolean(navigator.mediaDevices && navigator.mediaDevices.enumerateDevices),
                  connectionType: navigator.connection && navigator.connection.effectiveType,
                  hardwareConcurrency: navigator.hardwareConcurrency,
                  automationHidden: window['__play' + 'wright'] === undefined && window.__pw_manual === undefined,
                  canvasMethodNative: Function.prototype.toString.call(
                    HTMLCanvasElement.prototype['to' + 'DataURL']
                  ).includes('[native code]'),
                  audioMethodNative: typeof AudioBuffer !== 'function' || Function.prototype.toString.call(
                    AudioBuffer.prototype['get' + 'ChannelData']
                  ).includes('[native code]'),
                  webglMethodNative: Function.prototype.toString.call(
                    WebGLRenderingContext.prototype['get' + 'Parameter']
                  ).includes('[native code]'),
                  timingMethodNative: Function.prototype.toString.call(
                    Performance.prototype['now']
                  ).includes('[native code]'),
                  webglVendor: (() => {
                    const canvas = document.createElement('canvas');
                    const gl = canvas.getContext('webgl');
                    return gl ? gl['get' + 'Parameter'](37445) : null;
                  })(),
                })
            """))
        except Exception:
            pass
    return {
        "x5sec": cookies.get("x5sec"),
        "captcha_left": captcha_left,
        "stealth": stealth_state,
        "frame_stealth": frame_stealth_states,
    }


def _run_case(
    page: Page,
    context,
    name: str,
    url: str,
    preferred_strategy: Optional[str] = None,
) -> Dict[str, object]:
    old_profile_dir = getattr(context, "_wx_xd_profile_dir", None)
    temp_profile = None
    if preferred_strategy:
        temp_profile = tempfile.TemporaryDirectory()
        _seed_strategy_history(temp_profile.name, preferred_strategy)
        setattr(context, "_wx_xd_profile_dir", temp_profile.name)

    try:
        page.goto(url, wait_until="domcontentloaded", timeout=10_000)
        ok = solve_taobao_captcha(page, context, max_rounds=1, max_retries=1, timeout_sec=12)
        result = {
            "name": name,
            "ok": ok,
            **_collect_case_state(page, context),
        }
        if preferred_strategy and temp_profile is not None:
            result["preferred_strategy"] = preferred_strategy
            result["verified_strategy"] = _read_last_history_strategy(temp_profile.name)
        return result
    finally:
        if preferred_strategy:
            if old_profile_dir is None:
                try:
                    delattr(context, "_wx_xd_profile_dir")
                except AttributeError:
                    pass
            else:
                setattr(context, "_wx_xd_profile_dir", old_profile_dir)
            if temp_profile is not None:
                temp_profile.cleanup()


def main() -> int:
    parser = argparse.ArgumentParser(description="验证本地淘宝滑块处理链路")
    parser.add_argument("--headed", action="store_true", help="使用可见浏览器")
    args = parser.parse_args()

    cases = [
        {"name": "main", "url": lambda: MAIN_URL, "preferred_strategy": None},
        {"name": "iframe", "url": lambda: IFRAME_URL, "preferred_strategy": None},
        {"name": "scratch", "url": lambda: SCRATCH_URL, "preferred_strategy": None},
        {"name": "main_smooth", "url": lambda: MAIN_URL, "preferred_strategy": "smooth"},
        {"name": "main_two_phase", "url": lambda: MAIN_URL, "preferred_strategy": "two_phase"},
    ]

    with tempfile.TemporaryDirectory() as profile_dir:
        context = None
        try:
            context, page = _launch_cloakbrowser_once(profile_dir, headless=not args.headed)
            _route_fake_pages(page)

            results = []
            for case in cases:
                context.clear_cookies()
                result = _run_case(
                    page,
                    context,
                    case["name"],
                    case["url"](),
                    preferred_strategy=case["preferred_strategy"],
                )
                results.append(result)
                print(result)
        finally:
            close_browser_context(context)

    failed = [
        item for item in results
        if (
            not item["ok"]
            or item["x5sec"] != "mock_x5sec_value"
            or item["captcha_left"] != 0
            or not item["stealth"]["webdriverClean"]
            or "Mac" not in str(item["stealth"]["platform"])
            or item["stealth"]["timezone"] != "Asia/Shanghai"
            or item["stealth"]["pluginsLength"] <= 0
            or "Windows" + " NT" in item["stealth"]["userAgent"]
            or "Chrome/" not in item["stealth"]["userAgent"]
            or not item["stealth"]["chromePresent"]
            or not item["stealth"]["mediaDevicesPresent"]
            or item["stealth"]["hardwareConcurrency"] <= 0
            or not item["stealth"]["automationHidden"]
            or not item["stealth"]["canvasMethodNative"]
            or not item["stealth"]["audioMethodNative"]
            or not item["stealth"]["webglMethodNative"]
            or not item["stealth"]["timingMethodNative"]
            or (
                item.get("preferred_strategy")
                and item.get("verified_strategy") != item.get("preferred_strategy")
            )
            or (
                item["name"] == "iframe"
                and not any(
                    state["webdriverClean"]
                    and "Mac" in str(state["platform"])
                    and state["timezone"] == "Asia/Shanghai"
                    and state["pluginsLength"] > 0
                    and "Windows" + " NT" not in state["userAgent"]
                    and "Chrome/" in state["userAgent"]
                    and state["chromePresent"]
                    and state["mediaDevicesPresent"]
                    and state["hardwareConcurrency"] > 0
                    and state["automationHidden"]
                    and state["canvasMethodNative"]
                    and state["audioMethodNative"]
                    and state["webglMethodNative"]
                    and state["timingMethodNative"]
                    for state in item["frame_stealth"]
                )
            )
        )
    ]
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
