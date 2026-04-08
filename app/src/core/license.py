#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 授权模块（在线激活 + 本地缓存）。"""

import hashlib
import hmac
import json
import logging
import os
import subprocess
import sys
from datetime import datetime, timezone
from typing import Optional, Tuple

import requests

from ..constants import CONFIG_DIR_NAME

logger = logging.getLogger(__name__)

_LICENSE_FILE_NAME = "license.json"
# 本地缓存签名密钥（与设备指纹组合使用，提高篡改门槛）
_HMAC_SECRET = b"TLS-sph-2024-integrity-guard"


def _resolve_data_root() -> str:
    """解析授权数据目录（固定为用户主目录下）。"""
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


def _read_license_file() -> Optional[dict]:
    """读取本地 license.json。"""
    path = _license_path()
    if not os.path.isfile(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as file:
            return json.load(file)
    except Exception:  # noqa: BLE001
        return None


def _write_license_file(info: dict) -> None:
    """将 license 信息写入本地文件。"""
    path = _license_path()
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as file:
        json.dump(info, file, ensure_ascii=False, indent=2)


def _compute_signature(key: str, expires_at: str, device_id: str) -> str:
    """计算 license 关键字段的 HMAC-SHA256 签名。"""
    payload = f"{key}|{expires_at}|{device_id}".encode("utf-8")
    secret = _HMAC_SECRET + device_id.encode("utf-8")
    return hmac.new(secret, payload, hashlib.sha256).hexdigest()


def _verify_signature(info: dict) -> bool:
    """验证 license.json 中的 HMAC 签名是否完整。"""
    stored_sig = info.get("signature", "")
    if not stored_sig:
        return False
    expected = _compute_signature(
        info.get("key", ""),
        info.get("expires_at", ""),
        info.get("device_id", ""),
    )
    return hmac.compare_digest(stored_sig, expected)


def _save_license_file(info: dict) -> None:
    """将 license 信息（含签名）写入本地文件。"""
    info["signature"] = _compute_signature(
        info.get("key", ""),
        info.get("expires_at", ""),
        info.get("device_id", ""),
    )
    _write_license_file(info)


def _refresh_license_fields(info: dict, result: dict) -> bool:
    """用后端返回数据刷新本地 license 字段。"""
    updated = False
    field_aliases = {
        "key": ("key", "license_key", "licenseKey"),
        "expires_at": ("expires_at", "expiresAt"),
        "activated_at": ("activated_at", "activatedAt"),
        "plan_days": ("plan_days", "planDays"),
        "device_id": ("device_id", "deviceId"),
    }
    for target, aliases in field_aliases.items():
        for alias in aliases:
            if alias in result and result[alias]:
                if info.get(target) != result[alias]:
                    info[target] = result[alias]
                    updated = True
                break
    if info.get("key"):
        info["key"] = info["key"].upper()
    return updated


def _invalidate_license_file() -> None:
    """将本地 license 文件标记为无效（清除签名）。"""
    info = _read_license_file()
    if info is None:
        return
    info.pop("signature", None)
    try:
        _write_license_file(info)
    except Exception:  # noqa: BLE001
        pass


def _post_with_fallback(path: str, payload: dict) -> requests.Response:
    """依次尝试每个 API 地址发送 POST 请求，首个成功即返回。"""
    from ..constants import LICENSE_API_BASE_URLS, LICENSE_API_TIMEOUT

    last_exc: Optional[Exception] = None
    for base_url in LICENSE_API_BASE_URLS:
        url = f"{base_url}{path}"
        try:
            resp = requests.post(url, json=payload, timeout=LICENSE_API_TIMEOUT)
            return resp
        except requests.RequestException as exc:
            logger.warning("API 请求失败 %s: %s", url, exc)
            last_exc = exc
    detail = str(last_exc) if last_exc else "未知错误"
    if isinstance(last_exc, requests.Timeout):
        raise ValueError(f"请求失败：服务器响应超时（{detail}）")
    raise ValueError(f"请求失败：无法连接服务器（{detail}）")


def activate_license(key: str) -> dict:
    """激活许可证（在线验证 + 设备绑定 + 写入 license.json）。"""
    key = key.strip()
    if not key:
        raise ValueError("请输入卡密")

    device_id = get_device_id()
    raw_fingerprint = _collect_raw_fingerprint()

    resp = _post_with_fallback("/api/activate", {
        "key": key.upper(),
        "device_id": device_id,
        "device_fingerprint": raw_fingerprint,
    })

    try:
        result = resp.json()
    except ValueError:
        raise ValueError(f"激活失败：服务器返回了非 JSON 响应（HTTP {resp.status_code}）")

    if not result.get("success"):
        message = result.get("message", "未知错误")
        raise ValueError(f"激活失败：{message}")

    info = {
        "key": key.upper(),
        "activated_at": result.get("activated_at", datetime.now(timezone.utc).isoformat(timespec="seconds")),
        "expires_at": result.get("expires_at", ""),
        "device_id": device_id,
        "plan_days": result.get("plan_days", 0),
    }
    _save_license_file(info)

    logger.info("License activated via API: expires=%s, device=%s", info["expires_at"], device_id)
    return info


def check_stored_license() -> Tuple[Optional[dict], str]:
    """校验许可证，返回 (info, reason)。

    优先通过后端 /api/verify 在线校验，网络不可用时回退到本地缓存校验。
    """
    info, base_reason = _read_local_license_state()
    if base_reason != "ok":
        return info, base_reason

    key = info.get("key", "")
    device_id = info.get("device_id", "")

    # 在线校验（优先，多地址故障切换）
    try:
        resp = _post_with_fallback("/api/verify", {"key": key, "device_id": device_id})
        result = resp.json()
        if result.get("success"):
            refreshed = _refresh_license_fields(info, result)
            if refreshed or not _verify_signature(info):
                _save_license_file(info)
            return info, "ok"
        _invalidate_license_file()
        message = result.get("message", "")
        if "过期" in message or result.get("expired"):
            return info, "expired"
        if "设备" in message:
            return info, "device_mismatch"
        return info, "invalid"
    except (ValueError, requests.RequestException):
        logger.debug("在线校验失败，回退到本地缓存校验")

    return check_stored_license_local()


def _read_local_license_state() -> Tuple[Optional[dict], str]:
    """读取本地授权文件基础状态，不做签名/设备/到期校验。"""
    lpath = _license_path()
    if not os.path.isfile(lpath):
        return None, "not_found"

    info = _read_license_file()
    if info is None:
        return None, "invalid"

    key = info.get("key", "")
    device_id = info.get("device_id", "")
    if not key or not device_id:
        return None, "invalid"

    return info, "ok"


def check_stored_license_local() -> Tuple[Optional[dict], str]:
    """仅使用本地缓存快速校验授权状态，不触发任何网络请求。"""
    info, base_reason = _read_local_license_state()
    if base_reason != "ok":
        return info, base_reason
    device_id = info.get("device_id", "")

    # 离线回退：先验证本地文件的 HMAC 签名完整性
    if not _verify_signature(info):
        if not info.get("signature"):
            logger.warning("本地 license 缺少签名字段，需要联网校验或重新激活")
        else:
            logger.warning("本地 license 文件签名校验失败，疑似被篡改")
        return None, "invalid"

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
    return _read_license_file()


def deactivate_license():
    """删除许可证文件（调试/重置）。"""
    path = _license_path()
    if os.path.isfile(path):
        os.remove(path)
        logger.info("License deactivated: %s removed", path)
