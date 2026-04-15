# -*- coding: utf-8 -*-
"""TLS-shipinhao 主窗口。"""

import os
from datetime import datetime
import sys
import time

from PySide6.QtCore import QObject, Qt, QThread, QUrl, Signal
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (
    QApplication,
    QDialog,
    QFileDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPlainTextEdit,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QSpinBox,
    QStyle,
    QVBoxLayout,
    QWidget,
)

from settings import (
    ConfigNotFoundError,
    extract_biz_magic_from_cookie,
    get_config_dir_cache,
    get_default_config_dir,
    get_saved_user_config_dir,
    parse_batch_input,
    read_cookie_data,
    resolve_config_dir,
    resolve_config_files_in_dir,
    save_cookie_data,
    save_user_config_dir,
)
from settings import (
    APP_COLORS,
    AUTHOR_WECHAT,
    BADGE_HEIGHT,
    BADGE_MIN_WIDTH,
    BADGE_RADIUS,
    CARD_HEADER_GAP,
    CARD_HEADER_HEIGHT,
    CARD_PADDING,
    CARD_RADIUS,
    CONFIG_PATH_MIN_HEIGHT,
    DEFAULT_REVIEW_DAYS,
    FONT_SIZES,
    HERO_PADDING_X,
    HERO_PADDING_Y,
    HERO_RADIUS,
    INPUT_BADGE_HEIGHT,
    INPUT_BADGE_MIN_WIDTH,
    INPUT_BADGE_RADIUS,
    INPUT_EDIT_PADDING,
    INPUT_EDIT_RADIUS,
    INPUT_VISIBLE_LINES,
    LICENSE_REQUIRE_ONLINE_FOR_TASKS,
    LICENSE_STATUS_CACHE_TTL_SECONDS,
    LICENSE_TASK_BATCH_DELIVERY,
    LICENSE_TASK_CACHE_MANAGE,
    LICENSE_TASK_QUALITY_REFUND,
    LICENSE_TASK_REVIEW_FIND,
    LICENSE_TASK_REVIEW_FULL_SCAN,
    LOG_EDIT_PADDING,
    LOG_EDIT_RADIUS,
    LOG_PANEL_MIN_HEIGHT,
    MAX_BATCH_SIZE,
    MAX_UI_SCALE,
    MIN_WINDOW_HEIGHT,
    MIN_WINDOW_WIDTH,
    MIN_UI_SCALE,
    COMPACT_LAYOUT_MIN_WIDTH,
    HIGH_DPI_COMPACT_THRESHOLD,
    ORDER_CACHE_COVERAGE_DAYS,
    ORDER_CACHE_INCREMENTAL_DAYS,
    PAGE_GAP,
    PAGE_MARGIN,
    ROW_GAP,
    SETUP_SECTION_PADDING,
    APP_VERSION,
    VERY_HIGH_DPI_COMPACT_THRESHOLD,
    WIDE_LAYOUT_MIN_HEIGHT,
    WIDE_LAYOUT_MIN_WIDTH,
    WINDOW_TITLE,
    get_platform_default_window_size,
    scale_px,
    set_ui_scale,
)
from core.license import (
    authorize_task,
    check_stored_license,
    check_stored_license_local,
    load_runtime_state,
    validate_runtime_continuity,
)
from ui.window_dialogs import MessagePresenter
from ui.window_view import (
    build_badge_style,
    build_batch_section,
    build_header_section,
    build_license_section,
    build_main_window_stylesheet,
    build_setup_section,
    calculate_editor_height,
    resolve_height_profile,
    resolve_initial_window_size,
    resolve_layout_mode,
    resolve_ui_scale_for_size,
    scaled_padding,
    set_tutorial_badge_link,
    standard_layout_spacing,
)
from ui.widgets import (
    BatchInputEdit,
    LicenseDialog,
    build_fixed_font,
    build_font,
    get_license_reason_text,
    reset_font_caches,
)
from ui.batch_worker import BatchWorker
from ui.update_worker import UpdateCheckWorker
from ui.review_worker import (
    ReviewMatcherWorker,
    TERMINAL_STATUS_CANCELLED,
    TERMINAL_STATUS_ERROR,
    TERMINAL_STATUS_WARNING,
    TASK_CACHE_REBUILD,
    TASK_CACHE_REFRESH,
    TASK_QUALITY_REFUND,
    TASK_REVIEW_FULL_SCAN,
    TASK_REVIEW_MATCH,
)


class LicenseRefreshWorker(QObject):
    """后台授权状态刷新器。"""

    finished = Signal(object, str)
    failed = Signal(str)

    def run(self):
        """在线刷新授权状态。"""
        try:
            info, reason = check_stored_license()
        except Exception as exc:  # noqa: BLE001
            self.failed.emit(str(exc))
            return
        self.finished.emit(info or {}, reason)


class MainWindow(QWidget):
    """主窗口。"""

    update_check_finished_signal = Signal(object, bool)
    update_check_failed_signal = Signal(str, bool)

    def __init__(self, license_reason="ok", license_info=None):
        super().__init__()
        self.worker_thread = None
        self.worker = None
        self.is_paused = False
        self.review_worker_thread = None
        self.review_worker = None
        self.review_task_type = None
        self.license_refresh_thread = None
        self.license_refresh_worker = None
        self.update_check_thread = None
        self.update_check_worker = None
        self._update_prompt_version = None
        self._batch_rows = []
        self._license_reason = license_reason
        self._license_info = license_info or {}
        self._license_state_cache = {
            "info": self._license_info,
            "reason": self._license_reason,
            "checked_at": time.monotonic(),
            "source": "initial",
        }
        self._initial_height_fit_applied = False
        self.update_check_finished_signal.connect(self._on_update_check_finished)
        self.update_check_failed_signal.connect(self._on_update_check_failed)
        self._last_responsive_profile = None
        default_w, default_h = self._resolve_initial_window_size()
        self._ui_scale = self._resolve_ui_scale_for_size(default_w, default_h)
        set_ui_scale(self._ui_scale)
        reset_font_caches()

        self._sync_window_title_with_license(self._license_reason, self._license_info)
        self.setObjectName("AppRoot")
        self.setAttribute(Qt.WA_StyledBackground, True)
        self.setMinimumSize(
            scale_px(MIN_WINDOW_WIDTH, min_value=640),
            scale_px(MIN_WINDOW_HEIGHT, min_value=560),
        )
        self.resize(default_w, default_h)

        self._build_ui()
        self.setStyleSheet(self._build_stylesheet())
        self.refresh_config_path_label()
        self._fit_window_to_screen()
        self.refresh_input_metrics()
        self._sync_responsive_metrics()
        self.refresh_action_buttons()
        self._message_presenter = MessagePresenter(self)

    def _apply_section_refs(self, refs):
        for name, value in vars(refs).items():
            setattr(self, name, value)

    @staticmethod
    def _build_stylesheet():
        """构建全局 QSS 样式表。"""
        return build_main_window_stylesheet()

    # -----------------------------------------------------------------------
    # UI 构建
    # -----------------------------------------------------------------------

    def _build_ui(self):
        """构建主界面骨架。"""
        self._build_root_container()
        self._build_header_card()
        self._build_main_content()

    def _build_root_container(self):
        """创建根容器与可滚动页面。"""
        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self.scroll_area = QScrollArea()
        self.scroll_area.setWidgetResizable(True)
        self.scroll_area.setFrameShape(QFrame.NoFrame)
        self.scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.scroll_area.setStyleSheet("QScrollArea { border: none; background: transparent; }")
        viewport = self.scroll_area.viewport()
        viewport.setObjectName("ScrollViewport")
        viewport.setAttribute(Qt.WA_StyledBackground, True)
        root_layout.addWidget(self.scroll_area)

        self.page_widget = QWidget()
        self.page_widget.setObjectName("PageWidget")
        self.page_widget.setAttribute(Qt.WA_StyledBackground, True)
        self.scroll_area.setWidget(self.page_widget)

        self.page_layout = QVBoxLayout(self.page_widget)
        self.page_layout.setContentsMargins(
            scale_px(PAGE_MARGIN, min_value=10),
            scale_px(PAGE_MARGIN, min_value=10),
            scale_px(PAGE_MARGIN, min_value=10),
            scale_px(PAGE_MARGIN, min_value=10),
        )
        self.page_layout.setSpacing(scale_px(PAGE_GAP, min_value=6))

    def _build_header_card(self):
        """创建顶部标题卡片。"""
        refs = build_header_section(self)
        self._apply_section_refs(refs)
        self.page_layout.addWidget(self.header_card)
        self._sync_window_title_with_license(self._license_reason, self._license_info)

    def _set_tutorial_badge_link(self, url):
        """刷新教程入口链接。"""
        set_tutorial_badge_link(self.tutorial_badge, url)

    @staticmethod
    def _build_badge_style(background, text_color, border_color, *, radius=None, padding=None):
        """构建徽标样式。"""
        return build_badge_style(background, text_color, border_color, radius=radius, padding=padding)

    @staticmethod
    def _scaled_padding(vertical, horizontal):
        """按当前 UI 缩放系数返回统一 padding 字符串。"""
        return scaled_padding(vertical, horizontal)

    @staticmethod
    def _standard_layout_spacing():
        """主界面统一内容间距。"""
        return standard_layout_spacing()

    def _build_main_content(self):
        """构建主内容区。"""
        setup_refs = build_setup_section(self)
        batch_refs = build_batch_section(self)
        license_refs = build_license_section(self)
        self._apply_section_refs(setup_refs)
        self._apply_section_refs(batch_refs)
        self._apply_section_refs(license_refs)

        self.order_edit.textChanged.connect(self.refresh_input_metrics)
        self.tracking_edit.textChanged.connect(self.refresh_input_metrics)
        self.order_edit.normalized.connect(self.refresh_input_metrics)
        self.tracking_edit.normalized.connect(self.refresh_input_metrics)

        self.license_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.action_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.config_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.order_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.tracking_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.log_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)

        self.main_content = QWidget()
        self.main_content.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.main_content_layout = QHBoxLayout(self.main_content)
        self.main_content_layout.setContentsMargins(0, 0, 0, 0)
        self.main_content_layout.setSpacing(scale_px(ROW_GAP, min_value=6))
        self.main_content_layout.setAlignment(Qt.AlignTop)

        self.left_column_layout = QVBoxLayout()
        self.left_column_layout.setContentsMargins(0, 0, 0, 0)
        self.left_column_layout.setSpacing(self._standard_layout_spacing())
        self.left_column_layout.addWidget(self.config_card)
        self.left_column_layout.addWidget(self.action_card)
        self.left_column_layout.addWidget(self.license_card)
        self.left_column_layout.addStretch(1)

        self.right_column_layout = QVBoxLayout()
        self.right_column_layout.setContentsMargins(0, 0, 0, 0)
        self.right_column_layout.setSpacing(self._standard_layout_spacing())

        self.input_row_layout = QHBoxLayout()
        self.input_row_layout.setContentsMargins(0, 0, 0, 0)
        self.input_row_layout.setSpacing(self._standard_layout_spacing())
        self.input_row_layout.addWidget(self.order_card, 1)
        self.input_row_layout.addWidget(self.tracking_card, 1)

        self.input_wrap = QWidget()
        self.input_wrap.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.input_wrap_layout = QVBoxLayout(self.input_wrap)
        self.input_wrap_layout.setContentsMargins(0, 0, 0, 0)
        self.input_wrap_layout.setSpacing(0)
        self.input_wrap_layout.addLayout(self.input_row_layout)

        self.right_column_layout.addWidget(self.input_wrap, 1)
        self.right_column_layout.addWidget(self.log_card, 1)

        self.main_content_layout.addLayout(self.left_column_layout, 1)
        self.main_content_layout.addLayout(self.right_column_layout, 2)
        self.page_layout.addWidget(self.main_content, 0, Qt.AlignTop)
        self._sync_window_title_with_license(self._license_reason, self._license_info)

    # -----------------------------------------------------------------------
    # 窗口尺寸 / 响应式
    # -----------------------------------------------------------------------

    def _calculate_editor_height(self, editor, visible_lines=10):
        """按指定可见行数计算输入框高度。"""
        return calculate_editor_height(editor, visible_lines)

    @staticmethod
    def _resolve_height_profile(viewport_height):
        """根据当前可用高度返回垂直紧凑模式。"""
        return resolve_height_profile(viewport_height)

    def _apply_right_column_strict_split(self):
        """将右侧输入区与日志区按严格 5:5 分配高度。"""
        if not hasattr(self, "input_wrap") or not hasattr(self, "main_content"):
            return

        self.main_content_layout.activate()
        self.right_column_layout.activate()
        self.input_row_layout.activate()

        available_height = self.main_content.height()
        if available_height <= 0:
            return

        gap = max(0, self.right_column_layout.spacing())
        split_height = max(0, available_height - gap)
        top_height = split_height // 2
        bottom_height = split_height - top_height
        if top_height <= 0 or bottom_height <= 0:
            return

        self.input_wrap.setFixedHeight(top_height)
        self.log_card.setFixedHeight(bottom_height)
        self.order_card.setFixedHeight(top_height)
        self.tracking_card.setFixedHeight(top_height)

        def _content_height(card, target_height, fallback_min):
            layout = card.layout()
            if layout is None:
                return fallback_min
            margins = layout.contentsMargins()
            used_height = margins.top() + margins.bottom() + max(0, layout.spacing())
            if layout.count() > 1:
                header_item = layout.itemAt(0)
                if header_item is not None and header_item.widget() is not None:
                    header = header_item.widget()
                    used_height += max(header.minimumHeight(), header.sizeHint().height())
            return max(fallback_min, target_height - used_height)

        editor_min = scale_px(96, min_value=84)
        log_min = scale_px(140, min_value=120)
        self.order_edit.setFixedHeight(_content_height(self.order_card, top_height, editor_min))
        self.tracking_edit.setFixedHeight(_content_height(self.tracking_card, top_height, editor_min))
        self.log_view.setFixedHeight(_content_height(self.log_card, bottom_height, log_min))

    def _sync_responsive_component_heights(self, viewport_height):
        """窗口高度变化时同步关键控件高度，优先压缩非关键留白与编辑区。"""
        profile = self._resolve_height_profile(viewport_height)
        if profile == self._last_responsive_profile:
            return

        base_editor_lines = max(6, scale_px(INPUT_VISIBLE_LINES, min_value=6))
        base_log_min_height = scale_px(LOG_PANEL_MIN_HEIGHT, min_value=128)
        base_path_panel_height = scale_px(CONFIG_PATH_MIN_HEIGHT, min_value=48)
        metrics_by_profile = {
            "comfortable": {
                "editor_lines": base_editor_lines,
                "log_min_height": base_log_min_height,
                "days_spin_height": scale_px(36, min_value=28),
                "path_panel_height": base_path_panel_height,
                "hero_padding_x": scale_px(HERO_PADDING_X, min_value=14),
                "hero_padding_y": scale_px(HERO_PADDING_Y, min_value=6),
                "hero_gap": scale_px(12, min_value=6),
                "title_gap": scale_px(4, min_value=2),
            },
            "compact": {
                "editor_lines": 16,
                "log_min_height": scale_px(156, min_value=132),
                "days_spin_height": scale_px(34, min_value=30),
                "path_panel_height": scale_px(68, min_value=56),
                "hero_padding_x": scale_px(max(HERO_PADDING_X - 6, 16), min_value=12),
                "hero_padding_y": scale_px(max(HERO_PADDING_Y - 8, 12), min_value=6),
                "hero_gap": scale_px(10, min_value=5),
                "title_gap": scale_px(3, min_value=2),
            },
            "dense": {
                "editor_lines": 12,
                "log_min_height": scale_px(140, min_value=120),
                "days_spin_height": scale_px(32, min_value=28),
                "path_panel_height": scale_px(60, min_value=48),
                "hero_padding_x": scale_px(max(HERO_PADDING_X - 10, 14), min_value=10),
                "hero_padding_y": scale_px(max(HERO_PADDING_Y - 12, 10), min_value=6),
                "hero_gap": scale_px(8, min_value=4),
                "title_gap": scale_px(2, min_value=1),
            },
        }
        metrics = metrics_by_profile[profile]
        self._last_responsive_profile = profile

        editor_height = self._calculate_editor_height(self.order_edit, metrics["editor_lines"])
        self.order_edit.setMinimumHeight(editor_height)
        self.tracking_edit.setMinimumHeight(editor_height)
        self.log_view.setMinimumHeight(metrics["log_min_height"])

        if hasattr(self, "config_path_panel"):
            self.config_path_panel.setMinimumHeight(metrics["path_panel_height"])

        self.review_days_spin.setFixedHeight(metrics["days_spin_height"])
        self.header_box.setContentsMargins(
            metrics["hero_padding_x"],
            metrics["hero_padding_y"],
            metrics["hero_padding_x"],
            metrics["hero_padding_y"],
        )
        self.header_box.setSpacing(metrics["hero_gap"])
        self.title_box.setSpacing(metrics["title_gap"])

        for widget in [
            getattr(self, "config_path_panel", None),
            self.order_edit,
            self.tracking_edit,
            self.log_view,
            self.order_card,
            self.tracking_card,
            self.log_card,
            self.config_card,
            self.action_card,
            self.license_card,
            self.main_content,
            self.header_card,
            self.page_widget,
        ]:
            if widget is not None:
                widget.updateGeometry()

        for layout in [
            self.input_wrap_layout,
            self.input_row_layout,
            self.right_column_layout,
            self.left_column_layout,
            self.main_content_layout,
            self.page_layout,
            self.setup_content_layout,
            self.config_content_layout,
            self.review_content_layout,
            self.action_content_layout,
            self.license_content_layout,
        ]:
            layout.invalidate()
            layout.activate()

    def _fit_window_to_screen(self):
        """首次打开时先锁定默认宽度，允许高度按内容收口后再最终锁定。"""
        default_w, default_h = self._resolve_initial_window_size()
        self.setMinimumWidth(default_w)
        self.setMaximumWidth(default_w)
        self.resize(default_w, default_h)

    @staticmethod
    def _resolve_layout_mode(width, height):
        """根据当前可用尺寸判定缩放模式，但不改变布局结构。"""
        return resolve_layout_mode(width, height)

    def _resolve_ui_scale_for_size(self, width, height):
        """根据给定窗口尺寸与平台/DPI 计算缩放系数，用于首次打开时与默认窗口匹配。"""
        return resolve_ui_scale_for_size(width, height)

    def _resolve_initial_window_size(self):
        """结合平台默认值与屏幕可用区域，计算首次打开时的窗口尺寸（逻辑像素，可缩放）。"""
        return resolve_initial_window_size(self)

    def _sync_responsive_metrics(self):
        """窗口变化时同步页面边距。"""
        viewport = self.scroll_area.viewport()
        viewport_width = viewport.width()
        viewport_height = viewport.height()
        if not viewport_width or not viewport_height:
            return
        ratio = min(1.0, max(0.6, viewport_width / 960))
        base_margin = scale_px(PAGE_MARGIN, min_value=10)
        margin = max(scale_px(10, min_value=8), int(round(base_margin * ratio)))
        self.page_layout.setContentsMargins(margin, margin, margin, margin)
        self._sync_responsive_component_heights(viewport_height)
        self._apply_right_column_strict_split()

    def resizeEvent(self, event):
        """窗口尺寸变化时同步内部尺寸。"""
        super().resizeEvent(event)
        self._sync_responsive_metrics()
        self._apply_right_column_strict_split()

    def showEvent(self, event):
        """首次展示时先按内容收口高度，再锁定初始窗口尺寸。"""
        super().showEvent(event)
        self._sync_responsive_metrics()
        if self._initial_height_fit_applied:
            self._apply_right_column_strict_split()
            return
        self._apply_right_column_strict_split()
        self._fit_window_height_to_content()
        self._apply_right_column_strict_split()
        self.setFixedSize(self.width(), self.height())
        self._apply_right_column_strict_split()
        self._initial_height_fit_applied = True

    def _fit_window_height_to_content(self):
        """按内容实际高度调整窗口（双向：偏高则缩、偏矮则扩），防止出现滚动条或大块空白。"""
        if not hasattr(self, "page_widget") or not hasattr(self, "scroll_area"):
            return

        root_layout = self.layout()
        if root_layout is not None:
            root_layout.activate()
        self.page_layout.activate()

        viewport_height = self.scroll_area.viewport().height()
        if viewport_height <= 0:
            return

        chrome_height = max(0, self.height() - viewport_height)
        scroll_guard = scale_px(8, min_value=4)
        content_height = self.page_widget.sizeHint().height() + chrome_height + scroll_guard
        target_height = max(self.minimumHeight(), content_height)

        screen = self.screen() or QApplication.primaryScreen()
        if screen is not None:
            available_height = screen.availableGeometry().height()
            max_ratio = 0.97 if sys.platform.startswith("win") else 0.92
            target_height = min(target_height, int(available_height * max_ratio))

        tolerance = scale_px(8, min_value=4)
        if abs(self.height() - target_height) > tolerance:
            self.resize(self.width(), target_height)

    # -----------------------------------------------------------------------
    # 输入 / 日志
    # -----------------------------------------------------------------------

    def refresh_input_metrics(self):
        """刷新两个输入框的数量徽标。"""
        order_count = len(parse_batch_input(self.order_edit.toPlainText()))
        tracking_count = len(parse_batch_input(self.tracking_edit.toPlainText()))
        self.order_count_badge.setText(f"{order_count}/{MAX_BATCH_SIZE}")
        self.tracking_count_badge.setText(f"{tracking_count}/{MAX_BATCH_SIZE}")

    def normalize_inputs(self):
        """整理两个输入框内容。"""
        self.order_edit.normalize_content()
        self.tracking_edit.normalize_content()
        self.refresh_input_metrics()

    def append_result_log(self, text):
        """追加运行日志。"""
        self.log_view.appendPlainText(text)
        scrollbar = self.log_view.verticalScrollBar()
        scrollbar.setValue(scrollbar.maximum())

    def clear_result_log(self):
        """清空运行日志。"""
        self.log_view.clear()

    @staticmethod
    def _set_plain_text_without_signals(editor, text):
        """在不触发 textChanged 的情况下更新输入框内容。"""
        editor.blockSignals(True)
        try:
            editor.setPlainText(text)
        finally:
            editor.blockSignals(False)

    def _set_order_input_values(self, order_ids):
        """用给定订单号列表覆盖订单输入框。"""
        self._set_plain_text_without_signals(self.order_edit, "\n".join(order_ids))
        self.refresh_input_metrics()

    def _set_batch_input_values(self, order_ids, tracking_numbers):
        """同步覆盖订单号和物流单号输入框。"""
        self._set_plain_text_without_signals(self.order_edit, "\n".join(order_ids))
        self._set_plain_text_without_signals(self.tracking_edit, "\n".join(tracking_numbers))
        self.refresh_input_metrics()

    def _sync_batch_inputs_from_rows(self):
        """将未成功的批次行回写到输入框。"""
        pending_rows = [row for row in self._batch_rows if not row["succeeded"]]
        order_ids = [row["order_id"] for row in pending_rows]
        tracking_numbers = [row["tracking_number"] for row in pending_rows]
        self._set_batch_input_values(order_ids, tracking_numbers)

    def _mark_batch_row_succeeded(self, batch_index):
        """将一条批次行标记为成功，并同步移出输入框。"""
        row_index = batch_index - 1
        if 0 <= row_index < len(self._batch_rows):
            self._batch_rows[row_index]["succeeded"] = True
            self._sync_batch_inputs_from_rows()

    # -----------------------------------------------------------------------
    # 弹窗
    # -----------------------------------------------------------------------

    def _create_message_dialog_base(self, level, title, text, informative_text="", *, min_width=560):
        """构建统一样式弹窗骨架，返回 (dialog, actions_layout)。"""
        return self._message_presenter.create_message_dialog_base(level, title, text, informative_text, min_width=min_width)

    def _show_action_dialog(self, level, title, text, informative_text="", *, min_width=560, action_specs=()):
        """按声明式动作配置构建并显示通用操作弹窗。"""
        return self._message_presenter.show_action_dialog(level, title, text, informative_text, min_width=min_width, action_specs=action_specs)

    def show_message(self, level, title, text, informative_text=""):
        """显示统一样式的提示弹窗。"""
        return self._message_presenter.show_message(level, title, text, informative_text)

    def _append_logs(self, *messages):
        """按顺序追加多条日志，自动跳过空值。"""
        for message in messages:
            if message:
                self.append_result_log(message)

    def _log_and_show_message(
        self,
        level,
        title,
        text,
        informative_text="",
        *,
        log_messages=(),
    ):
        """统一处理“追加日志 + 显示弹窗”组合。"""
        self._append_logs(*log_messages)
        self.show_message(level, title, text, informative_text)

    def _show_task_terminal_error(self, *, task_type, message):
        """统一处理任务失败日志与弹窗。"""
        title = (
            "缓存任务失败"
            if task_type in (TASK_CACHE_REFRESH, TASK_CACHE_REBUILD)
            else "查找失败"
        )
        self._log_and_show_message(
            QMessageBox.Critical,
            title,
            message,
            log_messages=(f"❌ 错误: {message}",),
        )

    def _show_task_summary_message(
        self,
        *,
        summary,
        warning_text,
        warning_title,
        success_title,
        success_level=QMessageBox.Information,
    ):
        """统一处理任务完成类结果的日志与弹窗。"""
        log_messages = [summary]
        if warning_text:
            warning_message = f"⚠️ 提醒: {warning_text}"
            log_messages.append(warning_message)
            self._append_logs(*log_messages)
            self.show_message(
                QMessageBox.Warning,
                warning_title,
                f"{summary}\n\n{warning_text}",
            )
            return
        self._log_and_show_message(
            success_level,
            success_title,
            summary,
            log_messages=tuple(log_messages),
        )

    def _append_license_status_log(self, reason, *, prefix="授权状态"):
        """统一追加授权状态日志。"""
        self._append_logs(f"{prefix}：{get_license_reason_text(reason)}")

    def _append_license_success_logs(self, info):
        """统一追加授权成功后的日志。"""
        expires = str((info or {}).get("expires_at", ""))[:10]
        self._append_logs(
            "卡密激活成功，已解锁批量处理功能。",
            f"授权有效期至：{expires}" if expires else "",
        )

    def _build_license_expiry_hint(self):
        """返回授权到期提示文案。"""
        if self._license_reason != "ok":
            return ""
        info = self._license_info or {}
        expires_at = str(info.get("license_expires_at") or info.get("expires_at") or "").strip()
        if not expires_at:
            return ""
        try:
            expire_dt = datetime.fromisoformat(expires_at)
        except ValueError:
            return ""
        now = datetime.now(expire_dt.tzinfo) if expire_dt.tzinfo else datetime.now()
        days_left = (expire_dt.date() - now.date()).days
        if days_left < 0:
            return "授权已过期，请尽快续费。"
        if days_left <= 7:
            return f"授权还有 {days_left} 天到期，建议提前续费。"
        return ""

    def _get_startup_license_state(self):
        """返回启动阶段使用的授权状态。"""
        cache = self._get_cached_license_state(max_age_seconds=None)
        if cache is not None:
            return cache.get("info") or {}, cache.get("reason", "invalid")
        return self._refresh_license_state_with_mode(local_only=True)

    @staticmethod
    def _shutdown_thread(thread, *, graceful_timeout=3000, terminate_timeout=1000):
        """优雅停止后台线程，必要时强制终止。"""
        if thread is None or not thread.isRunning():
            return
        thread.quit()
        if not thread.wait(graceful_timeout):
            thread.terminate()
            thread.wait(terminate_timeout)

    @staticmethod
    def _resolve_config_status_content(saved_dir, resolved_dir):
        """返回配置卡片展示文案和徽标状态。"""
        if resolved_dir:
            return (
                resolved_dir,
                "软件会使用这里的 cookie 文件进行自动化操作。",
                "已连接",
                (
                    APP_COLORS["green_soft"],
                    APP_COLORS["green"],
                    APP_COLORS["border_strong"],
                ),
            )
        if saved_dir:
            return (
                saved_dir,
                "以上目录未找到 cookie.txt 文件，请重新获取。",
                "待修复",
                (
                    APP_COLORS["neutral_bg"],
                    APP_COLORS["muted"],
                    APP_COLORS["neutral_border"],
                ),
            )
        return (
            "请点击下方按钮自动获取 cookie 文件。",
            "自动获取时会自动保存 cookie.txt 文件。",
            "未配置",
            (
                APP_COLORS["red_soft"],
                APP_COLORS["red"],
                APP_COLORS["red"],
            ),
        )

    def _build_license_status_badge_style(self, is_active):
        """根据授权状态构建徽标样式。"""
        if is_active:
            colors = (
                APP_COLORS["green_soft"],
                APP_COLORS["green"],
                APP_COLORS["border_strong"],
            )
        else:
            colors = (
                APP_COLORS["red_soft"],
                APP_COLORS["red"],
                APP_COLORS["red"],
            )
        return self._build_badge_style(
            *colors,
            radius=scale_px(BADGE_RADIUS, min_value=10),
            padding=self._scaled_padding(7, 12),
        )

    @staticmethod
    def _activation_required_message(task_label):
        """返回未激活时的统一提示文案。"""
        return f"软件尚未激活，无法执行{task_label}。\n请先输入有效卡密完成激活。"

    def _show_activation_required(self, task_label):
        """显示未激活提示。"""
        self.show_message(
            QMessageBox.Warning,
            "未激活",
            self._activation_required_message(task_label),
        )

    def _bind_thread_lifecycle(self, thread, worker, clear_callback):
        """绑定通用线程生命周期。"""
        thread.started.connect(worker.run)
        thread.finished.connect(worker.deleteLater)
        thread.finished.connect(thread.deleteLater)
        thread.finished.connect(clear_callback)

    def _start_thread_worker(
        self,
        *,
        thread_attr,
        worker_attr,
        worker,
        clear_callback,
        signal_bindings=(),
        quit_signals=(),
    ):
        """统一创建线程、绑定 worker 信号并启动后台任务。"""
        thread = QThread(self)
        setattr(self, thread_attr, thread)
        setattr(self, worker_attr, worker)
        worker.moveToThread(thread)

        for signal, slot in signal_bindings:
            signal.connect(slot)
        for signal in quit_signals:
            signal.connect(thread.quit)

        self._bind_thread_lifecycle(thread, worker, clear_callback)
        thread.start()
        return thread, worker

    # -----------------------------------------------------------------------
    # 按钮状态
    # -----------------------------------------------------------------------

    def _apply_button_updates(self, button_updates):
        """批量同步按钮禁用态与文案。"""
        for button, disabled, text in button_updates:
            button.setDisabled(disabled)
            if text is not None:
                button.setText(text)

    def _resolve_action_button_updates(self, *, running):
        """返回批量处理区按钮的状态描述。"""
        button_updates = [
            (
                self.pause_button,
                (not running) or self.is_paused,
                "已暂停" if self.is_paused else "暂停批量处理",
            ),
            (
                self.start_button,
                running and not self.is_paused,
                "继续批量处理" if self.is_paused else "开始批量处理",
            ),
        ]
        if hasattr(self, "auto_cookie_button"):
            button_updates.append((self.auto_cookie_button, running, None))
        return button_updates

    def refresh_action_buttons(self):
        """同步开始/暂停按钮状态。"""
        running = self.worker is not None
        self.order_edit.setReadOnly(running)
        self.tracking_edit.setReadOnly(running)
        self._apply_button_updates(self._resolve_action_button_updates(running=running))

    def set_submit_running(self, is_running):
        """切换按钮和输入框状态。"""
        if not is_running:
            self.is_paused = False
        self.refresh_action_buttons()

    # -----------------------------------------------------------------------
    # 配置目录
    # -----------------------------------------------------------------------

    def refresh_config_path_label(self):
        """刷新配置目录卡片文案。"""
        saved_dir = get_config_dir_cache() or get_saved_user_config_dir()
        try:
            resolved_dir = resolve_config_dir()
        except ConfigNotFoundError:
            resolved_dir = None

        path_text, note_text, badge_text, badge_colors = self._resolve_config_status_content(
            saved_dir,
            resolved_dir,
        )
        badge_style = self._build_badge_style(*badge_colors)
        self.config_path_label.setText(path_text)
        self.config_note_label.setText(note_text)
        if hasattr(self, "config_badge"):
            self.config_badge.setText(badge_text)
            self.config_badge.setStyleSheet(badge_style)

    def _resolve_saved_config_start_dir(self):
        """返回配置目录相关操作的默认起始目录。"""
        return get_config_dir_cache() or get_saved_user_config_dir() or os.path.expanduser("~")

    def _validate_selected_config_dir(self, selected_dir):
        """校验用户选择的配置目录，返回缺失项列表。"""
        resolved_files = resolve_config_files_in_dir(selected_dir)
        missing_files = []
        if not resolved_files or "cookie" not in resolved_files:
            missing_files.append("cookie(.txt)")
            return missing_files

        try:
            cookie_data = read_cookie_data(resolved_files["cookie"])
        except Exception:  # noqa: BLE001
            missing_files.append("cookie(.txt) 内容不可读")
            return missing_files

        if not extract_biz_magic_from_cookie(cookie_data):
            missing_files.append("cookie 中 biz_magic 键")
        return missing_files

    def _show_invalid_config_dir_message(self, missing_files):
        """提示所选配置目录不完整。"""
        self._log_and_show_message(
            QMessageBox.Warning,
            "目录不完整",
            "所选目录缺少以下文件：" + "、".join(missing_files),
        )

    def _show_config_dir_updated_message(self, selected_dir):
        """提示配置目录已更新。"""
        self._log_and_show_message(
            QMessageBox.Information,
            "配置目录已更新",
            f"后续将优先使用：\n{selected_dir}",
        )

    def choose_config_dir(self):
        """选择配置文件所在目录并记住。"""
        selected_dir = QFileDialog.getExistingDirectory(
            self,
            "选择配置目录",
            self._resolve_saved_config_start_dir(),
        )
        if not selected_dir:
            return

        missing_files = self._validate_selected_config_dir(selected_dir)
        if missing_files:
            self._show_invalid_config_dir_message(missing_files)
            return

        save_user_config_dir(selected_dir)
        self.refresh_config_path_label()
        self._show_config_dir_updated_message(selected_dir)

    def _show_qtwebengine_unavailable(self, error_detail):
        """提示当前环境无法使用 QtWebEngine。"""
        self.show_message(
            QMessageBox.Warning,
            "当前环境缺少 QtWebEngine",
            "暂时无法打开内置网页登录窗口。",
            "请先安装支持 QtWebEngine 的 PySide6 组件后重试。\n"
            f"错误详情：{error_detail}",
        )

    def _show_cookie_browser_init_error(self, error_detail):
        """提示内置网页登录窗口初始化失败。"""
        self.show_message(
            QMessageBox.Critical,
            "打开网页登录窗口失败",
            "QtWebEngine 已检测到，但初始化内置浏览器时失败。",
            str(error_detail),
        )

    def _select_cookie_save_dir(self):
        """选择 Cookie 保存目录。"""
        return QFileDialog.getExistingDirectory(
            self,
            "选择 Cookie 保存位置",
            self._resolve_saved_config_start_dir(),
        )

    def _show_cookie_not_ready_message(self):
        """提示 Cookie 尚未具备 biz_magic。"""
        self.show_message(
            QMessageBox.Warning,
            "Cookie 尚未准备好",
            "当前未检测到 biz_magic，请在页面里完成登录后重试。",
        )

    def _show_cookie_save_error(self, error_detail):
        """提示 Cookie 保存失败。"""
        self.show_message(
            QMessageBox.Critical,
            "保存 Cookie 失败",
            "已抓取到登录态，但写入 cookie.txt 失败。",
            str(error_detail),
        )

    def _show_cookie_saved_message(self, cookie_path):
        """提示 Cookie 保存成功。"""
        self._log_and_show_message(
            QMessageBox.Information,
            "Cookie 获取成功",
            f"已保存到：\n{cookie_path}",
            "已记住该配置目录，后续获取差评、获取品退和批量处理会直接使用此目录下的 cookie.txt。",
            log_messages=(
                f"Cookie 已保存到：{cookie_path}",
                "已记住该路径，后续将从此目录读取 cookie.txt。",
            ),
        )

    def open_cookie_capture_dialog(self):
        """打开网页登录窗口并自动抓取 Cookie；保存时选择目录并记住。"""
        # 延迟导入 QtWebEngine 相关模块，避免启动时加载
        try:
            from ui.cookie_dialog import CookieCaptureDialog, QTWEBENGINE_AVAILABLE, QTWEBENGINE_IMPORT_ERROR
        except ImportError as exc:
            self._show_qtwebengine_unavailable(exc)
            return

        if not QTWEBENGINE_AVAILABLE:
            self._show_qtwebengine_unavailable(QTWEBENGINE_IMPORT_ERROR or "QtWebEngine 不可用")
            return

        self._append_logs("正在打开内置网页登录窗口，登录成功后点击保存并选择目录即可。")
        try:
            dialog = CookieCaptureDialog(get_default_config_dir(), self)
        except Exception as exc:  # noqa: BLE001
            self._show_cookie_browser_init_error(exc)
            return

        if dialog.exec() != QDialog.Accepted:
            self._append_logs("已取消自动获取 Cookie。")
            return

        cookie_data = dialog.cookie_data
        if not extract_biz_magic_from_cookie(cookie_data):
            self._show_cookie_not_ready_message()
            return

        selected_dir = self._select_cookie_save_dir()
        if not selected_dir:
            self._append_logs("已取消选择目录，Cookie 未保存。")
            return

        try:
            cookie_path = save_cookie_data(cookie_data, config_dir=selected_dir, remember_dir=True)
        except Exception as exc:  # noqa: BLE001
            self._show_cookie_save_error(exc)
            return

        self.refresh_config_path_label()
        self._show_cookie_saved_message(cookie_path)

    def show_missing_config_error(self, searched_dirs):
        """提示缺少配置文件，并允许用户直接选择目录。"""
        if isinstance(searched_dirs, str):
            details = searched_dirs
        else:
            details = "\n".join(str(item) for item in searched_dirs)
        info_text = f"错误详情:\n{details}"
        result = self._show_action_dialog(
            QMessageBox.Warning,
            "缺少配置文件",
            "当前未找到可用配置目录，请先手动选择目录。",
            info_text,
            min_width=620,
            action_specs=(
                ("关闭", "MessageSecondary", lambda: QDialog.Rejected),
                ("选择配置目录", "MessagePrimary", lambda: QDialog.Accepted),
            ),
        )
        if result == QDialog.Accepted:
            self.choose_config_dir()

    # -----------------------------------------------------------------------
    # 授权
    # -----------------------------------------------------------------------

    def _store_license_state(self, info, reason, *, source):
        """更新当前授权状态与短时缓存。"""
        self._license_reason = reason
        self._license_info = info or {}
        self._license_state_cache = {
            "info": self._license_info,
            "reason": reason,
            "checked_at": time.monotonic(),
            "source": source,
        }
        self._sync_window_title_with_license(reason, info)

    def _refresh_license_state_with_mode(self, *, local_only):
        """按模式刷新授权状态。"""
        checker = check_stored_license_local if local_only else check_stored_license
        info, reason = checker()
        self._store_license_state(info, reason, source="local" if local_only else "online")
        return info, reason

    def _get_cached_license_state(self, *, max_age_seconds=LICENSE_STATUS_CACHE_TTL_SECONDS):
        """返回短时缓存命中的授权状态。"""
        cache = self._license_state_cache
        if not cache:
            return None
        if max_age_seconds is not None:
            age = time.monotonic() - cache.get("checked_at", 0.0)
            if age > max_age_seconds:
                return None
        return cache

    def _clear_license_refresh_refs(self):
        """清理后台授权刷新线程引用。"""
        self.license_refresh_worker = None
        self.license_refresh_thread = None

    def _on_license_refresh_finished(self, info, reason):
        """后台授权刷新完成后更新授权状态。"""
        previous = self._license_state_cache or {}
        self._store_license_state(info, reason, source="online")
        if previous.get("reason") != reason:
            self._append_logs(f"授权状态已更新：{get_license_reason_text(reason)}")

    def _on_license_refresh_failed(self, error_message):
        """后台授权刷新失败时保留当前状态。"""
        if error_message:
            self._append_logs(f"授权在线刷新失败，已继续使用本地授权状态：{error_message}")

    def _schedule_license_refresh(self, *, force=False):
        """在后台线程中刷新授权状态，避免阻塞主线程。"""
        if self.license_refresh_thread is not None:
            return
        if not force and self._get_cached_license_state() is not None:
            return

        worker = LicenseRefreshWorker()
        self._start_thread_worker(
            thread_attr="license_refresh_thread",
            worker_attr="license_refresh_worker",
            worker=worker,
            clear_callback=self._clear_license_refresh_refs,
            signal_bindings=(
                (worker.finished, self._on_license_refresh_finished),
                (worker.failed, self._on_license_refresh_failed),
            ),
            quit_signals=(worker.finished, worker.failed),
        )

    def _clear_update_check_refs(self):
        """清理后台更新检查线程引用。"""
        self.update_check_worker = None
        self.update_check_thread = None

    def _show_manual_update_latest_message(self):
        """提示当前已是最新版本。"""
        self.show_message(QMessageBox.Information, "检查更新", f"当前已是最新版本：{APP_VERSION}")

    def _show_manual_update_failed_message(self, error_message):
        """提示手动检查更新失败。"""
        self.show_message(QMessageBox.Warning, "检查更新失败", error_message or "无法获取更新信息，请稍后重试。")

    def _show_missing_download_url_message(self):
        """提示远端未提供更新下载链接。"""
        self.show_message(QMessageBox.Warning, "下载链接缺失", "远端配置未提供更新下载链接，请稍后重试。")

    def _open_update_download_url(self, download_url):
        """打开更新下载链接并记录日志。"""
        if not download_url:
            self._show_missing_download_url_message()
            return QDialog.Accepted

        QDesktopServices.openUrl(QUrl(download_url))
        self._append_logs(f"已打开更新下载链接：{download_url}")
        return QDialog.Accepted

    def trigger_background_update_check(self, *, manual=False):
        """后台检查更新，避免阻塞主线程。"""
        if self.update_check_thread is not None:
            return
        if manual:
            self._append_logs("正在检查软件更新...")

        worker = UpdateCheckWorker(APP_VERSION)
        self._start_thread_worker(
            thread_attr="update_check_thread",
            worker_attr="update_check_worker",
            worker=worker,
            clear_callback=self._clear_update_check_refs,
            signal_bindings=(
                (worker.finished, lambda info, m=manual: self.update_check_finished_signal.emit(info, m)),
                (worker.failed, lambda message, m=manual: self.update_check_failed_signal.emit(message, m)),
            ),
            quit_signals=(worker.finished, worker.failed),
        )

    def _on_update_check_finished(self, info, manual):
        """更新检查完成。"""
        if getattr(info, 'tutorial_url', None):
            self._set_tutorial_badge_link(info.tutorial_url)

        if not info.has_update:
            if manual:
                self._show_manual_update_latest_message()
            return

        if not manual and self._update_prompt_version == info.version:
            return

        self._update_prompt_version = info.version
        self._append_logs(f"发现新版本：{info.version}")
        self._show_update_dialog(info)

    def _on_update_check_failed(self, error_message, manual):
        """更新检查失败。"""
        if manual:
            self._show_manual_update_failed_message(error_message)
        elif error_message:
            self._append_logs(f"检查更新失败：{error_message}")

    def _show_update_dialog(self, info):
        """显示更新提示弹窗。"""
        notes = info.notes or ["本次版本包含若干优化与修复。"]
        informative_text = "当前版本：{}\n最新版本：{}\n\n更新内容：\n{}".format(
            APP_VERSION,
            info.version,
            "\n".join(f"- {item}" for item in notes),
        )

        self._show_action_dialog(
            QMessageBox.Information,
            f"发现新版本 {info.version}",
            "检测到可用更新，是否立即前往网盘下载？",
            informative_text,
            min_width=620,
            action_specs=(
                ("稍后再说", "MessageSecondary", lambda: QDialog.Rejected),
                ("前往下载", "MessagePrimary", lambda: self._open_update_download_url(info.download_url)),
            ),
        )

    def _sync_window_title_with_license(self, license_reason=None, license_info=None):
        """同步窗口标题与授权状态卡片。"""
        if license_reason is not None:
            self._license_reason = license_reason
        if license_info is not None:
            self._license_info = license_info or {}

        reason = self._license_reason
        info = self._license_info or {}
        status_map = {
            "ok": "已激活",
            "renewal_due": "待续签",
            "expired": "已过期",
            "device_mismatch": "设备不符",
            "invalid": "状态异常",
            "not_found": "未激活",
            "reactivation_required": "需迁移",
            "online_refresh_required": "需联网",
            "compromised": "环境异常",
            "revoked": "已吊销",
        }
        status = status_map.get(reason, "未激活")
        self.setWindowTitle(WINDOW_TITLE)
        if hasattr(self, "license_status_badge"):
            self.license_status_badge.setText(status)
            self.license_status_badge.setStyleSheet(
                self._build_license_status_badge_style(reason == "ok")
            )
        expires_at = str(info.get("license_expires_at") or info.get("expires_at") or "")[:10]
        if hasattr(self, "license_summary_label"):
            if reason == "ok":
                self.license_summary_label.setText("软件已激活，可正常执行批量处理。")
            elif reason == "renewal_due":
                self.license_summary_label.setText("授权租约待续签，当前仍可继续执行任务。")
            else:
                self.license_summary_label.setText(get_license_reason_text(reason))
        if hasattr(self, "license_meta_label"):
            if reason in {"ok", "renewal_due"}:
                meta_parts = []
                if expires_at:
                    meta_parts.append(f"有效期至：{expires_at}")
                lease_exp = str(info.get("lease_expires_at", ""))[:16]
                if lease_exp:
                    meta_parts.append(f"租约至：{lease_exp}")
                renew_after = str(info.get("renew_after", ""))[:16]
                if renew_after:
                    meta_parts.append(f"建议续签：{renew_after}")
                device_id = str(info.get("device_id", "")).strip() or str(info.get("device_id_suffix", "")).strip()
                if device_id:
                    suffix = device_id[-6:] if len(device_id) > 6 else device_id
                    meta_parts.append(f"设备尾号：{suffix}")
                backend = str(info.get("runtime_backend", "")).strip()
                if backend:
                    meta_parts.append(f"安全核：{backend}")
                self.license_meta_label.setText(
                    "  |  ".join(meta_parts) or "授权租约已写入本地，可直接开始执行任务。"
                )
            elif reason == "expired" and expires_at:
                self.license_meta_label.setText(
                    f"当前授权已于 {expires_at} 到期，请联系微信 {AUTHOR_WECHAT} 获取新卡密。"
                )
            elif reason == "device_mismatch":
                self.license_meta_label.setText(
                    f"当前设备与原授权绑定设备不一致，请联系微信 {AUTHOR_WECHAT} 处理重绑。"
                )
            elif reason == "online_refresh_required":
                self.license_meta_label.setText("当前授权租约已超出本地硬过期窗口，需要联网续签后才能继续执行核心任务。")
            elif reason == "reactivation_required":
                self.license_meta_label.setText("授权协议已升级，请联网重新校验卡密以完成迁移。")
            elif reason == "compromised":
                self.license_meta_label.setText("检测到安全核或完整性清单异常，已暂停高价值任务，请重新下载安装包。")
            elif reason == "revoked":
                self.license_meta_label.setText(f"当前卡密已被吊销，请联系微信 {AUTHOR_WECHAT} 处理。")
            else:
                self.license_meta_label.setText(
                    f"联系微信 {AUTHOR_WECHAT} 获取卡密后，即可完成激活并开始使用。"
                )

    def _prompt_license_activation(self, reason=None):
        """弹出激活窗口，返回是否激活成功。"""
        if reason is None:
            _, reason = self._refresh_license_state_with_mode(local_only=True)
        self._append_license_status_log(reason)
        self._append_logs("正在打开卡密激活窗口...")

        dialog = LicenseDialog(self, reason=reason)
        result = dialog.exec()
        if result == QDialog.Accepted and dialog.activated:
            info, refreshed_reason = self._refresh_license_state_with_mode(local_only=True)
            if refreshed_reason == "ok":
                self._append_license_success_logs(info)
                return True

            self._append_logs("卡密激活结果校验失败，请重试。")
            self._append_license_status_log(refreshed_reason, prefix="当前状态")
            return False

        _, refreshed_reason = self._refresh_license_state_with_mode(local_only=True)
        self._append_logs("卡密激活未完成。")
        self._append_license_status_log(refreshed_reason, prefix="当前状态")
        return False

    def prompt_license_on_startup(self):
        """启动后提示激活（仅在未激活或协议升级时弹出）。"""
        info, reason = self._get_startup_license_state()
        if reason == "ok":
            self._append_license_status_log(reason)
            expires = str((info or {}).get("license_expires_at") or (info or {}).get("expires_at") or "")[:10]
            self._append_logs(f"授权有效期至：{expires}" if expires else "")
            return True
        if reason == "online_refresh_required":
            self._append_license_status_log(reason)
            self._append_logs("当前授权需要联网刷新短期票据后，才能执行查单与批量处理。")
            return True

        self._append_logs("当前未激活，执行前需先输入卡密。")
        return self._prompt_license_activation(reason)

    # -----------------------------------------------------------------------
    # 批量处理
    # -----------------------------------------------------------------------

    def _resume_batch_processing(self):
        """继续已暂停的批量处理任务。"""
        self.worker.resume()
        self.is_paused = False
        self.refresh_action_buttons()
        self._append_logs("已继续执行剩余任务。")

    def _parse_batch_inputs(self):
        """解析批量处理输入框内容。"""
        return (
            parse_batch_input(self.order_edit.toPlainText()),
            parse_batch_input(self.tracking_edit.toPlainText()),
        )

    def _validate_batch_inputs(self, order_ids, tracking_numbers):
        """校验批量处理输入。"""
        if not order_ids or not tracking_numbers:
            self.show_message(QMessageBox.Information, "提示", "请输入订单号和新物流单号。")
            return False

        if len(order_ids) != len(tracking_numbers):
            self.show_message(
                QMessageBox.Critical,
                "数量不匹配",
                f"订单号共 {len(order_ids)} 个，新物流单号共 {len(tracking_numbers)} 个。\n"
                "请确保一一对应后再执行。",
            )
            return False

        if len(order_ids) > MAX_BATCH_SIZE:
            self.show_message(
                QMessageBox.Critical,
                "超出数量限制",
                f"一次最多处理 {MAX_BATCH_SIZE} 条，请拆分后再执行。",
            )
            return False

        return True

    def _prepare_batch_rows(self, order_ids, tracking_numbers):
        """根据输入构建批量任务行。"""
        self._batch_rows = [
            {
                "order_id": order_id,
                "tracking_number": tracking_number,
                "succeeded": False,
            }
            for order_id, tracking_number in zip(order_ids, tracking_numbers)
        ]

    def _start_batch_worker(self, order_ids, tracking_numbers):
        """启动批量处理后台任务。"""
        worker = BatchWorker(order_ids, tracking_numbers)
        self._start_thread_worker(
            thread_attr="worker_thread",
            worker_attr="worker",
            worker=worker,
            clear_callback=self._clear_worker_refs,
            signal_bindings=(
                (worker.started, self._on_worker_started),
                (worker.step_started, self._on_worker_step_started),
                (worker.step_succeeded, self._on_worker_step_succeeded),
                (worker.step_failed, self._on_worker_step_failed),
                (worker.fatal_error, self._on_worker_fatal_error),
                (worker.missing_config, self.show_missing_config_error),
                (worker.finished, self._on_worker_finished),
            ),
            quit_signals=(worker.finished,),
        )
        self.refresh_action_buttons()

    def on_start_clicked(self):
        """开始或继续批量处理。"""
        if self.worker is not None and self.is_paused:
            self._resume_batch_processing()
            return

        if not self._can_start_task(self.worker, "批量处理"):
            return

        self.normalize_inputs()
        order_ids, tracking_numbers = self._parse_batch_inputs()
        if not self._validate_batch_inputs(order_ids, tracking_numbers):
            return

        self._prepare_batch_rows(order_ids, tracking_numbers)
        self.clear_result_log()
        self._append_logs(f"开始执行：共 {len(order_ids)} 条。")
        self.set_submit_running(True)
        self._start_batch_worker(order_ids, tracking_numbers)

    def on_pause_clicked(self):
        """暂停后续批量任务。"""
        if self.worker is None or self.is_paused:
            return
        self.worker.pause()
        self.is_paused = True
        self.refresh_action_buttons()
        self._append_logs("已暂停处理，当前单完成后将停止继续执行。")

    def _clear_worker_refs(self):
        """清理线程引用。"""
        self.worker = None
        self.worker_thread = None
        self._batch_rows = []
        self.is_paused = False
        self.refresh_action_buttons()

    def closeEvent(self, event):
        """窗口关闭时安全终止后台线程。"""
        if self.worker is not None:
            self.worker.stop()
        self._shutdown_thread(self.worker_thread)
        if self.review_worker is not None:
            self.review_worker.stop()
        self._shutdown_thread(self.review_worker_thread)
        self._shutdown_thread(self.license_refresh_thread)
        super().closeEvent(event)

    def _on_worker_started(self, total_count):
        """记录任务开始。"""
        self._append_logs(f"任务已创建：共 {total_count} 条，准备顺序执行。")

    def _on_worker_step_started(self, index, total_count, order_id):
        """记录单条开始。"""
        self._append_logs(f"[{index}/{total_count}] 开始处理订单 {order_id}")

    def _on_worker_step_succeeded(self, index, total_count, order_id, tracking_number, old_waybill):
        """记录单条成功。"""
        self._mark_batch_row_succeeded(index)
        self._append_logs(
            f"[{index}/{total_count}] 订单 {order_id} 成功：{old_waybill} -> {tracking_number}"
        )

    def _on_worker_step_failed(self, index, total_count, order_id, tracking_number, error_message):
        """记录单条失败。"""
        self._append_logs(
            f"[{index}/{total_count}] 订单 {order_id} -> {tracking_number} 失败：{error_message}"
        )

    def _on_worker_fatal_error(self, error_message):
        """记录批量中断。"""
        self._append_logs(f"批量执行中断：{error_message}")

    def _on_worker_finished(self, success_count, failure_count, total_count, aborted):
        """恢复界面并汇总结果。"""
        self.set_submit_running(False)

        if aborted:
            return

        summary = (
            f"批量执行完成：共 {total_count} 条，成功 {success_count} 条，失败 {failure_count} 条。"
        )
        self._log_and_show_message(
            QMessageBox.Warning if failure_count > 0 else QMessageBox.Information,
            "批量执行完成",
            summary,
            log_messages=(summary,),
        )

    # -----------------------------------------------------------------------
    # 中差评查找
    # -----------------------------------------------------------------------

    def _resolve_review_task_button_updates(self, *, running, active_task=None):
        """返回中差评 / 品退 / 缓存按钮的状态描述。"""
        return [
            (
                self.review_find_button,
                running,
                "正在获取..." if running and active_task == TASK_REVIEW_MATCH else "获取差评订单",
            ),
            (
                self.review_full_scan_button,
                running,
                "正在完整补查..." if running and active_task == TASK_REVIEW_FULL_SCAN else "完整补查订单",
            ),
            (
                self.quality_refund_button,
                running,
                "正在获取..." if running and active_task == TASK_QUALITY_REFUND else "获取品退订单",
            ),
            (
                self.order_cache_button,
                running,
                "正在刷新缓存..."
                if running and active_task == TASK_CACHE_REFRESH
                else (
                    "正在重建缓存..."
                    if running and active_task == TASK_CACHE_REBUILD
                    else "订单缓存管理"
                ),
            ),
        ]

    def _set_review_task_buttons(self, *, running, active_task=None):
        """同步中差评 / 品退按钮状态。"""
        self._apply_button_updates(
            self._resolve_review_task_button_updates(running=running, active_task=active_task)
        )

    @staticmethod
    def _session_task_type_for(task_label):
        mapping = {
            "批量处理": LICENSE_TASK_BATCH_DELIVERY,
            "中差评查找": LICENSE_TASK_REVIEW_FIND,
            "品质退款订单获取": LICENSE_TASK_QUALITY_REFUND,
            "完整补查订单": LICENSE_TASK_REVIEW_FULL_SCAN,
            "订单缓存同步": LICENSE_TASK_CACHE_MANAGE,
        }
        return mapping.get(task_label, LICENSE_TASK_BATCH_DELIVERY)

    def _ensure_task_license(self, task_label):
        """统一处理任务启动前的授权租约与运行能力校验。"""
        session_task_type = self._session_task_type_for(task_label)
        cached = self._get_cached_license_state()
        allowed_local = {"ok", "renewal_due"}
        if cached is None or cached.get("reason") not in allowed_local:
            _, reason = self._refresh_license_state_with_mode(local_only=True)
            if reason == "online_refresh_required" and LICENSE_REQUIRE_ONLINE_FOR_TASKS:
                _, reason = self._refresh_license_state_with_mode(local_only=False)
            if reason not in allowed_local:
                if reason in {"not_found", "invalid", "reactivation_required", "expired", "device_mismatch", "revoked"}:
                    if self._prompt_license_activation(reason):
                        cached = self._get_cached_license_state(max_age_seconds=None)
                        if cached is None or cached.get("reason") not in allowed_local:
                            self._show_activation_required(task_label)
                            return False
                    else:
                        self._show_activation_required(task_label)
                        return False
                elif reason == "compromised":
                    self.show_message(
                        QMessageBox.Warning,
                        "运行环境异常",
                        "检测到完整性或安全核异常，当前已暂停高价值任务，请重新下载安装包。",
                    )
                    return False
                else:
                    self.show_message(
                        QMessageBox.Warning,
                        "需要联网刷新授权",
                        f"执行{task_label}前，需要联网续签授权租约。",
                    )
                    return False

        grant = authorize_task(session_task_type)
        state = grant.state or load_runtime_state()
        reason = grant.degraded_reason or (grant.state.reason if grant.state else "invalid")
        if grant.granted and grant.state is not None:
            self._store_license_state(grant.state.to_info(), grant.state.reason, source="grant")
            if grant.state.reason == "renewal_due":
                self._schedule_license_refresh(force=True)
            return True

        if reason in {"not_found", "invalid", "reactivation_required", "expired", "device_mismatch", "revoked"}:
            state_info = state.to_info() if hasattr(state, "to_info") else (state if isinstance(state, dict) else {})
            self._store_license_state(state_info, reason, source="grant")
            if self._prompt_license_activation(reason):
                grant = authorize_task(session_task_type)
                if grant.granted and grant.state is not None:
                    self._store_license_state(grant.state.to_info(), grant.state.reason, source="grant")
                    return True
            self._show_activation_required(task_label)
            return False

        if reason == "compromised":
            self.show_message(
                QMessageBox.Warning,
                "运行环境异常",
                "检测到安全核或关键文件异常，当前已暂停高价值任务。",
            )
            return False

        self.show_message(
            QMessageBox.Warning,
            "需要联网刷新授权",
            f"执行{task_label}前，需要联网续签授权租约。",
        )
        return False

    def _can_start_task(self, active_worker, task_label):
        """统一处理任务启动前的占用与授权校验。"""
        if active_worker is not None:
            return False
        return self._ensure_task_license(task_label)

    def _show_order_cache_manage_dialog(self):
        """弹出订单缓存管理对话框，返回选中的任务类型。"""
        selected_task = {"task_type": None}

        def _select(task_type):
            selected_task["task_type"] = task_type
            return QDialog.Accepted

        result = self._show_action_dialog(
            QMessageBox.Question,
            "订单缓存管理",
            "请选择要执行的订单缓存任务。",
            (
                f"增量刷新会同步最近 {ORDER_CACHE_INCREMENTAL_DAYS} 天订单；"
                f"重建缓存会清空本地数据并重新抓取最近 {ORDER_CACHE_COVERAGE_DAYS} 天订单。"
            ),
            min_width=640,
            action_specs=(
                ("关闭", "MessageSecondary", lambda: QDialog.Rejected),
                (
                    f"重建最近 {ORDER_CACHE_COVERAGE_DAYS} 天",
                    "MessageSecondary",
                    lambda: _select(TASK_CACHE_REBUILD),
                ),
                (
                    f"增量刷新最近 {ORDER_CACHE_INCREMENTAL_DAYS} 天",
                    "MessagePrimary",
                    lambda: _select(TASK_CACHE_REFRESH),
                ),
            ),
        )
        if result != QDialog.Accepted:
            return None
        return selected_task["task_type"]

    def _start_review_worker(self, *, task_type, days, start_message, clear_order_input=True):
        """启动中差评 / 品退后台任务。"""
        self.review_task_type = task_type
        if clear_order_input:
            self._set_order_input_values([])
        self.clear_result_log()
        self._append_logs(start_message)
        self._set_review_task_buttons(running=True, active_task=task_type)

        worker = ReviewMatcherWorker(days=days, task_type=task_type)
        self._start_thread_worker(
            thread_attr="review_worker_thread",
            worker_attr="review_worker",
            worker=worker,
            clear_callback=self._clear_review_worker_refs,
            signal_bindings=(
                (worker.progress, self._on_review_progress),
                (worker.order_ids_ready, self._on_review_order_ids),
                (worker.missing_config, self.show_missing_config_error),
                (worker.finished, self._on_review_finished),
            ),
            quit_signals=(worker.finished,),
        )

    def on_review_find_clicked(self):
        """开始查找中差评订单。"""
        if not self._can_start_task(self.review_worker, "中差评查找"):
            return

        days = self.review_days_spin.value()
        self._start_review_worker(
            task_type=TASK_REVIEW_MATCH,
            days=days,
            start_message=f"开始查找最近 {days} 天的中差评订单...",
        )

    def on_quality_refund_clicked(self):
        """开始获取品质退款订单。"""
        if not self._can_start_task(self.review_worker, "品质退款订单获取"):
            return

        days = self.review_days_spin.value()
        self._start_review_worker(
            task_type=TASK_QUALITY_REFUND,
            days=days,
            start_message=f"开始获取最近 {days} 天的品质退款订单...",
        )

    def on_review_full_scan_clicked(self):
        """开始完整补查订单（最近 30 天用缓存，更早范围临时抓取）。"""
        if not self._can_start_task(self.review_worker, "完整补查订单"):
            return

        days = self.review_days_spin.value()
        self._start_review_worker(
            task_type=TASK_REVIEW_FULL_SCAN,
            days=days,
            start_message=f"开始完整补查最近 {days} 天差评订单（最近 30 天命中缓存，超出范围临时抓取）...",
        )

    def on_order_cache_manage_clicked(self):
        """打开订单缓存管理入口。"""
        if not self._can_start_task(self.review_worker, "订单缓存同步"):
            return

        task_type = self._show_order_cache_manage_dialog()
        if task_type is None:
            return

        days = self.review_days_spin.value()
        if task_type == TASK_CACHE_REBUILD:
            start_message = f"开始重建最近 {ORDER_CACHE_COVERAGE_DAYS} 天订单缓存..."
        else:
            start_message = f"开始增量刷新最近 {ORDER_CACHE_INCREMENTAL_DAYS} 天订单缓存..."

        self._start_review_worker(
            task_type=task_type,
            days=days,
            start_message=start_message,
            clear_order_input=False,
        )

    def _on_review_progress(self, message):
        """追加中差评查找进度日志。"""
        self._append_logs(message)

    def _on_review_order_ids(self, order_ids):
        """将匹配到的订单号回填到订单输入框。"""
        self._set_order_input_values(order_ids)

    def _handle_cache_task_finished(self, *, status, warning_text, matched_count):
        """处理订单缓存任务完成后的提示。"""
        action_label = "订单缓存重建" if self.review_task_type == TASK_CACHE_REBUILD else "订单缓存刷新"
        summary = f"{action_label}完成：写入/更新 {matched_count} 个订单。"
        self._show_task_summary_message(
            summary=summary,
            warning_text=warning_text if status == TERMINAL_STATUS_WARNING else "",
            warning_title="缓存任务提醒",
            success_title="缓存任务完成",
        )

    def _handle_quality_refund_finished(self, *, status, warning_text, matched_count, total_count):
        """处理品质退款任务完成后的提示。"""
        if total_count <= 0:
            self.show_message(QMessageBox.Warning, "查找完成", "未找到品质退款订单。")
            return

        summary = (
            f"品退订单获取完成：共 {total_count} 个订单，"
            f"回填 {matched_count} 个订单号。"
        )
        self._show_task_summary_message(
            summary=summary,
            warning_text=warning_text if status == TERMINAL_STATUS_WARNING else "",
            warning_title="查找提醒",
            success_title="查找完成",
        )

    def _handle_review_match_finished(self, *, status, warning_text, matched_count, total_count):
        """处理中差评/完整补查任务完成后的提示。"""
        if total_count <= 0:
            return

        task_label = "完整补查" if self.review_task_type == TASK_REVIEW_FULL_SCAN else "中差评查找"
        summary = (
            f"{task_label}完成：共 {total_count} 条差评，"
            f"匹配到 {matched_count} 个订单。"
        )
        self._show_task_summary_message(
            summary=summary,
            warning_text=warning_text if status == TERMINAL_STATUS_WARNING else "",
            warning_title="查找提醒",
            success_title="查找完成",
            success_level=QMessageBox.Information if matched_count > 0 else QMessageBox.Warning,
        )

    def _on_review_finished(self, status, message, matched_count, total_count):
        """中差评 / 品退查找完成。"""
        task_type = self.review_task_type
        self._set_review_task_buttons(running=False)
        warning_text = (message or "").strip()

        if status == TERMINAL_STATUS_CANCELLED:
            return

        if status == TERMINAL_STATUS_ERROR:
            self._show_task_terminal_error(task_type=task_type, message=message)
            return

        if task_type in (TASK_CACHE_REFRESH, TASK_CACHE_REBUILD):
            self._handle_cache_task_finished(
                status=status,
                warning_text=warning_text,
                matched_count=matched_count,
            )
            return

        if task_type == TASK_QUALITY_REFUND:
            self._handle_quality_refund_finished(
                status=status,
                warning_text=warning_text,
                matched_count=matched_count,
                total_count=total_count,
            )
            return

        self._handle_review_match_finished(
            status=status,
            warning_text=warning_text,
            matched_count=matched_count,
            total_count=total_count,
        )

    def _clear_review_worker_refs(self):
        """清理中差评查找线程引用。"""
        self.review_worker = None
        self.review_worker_thread = None
        self.review_task_type = None
