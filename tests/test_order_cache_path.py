# -*- coding: utf-8 -*-

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from services.order_cache import OrderCacheRepository


class OrderCachePathTests(unittest.TestCase):
    def test_default_cache_path_should_use_internal_cache_dir(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            internal_dir = Path(temp_dir) / 'internal-cache'
            with mock.patch('services.order_cache.get_order_cache_dir', return_value=internal_dir):
                with mock.patch('services.order_cache.get_home_config_dir', return_value=Path(temp_dir) / 'legacy-home'):
                    with mock.patch('services.order_cache.get_internal_order_cache_dir', return_value=Path(temp_dir) / 'old-internal-cache'):
                        repo = OrderCacheRepository()
            self.assertEqual(Path(repo.db_path), internal_dir / 'order_cache.sqlite3')

    def test_should_migrate_legacy_cache_into_internal_cache_dir(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = Path(temp_dir)
            internal_dir = temp_root / 'internal-cache'
            legacy_dir = temp_root / 'legacy-home'
            legacy_dir.mkdir(parents=True, exist_ok=True)
            legacy_db = legacy_dir / 'order_cache.sqlite3'
            legacy_db.write_text('legacy-cache', encoding='utf-8')

            with mock.patch('services.order_cache.get_order_cache_dir', return_value=internal_dir):
                with mock.patch('services.order_cache.get_internal_order_cache_dir', return_value=temp_root / 'old-internal-cache'):
                    with mock.patch('services.order_cache.get_home_config_dir', return_value=legacy_dir):
                        repo = OrderCacheRepository()

            self.assertTrue((internal_dir / 'order_cache.sqlite3').exists())
            self.assertFalse(legacy_db.exists())
            self.assertEqual(Path(repo.db_path), internal_dir / 'order_cache.sqlite3')


    def test_should_migrate_old_internal_cache_into_user_data_dir(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = Path(temp_dir)
            user_data_dir = temp_root / 'user-data'
            old_internal_dir = temp_root / 'old-internal-cache'
            old_internal_dir.mkdir(parents=True, exist_ok=True)
            old_db = old_internal_dir / 'order_cache.sqlite3'
            old_db.write_text('old-internal-cache', encoding='utf-8')

            with mock.patch('services.order_cache.get_order_cache_dir', return_value=user_data_dir):
                with mock.patch('services.order_cache.get_internal_order_cache_dir', return_value=old_internal_dir):
                    with mock.patch('services.order_cache.get_home_config_dir', return_value=temp_root / 'legacy-home'):
                        repo = OrderCacheRepository()

            self.assertTrue((user_data_dir / 'order_cache.sqlite3').exists())
            self.assertFalse(old_db.exists())
            self.assertEqual(Path(repo.db_path), user_data_dir / 'order_cache.sqlite3')

    def test_get_summary_should_include_order_count_and_state(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            repo = OrderCacheRepository(db_path=str(Path(temp_dir) / "cache.sqlite3"))
            repo.initialize()
            repo.upsert_orders(
                [
                    {
                        "commonInfo": {"orderId": "A100", "createTime": 1710000000},
                        "buyerInfo": {"nickName": "alice"},
                        "orderProductInfo": [{"title": "课程 A", "saleParam": "红色"}],
                    }
                ]
            )
            repo.save_state(
                coverage_start=1709000000,
                coverage_end=1710000000,
                last_incremental_start=1709900000,
                last_incremental_end=1710000000,
                last_success_at=1710000100,
                last_mode="incremental",
            )

            summary = repo.get_summary()

        self.assertEqual(summary["order_count"], 1)
        self.assertEqual(summary["last_mode"], "incremental")
        self.assertEqual(summary["coverage_end"], 1710000000)


if __name__ == '__main__':
    unittest.main()
