import base64
import json
import os
import sys
import tempfile
import time
import unittest
from datetime import datetime
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

from core import security_runtime
from core import license as license_module
from ui.window import MainWindow


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")


def _make_signing_key():
    return Ed25519PrivateKey.from_private_bytes(bytes(range(11, 43)))


def _public_key_b64() -> str:
    public_bytes = _make_signing_key().public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    return _b64url(public_bytes)


def _sign_payload(payload: dict) -> str:
    encoded_payload = _b64url(json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8"))
    signature = _make_signing_key().sign(encoded_payload.encode("utf-8"))
    return f"{encoded_payload}.{_b64url(signature)}"


def make_lease_payload(now_ts: int | None = None):
    now_ts = now_ts or int(time.time())
    return {
        "kind": "license_lease",
        "issuer": "tls-license-backend",
        "license_key": "TLS-Q2-TEST",
        "device_id": "11223322eacf",
        "license_status": "active",
        "license_expires_at": "2120-07-18T00:18:13+00:00",
        "lease_expires_at": "2120-07-17T00:18:13+00:00",
        "renew_after": "2120-07-16T00:18:13+00:00",
        "task_policy": ["review_find", "batch_delivery"],
        "keyset_version": 1,
        "binding_version": 3,
        "issued_at": "2026-03-10T00:18:13+00:00",
        "iat": now_ts,
        "exp": now_ts + 86400,
    }


def make_license_info():
    payload = make_lease_payload()
    return {
        "license_version": 3,
        "license_key": payload["license_key"],
        "key": payload["license_key"],
        "license_expires_at": payload["license_expires_at"],
        "expires_at": payload["license_expires_at"],
        "lease_expires_at": payload["lease_expires_at"],
        "renew_after": payload["renew_after"],
        "device_id": payload["device_id"],
        "issued_at": payload["issued_at"],
        "task_policy": payload["task_policy"],
        "status_hint": "ok",
    }


class LicenseFlowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.tmpdir = Path(tempfile.mkdtemp(prefix="license-flow-"))
        patchers = [
            mock.patch.object(security_runtime, "LICENSE_PUBLIC_KEY", _public_key_b64(), create=True),
            mock.patch.object(license_module, "verify_signed_claims", security_runtime.verify_signed_lease),
            mock.patch.object(security_runtime, "get_home_config_dir", return_value=self.tmpdir),
            mock.patch.object(security_runtime, "get_user_data_dir", return_value=self.tmpdir),
            mock.patch.object(security_runtime, "get_device_id", return_value="11223322eacf"),
        ]
        for patcher in patchers:
            patcher.start()
            self.addCleanup(patcher.stop)

        self.window = MainWindow(license_reason="ok", license_info=make_license_info())
        self.window.show()
        self.app.processEvents()

    def tearDown(self):
        self.window.close()
        self.app.processEvents()

    def test_missing_runtime_bundle_should_require_activation(self):
        _, reason = license_module.check_stored_license_local()
        self.assertEqual(reason, "not_found")

    def test_verify_signed_claims_should_accept_valid_lease_token(self):
        token = _sign_payload(make_lease_payload())
        payload = license_module.verify_signed_claims(token, expected_device_id="11223322eacf")
        self.assertEqual(payload["license_key"], "TLS-Q2-TEST")
        self.assertEqual(payload["device_id"], "11223322eacf")

    def test_review_find_click_should_authorize_task_before_starting_worker(self):
        self.window._license_state_cache = {
            "info": make_license_info(),
            "reason": "ok",
            "checked_at": time.monotonic(),
            "source": "initial",
        }

        fake_state = mock.Mock()
        fake_state.reason = "ok"
        fake_state.to_info.return_value = make_license_info()
        fake_grant = mock.Mock(granted=True, state=fake_state, degraded_reason="", task_type="review_find")

        with mock.patch("ui.window.authorize_task", return_value=fake_grant) as authorize_mock, mock.patch.object(
            self.window,
            "_start_review_worker",
        ) as start_mock:
            self.window.on_review_find_clicked()

        authorize_mock.assert_called_once()
        start_mock.assert_called_once()

    def test_license_expiry_hint_should_warn_when_expiring_soon(self):
        self.window._license_info = make_license_info() | {
            "expires_at": "2026-04-20T00:00:00+00:00",
            "license_expires_at": "2026-04-20T00:00:00+00:00",
        }
        self.window._license_reason = "ok"

        with mock.patch("ui.window.datetime") as dt_mock:
            dt_mock.now.return_value = datetime.fromisoformat("2026-04-15T00:00:00+00:00")
            dt_mock.fromisoformat.side_effect = datetime.fromisoformat
            hint = self.window._build_license_expiry_hint()

        self.assertIn("建议提前续费", hint)


if __name__ == "__main__":
    unittest.main()
