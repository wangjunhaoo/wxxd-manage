#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
淘宝商品详情采集脚本
基于 CloakBrowser 持久化上下文 + 网络拦截捕获商品数据
"""

import sys
import os
import hashlib
import json
import argparse
import time
import random
import subprocess
import sqlite3
import re
from datetime import datetime, timedelta, timezone

from taobao_captcha import (
    get_taobao_access_limited_reason,
    get_taobao_manual_verification_reason,
    install_minimal_taobao_stealth,
    is_taobao_captcha_page,
    solve_taobao_captcha,
)

SHANGHAI_TZ = timezone(timedelta(hours=8), name="Asia/Shanghai")
ACCESS_LIMIT_COOLDOWN_FILE = "taobao_access_limited.json"
CAPTCHA_FAILURE_COOLDOWN_FILE = "taobao_captcha_failure_cooldown.json"
ACCESS_LIMIT_COOLDOWN_BYPASS_ENV = "WX_XD_TAOBAO_IGNORE_ACCESS_LIMIT_COOLDOWN"
CAPTCHA_FAILURE_COOLDOWN_BYPASS_ENV = "WX_XD_TAOBAO_IGNORE_CAPTCHA_FAILURE_COOLDOWN"
CAPTCHA_FAILURE_COOLDOWN_MINUTES_ENV = "WX_XD_TAOBAO_CAPTCHA_FAILURE_COOLDOWN_MINUTES"
SKIP_WARMUP_ENV = "WX_XD_TAOBAO_SKIP_WARMUP"
BATCH_DELAY_MIN_ENV = "WX_XD_TAOBAO_BATCH_DELAY_MIN_SEC"
BATCH_DELAY_MAX_ENV = "WX_XD_TAOBAO_BATCH_DELAY_MAX_SEC"
CAPTURE_TIMEOUT_ENV = "WX_XD_TAOBAO_CAPTURE_TIMEOUT_SEC"
CAPTURE_DETAIL_GRACE_ENV = "WX_XD_TAOBAO_CAPTURE_DETAIL_GRACE_SEC"
CAPTURE_POLL_INTERVAL_ENV = "WX_XD_TAOBAO_CAPTURE_POLL_INTERVAL_SEC"
CAPTURE_SHORT_GRACE_ENV = "WX_XD_TAOBAO_CAPTURE_SHORT_GRACE_SEC"
# 批量长停顿节奏（可配，默认更短且概率触发，仍保留真人"歇一会"特征）
LONG_PAUSE_MIN_ENV = "WX_XD_TAOBAO_LONG_PAUSE_MIN_SEC"
LONG_PAUSE_MAX_ENV = "WX_XD_TAOBAO_LONG_PAUSE_MAX_SEC"
LONG_PAUSE_EVERY_MIN_ENV = "WX_XD_TAOBAO_LONG_PAUSE_EVERY_MIN"
LONG_PAUSE_EVERY_MAX_ENV = "WX_XD_TAOBAO_LONG_PAUSE_EVERY_MAX"
LONG_PAUSE_PROBABILITY_ENV = "WX_XD_TAOBAO_LONG_PAUSE_PROBABILITY"
TAOBAO_HOME_WARMUP_URL = "https://www.taobao.com/"
PROFILE_LOCK_FILE = "taobao_collector.lock"
PROFILE_PROBE_COMMAND = "__probe-cloak-profile"
PROFILE_PROBE_TIMEOUT_SEC = 45
PROFILE_LOCK_TIMEOUT_ENV = "WX_XD_TAOBAO_PROFILE_LOCK_TIMEOUT_SECONDS"
TAOBAO_LOGIN_COOKIE_NAMES = {"unb", "tracknick", "_l_g_", "lgc", "_nk_", "cookie17"}
LOGIN_CONFIRM_KEYWORDS = ("快速进入", "确认登录", "安全登录", "一键登录")
LOGIN_CONFIRM_CLICK_TEXTS = ("快速进入", "确认登录", "安全登录", "一键登录")
GLOBAL_BROWSER_SLOT_DIR = "taobao_browser_slots"
GLOBAL_BROWSER_SLOT_MUTEX_DIR = "taobao_browser_slots.mutex"
GLOBAL_BROWSER_SLOT_ROOT_ENV = "WX_XD_TAOBAO_BROWSER_SLOT_ROOT"
GLOBAL_BROWSER_SLOT_LIMIT_ENV = "WX_XD_TAOBAO_BROWSER_SLOT_LIMIT"
GLOBAL_BROWSER_SLOT_TIMEOUT_ENV = "WX_XD_TAOBAO_BROWSER_SLOT_TIMEOUT_SECONDS"
STALE_CHROMIUM_LOCK_CLEAN_ENV = "WX_XD_TAOBAO_CLEAN_STALE_CHROMIUM_LOCKS"


# ======================== 工具函数 ========================

def rand_sleep(min_ms=300, max_ms=1500):
    time.sleep(random.uniform(min_ms, max_ms) / 1000.0)


def human_like_scroll(page, distance):
    """触发滚动信号，节奏交给 CloakBrowser humanize 处理。"""
    if not distance:
        return
    try:
        page.mouse.wheel(0, distance)
    except Exception:
        return
    time.sleep(random.uniform(0.25, 0.65))


def _hover_first_visible(page, selector_group):
    for candidate in (selector_group or "").split(","):
        candidate = candidate.strip()
        if not candidate:
            continue
        try:
            el = page.query_selector(candidate)
            if not el:
                continue
            try:
                if not el.is_visible():
                    continue
            except Exception:
                pass
            el.hover(timeout=2000)
            time.sleep(random.uniform(0.25, 0.75))
            return True
        except Exception:
            continue
    return False


def _random_browse_interaction(page, allow_reload=False):
    """随机化商品页浏览行为，避免固定的交互模式被风控识别。

    每次调用随机选择一种浏览策略，模拟不同用户的浏览习惯：
    - quick_scan (25%): 快速扫一眼，轻滚一下就走
    - normal_browse (45%): 正常逛逛，hover 主图 + 滚 2-3 屏
    - deep_look (30%): 认真看，hover 主图 + hover SKU + 滚详情区

    各策略内部的顺序和时机也会随机抖动。
    """
    roll = random.random()

    if roll < 0.25:
        # quick_scan：几乎不交互，只轻滚一下
        try:
            if random.random() < 0.5:
                time.sleep(random.uniform(0.6, 1.5))
            human_like_scroll(page, random.randint(200, 500))
        except Exception:
            pass
        return True

    if roll < 0.70:
        # normal_browse：hover 主图 + 滚动浏览
        try:
            time.sleep(random.uniform(0.3, 0.9))
            targets = [
                "#J_ImgBooth, .tb-main-pic img, .gallery img, .pic img",
                ".tb-sku, .sku-content, .J_TSaleProp, .tb-prop",
                ".tm-price, .tb-rmb-num, .tb-price",
            ]
            random.shuffle(targets)
            _hover_first_visible(page, targets[0])
            time.sleep(random.uniform(0.3, 0.8))
            human_like_scroll(page, random.randint(300, 700))
            time.sleep(random.uniform(0.4, 1.0))
            if random.random() < 0.5:
                human_like_scroll(page, random.randint(200, 500))
                time.sleep(random.uniform(0.3, 0.7))
            if random.random() < 0.4:
                _hover_first_visible(page, targets[1])
        except Exception:
            pass
        return True

    # deep_look：hover 主图 + hover SKU + 滚动详情区
    try:
        time.sleep(random.uniform(0.5, 1.2))
        targets = [
            "#J_ImgBooth, .tb-main-pic img, .gallery img, .pic img",
            ".tb-sku, .sku-content, .J_TSaleProp, .tb-prop, [class*='sku']",
            ".tm-price, .tb-rmb-num, .tb-price",
            ".ShopHeader, .shop-header, .tb-shop, [class*='shop']",
        ]
        random.shuffle(targets)
        _hover_first_visible(page, targets[0])
        time.sleep(random.uniform(0.35, 0.8))
        human_like_scroll(page, random.randint(400, 800))
        time.sleep(random.uniform(0.45, 0.9))
        _hover_first_visible(page, targets[1])
        time.sleep(random.uniform(0.25, 0.6))

        # 有时 hover 一个未选中的 SKU 项
        if random.random() < 0.5:
            _hover_first_visible(
                page,
                ".tb-sku li:not(.tb-selected), .J_TSaleProp li:not(.tb-selected),"
                " [class*='sku'] [class*='item']:not([class*='selected'])",
            )
            time.sleep(random.uniform(0.2, 0.5))

        human_like_scroll(page, random.randint(300, 700))
        time.sleep(random.uniform(0.3, 0.7))
        if random.random() < 0.35:
            _hover_first_visible(page, targets[2])
        if random.random() < 0.25:
            human_like_scroll(page, -random.randint(100, 250))
    except Exception:
        pass
    return True


def warm_up_taobao_home(page, skip_by_default=False):
    """快速访问淘宝首页建立 session 上下文。

    批量采集默认跳过；如需强制预热，可设置 WX_XD_TAOBAO_SKIP_WARMUP=0。
    """
    skip_env = os.environ.get(SKIP_WARMUP_ENV)
    if skip_env == "1" or (skip_by_default and skip_env is None):
        return False
    try:
        page.goto(TAOBAO_HOME_WARMUP_URL, timeout=30000, wait_until="domcontentloaded")
        _wait_for_page_stable(page, timeout_sec=6)
        time.sleep(random.uniform(1.5, 3.0))
        human_like_scroll(page, random.randint(300, 600))
        return True
    except Exception as exc:
        print(f"淘宝首页预热异常（继续打开商品页）: {exc}", file=sys.stderr)
        return False


def _collect_single_product(ctx, page, url, profile_dir):
    """在已打开的浏览器上下文中采集单个商品页。

    这是 run_collect 和 run_batch_collect 共享的核心逻辑。
    调用方负责浏览器生命周期管理。
    """
    captured = _capture_product_data(
        page,
        trigger=lambda: _navigate_to_product_detail(page, url),
    )

    _raise_if_access_limited(page, "采集响应", profile_dir=profile_dir)

    if _is_blocked(page):
        _handle_possible_captcha(ctx, page, url, "首次采集")
        _raise_if_access_limited(page, "验证码处理后", profile_dir=profile_dir)
        if not captured.get("detail"):
            captured = _capture_product_data(
                page,
                trigger=lambda: _trigger_product_requests(page, url, allow_reload=True),
            )
            _raise_if_access_limited(page, "二次采集", profile_dir=profile_dir)

    if not captured.get("detail"):
        captured = _capture_product_data(
            page,
            trigger=lambda: _trigger_product_requests(page, url, allow_reload=True),
        )
        _raise_if_access_limited(page, "交互触发采集", profile_dir=profile_dir)

    if not captured.get("detail"):
        for var_name in ("__INITIAL_DATA__", "__DATA__", "g_config", "_DATA_"):
            try:
                val = page.evaluate(f"() => window.{var_name}")
                if val and isinstance(val, dict):
                    captured["detail"] = val
                    break
            except Exception:
                pass

    if not captured.get("detail"):
        _raise_if_access_limited(page, "兜底解析", profile_dir=profile_dir)
        if _is_blocked(page):
            _handle_possible_captcha(ctx, page, url, "兜底解析")
            captured = _capture_product_data(
                page,
                trigger=lambda: _trigger_product_requests(page, url, allow_reload=True),
            )
            _raise_if_access_limited(page, "兜底解析后", profile_dir=profile_dir)
        if not captured.get("detail"):
            if _is_blocked(page):
                raise RuntimeError("被淘宝滑块验证码拦截，自动处理后仍未放行。")
            raise RuntimeError("未能捕获商品数据，页面可能改版或需要重新登录。")

    parsed_product = _parse_mtop_response(captured, url)
    if _is_empty_collected_product(parsed_product):
        parsed_product = _extract_product_from_dom(page, url)
    elif _needs_dom_enrichment(parsed_product):
        # mtop 已有详情图时，DOM 只用于补 SKU/参数，避免重复慢滚详情区。
        preload_detail = not bool(parsed_product.get("detail_images"))
        parsed_product = _merge_product_with_dom(
            parsed_product,
            _extract_product_from_dom(page, url, preload_detail=preload_detail),
        )
    return parsed_product


# ======================== CloakBrowser 依赖 ========================

class ProfileLock:
    """同一淘宝 profile 的跨进程互斥锁。"""

    def __init__(self, profile_dir, timeout_sec=120):
        self.profile_dir = profile_dir
        self.timeout_sec = timeout_sec
        self.lock_path = os.path.join(profile_dir, PROFILE_LOCK_FILE)
        self.acquired = False

    def __enter__(self):
        os.makedirs(self.profile_dir, exist_ok=True)
        deadline = time.time() + self.timeout_sec
        while True:
            try:
                fd = os.open(self.lock_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
                payload = {
                    "pid": os.getpid(),
                    "created_at": _format_shanghai_time(datetime.now(SHANGHAI_TZ)),
                    "timezone": "Asia/Shanghai",
                }
                with os.fdopen(fd, "w", encoding="utf-8") as f:
                    json.dump(payload, f, ensure_ascii=False)
                self.acquired = True
                return self
            except FileExistsError:
                if self._remove_stale_lock():
                    continue
                if time.time() >= deadline:
                    raise RuntimeError("淘宝采集浏览器 profile 正在被另一个采集/登录进程使用，请稍后再试。")
                time.sleep(0.5)

    def __exit__(self, exc_type, exc, tb):
        if not self.acquired:
            return
        try:
            payload = self._read_lock_payload()
            if payload.get("pid") == os.getpid():
                os.remove(self.lock_path)
        except Exception:
            pass
        finally:
            self.acquired = False

    def _read_lock_payload(self):
        try:
            with open(self.lock_path, "r", encoding="utf-8") as f:
                payload = json.load(f)
        except Exception:
            return {}
        return payload if isinstance(payload, dict) else {}

    def _remove_stale_lock(self):
        payload = self._read_lock_payload()
        pid = payload.get("pid")
        if not isinstance(pid, int):
            return False
        if _process_exists(pid):
            return False
        try:
            os.remove(self.lock_path)
            print(f"已清理过期淘宝采集 profile 锁: {self.lock_path}", file=sys.stderr)
            return True
        except Exception:
            return False


def _process_exists(pid):
    if not isinstance(pid, int) or pid <= 0:
        return False
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def _profile_lock(profile_dir, timeout_sec=None):
    resolved_timeout = (
        timeout_sec
        if timeout_sec is not None
        else _env_float_range(PROFILE_LOCK_TIMEOUT_ENV, 120.0, min_value=1.0, max_value=300.0)
    )
    return ProfileLock(profile_dir, timeout_sec=resolved_timeout)


class BrowserSlotLock:
    """跨 profile 的淘宝浏览器并发槽位锁。

    复用源项目的浏览器槽位保护思路，默认只允许一个自动化浏览器实例运行，
    避免多任务并发打开淘宝页面导致风控升级。
    """

    def __init__(self, profile_dir, timeout_sec=None):
        self.profile_dir = profile_dir
        self.timeout_sec = timeout_sec if timeout_sec is not None else _browser_slot_timeout_seconds()
        self.max_slots = _browser_slot_limit()
        base_dir = _browser_slot_root(profile_dir)
        self.slot_dir = os.path.join(base_dir, GLOBAL_BROWSER_SLOT_DIR)
        self.mutex_dir = os.path.join(base_dir, GLOBAL_BROWSER_SLOT_MUTEX_DIR)
        self.slot_path = None
        self.acquired = False

    def __enter__(self):
        os.makedirs(self.slot_dir, exist_ok=True)
        deadline = time.time() + self.timeout_sec
        while True:
            self._acquire_mutex()
            try:
                self._clean_stale_slots()
                active_slots = self._active_slot_files()
                if len(active_slots) < self.max_slots:
                    self.slot_path = self._create_slot_file()
                    self.acquired = True
                    return self
            finally:
                self._release_mutex()

            if time.time() >= deadline:
                raise RuntimeError(
                    f"淘宝采集浏览器槽位已满（{len(active_slots)}/{self.max_slots}），请稍后再试。"
                )
            time.sleep(0.5)

    def __exit__(self, exc_type, exc, tb):
        if not self.acquired or not self.slot_path:
            return
        try:
            payload = self._read_json(self.slot_path)
            if payload.get("pid") == os.getpid():
                os.remove(self.slot_path)
        except Exception:
            pass
        finally:
            self.acquired = False
            self.slot_path = None

    def _acquire_mutex(self):
        deadline = time.time() + min(self.timeout_sec, 10)
        while True:
            try:
                os.mkdir(self.mutex_dir, 0o700)
                payload = {
                    "pid": os.getpid(),
                    "created_at": _format_shanghai_time(datetime.now(SHANGHAI_TZ)),
                    "timezone": "Asia/Shanghai",
                }
                with open(os.path.join(self.mutex_dir, "owner.json"), "w", encoding="utf-8") as f:
                    json.dump(payload, f, ensure_ascii=False)
                return
            except FileExistsError:
                self._remove_stale_mutex()
                if time.time() >= deadline:
                    raise RuntimeError("淘宝采集浏览器槽位锁正忙，请稍后再试。")
                time.sleep(0.1)

    def _release_mutex(self):
        try:
            owner_path = os.path.join(self.mutex_dir, "owner.json")
            payload = self._read_json(owner_path)
            if payload.get("pid") == os.getpid():
                try:
                    os.remove(owner_path)
                except FileNotFoundError:
                    pass
                os.rmdir(self.mutex_dir)
        except Exception:
            pass

    def _remove_stale_mutex(self):
        owner_path = os.path.join(self.mutex_dir, "owner.json")
        payload = self._read_json(owner_path)
        pid = payload.get("pid")
        if isinstance(pid, int) and _process_exists(pid):
            return False
        try:
            if os.path.exists(owner_path):
                os.remove(owner_path)
            os.rmdir(self.mutex_dir)
            print(f"已清理过期淘宝采集槽位互斥锁: {self.mutex_dir}", file=sys.stderr)
            return True
        except Exception:
            return False

    def _active_slot_files(self):
        try:
            return [
                os.path.join(self.slot_dir, name)
                for name in os.listdir(self.slot_dir)
                if name.endswith(".json")
            ]
        except FileNotFoundError:
            return []

    def _clean_stale_slots(self):
        for path in self._active_slot_files():
            payload = self._read_json(path)
            pid = payload.get("pid")
            if isinstance(pid, int) and _process_exists(pid):
                continue
            try:
                os.remove(path)
                print(f"已清理过期淘宝采集浏览器槽位: {path}", file=sys.stderr)
            except Exception:
                pass

    def _create_slot_file(self):
        for _ in range(20):
            path = os.path.join(
                self.slot_dir,
                f"slot-{os.getpid()}-{random.randint(100000, 999999)}.json",
            )
            try:
                fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
            except FileExistsError:
                continue
            payload = {
                "pid": os.getpid(),
                "profile_dir": self.profile_dir,
                "created_at": _format_shanghai_time(datetime.now(SHANGHAI_TZ)),
                "timezone": "Asia/Shanghai",
            }
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(payload, f, ensure_ascii=False)
            return path
        raise RuntimeError("创建淘宝采集浏览器槽位失败，请稍后再试。")

    @staticmethod
    def _read_json(path):
        try:
            with open(path, "r", encoding="utf-8") as f:
                payload = json.load(f)
        except Exception:
            return {}
        return payload if isinstance(payload, dict) else {}


def _env_int_range(name, default_value, min_value=1, max_value=999):
    raw_value = os.environ.get(name)
    if raw_value is None or str(raw_value).strip() == "":
        return default_value
    try:
        value = int(str(raw_value).strip())
    except ValueError:
        return default_value
    return max(min_value, min(max_value, value))


def _env_float_range(name, default_value, min_value=0.1, max_value=60.0):
    raw_value = os.environ.get(name)
    if raw_value is None or str(raw_value).strip() == "":
        return default_value
    try:
        value = float(str(raw_value).strip())
    except ValueError:
        return default_value
    return max(min_value, min(max_value, value))


def _browser_slot_limit():
    return _env_int_range(GLOBAL_BROWSER_SLOT_LIMIT_ENV, 1, min_value=1, max_value=8)


def _browser_slot_timeout_seconds():
    return _env_int_range(GLOBAL_BROWSER_SLOT_TIMEOUT_ENV, 120, min_value=1, max_value=600)


def _browser_slot_root(profile_dir):
    configured_root = os.environ.get(GLOBAL_BROWSER_SLOT_ROOT_ENV, "").strip()
    if configured_root:
        return os.path.abspath(os.path.expanduser(configured_root))
    return os.path.dirname(os.path.abspath(profile_dir)) or os.path.abspath(profile_dir)


def _browser_slot_lock(profile_dir, timeout_sec=None):
    return BrowserSlotLock(profile_dir, timeout_sec=timeout_sec)

def ensure_cloakbrowser():
    try:
        import cloakbrowser  # noqa: F401
        return
    except ImportError:
        print(
            json.dumps(
                {"error": "缺少 cloakbrowser，请先运行：bash scripts/setup_python_env.sh"},
                ensure_ascii=False,
            )
        )
        sys.exit(1)


def launch_browser(profile_dir, headless=True):
    """启动 CloakBrowser 持久化上下文。"""
    profile_lock = _profile_lock(profile_dir)
    browser_slot_lock = None
    profile_lock.__enter__()
    try:
        browser_slot_lock = _browser_slot_lock(profile_dir)
        browser_slot_lock.__enter__()
        ctx, page = launch_cloakbrowser(profile_dir, headless=headless)
        setattr(ctx, "_wx_xd_profile_lock", profile_lock)
        setattr(ctx, "_wx_xd_browser_slot_lock", browser_slot_lock)
        return ctx, page
    except Exception:
        if browser_slot_lock is not None:
            browser_slot_lock.__exit__(None, None, None)
        profile_lock.__exit__(None, None, None)
        raise


def launch_cloakbrowser(profile_dir, headless=True):
    """使用 CloakBrowser 启动持久化上下文。"""
    _prepare_cloakbrowser_profile(profile_dir)
    try:
        return _launch_cloakbrowser_once(profile_dir, headless=headless)
    except Exception as e:
        if not _is_cloakbrowser_profile_crash(e):
            raise
        backup_dir = _repair_cloakbrowser_profile(profile_dir, e)
        raise RuntimeError(
            f"CloakBrowser profile 启动崩溃，已备份旧 profile 到 {backup_dir}，"
            "并重建干净 profile。请重新执行本次操作。"
        ) from e


def _prepare_cloakbrowser_profile(profile_dir):
    """用子进程探测 profile，避免崩溃污染当前采集进程。"""
    result = _run_cloakbrowser_profile_probe(profile_dir)
    if result.get("ok"):
        return
    if not result.get("profile_crash"):
        raise RuntimeError(result.get("error") or "CloakBrowser profile 探测失败。")

    backup_dir = _repair_cloakbrowser_profile(profile_dir, result.get("error") or "profile crash")
    print(
        f"CloakBrowser profile 启动崩溃，已备份旧 profile 到 {backup_dir}，"
        "并重建干净 profile 后重试。",
        file=sys.stderr,
    )

    retry_result = _run_cloakbrowser_profile_probe(profile_dir)
    if retry_result.get("ok"):
        return
    raise RuntimeError(retry_result.get("error") or "CloakBrowser profile 修复后仍无法启动。")


def _run_cloakbrowser_profile_probe(profile_dir):
    script_path = os.path.abspath(__file__)
    command = [
        sys.executable,
        "-B",
        script_path,
        PROFILE_PROBE_COMMAND,
        "--profile-dir",
        profile_dir,
    ]
    try:
        completed = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=PROFILE_PROBE_TIMEOUT_SEC,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return {"ok": False, "profile_crash": False, "error": "CloakBrowser profile 探测超时。"}

    payload = _parse_probe_output(completed.stdout)
    if payload is None:
        stderr = (completed.stderr or "").strip()
        stdout = (completed.stdout or "").strip()
        detail = stderr or stdout or f"退出码 {completed.returncode}"
        return {"ok": False, "profile_crash": False, "error": f"CloakBrowser profile 探测失败: {detail}"}
    return payload


def _parse_probe_output(output):
    for line in reversed((output or "").splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            return payload
    return None


def _derive_fingerprint_seed(profile_dir):
    """根据 profile 路径派生稳定的 CloakBrowser fingerprint seed。

    同一个 profile 多次启动需要复用同一指纹，避免老账号配新指纹的风控信号。
    """
    if not profile_dir:
        return 1
    abs_path = os.path.abspath(os.path.expanduser(str(profile_dir)))
    digest = hashlib.sha256(abs_path.encode("utf-8")).hexdigest()
    return int(digest[:8], 16) % (2**31 - 1) + 1


def _launch_cloakbrowser_once(profile_dir, headless=False):
    ensure_cloakbrowser()
    from cloakbrowser import launch_persistent_context

    _clean_chromium_singleton_files(profile_dir)
    fingerprint_seed = _derive_fingerprint_seed(profile_dir)
    ctx = launch_persistent_context(
        profile_dir,
        headless=headless,
        humanize=True,
        human_preset="careful",
        locale="zh-CN",
        timezone="Asia/Shanghai",
        viewport={"width": 1920, "height": 1080},
        args=[
            "--fingerprint-platform=macos",
            f"--fingerprint={fingerprint_seed}",
        ],
    )
    pages = ctx.pages if hasattr(ctx, "pages") else []
    page = pages[0] if pages else ctx.new_page()
    setattr(ctx, "_wx_xd_profile_dir", profile_dir)
    install_minimal_taobao_stealth(context=ctx, page=page)
    return ctx, page


def _is_cloakbrowser_profile_crash(error):
    message = str(error)
    return (
        "BrowserType.launch_persistent_context" in message
        and "Target page, context or browser has been closed" in message
        and ("SIGTRAP" in message or "SIGABRT" in message or "process did exit" in message)
    )


def _copy_profile_file(src_root, dst_root, relative_path):
    src = os.path.join(src_root, relative_path)
    if not os.path.exists(src):
        return
    dst = os.path.join(dst_root, relative_path)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    try:
        import shutil
        shutil.copy2(src, dst)
    except Exception as e:
        print(f"迁移 profile 文件失败（跳过）: {relative_path}: {e}", file=sys.stderr)


def _repair_cloakbrowser_profile(profile_dir, error):
    """备份崩溃 profile，并只迁移登录态所需的最小文件。"""
    if not os.path.isdir(profile_dir):
        os.makedirs(profile_dir, exist_ok=True)
        return None

    stamp = datetime.now(SHANGHAI_TZ).strftime("%Y%m%d-%H%M%S")
    backup_dir = f"{profile_dir}.cloakbrowser-broken-{stamp}"
    suffix = 1
    while os.path.exists(backup_dir):
        suffix += 1
        backup_dir = f"{profile_dir}.cloakbrowser-broken-{stamp}-{suffix}"

    os.rename(profile_dir, backup_dir)
    os.makedirs(os.path.join(profile_dir, "Default"), exist_ok=True)

    for relative_path in (
        "Local State",
        "Default/Cookies",
        "Default/Cookies-journal",
        "taobao_captcha_history.jsonl",
        ACCESS_LIMIT_COOLDOWN_FILE,
        CAPTCHA_FAILURE_COOLDOWN_FILE,
    ):
        _copy_profile_file(backup_dir, profile_dir, relative_path)

    marker = {
        "repaired_at": _format_shanghai_time(datetime.now(SHANGHAI_TZ)),
        "timezone": "Asia/Shanghai",
        "backup_dir": backup_dir,
        "reason": str(error).splitlines()[0],
    }
    try:
        with open(os.path.join(profile_dir, "taobao_profile_repaired.json"), "w", encoding="utf-8") as f:
            json.dump(marker, f, ensure_ascii=False, indent=2)
    except Exception:
        pass
    return backup_dir


def _clean_chromium_singleton_files(profile_dir):
    """清理 Chromium 崩溃后留下的 profile 锁文件。"""
    if _chromium_profile_lock_is_active(profile_dir):
        raise RuntimeError("淘宝采集浏览器 profile 正在被 Chromium/CloakBrowser 占用，请关闭相关窗口后重试。")

    for name in ("SingletonLock", "SingletonCookie", "SingletonSocket", "RunningChromeVersion"):
        path = os.path.join(profile_dir, name)
        try:
            if os.path.exists(path) or os.path.islink(path):
                if os.path.islink(path):
                    os.unlink(path)
                else:
                    os.remove(path)
                print(f"已清理残留浏览器锁文件: {path}", file=sys.stderr)
        except Exception as e:
            print(f"清理浏览器锁文件失败（继续尝试启动）: {path}: {e}", file=sys.stderr)


def _chromium_profile_lock_is_active(profile_dir):
    lock_path = os.path.join(profile_dir, "SingletonLock")
    if not os.path.islink(lock_path):
        return False
    try:
        target = os.readlink(lock_path)
    except OSError:
        return False
    match = re.search(r"-(\d+)$", target)
    if not match:
        return False
    return _process_exists(int(match.group(1)))


def close_browser_context(ctx):
    if ctx is None:
        return
    try:
        ctx.close()
    except Exception:
        pass
    browser_slot_lock = getattr(ctx, "_wx_xd_browser_slot_lock", None)
    if browser_slot_lock is not None:
        try:
            browser_slot_lock.__exit__(None, None, None)
        except Exception:
            pass
    profile_lock = getattr(ctx, "_wx_xd_profile_lock", None)
    if profile_lock is not None:
        try:
            profile_lock.__exit__(None, None, None)
        except Exception:
            pass


# ======================== 风控 & 登录处理 ========================

def _format_shanghai_time(value):
    return value.astimezone(SHANGHAI_TZ).strftime("%Y-%m-%d %H:%M:%S+08:00")


def _parse_shanghai_time(value):
    if not value:
        return None
    match = re.match(r"^(\d{4})-(\d{2})-(\d{2})\s+(\d{1,2}):(\d{2}):(\d{2})\+08:00$", value)
    if not match:
        return None
    year, month, day, hour, minute, second = (int(part) for part in match.groups())
    try:
        return datetime(year, month, day, hour, minute, second, tzinfo=SHANGHAI_TZ)
    except ValueError:
        return None


def _extract_access_resume_time(reason):
    if not reason:
        return None
    match = re.search(r"预计(\d{4})-(\d{2})-(\d{2})\s+(\d{1,2})时后恢复正常", reason)
    if not match:
        return None
    year, month, day, hour = (int(part) for part in match.groups())
    try:
        return datetime(year, month, day, hour, 0, 0, tzinfo=SHANGHAI_TZ)
    except ValueError:
        return None


def _access_limit_cooldown_path(profile_dir):
    return os.path.join(profile_dir, ACCESS_LIMIT_COOLDOWN_FILE)


def _captcha_failure_cooldown_path(profile_dir):
    return os.path.join(profile_dir, CAPTCHA_FAILURE_COOLDOWN_FILE)


def _record_access_limit_cooldown(profile_dir, reason, stage, resume_at=None):
    if not profile_dir:
        return
    os.makedirs(profile_dir, exist_ok=True)
    resume_at = resume_at or _extract_access_resume_time(reason)
    payload = {
        "stage": stage,
        "reason": reason,
        "detected_at": _format_shanghai_time(datetime.now(SHANGHAI_TZ)),
        "timezone": "Asia/Shanghai",
    }
    if resume_at:
        payload["resume_at"] = _format_shanghai_time(resume_at)
    try:
        with open(_access_limit_cooldown_path(profile_dir), "w", encoding="utf-8") as f:
            json.dump(payload, f, ensure_ascii=False, indent=2)
    except Exception as e:
        print(f"写入淘宝访问限制冷却记录失败（继续返回错误）: {e}", file=sys.stderr)


def _read_access_limit_cooldown(profile_dir):
    path = _access_limit_cooldown_path(profile_dir)
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            payload = json.load(f)
    except Exception:
        return None
    return payload if isinstance(payload, dict) else None


def _clear_access_limit_cooldown(profile_dir):
    path = _access_limit_cooldown_path(profile_dir)
    try:
        if os.path.exists(path):
            os.remove(path)
    except Exception as e:
        print(f"清理淘宝访问限制冷却记录失败（继续执行）: {e}", file=sys.stderr)


def _captcha_failure_cooldown_minutes():
    return _env_int(CAPTCHA_FAILURE_COOLDOWN_MINUTES_ENV, 30, min_value=5, max_value=360)


def _record_captcha_failure_cooldown(profile_dir, reason, stage, minutes=None):
    if not profile_dir:
        return
    os.makedirs(profile_dir, exist_ok=True)
    minutes = minutes or _captcha_failure_cooldown_minutes()
    resume_at = datetime.now(SHANGHAI_TZ) + timedelta(minutes=minutes)
    payload = {
        "stage": stage,
        "reason": reason,
        "detected_at": _format_shanghai_time(datetime.now(SHANGHAI_TZ)),
        "resume_at": _format_shanghai_time(resume_at),
        "timezone": "Asia/Shanghai",
        "cooldown_minutes": minutes,
    }
    try:
        with open(_captcha_failure_cooldown_path(profile_dir), "w", encoding="utf-8") as f:
            json.dump(payload, f, ensure_ascii=False, indent=2)
    except Exception as e:
        print(f"写入淘宝验证码失败冷却记录失败（继续返回错误）: {e}", file=sys.stderr)


def _read_captcha_failure_cooldown(profile_dir):
    path = _captcha_failure_cooldown_path(profile_dir)
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            payload = json.load(f)
    except Exception:
        return None
    return payload if isinstance(payload, dict) else None


def _clear_captcha_failure_cooldown(profile_dir):
    path = _captcha_failure_cooldown_path(profile_dir)
    try:
        if os.path.exists(path):
            os.remove(path)
    except Exception as e:
        print(f"清理淘宝验证码失败冷却记录失败（继续执行）: {e}", file=sys.stderr)


def _captcha_failure_cooldown_state(profile_dir):
    payload = _read_captcha_failure_cooldown(profile_dir)
    if not payload:
        return {
            "active": False,
            "cooldown": None,
            "message": None,
        }

    resume_at = _parse_shanghai_time(payload.get("resume_at"))
    if not resume_at:
        return {
            "active": True,
            "cooldown": payload,
            "message": (
                payload.get("reason")
                or "淘宝验证码自动处理失败，请先人工确认账号和页面状态。"
            ),
        }

    now = datetime.now(SHANGHAI_TZ)
    if now < resume_at:
        return {
            "active": True,
            "cooldown": payload,
            "message": (
                "淘宝验证码自动处理失败后处于本地保护冷却期，"
                f"建议恢复时间：{_format_shanghai_time(resume_at)}。"
            ),
        }

    _clear_captcha_failure_cooldown(profile_dir)
    return {
        "active": False,
        "cooldown": None,
        "message": None,
    }


def _access_limit_cooldown_state(profile_dir):
    payload = _read_access_limit_cooldown(profile_dir)
    if not payload:
        return {
            "active": False,
            "cooldown": None,
            "message": None,
        }

    resume_at = _parse_shanghai_time(payload.get("resume_at"))
    if not resume_at:
        return {
            "active": True,
            "cooldown": payload,
            "message": (
                payload.get("reason")
                or "淘宝账号处于访问限制状态，请先人工确认账号已恢复。"
            ),
        }

    now = datetime.now(SHANGHAI_TZ)
    if now < resume_at:
        return {
            "active": True,
            "cooldown": payload,
            "message": (
                "淘宝账号仍处于访问限制冷却期，"
                f"平台提示恢复时间：{_format_shanghai_time(resume_at)}。"
            ),
        }

    _clear_access_limit_cooldown(profile_dir)
    return {
        "active": False,
        "cooldown": None,
        "message": None,
    }


def _check_access_limit_cooldown(profile_dir):
    if os.environ.get(ACCESS_LIMIT_COOLDOWN_BYPASS_ENV) == "1":
        return

    state = _access_limit_cooldown_state(profile_dir)
    if not state["active"]:
        return

    payload = state.get("cooldown") or {}
    resume_at = _parse_shanghai_time(payload.get("resume_at"))
    if not resume_at:
        reason = state.get("message") or "淘宝账号处于访问限制状态。"
        raise RuntimeError(f"{reason}请先人工确认账号已恢复，再删除本地冷却记录或设置临时绕过环境变量。")

    raise RuntimeError(f"{state['message']}本次未打开浏览器访问淘宝。")


def _check_captcha_failure_cooldown(profile_dir):
    if os.environ.get(CAPTCHA_FAILURE_COOLDOWN_BYPASS_ENV) == "1":
        return

    state = _captcha_failure_cooldown_state(profile_dir)
    if not state["active"]:
        return

    raise RuntimeError(f"{state['message']}本次未打开浏览器访问淘宝。")


def _is_blocked(page):
    return is_taobao_captcha_page(page)


def _env_int(name, default_value, min_value=1, max_value=5):
    raw = os.environ.get(name)
    if raw is None:
        return default_value
    try:
        value = int(raw)
    except ValueError:
        return default_value
    return max(min_value, min(value, max_value))


def _captcha_attempt_limits():
    """真实账号默认保守重试，避免验证失败时反复触发平台风控。"""
    return (
        _env_int("WX_XD_TAOBAO_CAPTCHA_MAX_ROUNDS", 1, min_value=1, max_value=3),
        _env_int("WX_XD_TAOBAO_CAPTCHA_MAX_RETRIES", 2, min_value=1, max_value=5),
    )


def is_taobao_collection_protection_error(message):
    """检测是否为淘宝保护性错误，此类错误应终止批量采集而非继续。"""
    keywords = [
        "本次未打开浏览器访问淘宝",
        "淘宝账号仍处于访问限制冷却期",
        "淘宝验证码自动处理失败后处于本地保护冷却期",
        "淘宝滑块验证码自动处理失败",
        "淘宝触发人脸验证",
        "淘宝触发短信验证",
        "淘宝触发二维码/扫码验证",
        "请打开淘宝登录窗口人工完成验证后重试",
        "账号近期访问行为异常",
    ]
    return any(kw in (message or "") for kw in keywords)


def _raise_if_access_limited(page, stage, profile_dir=None):
    reason = get_taobao_access_limited_reason(page)
    if reason:
        _record_access_limit_cooldown(profile_dir, reason, stage)
        raise RuntimeError(f"{stage}检测到{reason}")


def _profile_dir_from_page(page):
    try:
        ctx = page.context
    except Exception:
        ctx = None
    return getattr(ctx, "_wx_xd_profile_dir", None)


def _handle_possible_captcha(ctx, page, source_url, stage):
    """检测并处理淘宝验证码，成功后重新回到商品页触发数据请求。"""
    profile_dir = getattr(ctx, "_wx_xd_profile_dir", None)
    _raise_if_access_limited(page, stage, profile_dir=profile_dir)
    if not _is_blocked(page):
        return False

    manual_reason = get_taobao_manual_verification_reason(page)
    if manual_reason:
        _record_captcha_failure_cooldown(profile_dir, manual_reason, stage)
        raise RuntimeError(f"{stage}检测到{manual_reason}")

    print(f"在 {stage} 阶段检测到淘宝验证拦截，开始自动处理...", file=sys.stderr)
    max_rounds, max_retries = _captcha_attempt_limits()
    if not solve_taobao_captcha(
        page,
        ctx,
        max_rounds=max_rounds,
        max_retries=max_retries,
        timeout_sec=45,
    ):
        _raise_if_access_limited(page, stage, profile_dir=profile_dir)
        _record_captcha_failure_cooldown(
            profile_dir,
            "淘宝滑块验证码自动处理失败，请暂停自动采集并人工确认账号状态。",
            stage,
        )
        raise RuntimeError("被淘宝滑块验证码拦截，自动处理失败，请打开淘宝登录窗口手动验证后重试。")

    _handle_login_confirm(page)

    try:
        current_url = page.url or ""
    except Exception:
        current_url = ""

    if "item.taobao.com" not in current_url and "detail.tmall.com" not in current_url:
        page.goto(source_url, timeout=30000, wait_until="domcontentloaded")
        _handle_login_confirm(page)
    _wait_for_page_stable(page)
    _raise_if_access_limited(page, stage, profile_dir=profile_dir)
    return True


def _handle_login_confirm(page):
    """检测并处理淘宝「确认登录」页面，点击「快速进入」。"""
    try:
        body_text = page.evaluate("() => document.body?.innerText?.substring(0, 800) || ''")
        current_url = page.url or ""
        is_login_confirm = any(kw in body_text for kw in LOGIN_CONFIRM_KEYWORDS)
        if not is_login_confirm:
            return False

        print("检测到淘宝「确认登录」页面，自动点击「快速进入」...", file=sys.stderr)

        quick_selectors = [
            "text=快速进入",
            "button:has-text('快速进入')",
            "a:has-text('快速进入')",
            ".quick-login-btn",
            "#J_SubmitQuick",
            "#J_SubmitStatic",
            "#login-submit",
            "[type='submit']",
            "[role='button']",
            ".fm-btn",
        ]
        clicked = False
        for sel in quick_selectors:
            try:
                el = page.query_selector(sel)
                if el and el.is_visible():
                    el.click(timeout=3000)
                    clicked = True
                    break
            except Exception:
                continue
        if not clicked:
            clicked = bool(page.evaluate(
                """
                (texts) => {
                    const nodes = Array.from(document.querySelectorAll(
                        'button, a, div, span, [role="button"], [type="submit"], .fm-btn'
                    ));
                    const isVisible = (node) => {
                        const style = window.getComputedStyle(node);
                        const rect = node.getBoundingClientRect();
                        return style.visibility !== 'hidden'
                            && style.display !== 'none'
                            && rect.width > 0
                            && rect.height > 0;
                    };
                    for (const node of nodes) {
                        const text = (node.innerText || node.textContent || '').trim();
                        if (!text || !isVisible(node)) continue;
                        if (texts.some((keyword) => text.includes(keyword))) {
                            node.click();
                            return true;
                        }
                    }
                    return false;
                }
                """,
                list(LOGIN_CONFIRM_CLICK_TEXTS),
            ))

        if not clicked:
            return False

        _wait_for_login_confirm_resolved(page, body_text, current_url)
        return True
    except Exception:
        return False


def _wait_for_login_confirm_resolved(page, previous_text="", previous_url="", timeout_sec=12):
    deadline = time.time() + timeout_sec
    while time.time() < deadline:
        time.sleep(0.5)
        try:
            current_url = page.url or ""
            if current_url != previous_url and "login.taobao.com" not in current_url:
                _wait_for_page_stable(page, timeout_sec=5)
                return True
            body_text = page.evaluate("() => document.body?.innerText?.substring(0, 800) || ''")
            if body_text != previous_text and not any(kw in body_text for kw in LOGIN_CONFIRM_KEYWORDS):
                _wait_for_page_stable(page, timeout_sec=5)
                return True
        except Exception:
            return True
    return False


def _wait_for_page_stable(page, timeout_sec=15):
    """等待页面 URL 稳定后再继续。

    淘宝详情页常持续有后台请求，networkidle 往往迟迟不触发（真机实测），故仍以
    URL 稳定为准（连续两次 URL 不变即视为稳定），仅适度收紧轮询间隔削减纯等待。
    """
    deadline = time.time() + timeout_sec
    last_url = page.url
    while time.time() < deadline:
        time.sleep(0.35)
        try:
            cur = page.url
            if cur != last_url:
                last_url = cur
                continue
            time.sleep(random.uniform(0.4, 0.8))
            return True
        except Exception:
            return False
    return True


# ======================== 数据捕获 ========================

def _read_response_json(response):
    """读取 mtop 响应，兼容 JSON 与 JSONP。"""
    try:
        return response.json()
    except Exception:
        pass

    try:
        text = response.text()
    except Exception:
        return None

    text = (text or "").strip()
    if not text:
        return None

    try:
        return json.loads(text)
    except Exception:
        pass

    # 淘宝 mtop 偶尔返回 callback({...}) 形式。
    jsonp_text = text.rstrip(";").strip()
    if "(" in jsonp_text and jsonp_text.endswith(")"):
        payload = jsonp_text[jsonp_text.find("(") + 1:-1].strip()
        try:
            return json.loads(payload)
        except Exception:
            return None

    return None


def _capture_product_data(page, timeout_sec=None, trigger=None):
    """拦截网络响应，捕获淘宝 mtop 商品详情 API 返回的数据"""
    timeout_sec = (
        float(timeout_sec)
        if timeout_sec is not None
        else _env_float_range(CAPTURE_TIMEOUT_ENV, 12.0, min_value=5.0, max_value=30.0)
    )
    detail_grace_sec = _env_float_range(
        CAPTURE_DETAIL_GRACE_ENV,
        2.5,
        min_value=0.1,
        max_value=10.0,
    )
    # detail 命中但 desc 请求未在途时，只等 short_grace 便提前返回，
    # 避免对不调 getdesc 的页面每次都白等满 detail_grace（纯削减等待，不增访问频率）。
    short_grace_sec = _env_float_range(
        CAPTURE_SHORT_GRACE_ENV,
        1.2,
        min_value=0.1,
        max_value=10.0,
    )
    if short_grace_sec > detail_grace_sec:
        short_grace_sec = detail_grace_sec
    poll_interval_sec = _env_float_range(
        CAPTURE_POLL_INTERVAL_ENV,
        0.3,
        min_value=0.05,
        max_value=1.0,
    )
    captured = {"detail": None, "desc": None, "container": []}
    captured_urls = set()
    # 标记 desc 请求是否已发出（在途），用于动态决定 grace 时长。
    desc_request_seen = {"value": False}

    def _handle_response(response):
        try:
            url = response.url
            if url in captured_urls:
                return
            body = None
            if ("mtop.taobao.detail.getdetail" in url or
                    "mtop.tmall.detail.getdetail" in url):
                if captured["detail"] is None:
                    body = _read_response_json(response)
                    if body:
                        captured["detail"] = body
                        captured_urls.add(url)
            if ("mtop.taobao.detail.getdesc" in url or
                    "mtop.tmall.detail.getdesc" in url):
                if captured["desc"] is None:
                    body = _read_response_json(response)
                    if body:
                        captured["desc"] = body
                        captured_urls.add(url)
            if "mtop.alibaba.fc.api.maoxland.containerfacade.singleview" in url.lower():
                body = _read_response_json(response)
                if body:
                    captured["container"].append(body)
                    captured_urls.add(url)
        except Exception:
            pass

    def on_response(response):
        _handle_response(response)

    def on_request_finished(request):
        try:
            response = request.response()
            if response:
                _handle_response(response)
        except Exception:
            pass

    def on_request(request):
        # 仅标记 desc 请求是否已发出，不阻塞、不读响应体。
        try:
            url = (request.url or "").lower()
            if ("mtop.taobao.detail.getdesc" in url or
                    "mtop.tmall.detail.getdesc" in url):
                desc_request_seen["value"] = True
        except Exception:
            pass

    page.on("response", on_response)
    page.on("requestfinished", on_request_finished)
    page.on("request", on_request)

    try:
        trigger_ok = True
        if trigger:
            trigger_ok = trigger()

        deadline = time.time() + timeout_sec
        detail_seen_at = None
        while time.time() < deadline:
            if trigger_ok is False:
                break
            now = time.time()
            has_detail = captured["detail"] is not None or bool(captured.get("container"))
            if has_detail and detail_seen_at is None:
                detail_seen_at = now
            if has_detail and captured["desc"] is not None:
                break
            # desc 请求在途则等满 detail_grace 尽量拿描述；否则只等 short_grace 提前返回。
            if has_detail and detail_seen_at is not None:
                grace = detail_grace_sec if desc_request_seen["value"] else short_grace_sec
                if now - detail_seen_at >= grace:
                    break
            if has_detail and now > deadline - 2:
                break
            time.sleep(poll_interval_sec)

        time.sleep(random.uniform(0.4, 0.9))
    finally:
        try:
            page.remove_listener("response", on_response)
        except Exception:
            pass
        try:
            page.remove_listener("requestfinished", on_request_finished)
        except Exception:
            pass
        try:
            page.remove_listener("request", on_request)
        except Exception:
            pass
    return captured


def _trigger_product_requests(page, source_url, allow_reload=True):
    """通过交互触发商品页的 mtop 请求，避免不必要的页面重载。

    首选方案是在当前页面通过滚动和 hover 触发懒加载 API 调用（淘宝详情页
    切换 SKU、滚动到详情区域时通常会触发新的 mtop 请求）。
    当调用方明确允许重载时，重新进入商品页以确保响应监听器覆盖首包请求。
    """
    profile_dir = _profile_dir_from_page(page)
    try:
        current_url = page.url or ""
    except Exception:
        current_url = ""

    already_on_product = any(kw in current_url for kw in ("item.taobao.com", "detail.tmall.com"))

    if not already_on_product or allow_reload:
        # 重试采集时重新进入商品页，避免 mtop 首包在监听器安装前已经完成。
        page.goto(source_url, timeout=30000, wait_until="domcontentloaded")
        _wait_for_page_stable(page)
        _raise_if_access_limited(page, "重新进入商品页", profile_dir=profile_dir)
        _handle_login_confirm(page)
        _raise_if_access_limited(page, "确认登录后", profile_dir=profile_dir)

    if _is_blocked(page):
        return False

    # 随机化浏览行为，触发懒加载 mtop 请求
    _random_browse_interaction(page)

    return True


def _iter_json_nodes(value, depth=0, max_depth=8):
    if depth > max_depth:
        return
    yield value
    if isinstance(value, dict):
        for child in value.values():
            yield from _iter_json_nodes(child, depth + 1, max_depth=max_depth)
    elif isinstance(value, list):
        for child in value:
            yield from _iter_json_nodes(child, depth + 1, max_depth=max_depth)


def _json_first_string_by_key(value, key_names):
    key_names = {str(item).lower() for item in key_names}
    for node in _iter_json_nodes(value):
        if not isinstance(node, dict):
            continue
        for key, child in node.items():
            if str(key).lower() in key_names and isinstance(child, str) and child.strip():
                return child.strip()
    return ""


def _json_image_urls(value, limit=20):
    urls = []
    seen = set()
    for node in _iter_json_nodes(value):
        candidates = []
        if isinstance(node, str):
            candidates = re.findall(r'(?:https?:)?//[^"\'\s<>]+', node)
        elif isinstance(node, dict):
            for key in ("url", "src", "image", "img", "pic", "picUrl", "imageUrl"):
                child = node.get(key)
                if isinstance(child, str):
                    candidates.append(child)
        elif isinstance(node, list):
            candidates = [item for item in node if isinstance(item, str)]
        for raw_url in candidates:
            url = _normalize_url(str(raw_url).strip())
            if not url or url in seen:
                continue
            if not any(host in url for host in ["alicdn.com", "tbcdn.cn", "taobaocdn.com"]):
                continue
            if not any(marker in url for marker in ["item_pic", ".jpg", ".jpeg", ".png", ".webp"]):
                continue
            # 排除非商品图 URL 模式
            if _is_junk_image(url, 0, 0, 0, 0, ""):
                continue
            seen.add(url)
            urls.append(url)
            if len(urls) >= limit:
                return urls
    return urls


def _parse_flexible_mtop_response(captured, source_url):
    """解析新版 PC 详情页的容器化 mtop 响应，无法确定字段时交给 DOM 兜底补齐。"""
    containers = captured.get("container") or []
    if not containers:
        return None

    payloads = [item.get("data", item) if isinstance(item, dict) else item for item in containers]
    title = ""
    for payload in payloads:
        title = _json_first_string_by_key(payload, [
            "title",
            "itemTitle",
            "mainTitle",
            "subTitle",
        ])
        if title and len(title) >= 4:
            break

    images = []
    seen_images = set()
    for payload in payloads:
        for url in _json_image_urls(payload):
            if _looks_like_review_image(url):
                continue
            if _append_unique_image(images, seen_images, url, limit=12):
                break
        if len(images) >= 12:
            break

    if not title and not images:
        return None

    return {
        "external_product_id": source_url,
        "title": title or "淘宝未命名商品",
        "source_url": source_url,
        "images": images,
        "detail_images": [],
        "skus": [{
            "external_sku_id": "default",
            "specs": {"规格": "默认规格"},
            "cost_price": 9.9,
            "stock": 99,
        }],
        "supplier_name": "淘宝商家",
        "supplier_product_id": source_url,
        "category_hint": None,
        "brand_hint": "无品牌",
        "weight_gram": 500,
        "metadata": {"collection_source": "mtop_container_flexible"},
    }


def _extract_mtop_props(data):
    """从 mtop API 响应中提取结构化商品属性。

    淘宝 mtop getdetail 响应中，商品属性可能在以下位置：
    - data.props: [{name, value}, ...]
    - data.groupProps: [{group: ..., props: [{name, value}, ...]}, ...]
    - item.props: [{name, value}, ...]
    """
    if not isinstance(data, dict):
        return {}

    props = {}

    # 从 data.props 提取
    for prop in data.get("props") or []:
        if isinstance(prop, dict):
            name = str(prop.get("name", "")).strip()
            value = str(prop.get("value", "")).strip()
            if name and value:
                props[name] = value

    # 从 data.groupProps 提取（嵌套结构）
    for group in data.get("groupProps") or []:
        if isinstance(group, dict):
            for prop in group.get("props") or []:
                if isinstance(prop, dict):
                    name = str(prop.get("name", "")).strip()
                    value = str(prop.get("value", "")).strip()
                    if name and value and name not in props:
                        props[name] = value

    # 从 item.props 提取（兼容不同 API 版本）
    item = data.get("item") if isinstance(data.get("item"), dict) else {}
    for prop in item.get("props") or []:
        if isinstance(prop, dict):
            name = str(prop.get("name", "")).strip()
            value = str(prop.get("value", "")).strip()
            if name and value and name not in props:
                props[name] = value

    return props


def _merge_item_params(mtop_props, dom_params):
    """合并 mtop 结构化参数和 DOM 提取的参数。优先使用 mtop 数据。"""
    merged = {}
    # mtop 数据优先（结构化，更可靠）
    for key, value in (mtop_props or {}).items():
        if value and len(str(value)) <= 200:
            merged[key] = str(value)
    # DOM 数据作为补充（填补 mtop 缺失的字段）
    for key, value in (dom_params or {}).items():
        if key not in merged and value and len(str(value)) <= 200:
            merged[key] = str(value)
    return merged


def _parse_mtop_response(captured, source_url):
    """解析 mtop API 响应，转为 ExternalProductInput 格式"""
    detail = captured.get("detail")
    if not detail:
        flexible_product = _parse_flexible_mtop_response(captured, source_url)
        if flexible_product:
            return flexible_product
        raise ValueError("未能捕获到商品详情 API 响应")

    data = detail.get("data", detail)
    item = data.get("item", {}) if isinstance(data, dict) else {}

    title = item.get("title", "淘宝未命名商品")

    images = item.get("images", [])
    if isinstance(images, list):
        normalized_images = []
        seen_images = set()
        for img in images:
            if _looks_like_review_image(str(img)):
                continue
            if _append_unique_image(normalized_images, seen_images, img, limit=12):
                break
        images = normalized_images

    # 详情图
    detail_images = []
    desc_data = captured.get("desc")
    if desc_data:
        desc_inner = desc_data.get("data", desc_data)
        if isinstance(desc_inner, dict):
            desc_info = desc_inner.get("desc", {})
            if isinstance(desc_info, str):
                raw_detail_images = re.findall(r'<img[^>]+src=["\']([^"\']+)["\']', desc_info)
                detail_images = []
                seen_detail_images = set()
                for img in raw_detail_images:
                    if _append_unique_image(detail_images, seen_detail_images, img, limit=80):
                        break
            elif isinstance(desc_info, dict):
                imgs = desc_info.get("images", []) or desc_info.get("descDetailInfo", {}).get("images", [])
                detail_images = []
                seen_detail_images = set()
                for img in imgs:
                    if _append_unique_image(detail_images, seen_detail_images, img, limit=80):
                        break

    # SKU
    skus = []
    sku_data = data.get("sku", {}) if isinstance(data, dict) else {}
    val_item_info = sku_data.get("valItemInfo", {})
    sku_list = val_item_info.get("skuList", [])

    prop_path_map = {}
    for prop in val_item_info.get("props", []):
        prop_name = prop.get("name", "")
        for val in prop.get("values", []):
            val_id = str(val.get("valueId", ""))
            val_name = val.get("name", "")
            if prop_name and val_id and val_name:
                prop_path_map[val_id] = (prop_name, val_name)

    sku_core = data.get("skuCore", {}) if isinstance(data, dict) else {}
    sku_2_info = sku_core.get("sku2info", {})

    for idx, sku_item in enumerate(sku_list):
        sku_id = str(sku_item.get("skuId", f"sku_{idx}"))
        p_path = sku_item.get("pPath", "")

        specs = {}
        if p_path:
            for part in p_path.split(";"):
                if ":" in part:
                    _, val_id = part.split(":", 1)
                    if val_id in prop_path_map:
                        p_name, v_name = prop_path_map[val_id]
                        specs[p_name] = v_name

        sku_details = sku_2_info.get(sku_id, {})
        price_cents = 0
        if "price" in sku_details:
            price_cents = int(sku_details.get("price", {}).get("priceMoney", 0))
        elif "promotion" in sku_details:
            price_cents = int(sku_details.get("promotion", {}).get("priceMoney", 0))
        cost_price = float(price_cents) / 100.0 if price_cents > 0 else 9.9

        if not specs:
            specs = {"规格": sku_item.get("names", "默认规格")}

        stock = int(sku_details.get("quantity", 99))
        skus.append({
            "external_sku_id": sku_id,
            "specs": specs,
            "cost_price": cost_price,
            "stock": stock,
        })

    if not skus:
        skus.append({
            "external_sku_id": "default",
            "specs": {"规格": "默认规格"},
            "cost_price": 9.9,
            "stock": 99,
        })

    # 从 mtop 响应提取结构化商品属性
    mtop_props = _extract_mtop_props(data)

    # 从 mtop 属性中推断品牌
    brand_hint = mtop_props.get("品牌") or "无品牌"

    return {
        "external_product_id": source_url,
        "title": title,
        "source_url": source_url,
        "images": images,
        "detail_images": detail_images,
        "skus": skus,
        "supplier_name": "淘宝商家",
        "supplier_product_id": source_url,
        "category_hint": None,
        "brand_hint": brand_hint,
        "weight_gram": 500,
        "metadata": {
            "collection_source": "mtop_api",
            "taobao_item_params": mtop_props,
            "taobao_item_params_quality": _item_params_quality_report(mtop_props, "mtop_structured"),
        },
    }


def _normalize_url(url):
    if not url:
        return ""
    url = str(url).strip()
    if url.startswith("//"):
        return "https:" + url
    return url


def _normalize_taobao_image_url(url):
    """规范化淘宝图片 URL，去掉压缩裁剪后缀，避免同图不同尺寸重复。"""
    url = _normalize_url(url)
    if not url:
        return ""
    url = url.split("?", 1)[0].split("#", 1)[0]
    url = re.sub(r"(\.(?:jpg|jpeg|png|webp))_[^/?#]*$", r"\1", url, flags=re.IGNORECASE)
    return url


def _taobao_image_key(url):
    normalized = _normalize_taobao_image_url(url)
    if not normalized:
        return ""
    filename = normalized.rsplit("/", 1)[-1]
    return re.sub(r"(\.(?:jpg|jpeg|png|webp)).*$", r"\1", filename, flags=re.IGNORECASE)


def _append_unique_image(target, seen, url, limit=12):
    normalized = _normalize_taobao_image_url(url)
    if not normalized:
        return False
    if _is_junk_image(normalized, 0, 0, 0, 0, ""):
        return False
    key = _taobao_image_key(normalized) or normalized
    if key in seen:
        return False
    seen.add(key)
    target.append(normalized)
    return len(target) >= limit


def _is_taobao_image_host(url):
    return any(host in url for host in ["alicdn.com", "tbcdn.cn", "taobaocdn.com"])


def _looks_like_platform_asset_url(url):
    url_lower = (url or "").lower()
    if re.search(r"-\d+-tps-\d+-\d+", url_lower):
        return True
    return any(marker in url_lower for marker in [
        "-tps-",            # 淘宝/天猫平台素材，常见于 logo、贴片和活动图
        "-0-shopmanager",   # 店铺管理后台生成的店招/品牌图
        "shopmanager",
    ])


def _looks_like_review_image(url, context_text=""):
    text = f"{url} {context_text}".lower()
    return any(marker in text for marker in [
        "-0-rate",
        "/rate",
        "rate.jpg",
        "review",
        "comment",
        "评价",
        "用户评价",
        "买家秀",
        "问大家",
    ])


def _looks_like_detail_image(context_text=""):
    text = str(context_text or "").lower()
    return any(marker in text for marker in [
        "detail",
        "desc",
        "description",
        "richtext",
        "图文详情",
        "宝贝详情",
        "商品详情",
        "详情",
    ])


def _is_junk_image(url, width, height, natural_width, natural_height, context_text):
    """排除非商品图：logo、头像、badge、像素点、GIF 动画、网站/店铺装饰图等。"""
    url_lower = (url or "").lower()
    text_lower = (context_text or "").lower()

    if _looks_like_platform_asset_url(url_lower):
        return True

    # URL 模式黑名单
    junk_url_patterns = [
        "sns_logo",           # 社交 logo（闲鱼等）
        "userheaderimgshow",  # 店铺头像
        "fleamarket",         # 二手市场图标
        "s.gif",              # 跟踪像素
        "-0-userheaderimg",   # 用户头像
        "avatar",             # 头像
        "/shop-logo",         # 店铺 logo 路径
        "shoplogo",           # 店铺 logo
        "shop_logo",          # 店铺 logo
        "seller_logo",        # 卖家 logo
        "sellerlogo",         # 卖家 logo
        "/assets/global/",    # 站点全局资源
        "/global/",           # 全局资源
        "tbh-logo",           # 淘宝 header logo
        "tb_logo",            # 淘宝 logo
        "taobao-logo",        # 淘宝 logo
        "taobaologo",         # 淘宝 logo
        "site-logo",          # 站点 logo
        "/shophead/",         # 店铺头部图
        "headimg",            # 头像图
        "wwc.alicdn.com",     # 旺旺/用户头像 CDN
    ]
    if any(p in url_lower for p in junk_url_patterns):
        return True

    # GIF 动画不是商品主图
    if url_lower.endswith(".gif"):
        return True

    # 已知商品图 URL 模式优先放行，不受尺寸限制
    # 懒加载图片的 naturalWidth/Height 为 0，不能用尺寸判断
    known_product_url = any(marker in url_lower for marker in [
        "item_pic", "imgextra", "descpic",
    ]) or re.search(r"img\.alicdn\.com/imgextra/", url_lower)
    if known_product_url:
        return False

    # 有效尺寸维度（取 natural 和 rendered 的最大值）
    effective_width = max(natural_width or 0, width or 0)
    effective_height = max(natural_height or 0, height or 0)
    max_dim = max(effective_width, effective_height, 1)
    min_dim = min(effective_width, effective_height) if (effective_width > 0 and effective_height > 0) else 0

    # 超大宽高比的横幅广告图（宽 > 3 * 高 或 高 > 3 * 宽）
    if min_dim > 0 and max_dim / min_dim > 3.5:
        return True

    # 太小（小于 50px 的短边大概率是 badge/icon）
    # 注意：懒加载图片尺寸可能为 0，此条件不过滤尺寸未知的图片
    if min_dim > 0 and min_dim < 50:
        return True

    # 上下文文本包含非商品关键词（含 class/id/祖先链信息）
    junk_context_keywords = [
        "logo", "icon", "badge", "标签", "标识", "头像",
        "二维码", "扫码", "qrcode",
    ]
    if any(kw in text_lower for kw in junk_context_keywords):
        return True

    # 图片位于页面导航、页头、页脚、店铺信息等非商品区域
    # 祖先链 class/id/tag 中包含这些关键词则排除
    non_product_area_keywords = [
        "site-nav", "sitenav", "siteheader", "site-header",
        "tb-head", "tbhd", "tb-header",
        "shopheader", "shop-header", "shop-info",
        "shopcard", "shop-card",
        "sellermeta", "seller-meta", "seller-info", "sellerinfo",
        "shopname", "shop-name",
        "footer", "page-footer",
        "searchbar", "search-bar",
        "topbar", "top-bar",
        "headbar", "head-bar",
    ]
    if any(kw in text_lower for kw in non_product_area_keywords):
        return True

    # 淘宝页面中 <nav> 或 <header> 标签内的图片一定不是商品图
    if " nav " in f" {text_lower} " or "<nav" in text_lower:
        pass  # tag 名已经在 ancestorHints 中以原始形式出现
    # 检测 ancestorHints 中是否包含 nav/header/footer HTML 标签
    for html_tag in ["nav", "header", "footer"]:
        # ancestorHints 中 tag 名以空格分隔出现
        if re.search(rf"(?:^|\s){html_tag}(?:\s|$)", text_lower):
            return True

    return False


def _detail_image_metrics(width, height, natural_width, natural_height, context_text):
    return {
        "width": width,
        "height": height,
        "natural_width": natural_width,
        "natural_height": natural_height,
        "context_text": context_text,
    }


def _looks_like_leading_store_logo_detail(url, metrics, main_images):
    """识别详情区开头的独立店铺 Logo，避免把店招当成详情图。"""
    if not metrics or url in main_images:
        return False
    url_lower = (url or "").lower()
    if any(marker in url_lower for marker in ["-0-item_pic", "/bao/uploaded/"]):
        return False

    effective_width = max(
        float(metrics.get("natural_width") or 0),
        float(metrics.get("width") or 0),
    )
    effective_height = max(
        float(metrics.get("natural_height") or 0),
        float(metrics.get("height") or 0),
    )
    min_dim = min(effective_width, effective_height)
    max_dim = max(effective_width, effective_height)
    if min_dim <= 0:
        return False
    near_square = max_dim / min_dim <= 1.08
    return near_square and min_dim >= 800


def _prune_leading_detail_logo_images(detail_images, metrics_by_url, main_images):
    if len(detail_images) < 2:
        return detail_images
    first = detail_images[0]
    if _looks_like_leading_store_logo_detail(first, metrics_by_url.get(first), main_images):
        return detail_images[1:]
    return detail_images


def _extract_shop_name(shop_text):
    """从店铺文本中提取店铺名称。"""
    if not shop_text:
        return "淘宝商家"
    lines = shop_text.strip().split("\n")
    for line in lines:
        line = line.strip()
        if not line:
            continue
        # 跳过纯数字评分行
        if re.match(r"^[0-9.]+\s*$", line):
            continue
        # 跳过明显的非名称行
        if any(kw in line for kw in ["好评率", "超", "同行", "满意度", "VIP"]):
            continue
        if len(line) >= 2 and len(line) <= 30:
            return line
    return "淘宝商家"


def _preload_dom_detail_area(page):
    """轻量滚动触发懒加载图片，模拟真人浏览节奏。

    不再暴力滚到底再弹回顶部（那种行为是典型的爬虫信号），
    改为只滚 2-3 屏，每屏停留足够时间让 IntersectionObserver 触发加载。
    """
    try:
        page.evaluate("""
            async () => {
                const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));
                const viewHeight = window.innerHeight || 900;

                // 先停留在顶部，确保主图区可见并开始加载
                window.scrollTo(0, 0);
                await sleep(1800 + Math.random() * 1200);

                // 只滚 2-3 屏，模拟真人快速浏览商品详情
                const maxScrolls = Math.floor(2 + Math.random() * 2);
                for (let i = 1; i <= maxScrolls; i++) {
                    const targetY = viewHeight * i * (0.7 + Math.random() * 0.3);
                    window.scrollTo(0, targetY);
                    // 每屏停留 1.5-3 秒，让图片有时间加载
                    await sleep(1500 + Math.random() * 1500);
                }

                // 不弹回顶部 — 真人看完详情图后不会瞬间跳回顶部
                // 如果爬虫真的需要主图数据，mtop 拦截已经拿到了
            }
        """)
    except Exception:
        pass


def _clean_page_title(title):
    title = (title or "").strip()
    for suffix in ["-tmall.com天猫", "-淘宝网", "-淘宝网触屏版", "-天猫Tmall.com"]:
        if suffix in title:
            title = title.split(suffix, 1)[0].strip()
    return title


def _parse_first_price(text):
    """从价格文本中提取最高价（通常为优惠前原价），用于铺货定价参考。"""
    text = text or ""
    matches = re.findall(r"￥\s*([0-9]+(?:\.[0-9]+)?)", text)
    if not matches:
        matches = re.findall(r"([0-9]+(?:\.[0-9]+)?)", text)
    values = []
    for item in matches:
        try:
            value = float(item)
            if value > 0:
                values.append(value)
        except Exception:
            continue
    if not values:
        return 9.9
    # 取最高价：优惠前原价 > 店铺优惠价 > 平台补贴价
    return max(values)


TAOBAO_PARAM_LABELS = [
    "品牌",
    "货号",
    "图案",
    "厚薄",
    "裙型",
    "领型",
    "裙长",
    "腰型",
    "风格",
    "面料",
    "适用场景",
    "材质成分",
    "适用季节",
    "安全等级",
    "款式",
    "上市年份季节",
    "袖长",
    "袖型",
    "包装种类",
    "适用性别",
    "适用年龄",
    "组合形式",
    "产地",
    "省份",
    "地市",
]

SKU_LABELS = ["颜色分类", "颜色", "身高", "尺码", "尺寸", "规格"]
SKU_VALUE_STOP_LABELS = set(SKU_LABELS + [
    "数量",
    "配送",
    "服务",
    "参数",
    "参数信息",
    "图文详情",
    "用户评价",
    "本店推荐",
    "看了又看",
])
SKU_VALUE_NOISE = {
    "切换大图模式",
    "千人加购",
    "已选",
    "有货",
    "无货",
    "领券购买",
    "收藏",
}
VALUE_BEFORE_PARAM_LABELS = {"图案", "厚薄", "裙型", "领型", "裙长", "腰型"}


def _clean_dom_text_value(value):
    value = re.sub(r"\s+", " ", str(value or "")).strip()
    value = value.strip("：:｜|")
    return value


def _is_useful_dom_value(value):
    value = _clean_dom_text_value(value)
    if not value or value in SKU_VALUE_NOISE:
        return False
    if len(value) > 60:
        return False
    if re.fullmatch(r"[\ue000-\uf8ff\s]+", value):
        return False
    return True


def _is_plausible_param_value(label, value):
    _ = label
    value = _clean_dom_text_value(value)
    return _is_useful_dom_value(value)


def _item_params_quality_report(params, source="dom"):
    """评估提取参数的质量，返回质量报告。"""
    params = params or {}
    if not params:
        return {"parser": source, "trusted": False, "suspicious_keys": [], "reason": "no_params"}

    # 检查值是否看起来合理（不是其他字段的值错位）
    suspicious = []
    plausible_fabric = {"棉", "纯棉", "涤纶", "聚酯纤维", "锦纶", "氨纶", "丝绸", "亚麻", "雪纺", "牛仔", "棉麻", "冰丝"}
    plausible_age = {"通用", "3周岁以下", "3周岁以上", "6周岁以上", "8周岁以上", "14周岁以上"}
    plausible_safety = {"A类", "B类", "C类"}

    fabric = params.get("面料", "")
    if fabric and not any(f in fabric for f in plausible_fabric) and len(fabric) > 10:
        suspicious.append("面料")

    age = params.get("适用年龄", "")
    if age and age not in plausible_age and not any(a in age for a in ["岁", "月", "年", "通用"]):
        suspicious.append("适用年龄")

    style = params.get("风格", "")
    if style and any(c.isdigit() for c in style):
        suspicious.append("风格")  # 风格不应包含数字

    return {
        "parser": source,
        "trusted": len(suspicious) == 0,
        "suspicious_keys": suspicious,
    }


def _extract_sku_options_from_text_nodes(text_nodes):
    options = {}
    cleaned_nodes = [_clean_dom_text_value(item) for item in text_nodes if _clean_dom_text_value(item)]
    for idx, node in enumerate(cleaned_nodes):
        if node not in SKU_LABELS:
            continue
        values = []
        for child in cleaned_nodes[idx + 1:idx + 24]:
            if child in SKU_VALUE_STOP_LABELS:
                break
            if not _is_useful_dom_value(child):
                continue
            if child not in values:
                values.append(child)
        if values:
            options[node] = values
    return options


def _extract_params_from_text_nodes(text_nodes):
    params = {}
    labels = set(TAOBAO_PARAM_LABELS)
    cleaned_nodes = [_clean_dom_text_value(item) for item in text_nodes if _clean_dom_text_value(item)]
    for idx, node in enumerate(cleaned_nodes):
        if node not in labels:
            continue

        previous_value = cleaned_nodes[idx - 1] if idx > 0 else ""
        next_value = cleaned_nodes[idx + 1] if idx + 1 < len(cleaned_nodes) else ""
        candidates = []
        if node in VALUE_BEFORE_PARAM_LABELS:
            candidates.extend([previous_value, next_value])
        else:
            candidates.extend([next_value, previous_value])
        for value in candidates:
            if value and value not in labels and _is_plausible_param_value(node, value):
                params[node] = value
                break

    joined_text = " ".join(cleaned_nodes)
    for label in TAOBAO_PARAM_LABELS:
        if label in params:
            continue
        next_labels = "|".join(re.escape(item) for item in TAOBAO_PARAM_LABELS if item != label)
        pattern = rf"{re.escape(label)}\s+(.+?)(?=\s+(?:{next_labels})\s+|$)"
        match = re.search(pattern, joined_text)
        if match:
            value = _clean_dom_text_value(match.group(1))
            if value and len(value) <= 120 and _is_plausible_param_value(label, value):
                params[label] = value
    return params


def _build_skus_from_options(sku_options, price, has_stock=True, stock_quantity=None):
    option_items = [
        (name, values)
        for name, values in sku_options.items()
        if values and name in SKU_LABELS
    ]
    if stock_quantity and stock_quantity > 0:
        default_stock = stock_quantity
    elif has_stock:
        default_stock = 100
    else:
        default_stock = 0

    if not option_items:
        return [{
            "external_sku_id": "default",
            "specs": {"规格": "默认规格"},
            "cost_price": price,
            "stock": default_stock,
        }]

    skus = []
    combinations = [({}, "")]
    for name, values in option_items:
        next_combinations = []
        for specs, sku_id_prefix in combinations:
            for value in values:
                next_specs = dict(specs)
                next_specs[name] = value
                next_id = f"{sku_id_prefix};{name}={value}" if sku_id_prefix else f"{name}={value}"
                next_combinations.append((next_specs, next_id))
        combinations = next_combinations
        if len(combinations) > 60:
            break

    for specs, sku_id in combinations[:60]:
        skus.append({
            "external_sku_id": sku_id,
            "specs": specs,
            "cost_price": price,
            "stock": default_stock,
        })
    return skus


def _detect_stock_from_text_nodes(text_nodes):
    """从 DOM 文本节点中检测库存状态与数量。

    新版淘宝详情页可能不再显式展示「有货」文本，因此优先从「库存」数字
    判读，兜底时默认认为有货（避免大量误报为 0）。
    """
    has_out_of_stock = False
    quantity = None

    for item in text_nodes:
        cleaned = _clean_dom_text_value(item)

        # 明确无货 / 下架信号
        if cleaned in ("无货", "已售罄", "下架", "已下架") or "已售罄" in cleaned or "已下架" in cleaned:
            has_out_of_stock = True
            continue

        if "有货" in cleaned:
            if quantity is None:
                m = re.search(r'(\d+)', cleaned)
                if m:
                    quantity = max(quantity or 0, int(m.group(1)))

        # 提取库存数量：「库存 XXX」「库存:XXX」「库存XXX件」
        m = re.search(r'库存\s*[:：]?\s*(\d+)', cleaned)
        if m:
            qty = int(m.group(1))
            if qty > 0:
                quantity = max(quantity or 0, qty)

    if has_out_of_stock and quantity is None:
        return False, 0

    return True, quantity


def _infer_category_hint(title, params):
    _ = (title, params)
    return None


def _extract_product_from_dom(page, source_url, preload_detail=True):
    """从真实商品页 DOM 兜底提取标题、主图和价格。"""
    if preload_detail:
        _preload_dom_detail_area(page)
    try:
        data = page.evaluate("""
            () => {
                const textOf = (selectors) => {
                    for (const selector of selectors) {
                        const node = document.querySelector(selector);
                        const text = node && (node.innerText || node.textContent || "").trim();
                        if (text) return text;
                    }
                    return "";
                };
                const attrOf = (selectors, attr) => {
                    for (const selector of selectors) {
                        const node = document.querySelector(selector);
                        const value = node && node.getAttribute(attr);
                        if (value) return value;
                    }
                    return "";
                };
                // 收集祖先链上所有能暗示区域用途的 class/id/tag
                const collectAncestorHints = (el) => {
                    const hints = [];
                    let current = el?.parentElement;
                    while (current && current !== document.body) {
                        const tag = current.tagName?.toLowerCase() || '';
                        if (tag === 'nav' || tag === 'header' || tag === 'footer') {
                            hints.push(tag);
                        }
                        const cls = (current.className || '');
                        const id = (current.id || '');
                        if (cls) hints.push(cls);
                        if (id) hints.push(id);
                        current = current.parentElement;
                    }
                    return hints.join(' ');
                };
                // 收集 <img> 标签图片（含懒加载属性）
                const imgElements = Array.from(document.querySelectorAll('img')).map((img) => {
                    const rect = img.getBoundingClientRect();
                    const ancestor = img.closest('section, article, li, ul, div');
                    const contextText = ancestor
                        ? (ancestor.innerText || ancestor.textContent || "").replace(/\\s+/g, " ").trim().slice(0, 160)
                        : "";
                    const classText = [
                        img.className || "",
                        img.parentElement?.className || "",
                        ancestor?.className || "",
                        ancestor?.id || ""
                    ].join(" ");
                    const ancestorHints = collectAncestorHints(img);
                    // 优先 currentSrc（已加载的），再取各种懒加载属性
                    const src = img.currentSrc
                        || img.src
                        || img.getAttribute('data-src')
                        || img.getAttribute('data-ks-lazyload')
                        || img.getAttribute('data-lazyload')
                        || img.getAttribute('data-original')
                        || "";
                    return {
                        src,
                        naturalWidth: img.naturalWidth || 0,
                        naturalHeight: img.naturalHeight || 0,
                        width: rect.width || 0,
                        height: rect.height || 0,
                        top: rect.top + window.scrollY,
                        alt: img.alt || "",
                        contextText,
                        classText,
                        ancestorHints
                    };
                });

                // 收集 CSS 背景图（新版淘宝主图轮播可能用背景图）
                const bgImages = [];
                const bgCandidates = document.querySelectorAll(
                    '[class*="PicGallery"] [style*="background"], ' +
                    '[class*="pic-gallery"] [style*="background"], ' +
                    '[class*="mainPic"] [style*="background"], ' +
                    '[class*="ItemImage"] [style*="background"], ' +
                    '[class*="slider"] [style*="background"]'
                );
                for (const el of bgCandidates) {
                    const style = window.getComputedStyle(el);
                    const bgImg = style.backgroundImage || '';
                    const match = bgImg.match(/url[(]["']?([^"')]+)["']?[)]/);
                    if (match && match[1] && match[1].includes('alicdn.com')) {
                        const rect = el.getBoundingClientRect();
                        const ancestor = el.closest('section, article, li, ul, div');
                        bgImages.push({
                            src: match[1],
                            naturalWidth: 0,
                            naturalHeight: 0,
                            width: rect.width || 0,
                            height: rect.height || 0,
                            top: rect.top + window.scrollY,
                            alt: '',
                            contextText: ancestor ? (ancestor.className || '') : '',
                            classText: (el.className || '') + ' ' + (el.parentElement?.className || '')
                        });
                    }
                }

                const imgs = [...bgImages, ...imgElements];
                const textNodes = [];
                const walker = document.createTreeWalker(
                    document.body,
                    NodeFilter.SHOW_TEXT,
                    {
                        acceptNode(node) {
                            const text = (node.textContent || "").replace(/\\s+/g, " ").trim();
                            if (!text) return NodeFilter.FILTER_REJECT;
                            const parent = node.parentElement;
                            if (!parent) return NodeFilter.FILTER_REJECT;
                            const style = window.getComputedStyle(parent);
                            if (style.display === "none" || style.visibility === "hidden") {
                                return NodeFilter.FILTER_REJECT;
                            }
                            return NodeFilter.FILTER_ACCEPT;
                        }
                    }
                );
                while (textNodes.length < 500) {
                    const node = walker.nextNode();
                    if (!node) break;
                    textNodes.push((node.textContent || "").replace(/\\s+/g, " ").trim());
                }
                return {
                    documentTitle: document.title || "",
                    ogTitle: attrOf(['meta[property="og:title"]', 'meta[name="title"]'], 'content'),
                    titleText: textOf([
                        '.tb-main-title',
                        '[class*="ItemHeader"] [class*="title"]',
                        '[class*="ItemTitle--mainTitle"]',
                        '[class*="mainTitle"]',
                        'h1'
                    ]),
                    priceText: textOf([
                        '.tb-rmb-num',
                        '.tm-price',
                        '[class*="Price--priceText"]',
                        '[class*="priceText"]',
                        '[class*="price"]'
                    ]),
                    shopText: textOf([
                        '[class*="ShopHeader"]',
                        '[class*="shopName"]',
                        '[class*="Shop"] a',
                        'a[href*="shop"]'
                    ]),
                    textNodes,
                    images: imgs
                };
            }
        """)
    except Exception:
        data = {}

    title = (
        _clean_page_title(data.get("ogTitle"))
        or _clean_page_title(data.get("titleText"))
        or _clean_page_title(data.get("documentTitle"))
        or "淘宝未命名商品"
    )
    price = _parse_first_price(data.get("priceText"))
    text_nodes = data.get("textNodes") or []
    sku_options = _extract_sku_options_from_text_nodes(text_nodes)
    item_params = _extract_params_from_text_nodes(text_nodes)
    item_params_quality = _item_params_quality_report(item_params)
    brand_hint = item_params.get("品牌") or "无品牌"
    has_stock, stock_quantity = _detect_stock_from_text_nodes(text_nodes)
    skus = _build_skus_from_options(sku_options, price, has_stock=has_stock, stock_quantity=stock_quantity)

    images = []
    detail_images = []
    seen_images = set()
    seen_detail_images = set()
    detail_image_metrics = {}
    for item in data.get("images") or []:
        raw_url = item.get("src") if isinstance(item, dict) else ""
        url = _normalize_taobao_image_url(raw_url)
        if not url:
            continue
        if not _is_taobao_image_host(url):
            continue
        width = float(item.get("width") or 0)
        height = float(item.get("height") or 0)
        natural_width = float(item.get("naturalWidth") or 0)
        natural_height = float(item.get("naturalHeight") or 0)
        context_text = " ".join([
            str(item.get("alt") or ""),
            str(item.get("contextText") or ""),
            str(item.get("classText") or ""),
            str(item.get("ancestorHints") or ""),
        ])

        # 排除已知的垃圾图片
        if _is_junk_image(url, width, height, natural_width, natural_height, context_text):
            continue
        if _looks_like_review_image(url, context_text):
            continue

        url_lower = (url or "").lower()
        effective_width = max(width, natural_width)
        effective_height = max(height, natural_height)
        looks_large = effective_width >= 200 and effective_height >= 200
        looks_product = any(marker in url_lower for marker in ["item_pic", "imgextra"])
        # 懒加载图片尺寸可能为 0，但 URL 包含商品图特征时仍视为有效
        dimensions_unknown = (effective_width == 0 and effective_height == 0)
        is_alicdn_product_path = bool(
            re.search(r"img\.alicdn\.com/imgextra/", url_lower)
            or re.search(r"img\.alicdn\.com/bao/uploaded/", url_lower)
        )

        # 跳过：尺寸已知但太小，且不是已知商品图 URL
        if not looks_large and not dimensions_unknown and not looks_product:
            continue

        if _looks_like_detail_image(context_text):
            detail_image_metrics[url] = _detail_image_metrics(
                width,
                height,
                natural_width,
                natural_height,
                context_text,
            )
            if _append_unique_image(detail_images, seen_detail_images, url, limit=60):
                continue
            continue

        # 主图分类：已知商品图 URL、大尺寸、或懒加载的 alicdn 商品路径
        if looks_product or natural_width >= 400 or (dimensions_unknown and is_alicdn_product_path):
            if _append_unique_image(images, seen_images, url, limit=12):
                break
        elif looks_large or dimensions_unknown:
            # 尺寸足够大或尺寸未知的 alicdn 图归入详情图
            detail_image_metrics[url] = _detail_image_metrics(
                width,
                height,
                natural_width,
                natural_height,
                context_text,
            )
            if _append_unique_image(detail_images, seen_detail_images, url, limit=60):
                continue

    # 兜底：如果主图和详情图都为空，用更宽松的策略再扫一遍
    if not images and not detail_images:
        for item in data.get("images") or []:
            raw_url = item.get("src") if isinstance(item, dict) else ""
            url = _normalize_taobao_image_url(raw_url)
            if not url or not _is_taobao_image_host(url):
                continue
            width = float(item.get("width") or 0)
            height = float(item.get("height") or 0)
            natural_width = float(item.get("naturalWidth") or 0)
            natural_height = float(item.get("naturalHeight") or 0)
            effective_width = max(width, natural_width)
            effective_height = max(height, natural_height)
            url_lower = (url or "").lower()
            # 放宽条件：alicdn 图且（尺寸 >= 100 或尺寸未知）
            if url_lower.endswith(".gif"):
                continue
            is_big_enough = (effective_width >= 100 and effective_height >= 100)
            is_unknown_size = (effective_width == 0 and effective_height == 0)
            if is_big_enough or is_unknown_size:
                if _append_unique_image(images, seen_images, url, limit=12):
                    break

    detail_images = _prune_leading_detail_logo_images(
        detail_images,
        detail_image_metrics,
        set(images),
    )

    shop_text = data.get("shopText") or ""
    return {
        "external_product_id": source_url,
        "title": title,
        "source_url": source_url,
        "images": images,
        "detail_images": detail_images,
        "skus": skus,
        "supplier_name": _extract_shop_name(shop_text),
        "supplier_product_id": source_url,
        "category_hint": _infer_category_hint(title, item_params),
        "brand_hint": brand_hint,
        "weight_gram": 500,
        "metadata": {
            "collection_source": "dom_fallback",
            "taobao_item_params": item_params,
            "taobao_item_params_quality": item_params_quality,
            "taobao_sku_options": sku_options,
            "taobao_price_text": data.get("priceText"),
            "taobao_shop_text": shop_text,
        },
    }


def _is_empty_collected_product(product):
    return (
        not product.get("images")
        and not product.get("detail_images")
        and product.get("title") == "淘宝未命名商品"
        and len(product.get("skus") or []) == 1
        and (product.get("skus") or [{}])[0].get("external_sku_id") == "default"
    )


def _needs_dom_enrichment(product):
    metadata = product.get("metadata") or {}
    skus = product.get("skus") or []
    has_only_default_sku = (
        len(skus) == 1
        and (skus[0] or {}).get("external_sku_id") == "default"
    )
    return metadata.get("collection_source") == "mtop_container_flexible" or has_only_default_sku


def _merge_product_with_dom(primary, dom_product):
    """用 DOM 兜底结果补齐容器化 mtop 未暴露的 SKU 与参数。"""
    if not dom_product:
        return primary
    merged = dict(primary)
    if not merged.get("title") or merged.get("title") == "淘宝未命名商品":
        merged["title"] = dom_product.get("title") or merged.get("title")
    if not merged.get("images"):
        merged["images"] = dom_product.get("images") or []
    if not merged.get("detail_images"):
        merged["detail_images"] = dom_product.get("detail_images") or []

    primary_skus = merged.get("skus") or []
    dom_skus = dom_product.get("skus") or []
    if (
        dom_skus
        and len(primary_skus) == 1
        and (primary_skus[0] or {}).get("external_sku_id") == "default"
        and len(dom_skus) > 1
    ):
        merged["skus"] = dom_skus

    if not merged.get("category_hint") and dom_product.get("category_hint"):
        merged["category_hint"] = dom_product.get("category_hint")
    if (not merged.get("brand_hint") or merged.get("brand_hint") == "无品牌") and dom_product.get("brand_hint"):
        merged["brand_hint"] = dom_product.get("brand_hint")

    metadata = {}
    if isinstance(primary.get("metadata"), dict):
        metadata.update(primary.get("metadata") or {})
    if isinstance(dom_product.get("metadata"), dict):
        dom_meta = dom_product.get("metadata") or {}
        # 合并商品属性：mtop 结构化数据优先，DOM 数据补充缺失字段
        mtop_params = metadata.get("taobao_item_params") or {}
        dom_params = dom_meta.get("taobao_item_params") or {}
        merged_params = _merge_item_params(mtop_params, dom_params)
        if merged_params:
            metadata["taobao_item_params"] = merged_params
            metadata["taobao_item_params_quality"] = _item_params_quality_report(
                merged_params, "mtop_dom_merged"
            )
        # 其他 DOM 元数据补充（不覆盖已有的 mtop 数据）
        for key, value in dom_meta.items():
            if key not in ("taobao_item_params", "taobao_item_params_quality"):
                if key not in metadata:
                    metadata[key] = value
    if metadata:
        metadata.setdefault("collection_source", "mtop_dom_enriched")
        merged["metadata"] = metadata
    return merged


def _summarize_json_shape(value, max_depth=4, max_list=2, max_keys=20):
    """输出响应结构摘要，避免调试时打印完整大响应。"""
    if max_depth <= 0:
        return type(value).__name__
    if isinstance(value, dict):
        result = {}
        for idx, (key, child) in enumerate(value.items()):
            if idx >= max_keys:
                result["..."] = f"{len(value) - max_keys} more keys"
                break
            result[str(key)] = _summarize_json_shape(child, max_depth - 1, max_list, max_keys)
        return result
    if isinstance(value, list):
        return [
            _summarize_json_shape(child, max_depth - 1, max_list, max_keys)
            for child in value[:max_list]
        ] + ([f"... {len(value) - max_list} more items"] if len(value) > max_list else [])
    if isinstance(value, str):
        return value[:120] + ("..." if len(value) > 120 else "")
    return value


def _summarize_page_dom(page):
    try:
        return page.evaluate("""
            () => {
                const pickText = (selectors) => {
                    for (const selector of selectors) {
                        const node = document.querySelector(selector);
                        const text = node && (node.innerText || node.textContent || "").trim();
                        if (text) return { selector, text: text.slice(0, 160) };
                    }
                    return null;
                };
                const imgs = Array.from(document.querySelectorAll('img'))
                    .map((img) => img.currentSrc || img.src || img.getAttribute('data-src') || img.getAttribute('data-ks-lazyload'))
                    .filter(Boolean)
                    .filter((src) => /alicdn|tbcdn|taobaocdn/.test(src))
                    .slice(0, 20);
                return {
                    url: location.href,
                    title: document.title,
                    titleNode: pickText([
                        '.tb-main-title',
                        '.ItemTitle--mainTitle--',
                        '[class*="ItemTitle"]',
                        '[class*="item-title"]',
                        'h1'
                    ]),
                    priceNode: pickText([
                        '.tb-rmb-num',
                        '.tm-price',
                        '[class*="Price--priceText"]',
                        '[class*="priceText"]',
                        '[class*="price"]'
                    ]),
                    imageCandidates: imgs
                };
            }
        """)
    except Exception as e:
        return {"error": str(e)}


# ======================== 主功能 ========================

def _matched_login_cookie_names_from_context(ctx):
    try:
        cookies = ctx.cookies()
    except Exception:
        try:
            cookies = ctx.cookies(["https://www.taobao.com", "https://login.taobao.com"])
        except Exception:
            cookies = []

    matched = []
    for c in cookies or []:
        name = c.get("name") if isinstance(c, dict) else getattr(c, "name", None)
        domain = c.get("domain", "") if isinstance(c, dict) else getattr(c, "domain", "") or ""
        value = c.get("value", "") if isinstance(c, dict) else getattr(c, "value", "") or ""
        if not name or not value:
            continue
        if "taobao.com" not in domain and "tmall.com" not in domain:
            continue
        if name in TAOBAO_LOGIN_COOKIE_NAMES:
            matched.append(name)
    return sorted(set(matched))


def run_login(profile_dir):
    """拉起可见浏览器，让用户手动登录淘宝"""
    print(f"启动淘宝登录窗口，数据保存在: {profile_dir} ...", file=sys.stderr)
    os.makedirs(profile_dir, exist_ok=True)

    ctx = None
    login_detected = False
    login_reported = False
    try:
        _check_access_limit_cooldown(profile_dir)
        _check_captcha_failure_cooldown(profile_dir)
        ctx, page = launch_browser(profile_dir, headless=False)
        page.goto("https://login.taobao.com/", timeout=60000)
        print("请在浏览器中完成淘宝登录（扫码或密码）。出现「快速进入」时会自动点击；检测到登录态后窗口会继续保留，请确认登录完成后手动关闭。", file=sys.stderr)

        while True:
            time.sleep(1)
            try:
                current_pages = ctx.pages if hasattr(ctx, "pages") else []
            except Exception:
                break
            if not current_pages:
                break
            alive = False
            for p in current_pages:
                try:
                    if p.is_closed():
                        continue
                    _handle_login_confirm(p)
                    p.evaluate("1")
                    alive = True
                    matched = _matched_login_cookie_names_from_context(ctx)
                    if matched:
                        login_detected = True
                        if not login_reported:
                            print(
                                f"已检测到淘宝登录态（匹配 cookie: {', '.join(matched)}）。请在浏览器中确认账号状态，完成后手动关闭窗口。",
                                file=sys.stderr,
                            )
                            login_reported = True
                    break
                except Exception:
                    continue
            if not alive:
                break

        if login_detected:
            # 登录成功即代表账号已恢复可用：清除此前因短信/人脸/扫码验证或访问限制写下的本地冷却，
            # 否则后续采集会被入口冷却预检直接拦截（连浏览器都不打开）→ 表现为"登录后重试无效"
            _clear_captcha_failure_cooldown(profile_dir)
            _clear_access_limit_cooldown(profile_dir)
            print("淘宝登录会话已保存，已清除本地采集冷却。", file=sys.stderr)
        else:
            print("淘宝登录窗口已关闭，未检测到有效登录态。", file=sys.stderr)
            print(json.dumps({"error": "淘宝登录窗口已关闭，未检测到有效登录态。"}, ensure_ascii=False))
            sys.exit(1)
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))
        print(f"登录过程出错: {e}", file=sys.stderr)
        sys.exit(1)
    finally:
        close_browser_context(ctx)


def run_check_login(profile_dir):
    """检测当前 profile 是否含有效淘宝登录态"""
    os.makedirs(profile_dir, exist_ok=True)

    db_matched = _read_login_cookie_names_from_profile(profile_dir, TAOBAO_LOGIN_COOKIE_NAMES)
    if db_matched is not None:
        _print_login_state(db_matched, profile_dir=profile_dir)
        sys.exit(0)

    # 冷却期内 check-login 只返回本地状态，避免为了探测登录态启动隐藏浏览器。
    access_state = _access_limit_cooldown_state(profile_dir)
    captcha_state = _captcha_failure_cooldown_state(profile_dir)
    if access_state["active"] or captcha_state["active"]:
        _print_login_state([], profile_dir=profile_dir)
        sys.exit(0)

    ctx = None
    try:
        ctx, _page = launch_browser(profile_dir, headless=True)
        try:
            matched = _matched_login_cookie_names_from_context(ctx)
        finally:
            close_browser_context(ctx)
            ctx = None

        _print_login_state(matched, profile_dir=profile_dir)
        sys.exit(0)
    except Exception as e:
        print(json.dumps({"error": f"登录态检测失败: {e}"}, ensure_ascii=False))
        sys.exit(1)
    finally:
        close_browser_context(ctx)


def run_probe_cloak_profile(profile_dir):
    """隐藏命令：在独立进程里探测 CloakBrowser profile 是否可启动。"""
    ctx = None
    try:
        ctx, page = _launch_cloakbrowser_once(profile_dir, headless=True)
        print(json.dumps({"ok": True, "url": getattr(page, "url", "")}, ensure_ascii=False))
        sys.exit(0)
    except Exception as e:
        print(json.dumps({
            "ok": False,
            "profile_crash": _is_cloakbrowser_profile_crash(e),
            "error": str(e).splitlines()[0] if str(e) else e.__class__.__name__,
        }, ensure_ascii=False))
        sys.exit(75 if _is_cloakbrowser_profile_crash(e) else 1)
    finally:
        close_browser_context(ctx)


def _read_login_cookie_names_from_profile(profile_dir, login_cookie_names):
    """直接读取 Chromium Cookie 库中的 cookie 名称，避免为检测登录态启动浏览器。"""
    candidates = [
        os.path.join(profile_dir, "Default", "Cookies"),
        os.path.join(profile_dir, "Default", "Network", "Cookies"),
        os.path.join(profile_dir, "Profile 1", "Cookies"),
        os.path.join(profile_dir, "Profile 1", "Network", "Cookies"),
        os.path.join(profile_dir, "Cookies"),
        os.path.join(profile_dir, "Network", "Cookies"),
    ]

    existing = [path for path in candidates if os.path.exists(path)]
    if not existing:
        return []

    matched = []
    for db_path in existing:
        try:
            conn = sqlite3.connect(f"file:{db_path}?mode=ro&immutable=1", uri=True, timeout=1)
            try:
                rows = conn.execute(
                    "SELECT host_key, name, value, encrypted_value FROM cookies"
                ).fetchall()
            finally:
                conn.close()
        except sqlite3.DatabaseError:
            continue
        except Exception:
            continue

        for host_key, name, value, encrypted_value in rows:
            domain = host_key or ""
            if "taobao.com" not in domain and "tmall.com" not in domain:
                continue
            has_value = bool(value) or bool(encrypted_value)
            if name in login_cookie_names and has_value:
                matched.append(name)

    return sorted(set(matched))


def _print_login_state(matched, profile_dir=None):
    logged_in = len(matched) > 0
    detail = (
        f"已检测到有效的淘宝登录态（匹配 cookie: {', '.join(sorted(set(matched)))}）。"
        if logged_in
        else "未在 profile 中检测到淘宝登录 cookie，请先完成登录。"
    )
    cooldown_state = (
        _access_limit_cooldown_state(profile_dir)
        if profile_dir
        else {"active": False, "cooldown": None, "message": None}
    )
    captcha_cooldown_state = (
        _captcha_failure_cooldown_state(profile_dir)
        if profile_dir
        else {"active": False, "cooldown": None, "message": None}
    )
    print(json.dumps(
        {
            "logged_in": logged_in,
            "detail": detail,
            "matched_cookies": sorted(set(matched)),
            "access_limited": bool(cooldown_state["active"]),
            "access_limit_message": cooldown_state["message"],
            "access_limit_cooldown": cooldown_state["cooldown"],
            "captcha_cooling": bool(captcha_cooldown_state["active"]),
            "captcha_cooling_message": captcha_cooldown_state["message"],
            "captcha_failure_cooldown": captcha_cooldown_state["cooldown"],
        },
        ensure_ascii=False,
    ))


def run_collect(url, profile_dir, headed=False, debug_captured=False):
    """单次采集：启动浏览器 → 预热 → 采集 → 关闭"""
    os.makedirs(profile_dir, exist_ok=True)
    ctx = None
    try:
        _check_access_limit_cooldown(profile_dir)
        _check_captcha_failure_cooldown(profile_dir)
        ctx, page = launch_browser(profile_dir, headless=not headed)
        warm_up_taobao_home(page)
        parsed_product = _collect_single_product(ctx, page, url, profile_dir)
        if debug_captured:
            print(json.dumps({
                "captured": _summarize_json_shape({}),
                "dom": _summarize_page_dom(page),
            }, ensure_ascii=False, indent=2), file=sys.stderr)
        print(json.dumps(parsed_product, ensure_ascii=False))
        sys.exit(0)
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False))
        sys.exit(1)
    finally:
        close_browser_context(ctx)


def run_batch_collect(urls, profile_dir, headed=False):
    """批量采集：启动浏览器一次 → 预热一次 → 依次采集所有 URL → 关闭。

    url 列表中的每个元素可以是字符串 URL，或 {"url": "...", "id": "..."} 对象。
    输出为 NDJSON，每行一个采集结果（含 id 字段用于关联回原始任务）。
    """
    os.makedirs(profile_dir, exist_ok=True)

    results = []
    ctx = None
    try:
        _check_access_limit_cooldown(profile_dir)
        _check_captcha_failure_cooldown(profile_dir)
        ctx, page = launch_browser(profile_dir, headless=not headed)
        # 每批开始预热一次首页，建立正常浏览轨迹，降低"直达详情页"的爬虫特征。
        # 仅一次开销（~5s/批）即可显著降险；如需跳过可设 WX_XD_TAOBAO_SKIP_WARMUP=1。
        warm_up_taobao_home(page)

        delay_min = _env_float_range(BATCH_DELAY_MIN_ENV, 8.0, min_value=3.0, max_value=30.0)
        delay_max = _env_float_range(BATCH_DELAY_MAX_ENV, 25.0, min_value=5.0, max_value=60.0)
        if delay_max < delay_min:
            delay_max = delay_min

        # 长停顿节奏（可配）：累计若干商品后按概率插入一次长停顿，模拟真人"歇一会"。
        # 默认更短(60-180s)且概率触发(0.65)，在保留人类节奏特征的同时削减纯等待冗余。
        long_pause_min = _env_float_range(LONG_PAUSE_MIN_ENV, 60.0, min_value=10.0, max_value=600.0)
        long_pause_max = _env_float_range(LONG_PAUSE_MAX_ENV, 180.0, min_value=10.0, max_value=900.0)
        if long_pause_max < long_pause_min:
            long_pause_max = long_pause_min
        pause_every_min = _env_int_range(LONG_PAUSE_EVERY_MIN_ENV, 8, min_value=1, max_value=100)
        pause_every_max = _env_int_range(LONG_PAUSE_EVERY_MAX_ENV, 12, min_value=1, max_value=100)
        if pause_every_max < pause_every_min:
            pause_every_max = pause_every_min
        pause_probability = _env_float_range(
            LONG_PAUSE_PROBABILITY_ENV, 0.65, min_value=0.0, max_value=1.0
        )
        long_pause_every = random.randint(pause_every_min, pause_every_max)
        items_since_pause = 0

        for i, item in enumerate(urls):
            if isinstance(item, dict):
                url = item.get("url", "")
                task_id = item.get("id")
            else:
                url = str(item)
                task_id = None

            if not url:
                result = {"id": task_id, "url": "", "success": False, "error": "空 URL"}
                results.append(result)
                print(json.dumps(result, ensure_ascii=False), flush=True)
                continue

            print(json.dumps({
                "id": task_id,
                "url": url,
                "event": "start",
                "index": i + 1,
                "total": len(urls),
            }, ensure_ascii=False), flush=True)

            # 商品间随机间隔，模拟正常浏览节奏
            if i > 0:
                delay = random.uniform(delay_min, delay_max)
                # 用 Gamma 分布替代均匀分布，更接近真人节奏（大部分偏快，偶尔很慢）
                if random.random() < 0.25:
                    delay = random.gammavariate(2.0, delay_max / 4.0)
                time.sleep(delay)

            # 长停顿模拟：累计到阈值后按概率歇一会（更像真人，而非每隔固定个数硬停）。
            items_since_pause += 1
            if items_since_pause >= long_pause_every and i < len(urls) - 1:
                if pause_probability > 0 and random.random() < pause_probability:
                    pause_secs = random.uniform(long_pause_min, long_pause_max)
                    print(
                        f"已采集 {items_since_pause} 个商品，进入长停顿 {pause_secs:.0f} 秒……",
                        file=sys.stderr,
                    )
                    time.sleep(pause_secs)
                # 无论本次是否真停，都重置计数并重新随机下次阈值，避免每个商品都判定。
                items_since_pause = 0
                long_pause_every = random.randint(pause_every_min, pause_every_max)

            try:
                parsed = _collect_single_product(ctx, page, url, profile_dir)
                result = {"id": task_id, "url": url, "success": True, "data": parsed}
            except Exception as e:
                error_msg = str(e)
                result = {"id": task_id, "url": url, "success": False, "error": error_msg}
                # 保护性错误立即终止整批采集
                if is_taobao_collection_protection_error(error_msg):
                    results.append(result)
                    print(json.dumps(result, ensure_ascii=False), flush=True)
                    print(json.dumps(result, ensure_ascii=False), file=sys.stderr, flush=True)
                    break

            results.append(result)
            print(json.dumps(result, ensure_ascii=False), flush=True)

        sys.exit(0)
    except Exception as e:
        fatal = {"fatal_error": str(e)}
        results.append(fatal)
        print(json.dumps(fatal, ensure_ascii=False), flush=True)
        sys.exit(1)
    finally:
        close_browser_context(ctx)


def _navigate_to_product_detail(page, url):
    """导航到商品详情页并模拟交互，用于 _capture_product_data 的 trigger 回调。

    这是首次进入详情页，响应监听器已预先安装，mtop 请求会在页面加载和后续
    交互中自然触发并被捕获。
    """
    profile_dir = _profile_dir_from_page(page)
    page.goto(url, timeout=30000, wait_until="domcontentloaded")
    _wait_for_page_stable(page)
    _raise_if_access_limited(page, "首次打开商品页", profile_dir=profile_dir)
    _handle_login_confirm(page)
    _raise_if_access_limited(page, "确认登录后", profile_dir=profile_dir)

    if _is_blocked(page):
        return False

    # 随机化浏览行为，触发更多 mtop 请求
    _random_browse_interaction(page)

    return True


def run_access_limit_state(profile_dir):
    """只查询本地访问限制冷却状态，不访问淘宝。"""
    os.makedirs(profile_dir, exist_ok=True)
    state = _access_limit_cooldown_state(profile_dir)
    captcha_state = _captcha_failure_cooldown_state(profile_dir)
    print(json.dumps({
        "access_limited": bool(state["active"]),
        "message": state["message"],
        "cooldown": state["cooldown"],
        "captcha_cooling": bool(captcha_state["active"]),
        "captcha_cooling_message": captcha_state["message"],
        "captcha_failure_cooldown": captcha_state["cooldown"],
    }, ensure_ascii=False))
    sys.exit(0)


def run_clear_access_limit(profile_dir):
    """清除本地访问限制冷却记录，不访问淘宝。"""
    os.makedirs(profile_dir, exist_ok=True)
    _clear_access_limit_cooldown(profile_dir)
    _clear_captcha_failure_cooldown(profile_dir)
    state = _access_limit_cooldown_state(profile_dir)
    captcha_state = _captcha_failure_cooldown_state(profile_dir)
    print(json.dumps({
        "cleared": True,
        "access_limited": bool(state["active"]),
        "message": state["message"],
        "cooldown": state["cooldown"],
        "captcha_cooling": bool(captcha_state["active"]),
        "captcha_cooling_message": captcha_state["message"],
        "captcha_failure_cooldown": captcha_state["cooldown"],
    }, ensure_ascii=False))
    sys.exit(0)


def _parse_resume_at_arg(value):
    parsed = _parse_shanghai_time(value)
    if parsed:
        return parsed

    match = re.match(r"^(\d{4})-(\d{2})-(\d{2})\s+(\d{1,2})$", value or "")
    if match:
        year, month, day, hour = (int(part) for part in match.groups())
        try:
            return datetime(year, month, day, hour, 0, 0, tzinfo=SHANGHAI_TZ)
        except ValueError:
            return None

    return None


def run_mark_access_limited(profile_dir, resume_at, reason):
    """手动写入本地访问限制冷却记录，不访问淘宝。"""
    os.makedirs(profile_dir, exist_ok=True)
    parsed_resume_at = _parse_resume_at_arg(resume_at) if resume_at else None
    if resume_at and not parsed_resume_at:
        print(json.dumps({
            "error": "恢复时间格式不正确，请使用 'YYYY-MM-DD HH' 或 'YYYY-MM-DD HH:MM:SS+08:00'。",
        }, ensure_ascii=False))
        sys.exit(1)

    reason_text = reason or "淘宝拒绝访问：账号近期访问行为异常，平台已限制部分访问功能。"
    _record_access_limit_cooldown(
        profile_dir,
        reason_text,
        "manual",
        resume_at=parsed_resume_at,
    )
    state = _access_limit_cooldown_state(profile_dir)
    print(json.dumps({
        "marked": True,
        "access_limited": bool(state["active"]),
        "message": state["message"],
        "cooldown": state["cooldown"],
    }, ensure_ascii=False))
    sys.exit(0)


# ======================== CLI ========================

def main():
    parser = argparse.ArgumentParser(description="淘宝商品详情反检测采集脚本（CloakBrowser 驱动）")
    subparsers = parser.add_subparsers(dest="command", required=True)

    login_parser = subparsers.add_parser("login", help="手动登录淘宝以获取并保存 Cookie/Profile")
    login_parser.add_argument("--profile-dir", required=True, help="Profile 保存的绝对路径")

    check_parser = subparsers.add_parser("check-login", help="检测当前 Profile 是否含有效淘宝登录态")
    check_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")

    collect_parser = subparsers.add_parser("collect", help="自动采集淘宝商品详细数据")
    collect_parser.add_argument("--url", required=True, help="淘宝商品链接")
    collect_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")
    collect_parser.add_argument("--headed", action="store_true", help="以可见窗口方式运行")
    collect_parser.add_argument("--debug-captured", action="store_true", help=argparse.SUPPRESS)

    state_parser = subparsers.add_parser("access-limit-state", help="查询本地淘宝访问限制冷却状态")
    state_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")

    clear_parser = subparsers.add_parser("clear-access-limit", help="清除本地淘宝访问限制冷却状态")
    clear_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")

    mark_parser = subparsers.add_parser("mark-access-limited", help="手动写入本地淘宝访问限制冷却状态")
    mark_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")
    mark_parser.add_argument("--resume-at", help="恢复时间，格式 YYYY-MM-DD HH 或 YYYY-MM-DD HH:MM:SS+08:00")
    mark_parser.add_argument("--reason", help="限制原因摘要")

    batch_parser = subparsers.add_parser("batch-collect", help="批量采集：一次启动浏览器采集多个商品 URL")
    batch_parser.add_argument("--urls", required=True, help="JSON 数组格式的 URL 列表，或包含 id 字段的对象列表。也支持从 stdin 读取 '-'")
    batch_parser.add_argument("--profile-dir", required=True, help="持久化的 Profile 路径")
    batch_parser.add_argument("--headed", action="store_true", help="以可见窗口方式运行")

    probe_parser = subparsers.add_parser(PROFILE_PROBE_COMMAND, help=argparse.SUPPRESS)
    probe_parser.add_argument("--profile-dir", required=True, help=argparse.SUPPRESS)

    args = parser.parse_args()

    if args.command == "login":
        run_login(args.profile_dir)
    elif args.command == "check-login":
        run_check_login(args.profile_dir)
    elif args.command == "collect":
        run_collect(
            args.url,
            args.profile_dir,
            headed=bool(getattr(args, "headed", False)),
            debug_captured=bool(getattr(args, "debug_captured", False)),
        )
    elif args.command == "batch-collect":
        urls_raw = args.urls
        if urls_raw == "-":
            urls_raw = sys.stdin.read()
        urls = json.loads(urls_raw)
        if not isinstance(urls, list):
            print(json.dumps({"error": "--urls 必须为 JSON 数组"}), file=sys.stderr)
            sys.exit(1)
        run_batch_collect(urls, args.profile_dir, headed=bool(getattr(args, "headed", False)))
    elif args.command == "access-limit-state":
        run_access_limit_state(args.profile_dir)
    elif args.command == "clear-access-limit":
        run_clear_access_limit(args.profile_dir)
    elif args.command == "mark-access-limited":
        run_mark_access_limited(args.profile_dir, args.resume_at, args.reason)
    elif args.command == PROFILE_PROBE_COMMAND:
        run_probe_cloak_profile(args.profile_dir)


if __name__ == "__main__":
    main()
