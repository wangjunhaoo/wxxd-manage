"""淘宝/阿里滑块验证码处理模块。"""

from .solver import (
    get_taobao_access_limited_reason,
    get_taobao_manual_verification_reason,
    install_minimal_taobao_stealth,
    is_taobao_access_limited_page,
    is_taobao_captcha_page,
    is_taobao_manual_verification_page,
    solve_taobao_captcha,
)

__all__ = [
    "get_taobao_access_limited_reason",
    "get_taobao_manual_verification_reason",
    "install_minimal_taobao_stealth",
    "is_taobao_access_limited_page",
    "is_taobao_captcha_page",
    "is_taobao_manual_verification_page",
    "solve_taobao_captcha",
]
