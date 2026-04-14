import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from services import update_service
from services.versioning import is_newer_version, parse_version


class VersioningTests(unittest.TestCase):
    def test_parse_version_pads_missing_segments(self):
        self.assertEqual(parse_version("4.3"), (4, 3, 0))
        self.assertEqual(parse_version("4.4.1"), (4, 4, 1))

    def test_is_newer_version_compares_semver_like_values(self):
        self.assertTrue(is_newer_version("4.3", "4.4.0"))
        self.assertFalse(is_newer_version("4.4.0", "4.4.0"))
        self.assertFalse(is_newer_version("4.5.0", "4.4.9"))


class UpdateServiceTests(unittest.TestCase):
    def test_detect_platform_maps_darwin_and_windows(self):
        with mock.patch.object(update_service.sys, 'platform', 'darwin'):
            self.assertEqual(update_service.detect_platform(), 'mac')
        with mock.patch.object(update_service.os, 'name', 'nt'), mock.patch.object(update_service.sys, 'platform', 'win32'):
            self.assertEqual(update_service.detect_platform(), 'windows')

    def test_fetch_latest_version_info_returns_parsed_payload(self):
        fake_response = mock.Mock()
        fake_response.json.return_value = {
            'app': 'TLS-shipinhao',
            'version': '4.4.0',
            'build': 20260414,
            'mandatory': False,
            'mac': {'url': 'https://example.com/mac.zip'},
            'windows': {'url': 'https://example.com/win.zip'},
        }
        fake_response.raise_for_status.return_value = None

        with mock.patch.object(update_service.requests, 'get', return_value=fake_response) as get_mock, mock.patch.object(
            update_service,
            'detect_platform',
            return_value='mac',
        ):
            info = update_service.fetch_latest_version_info('4.3')

        self.assertEqual(info.version, '4.4.0')
        self.assertEqual(info.platform, 'mac')
        self.assertEqual(info.download_url, 'https://example.com/mac.zip')
        self.assertTrue(info.has_update)
        get_mock.assert_called_once()


if __name__ == '__main__':
    unittest.main()
