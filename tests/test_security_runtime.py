import base64
import json
import os
import shutil
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from core import security_runtime


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")


def _make_signing_key():
    return Ed25519PrivateKey.from_private_bytes(bytes(range(33, 65)))


def _sign_payload(payload: dict) -> str:
    encoded = _b64url(json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8"))
    signature = _make_signing_key().sign(encoded.encode("utf-8"))
    return f"{encoded}.{_b64url(signature)}"


def _public_key_b64() -> str:
    public_bytes = _make_signing_key().public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    return _b64url(public_bytes)


class SecurityRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.tmpdir = Path(tempfile.mkdtemp(prefix="security-runtime-"))
        self.addCleanup(lambda: shutil.rmtree(self.tmpdir, ignore_errors=True))
        self.device_id = "11223322eacf"
        self.now_ts = int(time.time())
        self.payload = {
            "kind": "license_lease",
            "issuer": "tls-license-backend",
            "license_key": "TLS-LEASE-TEST",
            "device_id": self.device_id,
            "license_status": "active",
            "license_expires_at": "2120-07-18T00:18:13+00:00",
            "lease_expires_at": "2120-07-17T00:18:13+00:00",
            "renew_after": "2000-01-01T00:00:00+00:00",
            "task_policy": ["review_find", "batch_delivery"],
            "keyset_version": 1,
            "binding_version": 3,
            "issued_at": "2026-03-10T00:18:13+00:00",
            "iat": self.now_ts,
            "exp": self.now_ts + 86400,
        }
        self.lease_token = _sign_payload(self.payload)

        patchers = [
            mock.patch.object(security_runtime, "LICENSE_PUBLIC_KEY", _public_key_b64(), create=True),
            mock.patch.object(security_runtime, "get_home_config_dir", return_value=self.tmpdir),
            mock.patch.object(security_runtime, "get_user_data_dir", return_value=self.tmpdir),
            mock.patch.object(security_runtime, "get_device_id", return_value=self.device_id),
        ]
        for patcher in patchers:
            patcher.start()
            self.addCleanup(patcher.stop)

    def _seed_runtime(self):
        security_runtime._store_runtime_bundle(
            {
                "lease_token": self.lease_token,
                "installation_secret": "installation-secret",
                "device_id": self.device_id,
                "last_integrity_summary": {"status": "ok"},
            }
        )
        security_runtime._write_public_license_metadata(
            {
                "license_key": "TLS-LEASE-TEST",
                "device_id_suffix": self.device_id[-6:],
                "license_expires_at": self.payload["license_expires_at"],
                "lease_expires_at": self.payload["lease_expires_at"],
                "last_verify_at": self.payload["issued_at"],
                "status_hint": "active",
            }
        )

    def test_load_runtime_state_marks_renewal_due_when_renew_after_has_passed(self):
        self._seed_runtime()

        state = security_runtime.load_runtime_state()

        self.assertEqual(state.reason, "renewal_due")
        self.assertEqual(state.status_hint, "renewal_due")
        self.assertEqual(state.license_key, "TLS-LEASE-TEST")

    def test_authorize_task_derives_runtime_grant_from_local_lease(self):
        self._seed_runtime()

        grant = security_runtime.authorize_task("review_find")

        self.assertTrue(grant.granted)
        self.assertEqual(grant.task_type, "review_find")
        self.assertTrue(grant.grant_id)
        self.assertEqual(grant.risk_level, "medium")

    def test_refresh_lease_if_due_keeps_local_state_when_network_fails_but_lease_is_valid(self):
        self._seed_runtime()

        with mock.patch.object(security_runtime, "_request_json", side_effect=ValueError("network down")):
            state = security_runtime.refresh_lease_if_due(force=True)

        self.assertEqual(state.reason, "renewal_due")
        self.assertEqual(state.status_hint, "renewal_due")

    def test_public_metadata_never_persists_raw_lease_token(self):
        self._seed_runtime()

        metadata = json.loads((self.tmpdir / "license.json").read_text(encoding="utf-8"))

        self.assertNotIn("lease_token", metadata)
        self.assertEqual(metadata["device_id_suffix"], self.device_id[-6:])


if __name__ == "__main__":
    unittest.main()
