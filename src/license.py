#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 的本地离线授权模块。"""

import base64
import hashlib
import hmac
import json
import logging
import os
import struct
import subprocess
import sys
from datetime import datetime, timedelta, timezone
from typing import Optional, Tuple

logger = logging.getLogger(__name__)

# 本地离线验签密钥（仅在本项目内使用）
_SECRET = b"TLS-shipinhao-2026-LicenseKey-HMAC"

PLAN_DAYS = 30
_KEY_PREFIX = "TLS-"
_PAYLOAD_LEN = 10  # 2 (days) + 2 (salt) + 6 (hmac truncated)

_CONFIG_DIR_NAME = ".tls-shipinhao"
_LICENSE_FILE_NAME = "license.json"


def _resolve_data_root() -> str:
    """解析授权数据目录。"""
    custom = os.environ.get("TLS_APP_DATA_ROOT")
    if custom:
        return os.path.abspath(os.path.expanduser(custom))
    return os.path.join(os.path.expanduser("~"), _CONFIG_DIR_NAME)


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
# 卡密生成与校验
# ---------------------------------------------------------------------------


def generate_key() -> str:
    """生成一个固定有效期卡密，格式 TLS-XXXX-XXXX-XXXX-XXXX。"""
    days_bytes = struct.pack(">H", PLAN_DAYS)
    salt = os.urandom(2)
    sig = hmac.new(_SECRET, days_bytes + salt, hashlib.sha256).digest()[:6]
    payload = days_bytes + salt + sig
    encoded = base64.b32encode(payload).decode("ascii").rstrip("=")
    return _KEY_PREFIX + "-".join(encoded[i:i + 4] for i in range(0, len(encoded), 4))


def validate_key(key: str) -> Tuple[bool, int]:
    """校验卡密并返回 (是否有效, 有效期天数)。"""
    try:
        body = key.strip().upper()
        if body.startswith(_KEY_PREFIX):
            body = body[len(_KEY_PREFIX) :]
        raw = body.replace("-", "")
        padding = (8 - len(raw) % 8) % 8
        decoded = base64.b32decode(raw + "=" * padding)
        if len(decoded) != _PAYLOAD_LEN:
            return False, 0

        days_bytes = decoded[:2]
        salt = decoded[2:4]
        sig_stored = decoded[4:10]
        sig_expected = hmac.new(_SECRET, days_bytes + salt, hashlib.sha256).digest()[:6]
        if not hmac.compare_digest(sig_stored, sig_expected):
            return False, 0
        return True, struct.unpack(">H", days_bytes)[0]
    except Exception:
        return False, 0


# ---------------------------------------------------------------------------
# 许可证存储
# ---------------------------------------------------------------------------


def _license_path() -> str:
    return os.path.join(_resolve_data_root(), _LICENSE_FILE_NAME)


def activate_license(key: str) -> dict:
    """激活许可证（校验卡密 + 绑定设备 + 写入 license.json）。"""
    valid, plan_days = validate_key(key)
    if not valid:
        raise ValueError("卡密无效：格式错误或签名不匹配")
    if plan_days <= 0:
        raise ValueError("卡密无效：有效期异常")

    now = datetime.now(timezone.utc)
    expires = now + timedelta(days=plan_days)
    device_id = get_device_id()

    info = {
        "key": key.strip().upper(),
        "activated_at": now.isoformat(timespec="seconds"),
        "expires_at": expires.isoformat(timespec="seconds"),
        "device_id": device_id,
        "plan_days": plan_days,
    }

    path = _license_path()
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as file:
        json.dump(info, file, ensure_ascii=False, indent=2)

    logger.info("License activated: expires=%s, device=%s", expires.isoformat(), device_id)
    return info


def check_stored_license() -> Tuple[Optional[dict], str]:
    """校验本地许可证，返回 (info, reason)。"""
    path = _license_path()
    if not os.path.isfile(path):
        return None, "not_found"

    try:
        with open(path, "r", encoding="utf-8") as file:
            info = json.load(file)
    except Exception:
        return None, "invalid"

    key = info.get("key", "")
    valid, _ = validate_key(key)
    if not valid:
        return None, "invalid"

    try:
        expires_at = datetime.fromisoformat(info["expires_at"])
    except Exception:
        return None, "invalid"

    if expires_at.tzinfo is None:
        expires_at = expires_at.replace(tzinfo=timezone.utc)

    if datetime.now(timezone.utc) > expires_at:
        return info, "expired"

    stored_device = info.get("device_id", "")
    current_device = get_device_id()
    if stored_device != current_device:
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
