import os
import sys
import unittest
from pathlib import Path
from unittest import mock

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from PySide6.QtWidgets import QApplication

from ui.window import MainWindow


class WindowStatusPanelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.window = MainWindow(license_reason="ok", license_info={})
        self.window.show()
        self.app.processEvents()

    def tearDown(self):
        self.window.close()
        self.app.processEvents()

    def test_refresh_product_status_panels_should_render_checklist_and_history(self):
        self.window._update_status = {
            "checked": True,
            "has_update": True,
            "version": "4.4.0",
            "message": "发现新版本，请及时升级",
        }
        self.window._task_history_entries = [
            {
                "summary": "中差评查找完成：共 5 条差评，匹配到 4 个订单。",
                "task_label": "中差评查找",
                "status_text": "完成",
                "triggered_at": "2026-04-15 10:00:00",
                "detail": "高置信度 3 条，低置信度 1 条。",
            }
        ]
        self.window._latest_task_rows = [{"task_label": "中差评查找"}]

        with mock.patch("ui.window.resolve_config_dir", return_value="/tmp/config"), mock.patch(
            "ui.window.get_config_dir_cache",
            return_value="/tmp/config",
        ), mock.patch.object(
            self.window,
            "_resolve_cache_status_html",
            return_value=("缓存状态良好，可直接优先使用", "订单数：12<br>最近成功同步：2026-04-15 09:30"),
        ):
            self.window.refresh_product_status_panels()

        self.assertIn("授权状态", self.window.startup_checklist_label.text())
        self.assertIn("发现新版本 4.4.0", self.window.startup_checklist_label.text())
        self.assertIn("缓存状态良好", self.window.cache_status_label.text())
        self.assertIn("中差评查找", self.window.task_history_label.text())
        self.assertTrue(self.window.export_task_button.isEnabled())


if __name__ == "__main__":
    unittest.main()
