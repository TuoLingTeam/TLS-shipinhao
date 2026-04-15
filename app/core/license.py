#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""TLS-shipinhao 授权模块（在线激活 + 票据缓存）。"""

import base64
import hashlib
import json
import logging
import os
import platform
import subprocess
import sys
from datetime import datetime, timezone
from typing import Optional, Tuple

import requests
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

from settings import (
    APP_VERSION,
    CONFIG_DIR_NAME,
    LICENSE_API_BASE_URLS,
    LICENSE_API_TIMEOUT,
    LICENSE_PROTOCOL_VERSION,
    LICENSE_PUBLIC_KEY,
    LICENSE_SESSION_REFRESH_THRESHOLD_MINUTES,
)

logger = logging.getLogger(__name__)

_LICENSE_FILE_NAME = "license.json"
_REASON_OK = "ok"
_REASON_NOT_FOUND = "not_found"
_REASON_INVALID = "invalid"
_REASON_EXPIRED = "expired"
_REASON_DEVICE_MISMATCH = "device_mismatch"
_REASON_REACTIVATION_REQUIRED = "reactivation_required"
_REASON_REVOKED = "revoked"
_REASON_ONLINE_REFRESH_REQUIRED = "online_refresh_required"
_TOKEN_KIND_DEVICE = "device_claims"
_TOKEN_KIND_OFFLINE = "offline_grant"
_TOKEN_KIND_SESSION = "session_token"


def _resolve_data_root() -> str:
    return os.path.join(os.path.expanduser("~"), CONFIG_DIR_NAME)


def _extract_first_non_header_line(output: str, *headers: str) -> str | None:
    ignored = {header.upper() for header in headers}
    for line in output.strip().splitlines():
        normalized = line.strip()
        if normalized and normalized.upper() not in ignored:
            return normalized
    return None


def get_device_id() -> str:
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
    commands = (
        ["wmic", "csproduct", "get", "UUID"],
        ["powershell", "-Command", "(Get-CimInstance Win32_ComputerSystemProduct).UUID"],
    )
    for cmd in commands:
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
            candidate = _extract_first_non_header_line(output, "UUID")
            if candidate:
                return candidate
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
    return f"{platform.node()}-{platform.machine()}-{platform.system()}"


def _license_path() -> str:
    return os.path.join(_resolve_data_root(), _LICENSE_FILE_NAME)


def _read_license_file() -> Optional[dict]:
    path = _license_path()
    if not os.path.isfile(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as file:
            return json.load(file)
    except Exception:
        return None


def _write_license_file(info: dict) -> None:
    path = _license_path()
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as file:
        json.dump(info, file, ensure_ascii=False, indent=2)


def _b64url_encode(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")


def _b64url_decode(value: str) -> bytes:
    padding = "=" * ((4 - len(value) % 4) % 4)
    return base64.urlsafe_b64decode(value + padding)


def _load_public_key() -> Ed25519PublicKey:
    return Ed25519PublicKey.from_public_bytes(_b64url_decode(LICENSE_PUBLIC_KEY))


def _now_utc() -> datetime:
    return datetime.now(timezone.utc)


def _parse_datetime(value: str | None) -> Optional[datetime]:
    if not value:
        return None
    try:
        parsed = datetime.fromisoformat(str(value))
    except Exception:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def _to_iso(dt: datetime | None) -> str:
    if dt is None:
        return ""
    return dt.astimezone(timezone.utc).isoformat(timespec="seconds")


def _extract_exp_iso(payload: dict) -> str:
    exp = payload.get("exp")
    if exp in (None, ""):
        return ""
    try:
        return _to_iso(datetime.fromtimestamp(int(exp), timezone.utc))
    except Exception:
        return ""


def verify_signed_claims(token: str, *, expected_kind: str | None = None, expected_task_type: str | None = None, expected_device_id: str | None = None, allow_expired: bool = False) -> Optional[dict]:
    try:
        encoded_payload, encoded_sig = token.split(".", 1)
        signature = _b64url_decode(encoded_sig)
        _load_public_key().verify(signature, encoded_payload.encode("utf-8"))
        payload = json.loads(_b64url_decode(encoded_payload).decode("utf-8"))
    except (ValueError, InvalidSignature, json.JSONDecodeError, TypeError):
        return None

    if expected_kind and payload.get("kind") != expected_kind:
        return None
    if expected_task_type and payload.get("task_type") != expected_task_type:
        return None
    if expected_device_id and payload.get("device_id") != expected_device_id:
        return None

    if not allow_expired:
        try:
            exp = int(payload.get("exp", 0) or 0)
        except Exception:
            return None
        if exp <= 0 or _now_utc().timestamp() >= exp:
            return None
    return payload


def _refresh_license_fields(info: dict, result: dict) -> bool:
    updated = False
    field_aliases = {
        "license_key": ("license_key", "key"),
        "device_id": ("device_id", "deviceId"),
        "activated_at": ("activated_at", "activatedAt"),
        "license_expires_at": ("license_expires_at", "expires_at", "expiresAt"),
        "plan_days": ("plan_days", "planDays"),
        "license_version": ("license_version",),
        "issuer": ("issuer",),
        "issued_at": ("issued_at",),
        "device_claims": ("device_claims",),
        "device_claims_expires_at": ("device_claims_expires_at",),
        "offline_grant": ("offline_grant",),
        "offline_grant_expires_at": ("offline_grant_expires_at",),
        "session_token": ("session_token",),
        "session_token_expires_at": ("session_token_expires_at",),
    }
    for target, aliases in field_aliases.items():
        for alias in aliases:
            if alias in result and result[alias] not in (None, ""):
                value = result[alias]
                if info.get(target) != value:
                    info[target] = value
                    updated = True
                break
    if info.get("license_key") and info.get("key") != info["license_key"]:
        info["key"] = info["license_key"]
        updated = True
    if info.get("license_expires_at") and info.get("expires_at") != info["license_expires_at"]:
        info["expires_at"] = info["license_expires_at"]
        updated = True
    return updated


def _normalize_license_info(result: dict, *, normalized_key: str | None = None, device_id: str | None = None) -> dict:
    info = {
        "license_version": int(result.get("license_version") or LICENSE_PROTOCOL_VERSION),
        "license_key": normalized_key or str(result.get("license_key") or result.get("key") or "").strip().upper(),
        "device_id": device_id or str(result.get("device_id") or result.get("deviceId") or "").strip(),
        "activated_at": result.get("activated_at") or result.get("activatedAt") or _to_iso(_now_utc()),
        "license_expires_at": result.get("license_expires_at") or result.get("expires_at") or result.get("expiresAt") or "",
        "plan_days": int(result.get("plan_days") or result.get("planDays") or 0),
        "issuer": result.get("issuer") or "tls-license-backend",
        "issued_at": result.get("issued_at") or result.get("server_time") or _to_iso(_now_utc()),
        "device_claims": result.get("device_claims") or "",
        "device_claims_expires_at": result.get("device_claims_expires_at") or "",
        "offline_grant": result.get("offline_grant") or "",
        "offline_grant_expires_at": result.get("offline_grant_expires_at") or "",
        "session_token": result.get("session_token") or "",
        "session_token_expires_at": result.get("session_token_expires_at") or "",
    }
    info["key"] = info["license_key"]
    info["expires_at"] = info["license_expires_at"]
    return info


def _clear_sensitive_tokens(info: dict) -> None:
    for key in (
        "device_claims",
        "device_claims_expires_at",
        "offline_grant",
        "offline_grant_expires_at",
        "session_token",
        "session_token_expires_at",
    ):
        info.pop(key, None)


def _read_local_license_state() -> Tuple[Optional[dict], str]:
    lpath = _license_path()
    if not os.path.isfile(lpath):
        return None, _REASON_NOT_FOUND

    info = _read_license_file()
    if info is None:
        return None, _REASON_INVALID

    key = str(info.get("license_key") or info.get("key") or "").strip().upper()
    device_id = str(info.get("device_id") or "").strip()
    if not key or not device_id:
        return None, _REASON_INVALID

    info["license_key"] = key
    info["key"] = key
    if int(info.get("license_version") or 0) != LICENSE_PROTOCOL_VERSION:
        return info, _REASON_REACTIVATION_REQUIRED

    if not info.get("license_expires_at") and info.get("expires_at"):
        info["license_expires_at"] = info["expires_at"]
    if info.get("license_expires_at"):
        info["expires_at"] = info["license_expires_at"]
    return info, _REASON_OK


def is_offline_grant_valid(info: dict) -> bool:
    token = str(info.get("offline_grant") or "").strip()
    device_id = str(info.get("device_id") or "").strip()
    return verify_signed_claims(token, expected_kind=_TOKEN_KIND_OFFLINE, expected_device_id=device_id) is not None


def is_session_token_valid(info: dict, task_type: str) -> bool:
    token = str(info.get("session_token") or "").strip()
    device_id = str(info.get("device_id") or "").strip()
    return verify_signed_claims(
        token,
        expected_kind=_TOKEN_KIND_SESSION,
        expected_task_type=task_type,
        expected_device_id=device_id,
    ) is not None


def check_stored_license_local() -> Tuple[Optional[dict], str]:
    info, base_reason = _read_local_license_state()
    if base_reason != _REASON_OK:
        return info, base_reason
    assert info is not None

    device_id = str(info.get("device_id") or "").strip()
    claims = verify_signed_claims(
        str(info.get("device_claims") or "").strip(),
        expected_kind=_TOKEN_KIND_DEVICE,
        expected_device_id=device_id,
    )
    if claims is None:
        logger.warning("本地 device_claims 验签失败或已过期，需要重新联网激活")
        return info, _REASON_REACTIVATION_REQUIRED

    license_expires_at = _parse_datetime(str(info.get("license_expires_at") or info.get("expires_at") or ""))
    if license_expires_at is None:
        return info, _REASON_INVALID
    if _now_utc() > license_expires_at:
        return info, _REASON_EXPIRED

    current_device = get_device_id()
    if device_id != current_device:
        return info, _REASON_DEVICE_MISMATCH

    if not is_offline_grant_valid(info):
        return info, _REASON_ONLINE_REFRESH_REQUIRED

    return info, _REASON_OK


def _post_with_fallback(path: str, payload: dict) -> requests.Response:
    last_exc: Optional[Exception] = None
    for base_url in LICENSE_API_BASE_URLS:
        url = f"{base_url}{path}"
        try:
            return requests.post(url, json=payload, timeout=LICENSE_API_TIMEOUT)
        except requests.RequestException as exc:
            logger.warning("API 请求失败 %s: %s", url, exc)
            last_exc = exc
    detail = str(last_exc) if last_exc else "未知错误"
    if isinstance(last_exc, requests.Timeout):
        raise ValueError(f"请求失败：服务器响应超时（{detail}）")
    raise ValueError(f"请求失败：无法连接服务器（{detail}）")


def _request_json(path: str, payload: dict) -> dict:
    resp = _post_with_fallback(path, payload)
    try:
        result = resp.json()
    except ValueError:
        raise ValueError(f"请求失败：服务器返回了非 JSON 响应（HTTP {resp.status_code}）")
    if not isinstance(result, dict):
        raise ValueError("请求失败：服务器返回数据格式异常")
    return result


def activate_license(key: str) -> dict:
    normalized_key = key.strip().upper()
    if not normalized_key:
        raise ValueError("请输入卡密")

    device_id = get_device_id()
    raw_fingerprint = _collect_raw_fingerprint()
    result = _request_json(
        "/api/activate",
        {
            "key": normalized_key,
            "device_id": device_id,
            "device_fingerprint": raw_fingerprint,
            "client_version": APP_VERSION,
            "platform": sys.platform,
            "build_channel": "desktop",
        },
    )
    if not result.get("success"):
        raise ValueError(f"激活失败：{result.get('message', '未知错误')}")

    info = _normalize_license_info(result, normalized_key=normalized_key, device_id=device_id)
    _write_license_file(info)
    logger.info("License activated via API: expires=%s, device=%s", info.get("license_expires_at"), device_id)
    return info


def check_stored_license() -> Tuple[Optional[dict], str]:
    info, base_reason = _read_local_license_state()
    if base_reason in (_REASON_NOT_FOUND, _REASON_INVALID, _REASON_REACTIVATION_REQUIRED):
        return info, base_reason
    assert info is not None

    key = info.get("license_key", "")
    device_id = info.get("device_id", "")
    payload = {
        "key": key,
        "device_id": device_id,
        "license_version": info.get("license_version") or LICENSE_PROTOCOL_VERSION,
        "session_id": verify_signed_claims(
            str(info.get("session_token") or ""),
            expected_kind=_TOKEN_KIND_SESSION,
            expected_device_id=device_id,
            allow_expired=True,
        ) or {},
        "client_version": APP_VERSION,
    }
    if isinstance(payload["session_id"], dict):
        payload["session_id"] = payload["session_id"].get("session_id", "")

    try:
        result = _request_json("/api/verify", payload)
    except (ValueError, requests.RequestException):
        logger.debug("在线校验失败，回退到本地缓存校验")
        return check_stored_license_local()

    if result.get("success"):
        refreshed = _normalize_license_info(result, normalized_key=key, device_id=device_id)
        _write_license_file(refreshed)
        return refreshed, _REASON_OK

    state = str(result.get("license_state") or "").strip() or _REASON_INVALID
    if state == _REASON_REVOKED:
        deactivate_license()
        return None, _REASON_REVOKED
    _clear_sensitive_tokens(info)
    _write_license_file(info)
    if state in {
        _REASON_EXPIRED,
        _REASON_DEVICE_MISMATCH,
        _REASON_REACTIVATION_REQUIRED,
        _REASON_ONLINE_REFRESH_REQUIRED,
        _REASON_INVALID,
    }:
        return info, state
    return info, _REASON_INVALID


def _session_refresh_due(info: dict) -> bool:
    expires_at = _parse_datetime(str(info.get("session_token_expires_at") or ""))
    if expires_at is None:
        return True
    remaining = (expires_at - _now_utc()).total_seconds()
    return remaining <= LICENSE_SESSION_REFRESH_THRESHOLD_MINUTES * 60


def issue_or_refresh_session_token(task_type: str, *, force: bool = False) -> Tuple[Optional[dict], str]:
    info, reason = check_stored_license_local()
    if reason == _REASON_ONLINE_REFRESH_REQUIRED:
        info, reason = check_stored_license()
    if reason != _REASON_OK:
        return info, reason
    assert info is not None

    if not force and is_session_token_valid(info, task_type) and not _session_refresh_due(info):
        return info, _REASON_OK

    if not LICENSE_PROTOCOL_VERSION:
        return info, _REASON_INVALID

    device_id = str(info.get("device_id") or "")
    token_payload = verify_signed_claims(
        str(info.get("session_token") or ""),
        expected_kind=_TOKEN_KIND_SESSION,
        expected_device_id=device_id,
        allow_expired=True,
    ) or {}
    endpoint = "/api/session/refresh" if token_payload and token_payload.get("task_type") == task_type else "/api/session/issue"
    payload = {
        "license_key": info.get("license_key"),
        "device_id": device_id,
        "device_claims": info.get("device_claims", ""),
        "task_type": task_type,
        "client_version": APP_VERSION,
    }
    if endpoint.endswith("refresh"):
        payload["session_token"] = info.get("session_token", "")

    try:
        result = _request_json(endpoint, payload)
    except (ValueError, requests.RequestException):
        logger.warning("授权会话刷新失败，需要联网重试")
        return info, _REASON_ONLINE_REFRESH_REQUIRED

    if not result.get("success"):
        state = str(result.get("license_state") or result.get("message") or _REASON_INVALID)
        normalized = {
            _REASON_REVOKED: _REASON_REVOKED,
            _REASON_EXPIRED: _REASON_EXPIRED,
            _REASON_DEVICE_MISMATCH: _REASON_DEVICE_MISMATCH,
            _REASON_REACTIVATION_REQUIRED: _REASON_REACTIVATION_REQUIRED,
        }.get(state, _REASON_ONLINE_REFRESH_REQUIRED)
        return info, normalized

    merged = dict(info)
    _refresh_license_fields(merged, result)
    _write_license_file(merged)
    return merged, _REASON_OK


def get_license_info() -> Optional[dict]:
    return _read_license_file()


def deactivate_license():
    path = _license_path()
    if os.path.isfile(path):
        os.remove(path)
        logger.info("License deactivated: %s removed", path)
