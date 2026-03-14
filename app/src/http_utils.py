# -*- coding: utf-8 -*-
"""TLS-shipinhao 统一 HTTP 工具函数。

封装微信小商店 API 常用的请求构建、响应解析等功能。
"""

import platform
from typing import Any

import requests

from .constants import REQUEST_TIMEOUT

# 检测当前平台，用于构建合适的请求头
_CURRENT_PLATFORM = platform.system()
_IS_MACOS = _CURRENT_PLATFORM == "Darwin"

# 基础请求头模板（不含平台相关字段）
_BASE_HEADERS = {
    "Accept": "application/json, text/plain, */*",
    "Accept-Language": "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive",
    "Content-Type": "application/json",
    "Origin": "https://store.weixin.qq.com",
    "Pragma": "no-cache",
    "Sec-Fetch-Dest": "empty",
    "Sec-Fetch-Mode": "cors",
    "Sec-Fetch-Site": "same-origin",
}

# 基础 User-Agent（跨平台通用）
_BASE_USER_AGENT = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
)

# macOS User-Agent
_MACOS_USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
)

# sec-ch-ua-platform 值
_SEC_CH_UA_PLATFORM_WINDOWS = '"Windows"'
_SEC_CH_UA_PLATFORM_MACOS = '"macOS"'


def get_user_agent() -> str:
    """获取当前平台对应的 User-Agent。"""
    if _IS_MACOS:
        return _MACOS_USER_AGENT
    return _BASE_USER_AGENT


def get_sec_ch_ua_platform() -> str:
    """获取当前平台对应的 sec-ch-ua-platform 值。"""
    if _IS_MACOS:
        return _SEC_CH_UA_PLATFORM_MACOS
    return _SEC_CH_UA_PLATFORM_WINDOWS


def _post_json_request(
    request_callable,
    *,
    url: str,
    data: dict[str, Any],
    headers: dict[str, str],
    params: dict[str, str] | None = None,
    timeout: int = REQUEST_TIMEOUT,
) -> requests.Response:
    """执行通用 POST JSON 请求并统一处理网络/HTTP 层错误。"""
    try:
        response = request_callable(
            url,
            params=params or build_request_params(),
            json=data,
            headers=headers,
            timeout=timeout,
        )
    except requests.RequestException as exc:
        raise RuntimeError(f"请求失败：{exc}") from exc

    if response.status_code != 200:
        raise RuntimeError(f"请求失败：{get_response_error(response)}")

    return response


def build_headers(magic: str, referer: str = "") -> dict[str, str]:
    """根据 magic 构建 HTTP 请求头。

    Args:
        magic: 微信商店 biz_magic 值
        referer: Referer 头（可选）

    Returns:
        完整的请求头字典
    """
    headers = {
        **_BASE_HEADERS,
        "User-Agent": get_user_agent(),
        "biz_magic": magic,
        "mcn_magic": "",
        "potter-scene": "weixinShop",
        "sec-ch-ua": '"Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144"',
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": get_sec_ch_ua_platform(),
        "supplier_magic": "",
        "talent_magic": "",
        "wecom_magic": "",
    }
    if referer:
        headers["Referer"] = referer
    return headers


def build_request_params() -> dict[str, str]:
    """构建通用请求参数（token 和语言）。"""
    return {"token": "", "lang": "zh_CN"}


def get_response_error(response: requests.Response) -> str:
    """从 HTTP 响应中提取可读错误信息。

    Args:
        response: requests 响应对象

    Returns:
        错误描述字符串
    """
    try:
        payload = response.json()
    except ValueError:
        text = response.text.strip()
        if text:
            return f"HTTP {response.status_code}: {text[:200]}"
        return f"HTTP {response.status_code}"

    if not isinstance(payload, dict):
        return f"HTTP {response.status_code}"

    for key in ("errmsg", "message", "msg"):
        value = payload.get(key)
        if value:
            return str(value)

    errcode = payload.get("errcode")
    if errcode not in (None, 0):
        return f"错误码 {errcode}"

    return f"HTTP {response.status_code}"


def get_payload_error(payload: dict[str, Any], default_message: str) -> str:
    """从业务响应负载中提取更具体的错误信息。

    Args:
        payload: 业务响应 JSON 解析后的字典
        default_message: 默认错误信息

    Returns:
        具体错误描述字符串
    """
    if not isinstance(payload, dict):
        return default_message

    for key in ("errmsg", "message", "msg"):
        value = payload.get(key)
        if value:
            return str(value)

    for key in ("code", "errcode", "ret"):
        value = payload.get(key)
        if value not in (None, 0):
            return f"{default_message}（错误码 {value}）"

    return default_message


def post_json(
    url: str,
    data: dict[str, Any],
    headers: dict[str, str],
    params: dict[str, str] | None = None,
    timeout: int = REQUEST_TIMEOUT,
) -> requests.Response:
    """发送 POST JSON 请求的简写方法。

    Args:
        url: 请求 URL
        data: JSON 请求体
        headers: 请求头
        params: URL 查询参数（可选）
        timeout: 超时时间（秒）

    Returns:
        requests 响应对象

    Raises:
        RuntimeError: 请求失败时抛出
    """
    return _post_json_request(
        requests.post,
        url=url,
        data=data,
        headers=headers,
        params=params,
        timeout=timeout,
    )


def post_json_with_session(
    session: requests.Session,
    url: str,
    data: dict[str, Any],
    headers: dict[str, str],
    params: dict[str, str] | None = None,
    timeout: int = REQUEST_TIMEOUT,
) -> requests.Response:
    """使用指定会话发送 POST JSON 请求。

    Args:
        session: requests 会话对象
        url: 请求 URL
        data: JSON 请求体
        headers: 请求头
        params: URL 查询参数（可选）
        timeout: 超时时间（秒）

    Returns:
        requests 响应对象

    Raises:
        RuntimeError: 请求失败时抛出
    """
    return _post_json_request(
        session.post,
        url=url,
        data=data,
        headers=headers,
        params=params,
        timeout=timeout,
    )


def check_api_response(response: requests.Response, error_prefix: str = "接口返回失败") -> dict[str, Any]:
    """检查 API 响应并解析 JSON，验证业务状态码。

    Args:
        response: requests 响应对象
        error_prefix: 错误信息前缀

    Returns:
        解析后的 JSON 字典

    Raises:
        RuntimeError: 响应状态异常时抛出
    """
    try:
        payload = response.json()
    except ValueError as exc:
        raise RuntimeError(f"{error_prefix}：接口返回了非 JSON 响应。") from exc

    if payload.get("success") is False:
        raise RuntimeError(f"{error_prefix}：{get_payload_error(payload, error_prefix)}")

    if payload.get("code") not in (None, 0):
        raise RuntimeError(f"{error_prefix}：{get_payload_error(payload, error_prefix)}")

    return payload
