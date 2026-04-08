import os
import sys
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

ROOT = Path(__file__).resolve().parents[1]
APP_ROOT = ROOT / "app"
if str(APP_ROOT) not in sys.path:
    sys.path.insert(0, str(APP_ROOT))

from PySide6.QtWidgets import QApplication, QHBoxLayout, QMessageBox, QSizePolicy

from src.constants import ROW_GAP, get_platform_default_window_size, scale_px
from src.ui.window import MainWindow
from src.ui import widgets as widgets_module
from src.ui.widgets import LicenseDialog


class ReviewButtonRowsTests(unittest.TestCase):
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

    def test_review_buttons_are_grouped_into_two_horizontal_rows(self):
        first_row_parent = self.window.review_find_button.parentWidget()
        second_button_parent = self.window.quality_refund_button.parentWidget()
        third_button_parent = self.window.review_full_scan_button.parentWidget()
        fourth_button_parent = self.window.order_cache_button.parentWidget()

        self.assertIs(
            first_row_parent,
            second_button_parent,
            "获取差评订单 和 获取品退订单 应放在同一行。",
        )
        self.assertIs(
            third_button_parent,
            fourth_button_parent,
            "完整补查订单 和 订单缓存管理 应放在同一行。",
        )
        self.assertIsNot(
            first_row_parent,
            third_button_parent,
            "两组按钮应拆成两行，而不是共用同一个容器。",
        )
        self.assertIsInstance(
            first_row_parent.layout(),
            QHBoxLayout,
            "每一行按钮都应使用横向布局。",
        )

    def test_primary_panel_layout_spacings_are_standardized(self):
        expected_spacing = scale_px(ROW_GAP, min_value=8)
        layout_names = [
            "left_column_layout",
            "right_column_layout",
            "setup_content_layout",
            "config_content_layout",
            "review_content_layout",
            "action_content_layout",
        ]

        for layout_name in layout_names:
            layout = getattr(self.window, layout_name, None)
            self.assertIsNotNone(layout, f"{layout_name} 应暴露为可复用布局对象。")
            self.assertEqual(
                layout.spacing(),
                expected_spacing,
                f"{layout_name} 的间距应统一为标准值。",
            )

    def test_input_cards_are_compact_enough_to_align_with_left_column(self):
        alignment_tolerance = scale_px(24, min_value=18)

        self.assertLessEqual(
            abs(self.window.config_card.height() - self.window.order_card.height()),
            alignment_tolerance,
            "默认窗口下，订单号卡片不应明显高于左侧配置卡。",
        )
        self.assertLessEqual(
            abs(self.window.config_card.height() - self.window.tracking_card.height()),
            alignment_tolerance,
            "默认窗口下，物流单号卡片不应明显高于左侧配置卡。",
        )
        self.assertLessEqual(
            abs(self.window.action_card.y() - self.window.log_card.y()),
            alignment_tolerance,
            "默认窗口下，日志区起点应与左侧执行区基本对齐。",
        )

    def test_columns_expand_through_log_panel_and_bottom_spacer_only(self):
        last_left_item = self.window.left_column_layout.itemAt(self.window.left_column_layout.count() - 1)

        self.assertEqual(
            self.window.right_column_layout.stretch(0),
            0,
            "右侧上半区输入卡片不应占用伸展高度。",
        )
        self.assertEqual(
            self.window.right_column_layout.stretch(1),
            1,
            "右侧剩余高度应优先分配给日志区。",
        )
        self.assertEqual(
            self.window.order_card.sizePolicy().verticalPolicy(),
            QSizePolicy.Maximum,
            "订单号卡片应保持自然高度，而不是纵向拉伸。",
        )
        self.assertEqual(
            self.window.tracking_card.sizePolicy().verticalPolicy(),
            QSizePolicy.Maximum,
            "物流单号卡片应保持自然高度，而不是纵向拉伸。",
        )
        self.assertEqual(
            self.window.license_card.sizePolicy().verticalPolicy(),
            QSizePolicy.Maximum,
            "授权卡应保持自然高度，左列多余空间应由底部 spacer 吸收。",
        )
        self.assertIsNotNone(
            last_left_item.spacerItem(),
            "左列最后一个布局项应为底部 spacer，而不是让授权卡纵向撑高。",
        )

    def test_window_uses_locked_default_size(self):
        helper_width, helper_height = get_platform_default_window_size()
        expected_width, initial_height = self.window._resolve_initial_window_size()

        self.assertEqual(
            (helper_width, helper_height),
            (880, 830),
            "Mac/Windows 统一默认尺寸为 880×830。",
        )

        self.assertEqual(
            self.window.width(),
            expected_width,
            "主窗口宽度应锁定为解析后的初始宽度。",
        )
        self.assertEqual(
            self.window.height(),
            self.window.minimumHeight(),
            "主窗口高度应锁定为最终收口后的固定高度。",
        )
        self.assertEqual(
            self.window.minimumWidth(),
            expected_width,
            "禁止缩放后，最小宽度应锁定为默认宽度。",
        )
        self.assertEqual(
            self.window.maximumWidth(),
            expected_width,
            "禁止缩放后，最大宽度应锁定为默认宽度。",
        )
        self.assertEqual(
            self.window.minimumHeight(),
            self.window.maximumHeight(),
            "禁止缩放后，高度上下限应锁定为同一个最终值。",
        )
        self.assertLessEqual(
            self.window.height(),
            initial_height,
            "最终固定高度不应高于解析后的初始高度。",
        )

    def test_window_cannot_be_resized_after_creation(self):
        baseline_size = self.window.size()

        self.window.resize(baseline_size.width() + 120, baseline_size.height() + 120)
        self.app.processEvents()

        self.assertEqual(
            self.window.size(),
            baseline_size,
            "窗口创建后不应再允许放大或缩小。",
        )

    def test_oversized_default_height_is_shrunk_before_window_is_locked(self):
        class OversizedWindow(MainWindow):
            def _resolve_initial_window_size(self):
                return 810, 920

        oversized_window = OversizedWindow(license_reason="ok", license_info={})
        oversized_window.show()
        self.app.processEvents()

        self.assertLess(
            oversized_window.height(),
            920,
            "如果默认高度明显高于内容，首次展示时应先收口，再锁定窗口尺寸。",
        )
        self.assertEqual(
            oversized_window.minimumHeight(),
            oversized_window.maximumHeight(),
            "收口完成后，高度应被锁定。",
        )

        oversized_window.close()
        self.app.processEvents()


class DialogSpacingTests(unittest.TestCase):
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

    def test_license_dialog_uses_standard_spacing_system(self):
        dialog = LicenseDialog(self.window, reason="not_found")
        get_dialog_content_margins = getattr(widgets_module, "get_dialog_content_margins", None)
        get_dialog_section_spacing = getattr(widgets_module, "get_dialog_section_spacing", None)
        get_dialog_action_spacing = getattr(widgets_module, "get_dialog_action_spacing", None)

        self.assertIsNotNone(get_dialog_content_margins, "应提供统一的弹窗外边距 helper。")
        self.assertIsNotNone(get_dialog_section_spacing, "应提供统一的弹窗主间距 helper。")
        self.assertIsNotNone(get_dialog_action_spacing, "应提供统一的弹窗按钮区间距 helper。")
        margins = get_dialog_content_margins()

        self.assertEqual(
            dialog.root_layout.contentsMargins().left(),
            margins[0],
            "LicenseDialog 左侧外边距应走统一 helper。",
        )
        self.assertEqual(
            dialog.root_layout.spacing(),
            get_dialog_section_spacing(),
            "LicenseDialog 主体间距应走统一 helper。",
        )
        self.assertEqual(
            dialog.action_row_layout.spacing(),
            get_dialog_action_spacing(),
            "LicenseDialog 按钮区间距应走统一 helper。",
        )

        dialog.close()

    def test_message_dialog_uses_standard_spacing_system(self):
        dialog, _actions = self.window._create_message_dialog_base(
            QMessageBox.Information,
            "标题",
            "正文",
            "补充信息",
        )
        get_dialog_content_margins = getattr(widgets_module, "get_dialog_content_margins", None)
        get_dialog_section_spacing = getattr(widgets_module, "get_dialog_section_spacing", None)
        get_dialog_action_spacing = getattr(widgets_module, "get_dialog_action_spacing", None)
        get_dialog_text_spacing = getattr(widgets_module, "get_dialog_text_spacing", None)

        self.assertIsNotNone(get_dialog_content_margins, "应提供统一的弹窗外边距 helper。")
        self.assertIsNotNone(get_dialog_section_spacing, "应提供统一的弹窗主间距 helper。")
        self.assertIsNotNone(get_dialog_action_spacing, "应提供统一的弹窗按钮区间距 helper。")
        self.assertIsNotNone(get_dialog_text_spacing, "应提供统一的弹窗文案区间距 helper。")
        margins = get_dialog_content_margins()

        self.assertEqual(
            dialog.root_layout.contentsMargins().top(),
            margins[1],
            "消息框顶部外边距应走统一 helper。",
        )
        self.assertEqual(
            dialog.root_layout.spacing(),
            get_dialog_section_spacing(),
            "消息框主容器间距应走统一 helper。",
        )
        self.assertEqual(
            dialog.body_layout.spacing(),
            get_dialog_section_spacing(),
            "消息框主体图标/文案区间距应走统一 helper。",
        )
        self.assertEqual(
            dialog.text_layout.spacing(),
            get_dialog_text_spacing(),
            "消息框文案堆叠间距应走统一 helper。",
        )
        self.assertEqual(
            dialog.actions_layout.spacing(),
            get_dialog_action_spacing(),
            "消息框按钮区间距应走统一 helper。",
        )

        dialog.close()


if __name__ == "__main__":
    unittest.main()
