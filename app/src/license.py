#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 授权模块（在线激活 + 本地缓存）。"""

import hashlib
import json
import logging
import os
import subprocess
import sys
from datetime import datetime, timezone
from typing import Optional, Tuple

import requests

from .constants import CONFIG_DIR_NAME

logger = logging.getLogger(__name__)

_LICENSE_FILE_NAME = "license.json"


def _resolve_data_root() -> str:
    """解析授权数据目录。"""
    custom = os.environ.get("TLS_APP_DATA_ROOT")
    if custom:
        return os.path.abspath(os.path.expanduser(custom))
    return os.path.join(os.path.expanduser("~"), CONFIG_DIR_NAME)


# ---------------------------------------------------------------------------
# 设备指纹
# ---------------------------------------------------------------------------


def get_device_id() -> str:
    """采集跨平台设备指纹，返回 SHA-256 前 16 位。"""
    raw = _collect_raw_fingerprint()
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]


def _collect_raw_fingerprint() -> str:
    if sys.platform == "darwin":
        return _fingerprint_macos()
    if sys.platform == "win32":
        return _fingerprint_windows()
    return _fingerprint_linux()


def _fingerprint_macos() -> str:
    try:
        output = subprocess.check_output(
            ["ioreg", "-rd1", "-c", "IOPlatformExpertDevice"],
            text=True,
            timeout=5,
        )
        for line in output.splitlines():
            if "IOPlatformSerialNumber" in line:
                parts = line.split("=")
                if len(parts) >= 2:
                    return parts[-1].strip().strip('"')
    except Exception:
        pass
    return _fallback_fingerprint()


def _fingerprint_windows() -> str:
    for cmd in (
        ["wmic", "csproduct", "get", "UUID"],
        ["powershell", "-Command", "(Get-CimInstance Win32_ComputerSystemProduct).UUID"],
    ):
        try:
            kwargs = {
                "text": True,
                "timeout": 5,
                "stdout": subprocess.PIPE,
                "stderr": subprocess.DEVNULL,
            }
            if sys.platform == "win32":
                kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW
            output = subprocess.run(cmd, **kwargs).stdout
            for line in output.strip().splitlines():
                line = line.strip()
                if line and line.upper() != "UUID":
                    return line
        except Exception:
            continue
    return _fallback_fingerprint()


def _fingerprint_linux() -> str:
    for path in ("/etc/machine-id", "/var/lib/dbus/machine-id"):
        try:
            with open(path, "r", encoding="utf-8") as file:
                machine_id = file.read().strip()
                if machine_id:
                    return machine_id
        except Exception:
            continue
    return _fallback_fingerprint()


def _fallback_fingerprint() -> str:
    import platform

    return f"{platform.node()}-{platform.machine()}-{platform.system()}"


# ---------------------------------------------------------------------------
# 许可证存储
# ---------------------------------------------------------------------------


def _license_path() -> str:
    return os.path.join(_resolve_data_root(), _LICENSE_FILE_NAME)


def activate_license(key: str) -> dict:
    """激活许可证（在线验证 + 设备绑定 + 写入 license.json）。

    流程：
    1. 调用后端 API 验证卡密并绑定设备
    2. 后端通过后写入本地 license.json
    """
    from .constants import LICENSE_ACTIVATE_URL, LICENSE_API_TIMEOUT

    key = key.strip()
    if not key:
        raise ValueError("请输入卡密")

    device_id = get_device_id()
    raw_fingerprint = _collect_raw_fingerprint()

    # 1. 调用后端 API 验证卡密并绑定设备
    try:
        resp = requests.post(
            LICENSE_ACTIVATE_URL,
            json={
                "key": key.upper(),
                "device_id": device_id,
                "device_fingerprint": raw_fingerprint,
            },
            timeout=LICENSE_API_TIMEOUT,
        )
    except requests.ConnectionError:
        raise ValueError("激活失败：无法连接服务器，请检查网络后重试。")
    except requests.Timeout:
        raise ValueError("激活失败：服务器响应超时，请稍后重试。")
    except requests.RequestException as exc:
        raise ValueError(f"激活失败：网络错误 - {exc}")

    try:
        result = resp.json()
    except ValueError:
        raise ValueError(f"激活失败：服务器返回了非 JSON 响应（HTTP {resp.status_code}）")

    if not result.get("success"):
        message = result.get("message", "未知错误")
        raise ValueError(f"激活失败：{message}")

    # 2. 后端验证通过，写入本地 license.json
    info = {
        "key": key.upper(),
        "activated_at": result.get("activated_at", datetime.now(timezone.utc).isoformat(timespec="seconds")),
        "expires_at": result.get("expires_at", ""),
        "device_id": device_id,
        "plan_days": result.get("plan_days", 0),
    }

    path = _license_path()
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as file:
        json.dump(info, file, ensure_ascii=False, indent=2)

    logger.info("License activated via API: expires=%s, device=%s", info["expires_at"], device_id)
    return info


def check_stored_license() -> Tuple[Optional[dict], str]:
    """校验许可证，返回 (info, reason)。

    优先通过后端 /api/verify 在线校验，网络不可用时回退到本地缓存校验。
    """
    from .constants import LICENSE_API_TIMEOUT, LICENSE_VERIFY_URL

    path = _license_path()
    if not os.path.isfile(path):
        return None, "not_found"

    try:
        with open(path, "r", encoding="utf-8") as file:
            info = json.load(file)
    except Exception:
        return None, "invalid"

    key = info.get("key", "")
    device_id = info.get("device_id", "")
    if not key or not device_id:
        return None, "invalid"

    # 在线校验（优先）
    try:
        resp = requests.post(
            LICENSE_VERIFY_URL,
            json={"key": key, "device_id": device_id},
            timeout=LICENSE_API_TIMEOUT,
        )
        result = resp.json()
        if result.get("success"):
            return info, "ok"
        # 后端明确返回失败
        message = result.get("message", "")
        if "过期" in message or result.get("expired"):
            return info, "expired"
        if "设备" in message:
            return info, "device_mismatch"
        return info, "invalid"
    except (requests.RequestException, ValueError):
        # 网络不可用，回退到本地缓存校验
        logger.debug("在线校验失败，回退到本地缓存校验")

    # 离线回退：仅检查本地缓存中的过期时间和设备
    try:
        expires_at = datetime.fromisoformat(info["expires_at"])
    except Exception:
        return None, "invalid"

    if expires_at.tzinfo is None:
        expires_at = expires_at.replace(tzinfo=timezone.utc)

    if datetime.now(timezone.utc) > expires_at:
        return info, "expired"

    current_device = get_device_id()
    if device_id != current_device:
        return info, "device_mismatch"

    return info, "ok"


def get_license_info() -> Optional[dict]:
    """读取许可证信息（用于 UI 展示）。"""
    path = _license_path()
    if not os.path.isfile(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as file:
            return json.load(file)
    except Exception:
        return None


def deactivate_license():
    """删除许可证文件（调试/重置）。"""
    path = _license_path()
    if os.path.isfile(path):
        os.remove(path)
        logger.info("License deactivated: %s removed", path)
