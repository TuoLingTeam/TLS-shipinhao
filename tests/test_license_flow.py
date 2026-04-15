import base64
import json
import os
import sys
import time
import unittest
from pathlib import Path
from unittest import mock

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from PySide6.QtWidgets import QApplication
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

from core import license as license_module
from ui.window import MainWindow


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")


def _make_signing_key():
    return Ed25519PrivateKey.from_private_bytes(bytes(range(1, 33)))


def _sign_payload(payload: dict) -> str:
    encoded_payload = _b64url(json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8"))
    signature = _make_signing_key().sign(encoded_payload.encode("utf-8"))
    return f"{encoded_payload}.{_b64url(signature)}"


def _public_key_b64() -> str:
    public_bytes = _make_signing_key().public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    return _b64url(public_bytes)


def make_v2_license_info(now_ts: int | None = None):
    now_ts = now_ts or int(time.time())
    license_expires_at = "2120-07-18T00:18:13+00:00"
    offline_payload = {
        "kind": "offline_grant",
        "issuer": "tls-license-backend",
        "license_key": "TLS-Q2-TEST",
        "device_id": "11223322eacf",
        "iat": now_ts,
        "exp": now_ts + 3600,
        "license_expires_at": license_expires_at,
    }
    device_payload = {
        "kind": "device_claims",
        "issuer": "tls-license-backend",
        "license_key": "TLS-Q2-TEST",
        "device_id": "11223322eacf",
        "iat": now_ts,
        "exp": now_ts + 86400,
        "license_expires_at": license_expires_at,
        "binding_version": 2,
    }
    session_payload = {
        "kind": "session_token",
        "issuer": "tls-license-backend",
        "license_key": "TLS-Q2-TEST",
        "device_id": "11223322eacf",
        "task_type": "review_find",
        "session_id": "sess-test",
        "iat": now_ts,
        "exp": now_ts + 600,
    }
    return {
        "license_version": 2,
        "key": "TLS-Q2-TEST",
        "license_key": "TLS-Q2-TEST",
        "expires_at": license_expires_at,
        "license_expires_at": license_expires_at,
        "device_id": "11223322eacf",
        "activated_at": "2026-03-10T00:18:13+00:00",
        "plan_days": 34463,
        "device_claims": _sign_payload(device_payload),
        "device_claims_expires_at": "2120-07-17T00:18:13+00:00",
        "offline_grant": _sign_payload(offline_payload),
        "offline_grant_expires_at": "2120-07-17T00:18:13+00:00",
        "session_token": _sign_payload(session_payload),
        "session_token_expires_at": "2120-07-17T00:18:13+00:00",
        "issuer": "tls-license-backend",
        "issued_at": "2026-03-10T00:18:13+00:00",
    }


class LicenseFlowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        patcher = mock.patch.object(license_module, "LICENSE_PUBLIC_KEY", _public_key_b64(), create=True)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.window = MainWindow(license_reason="ok", license_info=make_v2_license_info())
        self.window.show()
        self.app.processEvents()

    def tearDown(self):
        self.window.close()
        self.app.processEvents()

    def test_old_license_file_should_require_online_reactivation(self):
        old_info = {
            "key": "TLS-OLD",
            "device_id": "11223322eacf",
            "expires_at": "2120-07-18T00:18:13+00:00",
            "signature": "legacy-signature",
        }

        with mock.patch.object(license_module.os.path, "isfile", return_value=True), mock.patch.object(
            license_module,
            "_read_license_file",
            return_value=old_info,
        ):
            _, reason = license_module.check_stored_license_local()

        self.assertEqual(reason, "reactivation_required")

    def test_verify_signed_claims_should_accept_valid_ed25519_token(self):
        info = make_v2_license_info()

        payload = license_module.verify_signed_claims(info["offline_grant"], expected_kind="offline_grant")

        self.assertEqual(payload["license_key"], info["license_key"])
        self.assertEqual(payload["device_id"], info["device_id"])

    def test_review_find_click_should_request_task_session_before_starting_worker(self):
        self.window._license_state_cache = {
            "info": make_v2_license_info(),
            "reason": "ok",
            "checked_at": time.monotonic(),
            "source": "initial",
        }

        with mock.patch("ui.window.issue_or_refresh_session_token", create=True, return_value=(make_v2_license_info(), "ok")) as issue_mock, mock.patch.object(
            self.window,
            "_start_review_worker",
        ) as start_mock:
            self.window.on_review_find_clicked()

        issue_mock.assert_called_once()
        start_mock.assert_called_once()

    def test_license_expiry_hint_should_warn_when_expiring_soon(self):
        self.window._license_info = make_v2_license_info() | {
            "expires_at": "2026-04-20T00:00:00+00:00",
            "license_expires_at": "2026-04-20T00:00:00+00:00",
        }
        self.window._license_reason = "ok"

        with mock.patch("ui.window.datetime") as dt_mock:
            dt_mock.now.return_value = license_module.datetime.fromisoformat("2026-04-15T00:00:00+00:00")
            dt_mock.fromisoformat.side_effect = license_module.datetime.fromisoformat

            hint = self.window._build_license_expiry_hint()

        self.assertIn("建议提前续费", hint)


if __name__ == "__main__":
    unittest.main()
