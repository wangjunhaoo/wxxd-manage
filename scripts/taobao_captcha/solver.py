"""淘宝/阿里 NoCaptcha 滑块处理逻辑。

这部分迁移自 xianyu-auto-reply 的验证码模块，并改造成可直接复用
wx-xd 当前 CloakBrowser 持久化上下文的版本。核心仍保持旧项目的
策略：多 frame 查找滑块、生成三阶段人类轨迹、失败后刷新重试，
并用 URL 脱离 punish 与 x5sec cookie 更新做二次确认。
"""

from __future__ import annotations

import json
import os
import random
import re
import sys
import time
from datetime import datetime, timedelta, timezone
from typing import Any, Callable, Dict, List, Optional, Tuple


SHANGHAI_TZ = timezone(timedelta(hours=8), name="Asia/Shanghai")
CAPTCHA_HISTORY_FILE = "taobao_captcha_history.jsonl"
CAPTCHA_HISTORY_LIMIT = 200
DEFAULT_STRATEGY_ORDER = ("legacy", "smooth", "two_phase")

PUNISH_URL_KEYWORDS = (
    "punish",
    "x5step=2",
    "action=captcha",
    "pureCaptcha",
    "/captcha",
)

CAPTCHA_TEXT_KEYWORDS = (
    "请点击按钮验证",
    "请按住滑块",
    "拖动滑块",
    "安全验证",
    "验证码校验",
    "验证失败，点击框体重试",
    "Release the slider",
)

CAPTCHA_HTML_MARKERS = (
    'id="nc_1_n1z',
    "id='nc_1_n1z",
    'class="nc-container',
    "class='nc-container",
    "scratch-captcha",
    "baxia-punish",
    "captcha-question",
)

CAPTCHA_ELEMENT_SELECTORS = (
    "#nc_1_n1z",
    ".nc-container",
    "#nocaptcha",
    "#scratch-captcha-btn",
    ".scratch-captcha-container",
    "#baxia-punish",
    ".baxia-punish",
    "canvas#captcha-question",
)

MANUAL_VERIFICATION_TEXT_KEYWORDS = (
    "二维码验证",
    "扫码验证",
    "扫码登录",
    "手机淘宝扫码",
    "短信验证",
    "手机验证码",
    "人脸验证",
    "人脸识别",
    "面部验证",
    "拍摄脸部",
    "请进行人脸验证",
    "请完成人脸识别",
    "验证身份",
)

STRONG_MANUAL_VERIFICATION_TEXT_KEYWORDS = (
    "请进行人脸验证",
    "请完成人脸识别",
    "请使用手机淘宝扫码",
    "输入短信验证码",
    "请输入手机验证码",
)

MANUAL_VERIFICATION_HTML_MARKERS = (
    'id="alibaba-login-box"',
    "id='alibaba-login-box'",
    "mini_login",
    "face_verify",
    "face-verification",
    "sms-verification",
)

MANUAL_VERIFICATION_ELEMENT_SELECTORS = (
    "iframe#alibaba-login-box",
    "iframe[src*='mini_login']",
    "[class*='login-qrcode']",
    "[class*='qrcode-login']",
    "[class*='qr-code-login']",
    "[class*='scan-login']",
    "[class*='face-verify']",
    "[class*='face_verify']",
    "[class*='face-verification']",
    "[class*='sms-verify']",
    "[class*='sms_verify']",
    "[class*='sms-verification']",
)

PRODUCT_URL_KEYWORDS = (
    "item.taobao.com",
    "detail.tmall.com",
    "detail.tmall.hk",
)

ACCESS_LIMITED_TEXT_KEYWORDS = (
    "访问被拒绝",
    "您的账户近期访问行为存在异常",
    "系统将限制该账号的部分访问功能",
    "涉嫌不当获取使用平台商业信息",
)

ACCESS_LIMITED_HINT_KEYWORDS = (
    "不影响商家店铺正常经营",
    "再次违规将升级处置",
    "恢复正常",
    "点我反馈",
)


def _log(message: str) -> None:
    print(f"[taobao-captcha] {message}", file=sys.stderr)


def _safe_text(page: Any, limit: int = 600) -> str:
    try:
        return page.evaluate(
            """(limit) => {
                const text = document.body && document.body.innerText;
                return (text || "").slice(0, limit);
            }""",
            limit,
        ) or ""
    except Exception:
        return ""


def _safe_content(page: Any, limit: int = 20000) -> str:
    try:
        content = page.content() or ""
        return content[:limit]
    except Exception:
        return ""


def _is_punish_url(url: str) -> bool:
    return bool(url) and any(keyword in url for keyword in PUNISH_URL_KEYWORDS)


def _is_product_url(url: str) -> bool:
    return bool(url) and any(keyword in url for keyword in PRODUCT_URL_KEYWORDS)


def _contains_captcha_keyword(text: str) -> bool:
    return any(keyword in text for keyword in CAPTCHA_TEXT_KEYWORDS)


def _contains_captcha_html_marker(html: str) -> bool:
    return any(marker in html for marker in CAPTCHA_HTML_MARKERS)


def _has_visible_captcha_element(page: Any) -> bool:
    frames = [page]
    try:
        frames.extend(frame for frame in page.frames if frame != page)
    except Exception:
        pass

    for frame in frames:
        for selector in CAPTCHA_ELEMENT_SELECTORS:
            try:
                element = frame.query_selector(selector)
                if not element:
                    continue
                try:
                    if element.is_visible():
                        return True
                except Exception:
                    return True
            except Exception:
                continue
    return False


def _iter_page_frames(page: Any) -> List[Any]:
    frames = [page]
    try:
        frames.extend(frame for frame in page.frames if frame != page)
    except Exception:
        pass
    return frames


def _has_visible_manual_verification_element(page: Any) -> bool:
    for frame in _iter_page_frames(page):
        for selector in MANUAL_VERIFICATION_ELEMENT_SELECTORS:
            try:
                element = frame.query_selector(selector)
                if not element:
                    continue
                try:
                    if element.is_visible():
                        return True
                except Exception:
                    return True
            except Exception:
                continue
    return False


def _contains_manual_verification_text(text: str) -> bool:
    if not text:
        return False
    if any(keyword in text for keyword in STRONG_MANUAL_VERIFICATION_TEXT_KEYWORDS):
        return True
    matched_count = sum(1 for keyword in MANUAL_VERIFICATION_TEXT_KEYWORDS if keyword in text)
    return matched_count >= 2


def is_taobao_captcha_page(page: Any) -> bool:
    """判断当前页面是否处于淘宝/阿里验证拦截。"""
    if is_taobao_access_limited_page(page):
        return False

    try:
        current_url = page.url or ""
    except Exception:
        current_url = ""

    if _is_punish_url(current_url):
        return True

    # login.taobao.com 带 redirectURL 时可能是正常确认登录页，不能直接视为失败。
    if "login.taobao.com" in current_url and "redirectURL" not in current_url:
        return True

    if _has_visible_captcha_element(page):
        return True

    if is_taobao_manual_verification_page(page):
        return True

    text = _safe_text(page)
    if _contains_captcha_keyword(text):
        return True

    html = _safe_content(page, limit=8000)
    return _contains_captcha_html_marker(html)


def is_taobao_manual_verification_page(page: Any) -> bool:
    """判断是否进入需要人工处理的淘宝验证页。

    源项目会把二维码、人脸和短信验证从滑块验证中分离出来处理；
    当前采集脚本没有远程通知和人工等待流程，因此命中后只进入保护冷却。
    """
    if is_taobao_access_limited_page(page):
        return False

    try:
        current_url = page.url or ""
    except Exception:
        current_url = ""

    if "mini_login" in current_url:
        return True

    if _has_visible_manual_verification_element(page):
        return True

    text = _safe_text(page, limit=1600)
    if _contains_manual_verification_text(text):
        return True

    html = _safe_content(page, limit=12000)
    return any(marker in html for marker in MANUAL_VERIFICATION_HTML_MARKERS)


def get_taobao_manual_verification_reason(page: Any) -> Optional[str]:
    """返回非滑块验证的人工处理摘要。"""
    if not is_taobao_manual_verification_page(page):
        return None

    text = re.sub(r"\s+", " ", _safe_text(page, limit=500)).strip()
    if "人脸" in text or "拍摄脸部" in text:
        kind = "人脸验证"
    elif "短信" in text or "手机验证码" in text:
        kind = "短信验证"
    elif "二维码" in text or "扫码" in text:
        kind = "二维码/扫码验证"
    else:
        kind = "人工验证"
    return f"淘宝触发{kind}，当前脚本只能自动处理滑块验证码，请打开淘宝登录窗口人工完成验证后重试。"


def is_taobao_access_limited_page(page: Any) -> bool:
    """判断是否进入淘宝账号访问限制页，该状态不能用验证码重试解决。"""
    text = _safe_text(page, limit=1600)
    if not text:
        return False

    if any(keyword in text for keyword in ACCESS_LIMITED_TEXT_KEYWORDS):
        return True

    hint_count = sum(1 for keyword in ACCESS_LIMITED_HINT_KEYWORDS if keyword in text)
    return hint_count >= 2 and "Taobao.com" in text


def get_taobao_access_limited_reason(page: Any) -> Optional[str]:
    """返回适合错误输出的账号限制摘要。"""
    if not is_taobao_access_limited_page(page):
        return None

    text = re.sub(r"\s+", " ", _safe_text(page, limit=900)).strip()
    if not text:
        return "淘宝拒绝访问：账号近期访问行为异常，平台已限制部分访问功能。"

    restore_match = re.search(r"预计([0-9]{4}-[0-9]{2}-[0-9]{2}\s+[0-9]{1,2}时?)后恢复正常", text)
    restore_hint = f"平台提示预计{restore_match.group(1)}后恢复正常。" if restore_match else ""
    return (
        "淘宝拒绝访问：账号近期访问行为异常，平台已限制部分访问功能。"
        f"{restore_hint}请停止自动访问，等待恢复后再手动确认。"
    )


def _read_cookie_dict(context: Any) -> Dict[str, str]:
    try:
        cookies = context.cookies() if context else []
    except Exception:
        return {}

    result: Dict[str, str] = {}
    for cookie in cookies or []:
        if not isinstance(cookie, dict):
            continue
        name = cookie.get("name")
        value = cookie.get("value")
        if name and value:
            result[name] = value
    return result


def _read_x5sec(context: Any) -> Optional[str]:
    return _read_cookie_dict(context).get("x5sec")


def _format_shanghai_now() -> str:
    return datetime.now(SHANGHAI_TZ).strftime("%Y-%m-%d %H:%M:%S+08:00")


def _captcha_history_path(context: Any) -> Optional[str]:
    profile_dir = getattr(context, "_wx_xd_profile_dir", None)
    if not profile_dir:
        return None
    return os.path.join(profile_dir, CAPTCHA_HISTORY_FILE)


def _looks_unblocked(page: Any, context: Any, pre_x5sec: Optional[str]) -> bool:
    """二次确认是否已真正离开验证页。"""
    if is_taobao_access_limited_page(page):
        return False

    try:
        current_url = page.url or ""
    except Exception:
        current_url = ""

    if _is_punish_url(current_url):
        return False

    current_x5sec = _read_x5sec(context)
    if current_x5sec and current_x5sec != (pre_x5sec or ""):
        return True

    if _has_visible_captcha_element(page):
        return False

    text = _safe_text(page, limit=1200)
    if _contains_captcha_keyword(text):
        return False

    # 部分场景 x5sec 写入会滞后；URL 已回到商品页且页面不再含验证元素时，
    # 按通过处理，但调用方后续仍会重新请求商品详情验证结果。
    return _is_product_url(current_url) or not current_url


def _build_minimal_stealth_script() -> str:
    """最小化反检测脚本 — 只补充 CloakBrowser 底层未覆盖的部分。

    CloakBrowser v0.3+ 已在 Chromium C++ 层处理了:
    - navigator.webdriver 隐藏
    - window.chrome 对象伪造
    - Canvas/WebGL/Audio 指纹噪声（--fingerprint flag）
    - 屏幕分辨率/硬件参数随机化
    - User-Agent 与 Chromium 版本一致
    - CDP 信号屏蔽

    本脚本补充 CloakBrowser 不一定覆盖的 JS 层检测点:
    1. 清理自动化私有全局变量痕迹
    2. 安装鼠标轨迹收集器（淘宝 NoCaptcha 特有检测点）
    3. navigator.plugins 补充（空插件列表是已知的自动化特征）
    4. navigator.permissions.query 正常化
    """
    return """
        (() => {
            // 1. 清理自动化框架特有的全局变量痕迹
            const pwKeys = [
                'play' + 'wright', '__play' + 'wright', '__pw_manual', '__pw_original',
                '__PW_inspect', 'webdriver', '__webdriver_script_fn',
                '__webdriver_evaluate', '__webdriver_unwrapped',
                '__fxdriver_evaluate', '__driver_evaluate',
                '__webdriver_script_func', '_selenium', '_phantom',
                'callPhantom', 'phantom', 'Buffer', 'emit', 'spawn'
            ];
            for (const key of pwKeys) {
                try { delete window[key]; } catch (e) {}
            }

            // 2. 鼠标轨迹收集器（淘宝 NoCaptcha 会检查鼠标事件历史来评估人类行为）
            try {
                if (!window.__wx_xd_mouse_collector_installed) {
                    const mouseMovements = [];
                    let lastMouseTime = Date.now();
                    document.addEventListener('mousemove', function(e) {
                        const now = Date.now();
                        mouseMovements.push({
                            x: e.clientX,
                            y: e.clientY,
                            time: now,
                            timeDiff: now - lastMouseTime
                        });
                        lastMouseTime = now;
                        if (mouseMovements.length > 100) {
                            mouseMovements.shift();
                        }
                    }, true);
                    window.__wx_xd_mouse_collector_installed = true;
                }
            } catch (e) {}

            // 3. navigator.plugins 补充 — 空插件列表是已知的自动化特征
            try {
                if (!navigator.plugins || navigator.plugins.length === 0) {
                    Object.defineProperty(navigator, 'plugins', {
                        get: () => {
                            // 返回类数组对象，模拟常见插件
                            const pluginNames = [
                                {name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format'},
                                {name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', description: ''},
                                {name: 'Native Client', filename: 'internal-nacl-plugin', description: ''},
                            ];
                            const arr = Object.create(PluginArray.prototype);
                            pluginNames.forEach((p, i) => {
                                const plugin = Object.create(Plugin.prototype);
                                Object.defineProperties(plugin, {
                                    name: {value: p.name, enumerable: true},
                                    filename: {value: p.filename, enumerable: true},
                                    description: {value: p.description, enumerable: true},
                                    length: {value: 1, enumerable: false},
                                });
                                plugin[0] = Object.create(MimeType.prototype, {
                                    type: {value: 'application/pdf', enumerable: true},
                                    suffixes: {value: 'pdf', enumerable: true},
                                    description: {value: '', enumerable: true},
                                });
                                arr[i] = plugin;
                            });
                            Object.defineProperty(arr, 'length', {value: pluginNames.length, enumerable: false});
                            return arr;
                        },
                        configurable: true,
                        enumerable: true,
                    });
                }
            } catch (e) {}

            // 4. navigator.permissions.query 正常化
            try {
                const origQuery = window.Permissions && window.Permissions.prototype && window.Permissions.prototype.query;
                if (origQuery) {
                    window.Permissions.prototype.query = function(desc) {
                        return Promise.resolve(Object.create(PermissionStatus.prototype, {
                            state: {value: 'prompt', enumerable: true},
                            onchange: {value: null, writable: true, enumerable: true},
                        }));
                    };
                }
            } catch (e) {}
        })();
    """


def install_minimal_taobao_stealth(context: Any = None, page: Any = None) -> None:
    """安装最小化反检测脚本，配合 CloakBrowser 底层能力使用。

    不覆盖 navigator/canvas/webgl 等 CloakBrowser 已在 C++ 层处理的指纹，
    避免 JS 层 hook 被反爬系统检测到。
    """
    script = _build_minimal_stealth_script()
    for target in (context, page):
        if target is None:
            continue
        try:
            target.add_init_script(script)
        except Exception:
            continue
    if page is not None:
        try:
            page.evaluate(script)
        except Exception:
            pass
        try:
            frames = list(page.frames)
        except Exception:
            frames = []
        for frame in frames:
            try:
                if frame == page:
                    continue
            except Exception:
                pass
            try:
                frame.evaluate(script)
            except Exception:
                continue


class TrajectoryGenerator:
    """三阶段人类化滑动轨迹生成器。"""

    def __init__(self) -> None:
        self.current_data: Dict[str, Any] = {}

    def generate(self, distance: float, strategy: str = "legacy") -> List[Tuple[float, float, float]]:
        if distance <= 0:
            return []

        if strategy == "smooth":
            return self._generate_smooth(distance)
        if strategy == "two_phase":
            return self._generate_two_phase(distance)
        return self._generate_legacy(distance)

    def _generate_legacy(self, distance: float) -> List[Tuple[float, float, float]]:
        # 真实人手滑动通常 15-25 步，持续 0.6-1.2 秒
        total_steps = random.randint(15, 25)
        total_duration = random.uniform(0.6, 1.2)
        avg_delay = total_duration / total_steps

        accel_steps = max(3, int(round(total_steps * random.uniform(0.25, 0.35))))
        decel_steps = max(3, int(round(total_steps * random.uniform(0.20, 0.30))))
        const_steps = max(3, total_steps - accel_steps - decel_steps)
        total_steps = accel_steps + const_steps + decel_steps

        accel_dist = distance * random.uniform(0.20, 0.35)
        const_dist = distance * random.uniform(0.45, 0.60)
        decel_dist = distance - accel_dist - const_dist

        trajectory: List[Tuple[float, float, float]] = []

        for i in range(1, accel_steps + 1):
            t = i / accel_steps
            # Y 轴抖动逐步增大（手刚开始移动时抖动较多）
            y_jitter = random.uniform(-2.0, 2.0) * (1 - t * 0.3)
            trajectory.append((
                accel_dist * (t * t),
                y_jitter,
                avg_delay * random.uniform(0.7, 1.2),
            ))

        for i in range(1, const_steps + 1):
            t = i / const_steps
            delay = avg_delay * random.uniform(0.8, 1.2)
            # 随机微停顿（真实人手会有短暂犹豫）
            if random.random() < 0.08:
                delay *= random.uniform(1.5, 2.5)
            trajectory.append((
                accel_dist + const_dist * t,
                random.uniform(-1.5, 1.5),
                delay,
            ))

        base_x = accel_dist + const_dist
        for i in range(1, decel_steps + 1):
            t = i / decel_steps
            # 减速阶段 Y 轴抖动变小（接近目标时更稳定）
            y_jitter = random.uniform(-0.8, 0.8) * (1 - t * 0.6)
            trajectory.append((
                base_x + decel_dist * (1 - (1 - t) ** 2),
                y_jitter,
                avg_delay * random.uniform(1.0, 1.6),
            ))

        trajectory[-1] = (distance, trajectory[-1][1], trajectory[-1][2])
        self.current_data = {
            "distance": distance,
            "total_steps": len(trajectory),
            "strategy": "legacy",
            "final_left_px": 0,
        }
        return trajectory

    def _generate_smooth(self, distance: float) -> List[Tuple[float, float, float]]:
        total_steps = random.randint(24, 36)
        total_duration = random.uniform(0.75, 1.25)
        trajectory: List[Tuple[float, float, float]] = []

        for i in range(1, total_steps + 1):
            t = i / total_steps
            if t < 0.45:
                eased = 2.0 * t * t
            else:
                eased = 1 - pow(-2 * t + 2, 3) / 2
            x = min(distance, max(0, distance * eased + random.uniform(-0.4, 0.4)))
            if i == total_steps:
                x = distance
            y = random.uniform(-1.2, 1.2) * (1 - t * 0.5)
            delay = (total_duration / total_steps) * random.uniform(0.75, 1.35)
            trajectory.append((x, y, delay))

        self.current_data = {
            "distance": distance,
            "total_steps": len(trajectory),
            "strategy": "smooth",
            "final_left_px": 0,
        }
        return trajectory

    def _generate_two_phase(self, distance: float) -> List[Tuple[float, float, float]]:
        first_target = distance * random.uniform(0.84, 0.90)
        first_steps = random.randint(12, 18)
        finish_steps = random.randint(6, 10)
        trajectory: List[Tuple[float, float, float]] = []

        for i in range(1, first_steps + 1):
            t = i / first_steps
            x = first_target * (1 - (1 - t) ** 2)
            y = random.uniform(-1.0, 1.0)
            delay = random.uniform(0.018, 0.045)
            trajectory.append((x, y, delay))

        pause_x = first_target + random.uniform(-0.8, 0.8)
        trajectory.append((pause_x, random.uniform(-0.5, 0.5), random.uniform(0.12, 0.22)))

        for i in range(1, finish_steps + 1):
            t = i / finish_steps
            x = first_target + (distance - first_target) * (t ** 1.7)
            y = random.uniform(-0.45, 0.45)
            delay = random.uniform(0.025, 0.060)
            trajectory.append((x, y, delay))

        trajectory[-1] = (distance, trajectory[-1][1], trajectory[-1][2])
        self.current_data = {
            "distance": distance,
            "total_steps": len(trajectory),
            "strategy": "two_phase",
            "final_left_px": 0,
        }
        return trajectory

    def set_final_left(self, value: float) -> None:
        self.current_data["final_left_px"] = value


class SliderElementFinder:
    """滑块元素查找器，保留旧项目的多 frame 搜索策略。"""

    CONTAINER_SELECTORS = [
        "#nc_1_wrapper",
        ".nc-container",
        "#nocaptcha",
        ".nc_scale",
        "#nc_1__scale_text",
        ".nc-wrapper",
        "#baxia-dialog-content",
        "[class*='nc-container']",
        "[class*='nc_wrapper']",
        "[id*='nocaptcha']",
        "[class*='scratch-captcha']",
    ]

    BUTTON_SELECTORS = [
        "#nc_1_n1z",
        ".nc_iconfont",
        ".btn_slide",
        "#scratch-captcha-btn",
        ".scratch-captcha-slider .button",
        "[class*='slider']",
        "[class*='btn']",
        "[role='button']",
    ]

    TRACK_SELECTORS = [
        "#nc_1_n1t",
        ".nc_scale",
        ".nc_1_n1t",
        ".scratch-captcha-slider",
        "[class*='track']",
        "[class*='scale']",
    ]

    def __init__(self, page: Any):
        self.page = page
        self._detected_frame: Optional[Any] = None

    def get_detected_frame(self) -> Optional[Any]:
        return self._detected_frame

    def find(self) -> Tuple[Optional[Any], Optional[Any], Optional[Any]]:
        for idx, frame in enumerate(self._iter_frames()):
            button = self._visible_query(frame, "#nc_1_n1z")
            track = self._visible_query(frame, "#nc_1_n1t")
            if button and track:
                container = (
                    self._visible_query(frame, "#baxia-dialog-content")
                    or self._visible_query(frame, ".nc-container")
                    or self._visible_query(frame, "#nc_1_wrapper")
                    or button
                )
                self._detected_frame = None if frame == self.page else frame
                _log(f"在 {'主页面' if frame == self.page else f'Frame {idx}'} 找到标准 NC 滑块")
                return container, button, track

        for idx, frame in enumerate(self._iter_frames()):
            container = self._find_first_visible(frame, self.CONTAINER_SELECTORS, wait_ms=800)
            if not container:
                continue
            button = self._find_first_visible(frame, self.BUTTON_SELECTORS, wait_ms=800)
            track = self._find_first_visible(frame, self.TRACK_SELECTORS, wait_ms=500)
            self._detected_frame = None if frame == self.page else frame
            if button and track:
                _log(f"在 {'主页面' if frame == self.page else f'Frame {idx}'} 找到滑块组合")
                return container, button, track
            return container, button, track

        return None, None, None

    def _iter_frames(self) -> List[Any]:
        frames = [self.page]
        try:
            for frame in self.page.frames:
                if frame != self.page:
                    frames.append(frame)
        except Exception:
            pass
        return frames

    def _find_first_visible(self, frame: Any, selectors: List[str], wait_ms: int = 500) -> Optional[Any]:
        for selector in selectors:
            element = self._visible_query(frame, selector)
            if element:
                return element
            if frame == self.page and wait_ms > 0:
                try:
                    element = self.page.wait_for_selector(selector, timeout=wait_ms)
                    if element and self._is_visible(element):
                        return element
                except Exception:
                    pass
        return None

    @staticmethod
    def _visible_query(frame: Any, selector: str) -> Optional[Any]:
        try:
            element = frame.query_selector(selector)
            if element and SliderElementFinder._is_visible(element):
                return element
        except Exception:
            return None
        return None

    @staticmethod
    def _is_visible(element: Any) -> bool:
        try:
            return bool(element.is_visible())
        except Exception:
            return True


class VerificationChecker:
    """滑块距离计算和验证结果二次确认。"""

    SUCCESS_SELECTORS = [
        ".nc_ok_icon",
        ".nc-lang-cnt .nc_ok",
        "#nc_1_n1z.nc_ok",
    ]

    def __init__(self, page: Any, context: Any, frame_getter: Callable[[], Optional[Any]]):
        self.page = page
        self.context = context
        self.frame_getter = frame_getter
        self.pre_x5sec: Optional[str] = None

    def set_pre_x5sec(self, value: Optional[str]) -> None:
        self.pre_x5sec = value

    def is_scratch_captcha(self) -> bool:
        content = _safe_content(self.page)
        scratch_markers = (
            "scratch-captcha",
            "scratch-captcha-btn",
            "scratch-captcha-slider",
            "Release the slider",
            "fully appears",
        )
        return any(marker in content for marker in scratch_markers)

    def calculate_distance(self, button: Any, track: Any) -> float:
        try:
            button_box = button.bounding_box()
            track_box = track.bounding_box()
        except Exception:
            button_box = None
            track_box = None

        if not button_box or not track_box:
            return 0

        precise_distance = self._calculate_distance_by_js()
        if precise_distance and precise_distance > 0:
            distance = precise_distance
        else:
            distance = track_box["width"] - button_box["width"]

        if self.is_scratch_captcha():
            distance *= random.uniform(0.25, 0.35)
        else:
            distance += random.uniform(-2.0, 2.0)

        return max(distance, 0)

    def _calculate_distance_by_js(self) -> Optional[float]:
        frame = self.frame_getter() or self.page
        try:
            return frame.evaluate("""
                () => {
                    const button = document.querySelector('#nc_1_n1z')
                        || document.querySelector('.nc_iconfont')
                        || document.querySelector('#scratch-captcha-btn');
                    const track = document.querySelector('#nc_1_n1t')
                        || document.querySelector('.nc_scale')
                        || document.querySelector('.scratch-captcha-slider');
                    if (!button || !track) return null;
                    const buttonRect = button.getBoundingClientRect();
                    const trackRect = track.getBoundingClientRect();
                    return Math.max(trackRect.width - buttonRect.width, 0);
                }
            """)
        except Exception:
            return None

    def check_success(self) -> bool:
        time.sleep(0.35)

        if self._visual_passed():
            return self._confirm_success_after_visual()

        time.sleep(1.2)
        if self._visual_passed():
            return self._confirm_success_after_visual()

        return False

    def _visual_passed(self) -> bool:
        frame = self.frame_getter() or self.page
        container_exists, container_visible = self._container_status(frame)
        if not container_exists or not container_visible:
            return True

        for selector in self.SUCCESS_SELECTORS:
            try:
                element = self.page.query_selector(selector)
                if element and element.is_visible():
                    return True
            except Exception:
                continue

        try:
            slider = frame.query_selector("#nc_1_n1z")
            if not slider or not slider.is_visible():
                return True
        except Exception:
            return True

        return False

    def _container_status(self, frame: Any) -> Tuple[bool, bool]:
        try:
            container = frame.query_selector(".nc-container") or frame.query_selector("#nocaptcha")
            if not container:
                return False, False
            try:
                return True, bool(container.is_visible())
            except Exception:
                return True, True
        except Exception as exc:
            msg = str(exc).lower()
            if "detached" in msg or "disconnected" in msg:
                return False, False
            return True, True

    def _confirm_success_after_visual(self) -> bool:
        deadline = time.time() + 5.0
        while time.time() < deadline:
            if _looks_unblocked(self.page, self.context, self.pre_x5sec):
                return True
            time.sleep(0.35)
        return _looks_unblocked(self.page, self.context, self.pre_x5sec)


class CaptchaAttemptHistory:
    """轻量记录验证码轨迹结果，用最近成功策略优化后续尝试顺序。"""

    def __init__(self, path: Optional[str]):
        self.path = path

    def load(self) -> List[Dict[str, Any]]:
        if not self.path or not os.path.exists(self.path):
            return []

        records: List[Dict[str, Any]] = []
        try:
            with open(self.path, "r", encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        payload = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if isinstance(payload, dict):
                        records.append(payload)
        except Exception:
            return []
        return records[-CAPTCHA_HISTORY_LIMIT:]

    def strategy_order(self) -> List[str]:
        stats: Dict[str, Dict[str, float]] = {
            strategy: {"success": 0.0, "duration": 0.0}
            for strategy in DEFAULT_STRATEGY_ORDER
        }
        for record in self.load()[-100:]:
            strategy = record.get("strategy")
            if strategy not in stats or not record.get("success"):
                continue
            stats[strategy]["success"] += 1
            stats[strategy]["duration"] += float(record.get("duration_sec") or 0)

        ranked = sorted(
            (strategy for strategy in DEFAULT_STRATEGY_ORDER if stats[strategy]["success"] > 0),
            key=lambda strategy: (
                -stats[strategy]["success"],
                stats[strategy]["duration"] / max(stats[strategy]["success"], 1.0),
            ),
        )
        ranked.extend(strategy for strategy in DEFAULT_STRATEGY_ORDER if strategy not in ranked)
        return ranked

    def append(self, event: Dict[str, Any]) -> None:
        if not self.path:
            return

        safe_event = {
            "created_at": _format_shanghai_now(),
            "attempt": int(event.get("attempt") or 0),
            "strategy": event.get("strategy"),
            "success": bool(event.get("success")),
            "reason": event.get("reason"),
            "distance": round(float(event.get("distance") or 0), 2),
            "steps": int(event.get("steps") or 0),
            "duration_sec": round(float(event.get("duration_sec") or 0), 3),
            "final_left_px": round(float(event.get("final_left_px") or 0), 2),
            "had_x5sec_before": bool(event.get("had_x5sec_before")),
            "had_x5sec_after": bool(event.get("had_x5sec_after")),
            "x5sec_changed": bool(event.get("x5sec_changed")),
        }

        try:
            os.makedirs(os.path.dirname(self.path), exist_ok=True)
            with open(self.path, "a", encoding="utf-8") as f:
                f.write(json.dumps(safe_event, ensure_ascii=False) + "\n")
            self._trim()
        except Exception:
            return

    def _trim(self) -> None:
        if not self.path:
            return
        try:
            with open(self.path, "r", encoding="utf-8") as f:
                lines = [line for line in f if line.strip()]
            if len(lines) <= CAPTCHA_HISTORY_LIMIT + 20:
                return
            with open(self.path, "w", encoding="utf-8") as f:
                f.writelines(lines[-CAPTCHA_HISTORY_LIMIT:])
        except Exception:
            return


class TaobaoSliderSolver:
    """在当前页面和当前持久化上下文中处理淘宝滑块。"""

    def __init__(self, page: Any, context: Any):
        self.page = page
        self.context = context
        self.finder = SliderElementFinder(page)
        self.trajectory = TrajectoryGenerator()
        self.checker = VerificationChecker(page, context, self.finder.get_detected_frame)
        self.history = CaptchaAttemptHistory(_captcha_history_path(context))
        self.strategy_order = self.history.strategy_order()
        if self.strategy_order != list(DEFAULT_STRATEGY_ORDER):
            _log(f"按历史成功率调整滑块策略顺序：{', '.join(self.strategy_order)}")

    def solve(self, max_retries: int = 3, timeout_sec: int = 45) -> bool:
        started_at = time.time()
        pre_x5sec = _read_x5sec(self.context)
        self.checker.set_pre_x5sec(pre_x5sec)
        _log(f"滑动前 x5sec 快照：{(pre_x5sec[:40] + '...') if pre_x5sec else '<不存在>'}")

        # 真实用户看到验证码后不会立刻操作，先停顿观察 1-3 秒
        time.sleep(random.uniform(1.0, 3.0))

        for attempt in range(1, max_retries + 1):
            if time.time() - started_at > timeout_sec:
                _log(f"验证码处理超时，已耗时 {time.time() - started_at:.1f} 秒")
                return False

            _log(f"滑块验证尝试 {attempt}/{max_retries}")
            attempt_started_at = time.time()
            container, button, track = self.finder.find()
            if not button or not track:
                if _looks_unblocked(self.page, self.context, pre_x5sec):
                    _log("未找到滑块，但页面已离开验证态，判定为通过")
                    self._record_attempt(
                        attempt=attempt,
                        strategy="already_passed",
                        success=True,
                        reason="no_slider_unblocked",
                        started_at=attempt_started_at,
                    )
                    return True
                self._click_refresh()
                time.sleep(random.uniform(1.2, 2.0))
                continue

            distance = self.checker.calculate_distance(button, track)
            if distance <= 0:
                _log("滑动距离计算失败，刷新后重试")
                self._record_attempt(
                    attempt=attempt,
                    strategy=None,
                    success=False,
                    reason="distance_unavailable",
                    started_at=attempt_started_at,
                )
                self._click_refresh()
                continue

            strategy = self._strategy_for_attempt(attempt)
            path = self.trajectory.generate(distance, strategy=strategy)
            if not path:
                _log("轨迹生成失败")
                self._record_attempt(
                    attempt=attempt,
                    strategy=strategy,
                    success=False,
                    reason="trajectory_unavailable",
                    distance=distance,
                    started_at=attempt_started_at,
                )
                continue
            _log(f"滑动距离 {distance:.1f}px，轨迹策略 {strategy}，步数 {len(path)}")

            if not self._simulate_slide(button, path, strategy=strategy):
                _log("滑动执行失败")
                self._safe_mouse_up()
                self._record_attempt(
                    attempt=attempt,
                    strategy=strategy,
                    success=False,
                    reason="slide_failed",
                    distance=distance,
                    steps=len(path),
                    started_at=attempt_started_at,
                )
                continue

            if self.checker.check_success():
                _log(f"滑块验证成功，第 {attempt} 次通过")
                self._record_attempt(
                    attempt=attempt,
                    strategy=strategy,
                    success=True,
                    reason="verified",
                    distance=distance,
                    steps=len(path),
                    started_at=attempt_started_at,
                )
                return True

            _log(f"第 {attempt} 次滑块验证未通过")
            self._record_attempt(
                attempt=attempt,
                strategy=strategy,
                success=False,
                reason="verification_failed",
                distance=distance,
                steps=len(path),
                started_at=attempt_started_at,
            )
            if attempt < max_retries:
                self._click_refresh()
                time.sleep(random.uniform(1.0, 2.0))

        _log("滑块验证失败，已达到最大重试次数")
        return False

    def _strategy_for_attempt(self, attempt: int) -> str:
        if 1 <= attempt <= len(self.strategy_order):
            return self.strategy_order[attempt - 1]
        return self.strategy_order[-1] if self.strategy_order else "two_phase"

    def _record_attempt(
        self,
        attempt: int,
        strategy: Optional[str],
        success: bool,
        reason: str,
        started_at: float,
        distance: float = 0,
        steps: int = 0,
    ) -> None:
        current_x5sec = _read_x5sec(self.context)
        self.history.append({
            "attempt": attempt,
            "strategy": strategy,
            "success": success,
            "reason": reason,
            "distance": distance,
            "steps": steps,
            "duration_sec": time.time() - started_at,
            "final_left_px": self.trajectory.current_data.get("final_left_px", 0),
            "had_x5sec_before": bool(self.checker.pre_x5sec),
            "had_x5sec_after": bool(current_x5sec),
            "x5sec_changed": bool(current_x5sec and current_x5sec != (self.checker.pre_x5sec or "")),
        })

    def _simulate_slide(
        self,
        button: Any,
        path: List[Tuple[float, float, float]],
        strategy: str = "legacy",
    ) -> bool:
        try:
            # 滑块前预热：模拟真实用户在观察验证码后移动鼠标到滑块区域
            time.sleep(random.uniform(1.0, 2.8))
            box = button.bounding_box()
            if not box:
                return False

            start_x = box["x"] + box["width"] / 2
            start_y = box["y"] + box["height"] / 2

            # 从较远位置自然移入滑块区域
            for _ in range(random.randint(1, 3)):
                self.page.mouse.move(
                    start_x + random.uniform(-160, 120),
                    start_y + random.uniform(-80, 80),
                    steps=random.randint(6, 14),
                )
                time.sleep(random.uniform(0.18, 0.55))
            self.page.mouse.move(
                start_x + random.uniform(-60, -20),
                start_y + random.uniform(-30, 30),
                steps=random.randint(8, 15),
            )
            time.sleep(random.uniform(0.2, 0.5))
            self.page.mouse.move(
                start_x + random.uniform(-5, 5),
                start_y + random.uniform(-3, 3),
                steps=random.randint(5, 10),
            )
            time.sleep(random.uniform(0.15, 0.35))

            try:
                button.hover(timeout=2000)
                time.sleep(random.uniform(0.15, 0.4))
            except Exception:
                pass

            # 精确定位到滑块中心并按下
            self.page.mouse.move(start_x, start_y)
            time.sleep(random.uniform(0.05, 0.15))
            self.page.mouse.down()
            time.sleep(random.uniform(0.08, 0.2))

            current_x = start_x
            current_y = start_y
            for idx, (offset_x, offset_y, delay) in enumerate(path):
                current_x = start_x + offset_x
                current_y = start_y + offset_y
                self.page.mouse.move(current_x, current_y, steps=random.randint(1, 3))
                time.sleep(delay * random.uniform(0.9, 1.1))

                if idx == len(path) - 1:
                    self._record_final_left(button)

            # 到达终点后的微调抖动（所有策略都做，真实人手到终点时会有细微抖动）
            if self.checker.is_scratch_captcha():
                time.sleep(random.uniform(0.3, 0.5))
            else:
                time.sleep(random.uniform(0.08, 0.20))
                for _ in range(random.randint(1, 3)):
                    self.page.mouse.move(
                        current_x + random.uniform(-0.5, 0.5),
                        current_y + random.uniform(-0.4, 0.4),
                        steps=1,
                    )
                    time.sleep(random.uniform(0.03, 0.07))

            # 松手前短暂停留（真实人手不会立刻松开）
            time.sleep(random.uniform(0.04, 0.12))
            self.page.mouse.up()

            # 注意：不再 dispatch 额外的 click 事件
            # 真实用户松手时不会产生 click，这个额外事件反而会被检测为异常

            return True
        except Exception as exc:
            _log(f"滑动模拟异常: {exc}")
            self._safe_mouse_up()
            return False

    def _record_final_left(self, button: Any) -> None:
        try:
            style = button.get_attribute("style") or ""
            match = re.search(r"left:\s*([^;]+)", style)
            if not match:
                return
            value = float(match.group(1).strip().replace("px", ""))
            self.trajectory.set_final_left(value)
        except Exception:
            pass

    def _click_refresh(self) -> None:
        selectors = [
            "#nc_1_refresh1",
            ".nc_iconfont.btn_refresh",
            ".errloading",
            "[class*='refresh']",
            ".nc-container",
        ]
        frames = [self.finder.get_detected_frame(), self.page]
        for frame in frames:
            if frame is None:
                continue
            for selector in selectors:
                try:
                    element = frame.query_selector(selector)
                    if element and element.is_visible():
                        element.click()
                        _log(f"已点击滑块刷新/重试按钮: {selector}")
                        return
                except Exception:
                    continue

    def _safe_mouse_up(self) -> None:
        try:
            self.page.mouse.up()
        except Exception:
            pass


def solve_taobao_captcha(
    page: Any,
    context: Any,
    max_rounds: int = 2,
    max_retries: int = 3,
    timeout_sec: int = 45,
) -> bool:
    """如果当前页面被淘宝验证拦截，则尝试自动处理。"""
    if not is_taobao_captcha_page(page):
        return True

    install_minimal_taobao_stealth(context=context, page=page)

    for round_no in range(1, max_rounds + 1):
        _log(f"检测到淘宝验证拦截，开始第 {round_no}/{max_rounds} 轮处理")
        solver = TaobaoSliderSolver(page, context)
        if solver.solve(max_retries=max_retries, timeout_sec=timeout_sec):
            time.sleep(random.uniform(1.0, 2.0))
            if not is_taobao_captcha_page(page):
                return True
            if _looks_unblocked(page, context, solver.checker.pre_x5sec):
                return True
        time.sleep(random.uniform(1.0, 2.0))

    return not is_taobao_captcha_page(page)
