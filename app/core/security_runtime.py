# -*- coding: utf-8 -*-
"""运行时安全与租约授权管理。"""

from __future__ import annotations

import base64
import ctypes
import hashlib
import json
import logging
import os
import platform
import secrets
import stat
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional

import requests
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

from settings import (
    APP_VERSION,
    CONFIG_DIR_NAME,
    INTEGRITY_MANIFEST_FILE_NAME,
    INTEGRITY_MANIFEST_PUBLIC_KEY,
    LICENSE_API_BASE_URLS,
    LICENSE_API_TIMEOUT,
    LICENSE_LEASE_HARD_EXPIRY_HOURS,
    LICENSE_LEASE_RENEWAL_HOURS,
    LICENSE_PROTOCOL_VERSION,
    LICENSE_PUBLIC_KEY,
    LICENSE_TASK_BATCH_DELIVERY,
    LICENSE_TASK_CACHE_MANAGE,
    LICENSE_TASK_QUALITY_REFUND,
    LICENSE_TASK_REVIEW_FIND,
    LICENSE_TASK_REVIEW_FULL_SCAN,
    SECURITY_CORE_LIBRARY_BASENAME,
    get_home_config_dir,
    get_user_data_dir,
)

logger = logging.getLogger(__name__)

_LICENSE_FILE_NAME = "license.json"
_RUNTIME_BUNDLE_FILE = "runtime_bundle.json"
_KEYCHAIN_SERVICE = "com.tuoling.tls-shipinhao.runtime"
_KEYCHAIN_ACCOUNT = "runtime_bundle"
_REASON_OK = "ok"
_REASON_NOT_FOUND = "not_found"
_REASON_INVALID = "invalid"
_REASON_EXPIRED = "expired"
_REASON_DEVICE_MISMATCH = "device_mismatch"
_REASON_REACTIVATION_REQUIRED = "reactivation_required"
_REASON_REVOKED = "revoked"
_REASON_ONLINE_REFRESH_REQUIRED = "online_refresh_required"
_REASON_RENEWAL_DUE = "renewal_due"
_REASON_COMPROMISED = "compromised"
_TOKEN_KIND_LEASE = "license_lease"
_ALLOWED_LOCAL_REASONS = {_REASON_OK, _REASON_RENEWAL_DUE}
_TASK_POLICY = [
    LICENSE_TASK_REVIEW_FIND,
    LICENSE_TASK_REVIEW_FULL_SCAN,
    LICENSE_TASK_QUALITY_REFUND,
    LICENSE_TASK_BATCH_DELIVERY,
    LICENSE_TASK_CACHE_MANAGE,
]


@dataclass
class RuntimeState:
    license_key: str = ""
    device_id: str = ""
    reason: str = _REASON_NOT_FOUND
    status_hint: str = _REASON_NOT_FOUND
    license_expires_at: str = ""
    lease_expires_at: str = ""
    renew_after: str = ""
    last_verify_at: str = ""
    risk_level: str = "low"
    task_policy: list[str] = field(default_factory=list)
    compromised: bool = False
    runtime_backend: str = "python"

    def to_info(self) -> dict:
        info = {
            "license_version": LICENSE_PROTOCOL_VERSION,
            "license_key": self.license_key,
            "key": self.license_key,
            "device_id": self.device_id,
            "device_id_suffix": self.device_id[-6:] if self.device_id else "",
            "license_expires_at": self.license_expires_at,
            "expires_at": self.license_expires_at,
            "lease_expires_at": self.lease_expires_at,
            "renew_after": self.renew_after,
            "issued_at": self.last_verify_at,
            "last_verify_at": self.last_verify_at,
            "status_hint": self.status_hint,
            "risk_level": self.risk_level,
            "task_policy": list(self.task_policy),
            "runtime_backend": self.runtime_backend,
            "compromised": self.compromised,
        }
        return info


@dataclass
class RuntimeGrant:
    task_type: str
    granted: bool
    grant_id: str = ""
    valid_until: str = ""
    risk_level: str = "low"
    degraded_reason: str = ""
    state: RuntimeState | None = None


# ---------------------------------------------------------------------------
# 时间 / 编码 / 设备
# ---------------------------------------------------------------------------


def _resolve_data_root() -> str:
    return os.path.join(os.path.expanduser("~"), CONFIG_DIR_NAME)



def _extract_first_non_header_line(output: str, *headers: str) -> str | None:
    ignored = {header.upper() for header in headers}
    for line in output.strip().splitlines():
        normalized = line.strip()
        if normalized and normalized.upper() not in ignored:
            return normalized
    return None



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



def get_device_id() -> str:
    raw = _collect_raw_fingerprint()
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]



def _b64url_encode(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")



def _b64url_decode(value: str) -> bytes:
    padding = "=" * ((4 - len(value) % 4) % 4)
    return base64.urlsafe_b64decode(value + padding)



def _load_public_key(raw_key: str | None = None) -> Ed25519PublicKey:
    return Ed25519PublicKey.from_public_bytes(_b64url_decode(raw_key or LICENSE_PUBLIC_KEY))



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


# ---------------------------------------------------------------------------
# token / 存储
# ---------------------------------------------------------------------------


def verify_signed_lease(token: str, *, expected_device_id: str | None = None, allow_expired: bool = False) -> Optional[dict]:
    try:
        encoded_payload, encoded_sig = token.split(".", 1)
        signature = _b64url_decode(encoded_sig)
        _load_public_key().verify(signature, encoded_payload.encode("utf-8"))
        payload = json.loads(_b64url_decode(encoded_payload).decode("utf-8"))
    except (ValueError, InvalidSignature, json.JSONDecodeError, TypeError):
        return None

    if payload.get("kind") != _TOKEN_KIND_LEASE:
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



def _runtime_public_path() -> Path:
    return get_home_config_dir() / _LICENSE_FILE_NAME



def _runtime_bundle_path() -> Path:
    return get_user_data_dir() / _RUNTIME_BUNDLE_FILE



def _write_public_license_metadata(info: dict) -> None:
    path = _runtime_public_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    public_info = {
        "license_key": info.get("license_key", ""),
        "device_id_suffix": info.get("device_id_suffix", ""),
        "license_expires_at": info.get("license_expires_at", ""),
        "lease_expires_at": info.get("lease_expires_at", ""),
        "renew_after": info.get("renew_after", ""),
        "last_verify_at": info.get("last_verify_at", ""),
        "status_hint": info.get("status_hint", _REASON_NOT_FOUND),
        "risk_level": info.get("risk_level", "low"),
    }
    path.write_text(json.dumps(public_info, ensure_ascii=False, indent=2), encoding="utf-8")



def _read_public_license_metadata() -> dict | None:
    path = _runtime_public_path()
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None



def _select_store_backend() -> str:
    if os.environ.get("TLS_SECURITY_RUNTIME_STORE"):
        return os.environ["TLS_SECURITY_RUNTIME_STORE"]
    if not getattr(sys, "frozen", False):
        return "file"
    if sys.platform == "darwin":
        return "keychain"
    if sys.platform == "win32":
        return "dpapi"
    return "file"



def _store_runtime_bundle(bundle: dict) -> None:
    backend = _select_store_backend()
    if backend == "keychain":
        _store_runtime_bundle_keychain(bundle)
        return
    if backend == "dpapi":
        _store_runtime_bundle_dpapi(bundle)
        return
    _store_runtime_bundle_file(bundle)



def _load_runtime_bundle() -> dict | None:
    backend = _select_store_backend()
    if backend == "keychain":
        return _load_runtime_bundle_keychain()
    if backend == "dpapi":
        return _load_runtime_bundle_dpapi()
    return _load_runtime_bundle_file()



def _clear_runtime_bundle() -> None:
    backend = _select_store_backend()
    if backend == "keychain":
        _clear_runtime_bundle_keychain()
        return
    if backend == "dpapi":
        _clear_runtime_bundle_dpapi()
        return
    _clear_runtime_bundle_file()



def _store_runtime_bundle_file(bundle: dict) -> None:
    path = _runtime_bundle_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(bundle, ensure_ascii=False, indent=2), encoding="utf-8")
    try:
        os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)
    except OSError:
        pass



def _load_runtime_bundle_file() -> dict | None:
    path = _runtime_bundle_path()
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None



def _clear_runtime_bundle_file() -> None:
    _runtime_bundle_path().unlink(missing_ok=True)



def _store_runtime_bundle_keychain(bundle: dict) -> None:
    value = json.dumps(bundle, ensure_ascii=False)
    subprocess.run(
        [
            "security",
            "add-generic-password",
            "-U",
            "-s",
            _KEYCHAIN_SERVICE,
            "-a",
            _KEYCHAIN_ACCOUNT,
            "-w",
            value,
        ],
        check=True,
        capture_output=True,
        text=True,
    )



def _load_runtime_bundle_keychain() -> dict | None:
    try:
        result = subprocess.run(
            ["security", "find-generic-password", "-w", "-s", _KEYCHAIN_SERVICE, "-a", _KEYCHAIN_ACCOUNT],
            check=True,
            capture_output=True,
            text=True,
        )
    except Exception:
        return None
    try:
        return json.loads(result.stdout.strip())
    except Exception:
        return None



def _clear_runtime_bundle_keychain() -> None:
    subprocess.run(
        ["security", "delete-generic-password", "-s", _KEYCHAIN_SERVICE, "-a", _KEYCHAIN_ACCOUNT],
        check=False,
        capture_output=True,
        text=True,
    )


if sys.platform == "win32":
    class DATA_BLOB(ctypes.Structure):
        _fields_ = [("cbData", ctypes.c_ulong), ("pbData", ctypes.POINTER(ctypes.c_char))]


    def _blob_from_bytes(data: bytes) -> DATA_BLOB:
        buffer = ctypes.create_string_buffer(data)
        return DATA_BLOB(len(data), ctypes.cast(buffer, ctypes.POINTER(ctypes.c_char)))


    def _crypt_protect(data: bytes) -> bytes:
        crypt32 = ctypes.windll.crypt32
        kernel32 = ctypes.windll.kernel32
        in_blob = _blob_from_bytes(data)
        out_blob = DATA_BLOB()
        if not crypt32.CryptProtectData(ctypes.byref(in_blob), None, None, None, None, 0, ctypes.byref(out_blob)):
            raise OSError("CryptProtectData failed")
        try:
            return ctypes.string_at(out_blob.pbData, out_blob.cbData)
        finally:
            kernel32.LocalFree(out_blob.pbData)


    def _crypt_unprotect(data: bytes) -> bytes:
        crypt32 = ctypes.windll.crypt32
        kernel32 = ctypes.windll.kernel32
        in_blob = _blob_from_bytes(data)
        out_blob = DATA_BLOB()
        if not crypt32.CryptUnprotectData(ctypes.byref(in_blob), None, None, None, None, 0, ctypes.byref(out_blob)):
            raise OSError("CryptUnprotectData failed")
        try:
            return ctypes.string_at(out_blob.pbData, out_blob.cbData)
        finally:
            kernel32.LocalFree(out_blob.pbData)
else:
    def _crypt_protect(data: bytes) -> bytes:
        return data

    def _crypt_unprotect(data: bytes) -> bytes:
        return data



def _store_runtime_bundle_dpapi(bundle: dict) -> None:
    path = _runtime_bundle_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    encrypted = _crypt_protect(json.dumps(bundle, ensure_ascii=False).encode("utf-8"))
    path.write_bytes(encrypted)



def _load_runtime_bundle_dpapi() -> dict | None:
    path = _runtime_bundle_path()
    if not path.exists():
        return None
    try:
        raw = _crypt_unprotect(path.read_bytes())
        return json.loads(raw.decode("utf-8"))
    except Exception:
        return None



def _clear_runtime_bundle_dpapi() -> None:
    _runtime_bundle_path().unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# 原生安全核 / 完整性
# ---------------------------------------------------------------------------


def _iter_runtime_roots() -> list[Path]:
    roots: list[Path] = []
    runtime_dir = Path(sys.executable).resolve().parent if getattr(sys, "frozen", False) else Path(__file__).resolve().parents[1]
    roots.append(runtime_dir)
    roots.append(Path(__file__).resolve().parents[2])
    if getattr(sys, "frozen", False):
        roots.append(runtime_dir.parent / "Resources")
    unique: list[Path] = []
    seen: set[str] = set()
    for item in roots:
        key = str(item.resolve()) if item.exists() else str(item)
        if key in seen:
            continue
        seen.add(key)
        unique.append(item)
    return unique



def _security_core_library_candidates() -> list[Path]:
    suffix = ".dll" if sys.platform == "win32" else ".dylib" if sys.platform == "darwin" else ".so"
    names = [f"{SECURITY_CORE_LIBRARY_BASENAME}{suffix}"]
    if sys.platform != "win32":
        names.insert(0, f"lib{SECURITY_CORE_LIBRARY_BASENAME}{suffix}")
    candidates: list[Path] = []
    for root in _iter_runtime_roots():
        for name in names:
            candidates.append(root / name)
            candidates.append(root / "security-core" / name)
    return candidates



def _load_native_backend_name() -> tuple[bool, str]:
    for candidate in _security_core_library_candidates():
        if not candidate.exists():
            continue
        try:
            lib = ctypes.CDLL(str(candidate))
            if hasattr(lib, "security_core_backend_name"):
                lib.security_core_backend_name.restype = ctypes.c_char_p
                result = lib.security_core_backend_name()
                name = result.decode("utf-8") if result else "native"
                return True, name
            return True, "native"
        except Exception:
            continue
    return False, "python"



def _canonical_manifest_payload(payload: dict) -> bytes:
    normalized = {
        "version": payload.get("version", 1),
        "generated_at": payload.get("generated_at", ""),
        "files": payload.get("files", []),
    }
    return json.dumps(normalized, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")



def _hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()



def _verify_integrity_manifest() -> dict:
    if not getattr(sys, "frozen", False):
        return {"status": "dev", "message": "development mode"}

    manifest_path = next((root / INTEGRITY_MANIFEST_FILE_NAME for root in _iter_runtime_roots() if (root / INTEGRITY_MANIFEST_FILE_NAME).exists()), None)
    if manifest_path is None:
        return {"status": "compromised", "message": "manifest missing"}
    try:
        payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {"status": "compromised", "message": f"manifest parse error: {exc}"}

    signature = payload.get("signature")
    if not signature or not INTEGRITY_MANIFEST_PUBLIC_KEY:
        return {"status": "compromised", "message": "manifest signature missing"}
    try:
        _load_public_key(INTEGRITY_MANIFEST_PUBLIC_KEY).verify(_b64url_decode(signature), _canonical_manifest_payload(payload))
    except Exception as exc:
        return {"status": "compromised", "message": f"manifest signature invalid: {exc}"}

    base_dir = manifest_path.parent
    for file_info in payload.get("files", []):
        rel = file_info.get("path")
        expected = file_info.get("sha256")
        if not rel or not expected:
            return {"status": "compromised", "message": "manifest entry invalid"}
        target = (base_dir / rel).resolve()
        try:
            if not target.exists() or _hash_file(target) != expected:
                return {"status": "compromised", "message": f"integrity mismatch: {rel}"}
        except Exception as exc:
            return {"status": "compromised", "message": f"integrity error: {rel}: {exc}"}
    return {"status": "ok", "message": "integrity ok"}



def _collect_risk_signals() -> dict:
    issues: list[str] = []
    level = "low"
    if sys.gettrace() is not None:
        issues.append("debugger_attached")
        level = "high"
    suspicious_env = [name for name in ("PYTHONINSPECT", "PYTHONBREAKPOINT") if os.environ.get(name)]
    if suspicious_env:
        issues.append("env:" + ",".join(suspicious_env))
        level = "medium" if level == "low" else level
    native_present, backend_name = _load_native_backend_name()
    if getattr(sys, "frozen", False) and not native_present:
        issues.append("security_core_missing")
        level = "high"
    return {"level": level, "issues": issues, "backend": backend_name}


# ---------------------------------------------------------------------------
# 状态与授权
# ---------------------------------------------------------------------------


def _state_from_metadata(metadata: dict | None, *, reason: str) -> RuntimeState:
    metadata = metadata or {}
    return RuntimeState(
        license_key=str(metadata.get("license_key") or ""),
        reason=reason,
        status_hint=str(metadata.get("status_hint") or reason),
        license_expires_at=str(metadata.get("license_expires_at") or ""),
        lease_expires_at=str(metadata.get("lease_expires_at") or ""),
        renew_after=str(metadata.get("renew_after") or ""),
        last_verify_at=str(metadata.get("last_verify_at") or ""),
        risk_level=str(metadata.get("risk_level") or "low"),
    )



def _payload_to_state(payload: dict, *, risk: dict, integrity: dict, backend_name: str) -> RuntimeState:
    reason = _REASON_OK
    status_hint = _REASON_OK
    device_id = str(payload.get("device_id") or "")
    current_device = get_device_id()
    license_expires_at = str(payload.get("license_expires_at") or "")
    lease_expires_at = str(payload.get("lease_expires_at") or "")
    renew_after = str(payload.get("renew_after") or "")
    license_exp_dt = _parse_datetime(license_expires_at)
    lease_exp_dt = _parse_datetime(lease_expires_at)
    renew_after_dt = _parse_datetime(renew_after)
    now = _now_utc()

    if payload.get("license_status") == "revoked":
        reason = status_hint = _REASON_REVOKED
    elif current_device and device_id and current_device != device_id:
        reason = status_hint = _REASON_DEVICE_MISMATCH
    elif license_exp_dt and now > license_exp_dt:
        reason = status_hint = _REASON_EXPIRED
    elif integrity.get("status") == "compromised":
        reason = status_hint = _REASON_COMPROMISED
    elif lease_exp_dt and now > lease_exp_dt:
        reason = status_hint = _REASON_ONLINE_REFRESH_REQUIRED
    elif renew_after_dt and now >= renew_after_dt:
        reason = status_hint = _REASON_RENEWAL_DUE

    risk_level = risk.get("level") or "low"
    if reason == _REASON_RENEWAL_DUE and risk_level == "low":
        risk_level = "medium"
    if reason == _REASON_COMPROMISED:
        risk_level = "high"

    return RuntimeState(
        license_key=str(payload.get("license_key") or ""),
        device_id=device_id,
        reason=reason,
        status_hint=status_hint,
        license_expires_at=license_expires_at,
        lease_expires_at=lease_expires_at,
        renew_after=renew_after,
        last_verify_at=str(payload.get("issued_at") or ""),
        risk_level=risk_level,
        task_policy=list(payload.get("task_policy") or []),
        compromised=reason == _REASON_COMPROMISED,
        runtime_backend=backend_name,
    )



def load_runtime_state() -> RuntimeState:
    metadata = _read_public_license_metadata()
    bundle = _load_runtime_bundle()
    if not bundle:
        return _state_from_metadata(metadata, reason=_REASON_NOT_FOUND)
    payload = verify_signed_lease(str(bundle.get("lease_token") or ""), allow_expired=True)
    if payload is None:
        return _state_from_metadata(metadata, reason=_REASON_INVALID)
    integrity = _verify_integrity_manifest()
    risk = _collect_risk_signals()
    return _payload_to_state(payload, risk=risk, integrity=integrity, backend_name=risk.get("backend") or "python")



def _request_json(path: str, payload: dict) -> dict:
    last_exc: Exception | None = None
    for base_url in LICENSE_API_BASE_URLS:
        url = f"{base_url}{path}"
        try:
            resp = requests.post(url, json=payload, timeout=LICENSE_API_TIMEOUT)
            try:
                data = resp.json()
            except ValueError as exc:
                raise ValueError(f"请求失败：服务器返回了非 JSON 响应（HTTP {resp.status_code}）") from exc
            if not isinstance(data, dict):
                raise ValueError("请求失败：服务器返回数据格式异常")
            return data
        except requests.RequestException as exc:
            last_exc = exc
            logger.warning("API 请求失败 %s: %s", url, exc)
    detail = str(last_exc) if last_exc else "未知错误"
    if isinstance(last_exc, requests.Timeout):
        raise ValueError(f"请求失败：服务器响应超时（{detail}）")
    raise ValueError(f"请求失败：无法连接服务器（{detail}）")



def _extract_metadata_from_payload(payload: dict, *, device_id: str, status_hint: str | None = None) -> dict:
    return {
        "license_key": str(payload.get("license_key") or "").strip().upper(),
        "device_id_suffix": device_id[-6:] if device_id else "",
        "license_expires_at": str(payload.get("license_expires_at") or ""),
        "lease_expires_at": str(payload.get("lease_expires_at") or ""),
        "renew_after": str(payload.get("renew_after") or ""),
        "last_verify_at": str(payload.get("issued_at") or _to_iso(_now_utc())),
        "status_hint": status_hint or str(payload.get("license_status") or _REASON_OK),
    }



def _persist_lease(payload: dict, *, installation_secret: str | None = None) -> RuntimeState:
    device_id = str(payload.get("device_id") or get_device_id())
    bundle = _load_runtime_bundle() or {}
    _store_runtime_bundle(
        {
            "lease_token": payload["license_lease"],
            "installation_secret": installation_secret or bundle.get("installation_secret") or secrets.token_urlsafe(32),
            "device_id": device_id,
            "last_integrity_summary": _verify_integrity_manifest(),
        }
    )
    _write_public_license_metadata(_extract_metadata_from_payload(payload, device_id=device_id))
    return load_runtime_state()



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
    if not result.get("success") or not result.get("license_lease"):
        raise ValueError(f"激活失败：{result.get('message', '未知错误')}")
    state = _persist_lease(result)
    return state.to_info()



def refresh_lease_if_due(*, force: bool = False) -> RuntimeState:
    state = load_runtime_state()
    bundle = _load_runtime_bundle()
    if not bundle:
        return state
    if not force and state.reason not in {_REASON_RENEWAL_DUE, _REASON_ONLINE_REFRESH_REQUIRED}:
        return state
    if state.reason in {_REASON_NOT_FOUND, _REASON_INVALID, _REASON_EXPIRED, _REASON_DEVICE_MISMATCH, _REASON_REVOKED, _REASON_COMPROMISED}:
        return state

    try:
        result = _request_json(
            "/api/verify",
            {
                "key": state.license_key,
                "device_id": state.device_id or get_device_id(),
                "license_version": LICENSE_PROTOCOL_VERSION,
                "client_version": APP_VERSION,
            },
        )
    except Exception:
        return state

    if result.get("success") and result.get("license_lease"):
        return _persist_lease(result, installation_secret=bundle.get("installation_secret"))

    license_state = str(result.get("license_state") or "").strip()
    if license_state == _REASON_REVOKED:
        deactivate_license()
        return RuntimeState(reason=_REASON_REVOKED, status_hint=_REASON_REVOKED, risk_level="high")
    if license_state in {_REASON_EXPIRED, _REASON_DEVICE_MISMATCH, _REASON_REACTIVATION_REQUIRED}:
        _write_public_license_metadata(
            {
                **(_read_public_license_metadata() or {}),
                "status_hint": license_state,
                "risk_level": "medium" if license_state == _REASON_REACTIVATION_REQUIRED else "high",
            }
        )
        return load_runtime_state()
    return state



def authorize_task(task_type: str) -> RuntimeGrant:
    state = load_runtime_state()
    if state.reason not in _ALLOWED_LOCAL_REASONS:
        return RuntimeGrant(task_type=task_type, granted=False, degraded_reason=state.reason, risk_level=state.risk_level, state=state)
    if task_type not in state.task_policy:
        return RuntimeGrant(task_type=task_type, granted=False, degraded_reason=_REASON_INVALID, risk_level=state.risk_level, state=state)
    if state.risk_level == "high":
        return RuntimeGrant(task_type=task_type, granted=False, degraded_reason=_REASON_COMPROMISED, risk_level=state.risk_level, state=state)

    bundle = _load_runtime_bundle() or {}
    lease_token = str(bundle.get("lease_token") or "")
    installation_secret = str(bundle.get("installation_secret") or "")
    nonce = f"{installation_secret}:{task_type}:{lease_token}:{int(_now_utc().timestamp()) // 300}"
    grant_id = hashlib.sha256(nonce.encode("utf-8")).hexdigest()[:20]
    valid_until = state.lease_expires_at or _to_iso(_now_utc() + timedelta(hours=LICENSE_LEASE_HARD_EXPIRY_HOURS))
    return RuntimeGrant(
        task_type=task_type,
        granted=True,
        grant_id=grant_id,
        valid_until=valid_until,
        risk_level=state.risk_level,
        degraded_reason="" if state.reason == _REASON_OK else state.reason,
        state=state,
    )



def validate_runtime_continuity(task_type: str, grant: RuntimeGrant | None) -> RuntimeState:
    state = load_runtime_state()
    if grant is None or not grant.granted:
        return state
    if state.reason not in _ALLOWED_LOCAL_REASONS:
        return state
    if task_type not in state.task_policy:
        return RuntimeState(**{**state.__dict__, "reason": _REASON_INVALID, "status_hint": _REASON_INVALID, "risk_level": "high"})
    if _parse_datetime(grant.valid_until) and _now_utc() > _parse_datetime(grant.valid_until):
        return RuntimeState(**{**state.__dict__, "reason": _REASON_ONLINE_REFRESH_REQUIRED, "status_hint": _REASON_ONLINE_REFRESH_REQUIRED, "risk_level": "medium"})
    return state



def check_stored_license_local() -> tuple[Optional[dict], str]:
    state = load_runtime_state()
    info = state.to_info() if state.license_key else (_read_public_license_metadata() or None)
    return info, state.reason



def check_stored_license() -> tuple[Optional[dict], str]:
    state = refresh_lease_if_due(force=True)
    info = state.to_info() if state.license_key else (_read_public_license_metadata() or None)
    return info, state.reason



def issue_or_refresh_session_token(task_type: str, *, force: bool = False) -> tuple[Optional[dict], str]:
    if force:
        state = refresh_lease_if_due(force=False)
        if state.reason == _REASON_RENEWAL_DUE:
            state = refresh_lease_if_due(force=True)
    grant = authorize_task(task_type)
    if grant.granted and grant.state is not None:
        return grant.state.to_info(), grant.state.reason
    state = grant.state or load_runtime_state()
    return state.to_info() if state.license_key else None, grant.degraded_reason or state.reason



def get_license_info() -> Optional[dict]:
    state = load_runtime_state()
    if state.license_key:
        return state.to_info()
    return _read_public_license_metadata()



def deactivate_license() -> None:
    _clear_runtime_bundle()
    _runtime_public_path().unlink(missing_ok=True)
