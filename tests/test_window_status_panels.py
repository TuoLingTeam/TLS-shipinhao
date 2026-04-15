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
        self.window._latest_result_insight = "差评匹配结果解读\n- 高置信度：3\n- 低置信度：1"

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
        self.assertIn("高置信度：3", self.window.result_insight_view.toPlainText())
        self.assertTrue(self.window.export_task_button.isEnabled())

    def test_review_results_should_render_result_table(self):
        self.window.review_task_type = "review_match"
        sample_results = [
            {
                "evaluationId": "E1",
                "orderId": "O1001",
                "matchScore": 100,
                "matchStrategy": "exact",
                "buyerNickname": "alice",
                "matchReasons": ["标题完全匹配"],
                "matched": True,
                "productName": "课程A",
            },
            {
                "evaluationId": "E2",
                "orderId": "",
                "matchScore": 0,
                "matchStrategy": "",
                "buyerNickname": "bob",
                "matchReasons": [],
                "matched": False,
                "productName": "课程B",
            },
        ]

        self.window._on_review_results_ready(sample_results)

        self.assertEqual(self.window.result_table.rowCount(), 2)
        self.assertEqual(self.window.result_table.columnCount(), 7)
        self.assertEqual(self.window.result_table.item(0, 0).text(), "高置信度")
        self.assertEqual(self.window.result_table.item(0, 1).text(), "O1001")
        self.assertEqual(self.window.result_table.item(1, 0).text(), "未匹配")
        self.assertIn("建议完整补查", self.window.result_table.item(1, 6).text())

    def test_result_table_filter_should_reduce_visible_rows(self):
        self.window._set_latest_result_table(
            columns=[("confidence_bucket", "分组"), ("order_id", "订单号")],
            rows=[
                {"confidence_bucket": "高置信度", "order_id": "A1"},
                {"confidence_bucket": "低置信度", "order_id": "A2"},
                {"confidence_bucket": "未匹配", "order_id": ""},
            ],
        )

        self.window.result_filter_combo.setCurrentText("仅看低置信度")
        self.window._apply_result_table_filter()

        self.assertEqual(self.window.result_table.rowCount(), 1)
        self.assertEqual(self.window.result_table.item(0, 0).text(), "低置信度")

    def test_copy_result_table_selection_should_write_clipboard_text(self):
        self.window._set_latest_result_table(
            columns=[("confidence_bucket", "分组"), ("order_id", "订单号")],
            rows=[{"confidence_bucket": "高置信度", "order_id": "A1"}],
        )
        self.window.result_table.selectRow(0)

        self.window.copy_selected_result_row()

        self.assertIn("高置信度", self.app.clipboard().text())
        self.assertIn("A1", self.app.clipboard().text())

    def test_click_result_row_should_backfill_order_id(self):
        self.window._set_latest_result_table(
            columns=[("confidence_bucket", "分组"), ("order_id", "订单号")],
            rows=[{"confidence_bucket": "高置信度", "order_id": "ORDER-1"}],
        )

        self.window._apply_result_row_to_order_input(0)

        self.assertIn("ORDER-1", self.window.order_edit.toPlainText())


if __name__ == "__main__":
    unittest.main()
