# -*- coding: utf-8 -*-
"""TLS-shipinhao 主窗口。"""

import os
import sys

from PySide6.QtCore import Qt, QThread
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

from ..config import (
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
from ..constants import (
    APP_COLORS,
    AUTHOR_WECHAT,
    BADGE_HEIGHT,
    BADGE_MIN_WIDTH,
    BADGE_RADIUS,
    BUTTON_HEIGHT,
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
    TUTORIAL_URL,
    VERY_HIGH_DPI_COMPACT_THRESHOLD,
    WIDE_LAYOUT_MIN_HEIGHT,
    WIDE_LAYOUT_MIN_WIDTH,
    WINDOW_TITLE,
    get_platform_default_window_size,
    scale_px,
    set_ui_scale,
)
from ..core.license import check_stored_license
from .widgets import (
    BatchInputEdit,
    LicenseDialog,
    build_fixed_font,
    build_font,
    get_dialog_action_spacing,
    get_dialog_content_margins,
    get_dialog_section_spacing,
    get_dialog_text_spacing,
    get_license_reason_text,
    reset_font_caches,
)
from .worker import BatchWorker
from .review_worker import (
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


class MainWindow(QWidget):
    """主窗口。"""

    def __init__(self, license_reason="ok", license_info=None):
        super().__init__()
        self.worker_thread = None
        self.worker = None
        self.is_paused = False
        self.review_worker_thread = None
        self.review_worker = None
        self.review_task_type = None
        self._batch_rows = []
        self._license_reason = license_reason
        self._license_info = license_info or {}
        self._initial_height_fit_applied = False
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

    @staticmethod
    def _build_stylesheet():
        """构建全局 QSS 样式表。"""
        c = APP_COLORS
        hero_radius = scale_px(HERO_RADIUS, min_value=12)
        card_radius = scale_px(CARD_RADIUS, min_value=10)
        input_edit_radius = scale_px(INPUT_EDIT_RADIUS, min_value=8)
        input_edit_padding = scale_px(INPUT_EDIT_PADDING, min_value=8)
        log_edit_radius = scale_px(LOG_EDIT_RADIUS, min_value=8)
        log_edit_padding = scale_px(LOG_EDIT_PADDING, min_value=8)
        input_badge_radius = scale_px(INPUT_BADGE_RADIUS, min_value=7)
        button_radius = scale_px(12, min_value=8)
        setup_card_radius = scale_px(14, min_value=9)
        setup_title_radius = scale_px(10, min_value=7)
        scroll_width = scale_px(12, min_value=8)
        scroll_margin = scale_px(8, min_value=4)
        scroll_side_margin = scale_px(4, min_value=2)
        scroll_handle_height = scale_px(36, min_value=20)
        return f"""
            QWidget#AppRoot,
            QWidget#ScrollViewport,
            QWidget#PageWidget {{
                background: {c["window_base"]};
            }}
            QWidget {{
                color: {c["text"]};
            }}
            QLabel {{
                background: transparent;
            }}
            QFrame#HeroCard {{
                background: {c["surface_soft"]};
                border: 1px solid {c["hero_border"]};
                border-radius: {hero_radius}px;
            }}
            QFrame#OrderCard,
            QFrame#TrackingCard,
            QFrame#ConfigCard {{
                background: {c["surface"]};
                border: 1px solid {c["border"]};
                border-radius: {card_radius}px;
            }}
            QFrame#ReviewCard,
            QFrame#ActionCard,
            QFrame#LogCard,
            QFrame#LicenseCard {{
                background: {c["surface"]};
                border: 1px solid {c["border"]};
                border-radius: {card_radius}px;
            }}
            QPlainTextEdit#InputEdit {{
                background: {c["input_bg"]};
                color: {c["text"]};
                border: 1px solid {c["input_border"]};
                border-radius: {input_edit_radius}px;
                padding: {input_edit_padding}px;
                selection-background-color: {c["blue"]};
            }}
            QPlainTextEdit#InputEdit:focus {{
                border: 2px solid {c["input_border_focus"]};
                background: {c["input_bg"]};
            }}
            QPlainTextEdit#LogEdit {{
                background: {c["surface"]};
                color: {c["text"]};
                border: 1px solid {c["border"]};
                border-radius: {log_edit_radius}px;
                padding: {log_edit_padding}px;
                selection-background-color: {c["blue"]};
            }}
            QPushButton#PrimaryButton {{
                background: {c["orange"]};
                color: white;
                border: 1px solid {c["orange_deep"]};
                border-radius: {button_radius}px;
                padding: {scale_px(12, min_value=8)}px {scale_px(20, min_value=14)}px;
                font-weight: 700;
            }}
            QPushButton#PrimaryButton:hover {{
                background: {c["orange_deep"]};
            }}
            QPushButton#PrimaryButton:pressed {{
                background: {c["orange_deep"]};
            }}
            QPushButton#PrimaryButton:disabled {{
                background: {c["neutral_bg"]};
                color: {c["neutral_text"]};
                border: 1px solid {c["neutral_border"]};
            }}
            QPushButton#PauseButton {{
                background: {c["surface_soft"]};
                color: {c["blue_deep"]};
                border: 1px solid {c["border_strong"]};
                border-radius: {button_radius}px;
                padding: {scale_px(12, min_value=8)}px {scale_px(20, min_value=14)}px;
                font-weight: 700;
            }}
            QPushButton#PauseButton:hover {{
                background: {c["blue_tint"]};
            }}
            QPushButton#PauseButton:pressed {{
                background: {c["blue_soft"]};
            }}
            QPushButton#PauseButton:disabled {{
                background: {c["neutral_bg"]};
                color: {c["neutral_text"]};
                border: 1px solid {c["neutral_border"]};
            }}
            QLabel#HeroTitle {{
                color: {c["heading"]};
            }}
            QLabel#HeroSubtitle {{
                color: {c["muted"]};
            }}
            QLabel#SectionTitle {{
                color: {c["heading"]};
            }}
            QLabel#MetricChip {{
                background: {c["blue_soft"]};
                color: {c["blue_deep"]};
                border: 1px solid {c["blue_tint"]};
                border-radius: {input_badge_radius}px;
                padding: {scale_px(6, min_value=4)}px {scale_px(10, min_value=8)}px;
            }}
            QLabel#StatusBadge {{
                border-radius: {input_badge_radius}px;
                padding: {scale_px(6, min_value=4)}px {scale_px(10, min_value=8)}px;
                font-weight: 700;
            }}
            QLabel#LogTitle {{
                color: {c["heading"]};
            }}
            QLabel#LogHint {{
                color: {c["muted"]};
            }}
            QLabel#ConfigPath {{
                color: {c["heading"]};
            }}
            QLabel#ConfigNote {{
                color: {c["muted"]};
            }}
            QFrame#SetupSectionCard {{
                background: {c["surface_soft"]};
                border: 1px solid {c["blue_tint"]};
                border-radius: {setup_card_radius}px;
            }}
            QLabel#SetupSectionTitle {{
                background: {c["blue_soft"]};
                color: {c["blue_deep"]};
                border: 1px solid {c["blue_tint"]};
                border-radius: {setup_title_radius}px;
                padding: {scale_px(4, min_value=3)}px {scale_px(10, min_value=8)}px;
                font-weight: 700;
            }}
            QFrame#ConfigPathPanel {{
                background: {c["surface"]};
                border: 1px solid {c["border"]};
                border-radius: {button_radius}px;
            }}
            QFrame#LicenseInfoPanel {{
                background: {c["surface_soft"]};
                border: 1px solid {c["blue_tint"]};
                border-radius: {setup_card_radius}px;
            }}
            QLabel#LicenseSummary {{
                color: {c["heading"]};
            }}
            QLabel#LicenseMeta {{
                color: {c["muted"]};
            }}
            QLabel#LogHintPill {{
                background: {c["blue_soft"]};
                color: {c["blue_deep"]};
                border: 1px solid {c["blue_tint"]};
                border-radius: {setup_title_radius}px;
                padding: {scale_px(4, min_value=3)}px {scale_px(10, min_value=8)}px;
                font-weight: 700;
            }}
            QPushButton#SecondaryButton {{
                background: {c["blue_soft"]};
                color: {c["blue_deep"]};
                border: 1px solid {c["blue_tint"]};
                border-radius: {button_radius}px;
                padding: {scale_px(10, min_value=7)}px {scale_px(16, min_value=12)}px;
                font-weight: 700;
            }}
            QPushButton#SecondaryButton:hover {{
                background: {c["blue_tint"]};
                border-color: {c["border_strong"]};
            }}
            QPushButton#SecondaryButton:pressed {{
                background: {c["blue_soft"]};
            }}
            QScrollArea {{
                border: none;
                background: transparent;
            }}
            QScrollBar:vertical {{
                background: {c["border"]};
                width: {scroll_width}px;
                margin: {scroll_margin}px {scroll_side_margin}px {scroll_margin}px 0;
                border-radius: {scale_px(8, min_value=6)}px;
            }}
            QScrollBar::handle:vertical {{
                background: {c["border_strong"]};
                min-height: {scroll_handle_height}px;
                border-radius: {scale_px(8, min_value=6)}px;
            }}
            QScrollBar::handle:vertical:hover {{
                background: {c["muted_soft"]};
            }}
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
                height: 0;
            }}
            QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{
                background: transparent;
            }}
            """

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
        self.header_card = QFrame()
        self.header_card.setObjectName("HeroCard")
        self.header_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.page_layout.addWidget(self.header_card)

        self.header_box = QHBoxLayout(self.header_card)
        self.header_box.setContentsMargins(
            scale_px(HERO_PADDING_X, min_value=14),
            scale_px(HERO_PADDING_Y, min_value=6),
            scale_px(HERO_PADDING_X, min_value=14),
            scale_px(HERO_PADDING_Y, min_value=6),
        )
        self.header_box.setSpacing(scale_px(12, min_value=6))

        title_wrap = QWidget()
        self.title_box = QVBoxLayout(title_wrap)
        self.title_box.setContentsMargins(0, 0, 0, 0)
        self.title_box.setSpacing(scale_px(4, min_value=2))

        title_label = QLabel("驼铃视频小店中差评处理")
        title_label.setObjectName("HeroTitle")
        title_label.setFont(build_font(FONT_SIZES["title"], bold=True))
        self.hero_title_label = title_label
        self.title_box.addWidget(title_label)

        self.title_description_label = QLabel(
            "软件实现自动化批量处理中差评、品质退款订单的功能。"
        )
        self.title_description_label.setObjectName("HeroSubtitle")
        self.title_description_label.setWordWrap(True)
        self.title_description_label.setFont(build_font(FONT_SIZES["badge"]))
        self.title_box.addWidget(self.title_description_label)

        self.header_box.addWidget(title_wrap, 1)

        badge_wrap = QWidget()
        badge_layout = QHBoxLayout(badge_wrap)
        badge_layout.setContentsMargins(0, 0, 0, 0)
        badge_layout.setSpacing(scale_px(12, min_value=6))

        self.author_badge = QLabel(f"微信：{AUTHOR_WECHAT}")
        self.author_badge.setAlignment(Qt.AlignCenter)
        self.author_badge.setFont(build_font(FONT_SIZES["badge"], bold=True))
        self.author_badge.setMinimumSize(
            scale_px(BADGE_MIN_WIDTH, min_value=64),
            scale_px(BADGE_HEIGHT, min_value=28),
        )
        self.author_badge.setStyleSheet(
            self._build_badge_style(
                APP_COLORS["blue_soft"],
                APP_COLORS["blue_deep"],
                APP_COLORS["blue_tint"],
                radius=scale_px(BADGE_RADIUS, min_value=10),
                padding=self._scaled_padding(7, 12),
            )
        )
        self.author_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
        badge_layout.addWidget(self.author_badge, 0, Qt.AlignVCenter)

        self.tutorial_badge = QLabel()
        self.tutorial_badge.setAlignment(Qt.AlignCenter)
        self.tutorial_badge.setTextFormat(Qt.RichText)
        self.tutorial_badge.setTextInteractionFlags(Qt.TextBrowserInteraction)
        self.tutorial_badge.setOpenExternalLinks(True)
        self.tutorial_badge.setCursor(Qt.PointingHandCursor)
        self.tutorial_badge.setFont(build_font(FONT_SIZES["badge"], bold=True))
        self.tutorial_badge.setMinimumSize(
            scale_px(BADGE_MIN_WIDTH, min_value=64),
            scale_px(BADGE_HEIGHT, min_value=28),
        )
        self.tutorial_badge.setText(
            f'<a href="{TUTORIAL_URL}" style="color: {APP_COLORS["blue_deep"]}; text-decoration: none;">查看使用教程</a>'
        )
        self.tutorial_badge.setStyleSheet(
            self._build_badge_style(
                APP_COLORS["surface_soft"],
                APP_COLORS["blue_deep"],
                APP_COLORS["border"],
                radius=scale_px(BADGE_RADIUS, min_value=10),
                padding=self._scaled_padding(7, 12),
            )
        )
        self.tutorial_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
        badge_layout.addWidget(self.tutorial_badge, 0, Qt.AlignVCenter)

        self.header_box.addWidget(badge_wrap, 0, Qt.AlignVCenter | Qt.AlignRight)
        self._sync_window_title_with_license(self._license_reason)

    @staticmethod
    def _build_badge_style(
        background,
        text_color,
        border_color,
        *,
        radius=None,
        padding=None,
    ):
        """构建徽标样式。"""
        if radius is None:
            radius = scale_px(INPUT_BADGE_RADIUS, min_value=7)
        if padding is None:
            padding = f"{scale_px(5, min_value=3)}px {scale_px(9, min_value=6)}px"
        return (
            f"background: {background};"
            f"color: {text_color};"
            f"border: 1px solid {border_color};"
            f"border-radius: {radius}px;"
            f"padding: {padding};"
        )

    @staticmethod
    def _scaled_padding(vertical, horizontal):
        """按当前 UI 缩放系数返回统一 padding 字符串。"""
        return (
            f"{scale_px(vertical, min_value=1)}px "
            f"{scale_px(horizontal, min_value=1)}px"
        )

    @staticmethod
    def _standard_layout_spacing():
        """主界面统一内容间距。"""
        return scale_px(ROW_GAP, min_value=8)

    def _build_main_content(self):
        """构建主内容区。"""
        self.order_count_badge = self._create_count_badge()
        self.tracking_count_badge = self._create_count_badge()

        self.order_edit = self._create_input_editor("请用英文逗号、换行分隔，最多100个")
        self.tracking_edit = self._create_input_editor("请用英文逗号、换行分隔，最多100个")
        self.order_edit.textChanged.connect(self.refresh_input_metrics)
        self.tracking_edit.textChanged.connect(self.refresh_input_metrics)
        self.order_edit.normalized.connect(self.refresh_input_metrics)
        self.tracking_edit.normalized.connect(self.refresh_input_metrics)

        self.config_card = self.create_card(
            "第1步:系统配置与订单获取",
            self._create_config_badge(),
            self._build_setup_content(),
            "ConfigCard",
        )
        self.config_title_label = self.config_card.title_label

        self.order_card = self.create_card(
            "第2步:填写订单号",
            self.order_count_badge,
            self.order_edit,
            "OrderCard",
        )
        self.tracking_card = self.create_card(
            "第3步:填写物流单号",
            self.tracking_count_badge,
            self.tracking_edit,
            "TrackingCard",
        )

        self.action_card = self.create_card(
            "第4步:执行批量处理",
            None,
            self._build_action_content(),
            "ActionCard",
        )
        self.action_title_label = self.action_card.title_label
        self.license_card = self.create_card(
            None,
            None,
            self._build_license_content(),
            "LicenseCard",
        )

        self.log_hint_label = QLabel("按时间顺序滚动")
        self.log_hint_label.setObjectName("LogHintPill")
        self.log_hint_label.setFont(build_font(FONT_SIZES["hint"], bold=True))
        self.log_hint_label.setAlignment(Qt.AlignCenter)
        self.log_hint_label.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
        self.log_hint_label.setToolTip("最近执行记录会按时间顺序滚动显示")

        self.log_view = QPlainTextEdit()
        self.log_view.setObjectName("LogEdit")
        self.log_view.setReadOnly(True)
        self.log_view.setFont(build_fixed_font(11))
        self.log_view.setMinimumHeight(scale_px(LOG_PANEL_MIN_HEIGHT, min_value=128))
        self.log_view.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
        self.log_view.setPlaceholderText("执行日志会显示在这里")

        self.log_card = self.create_card(
            "执行日志",
            self.log_hint_label,
            self.log_view,
            "LogCard",
        )
        self.log_title_label = self.log_card.title_label

        self.main_content = QWidget()
        self.main_content.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.main_content_layout = QHBoxLayout(self.main_content)
        self.main_content_layout.setContentsMargins(0, 0, 0, 0)
        self.main_content_layout.setSpacing(scale_px(ROW_GAP, min_value=6))
        self.main_content_layout.setAlignment(Qt.AlignTop)

        self.action_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.config_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.license_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.order_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.tracking_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.log_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)

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

        self.right_column_layout.addLayout(self.input_row_layout, 0)
        self.right_column_layout.addWidget(self.log_card, 1)

        self.main_content_layout.addLayout(self.left_column_layout, 4)
        self.main_content_layout.addLayout(self.right_column_layout, 7)
        self.page_layout.addWidget(self.main_content, 0, Qt.AlignTop)
        self.page_layout.addStretch(1)
        self._sync_window_title_with_license(self._license_reason, self._license_info)

    # -----------------------------------------------------------------------
    # 卡片 / 输入框工厂
    # -----------------------------------------------------------------------

    def create_card(self, title, title_right, content, object_name):
        """创建统一卡片。"""
        card = QFrame()
        card.setObjectName(object_name)
        card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)

        card_layout = QVBoxLayout(card)
        card_layout.setContentsMargins(
            scale_px(CARD_PADDING, min_value=6),
            scale_px(CARD_PADDING, min_value=6),
            scale_px(CARD_PADDING, min_value=6),
            scale_px(CARD_PADDING, min_value=6),
        )
        card_layout.setSpacing(scale_px(CARD_HEADER_GAP, min_value=3))

        card.title_label = None
        if title or title_right is not None:
            header = QWidget()
            header_layout = QHBoxLayout(header)
            header_layout.setContentsMargins(0, 0, 0, 0)
            header_layout.setSpacing(max(scale_px(8, min_value=4), scale_px(ROW_GAP, min_value=6) // 2))
            header_height = scale_px(CARD_HEADER_HEIGHT, min_value=22)
            if title_right is not None:
                header_height = max(header_height, title_right.sizeHint().height())
            header.setMinimumHeight(header_height)

            if title:
                title_label = QLabel(title)
                title_label.setObjectName("LogTitle" if object_name == "LogCard" else "SectionTitle")
                title_label.setFont(build_font(FONT_SIZES["section_log"] if object_name == "LogCard" else FONT_SIZES["section"], bold=True))
                title_label.setWordWrap(True)
                title_label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
                header_layout.addWidget(title_label, 1)
                card.title_label = title_label
            else:
                header_layout.addStretch(1)

            if title_right is not None:
                header_layout.addWidget(title_right, 0, Qt.AlignRight | Qt.AlignVCenter)

            card_layout.addWidget(header)

        card_layout.addWidget(content, 1)
        return card

    def _create_count_badge(self):
        """创建输入数量徽标。"""
        badge = QLabel()
        badge.setObjectName("MetricChip")
        badge.setAlignment(Qt.AlignCenter)
        badge.setMinimumWidth(scale_px(INPUT_BADGE_MIN_WIDTH, min_value=52))
        badge.setFixedHeight(scale_px(INPUT_BADGE_HEIGHT, min_value=24))
        badge.setFont(build_fixed_font(11))
        badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
        return badge

    def _create_config_badge(self):
        """创建配置状态徽标。"""
        self.config_badge = QLabel("未配置")
        self.config_badge.setObjectName("StatusBadge")
        self.config_badge.setAlignment(Qt.AlignCenter)
        self.config_badge.setMinimumWidth(scale_px(INPUT_BADGE_MIN_WIDTH, min_value=52))
        self.config_badge.setFixedHeight(scale_px(INPUT_BADGE_HEIGHT, min_value=24))
        self.config_badge.setFont(build_font(FONT_SIZES["secondary"], bold=True))
        self.config_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
        self.config_badge.setStyleSheet(
            self._build_badge_style(
                APP_COLORS["red_soft"],
                APP_COLORS["red"],
                APP_COLORS["red"],
            )
        )
        return self.config_badge

    def _create_input_editor(self, placeholder):
        """创建批量输入框。"""
        editor = BatchInputEdit(placeholder)
        editor.setMinimumHeight(
            self._calculate_editor_height(editor, max(6, scale_px(INPUT_VISIBLE_LINES, min_value=6)))
        )
        editor.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
        return editor

    def _create_review_button(self, text):
        """创建中差评卡片按钮。"""
        button = QPushButton(text)
        button.setObjectName("ReviewButton")
        button.setCursor(Qt.PointingHandCursor)
        button.setFont(build_font(FONT_SIZES["button"], bold=True))
        button.setFixedHeight(scale_px(BUTTON_HEIGHT, min_value=30))
        button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        button.setStyleSheet(
            f"""QPushButton#ReviewButton {{
                background: {APP_COLORS['blue']};
                color: white;
                border: 1px solid {APP_COLORS['blue_deep']};
                border-radius: {scale_px(12, min_value=8)}px;
                padding: {self._scaled_padding(10, 18)};
                font-weight: 700;
            }}
            QPushButton#ReviewButton:hover {{
                background: {APP_COLORS['blue_deep']};
            }}
            QPushButton#ReviewButton:pressed {{
                background: {APP_COLORS['blue_deep']};
            }}
            QPushButton#ReviewButton:disabled {{
                background: {APP_COLORS['neutral_bg']};
                color: {APP_COLORS['neutral_text']};
                border: 1px solid {APP_COLORS['neutral_border']};
            }}"""
        )
        return button

    def _create_setup_section_label(self, text):
        """创建组合卡中的小节标题。"""
        label = QLabel(text)
        label.setObjectName("SetupSectionTitle")
        label.setFont(build_font(FONT_SIZES["secondary"], bold=True))
        label.setAlignment(Qt.AlignCenter)
        label.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
        return label

    def _build_setup_section_card(self, title, content):
        """构建组合卡中的分组容器。"""
        card = QFrame()
        card.setObjectName("SetupSectionCard")
        card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)

        layout = QVBoxLayout(card)
        layout.setContentsMargins(
            scale_px(SETUP_SECTION_PADDING, min_value=8),
            scale_px(SETUP_SECTION_PADDING, min_value=8),
            scale_px(SETUP_SECTION_PADDING, min_value=8),
            scale_px(SETUP_SECTION_PADDING, min_value=8),
        )
        layout.setSpacing(self._standard_layout_spacing())
        if title:
            layout.addWidget(self._create_setup_section_label(title), 0, Qt.AlignLeft)
        layout.addWidget(content)
        return card

    def _build_setup_content(self):
        """构建系统配置与订单获取组合区域。"""
        content = QWidget()
        self.setup_content_layout = QVBoxLayout(content)
        self.setup_content_layout.setContentsMargins(0, 0, 0, 0)
        self.setup_content_layout.setSpacing(self._standard_layout_spacing())

        config_content = self._build_config_content()
        config_content.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.setup_content_layout.addWidget(self._build_setup_section_card("配置目录", config_content))

        review_content = self._build_review_content()
        review_content.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.setup_content_layout.addWidget(self._build_setup_section_card(None, review_content))
        return content

    def _build_config_content(self):
        """构建配置卡片内容。"""
        content = QWidget()
        self.config_content_layout = QVBoxLayout(content)
        self.config_content_layout.setContentsMargins(0, 0, 0, 0)
        self.config_content_layout.setSpacing(self._standard_layout_spacing())

        self.config_path_panel = QFrame()
        self.config_path_panel.setObjectName("ConfigPathPanel")
        self.config_path_panel.setMinimumHeight(scale_px(CONFIG_PATH_MIN_HEIGHT, min_value=48))
        self.config_path_panel.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        path_layout = QVBoxLayout(self.config_path_panel)
        path_layout.setContentsMargins(
            scale_px(12, min_value=8),
            scale_px(12, min_value=8),
            scale_px(12, min_value=8),
            scale_px(12, min_value=8),
        )
        path_layout.setSpacing(self._standard_layout_spacing())

        self.config_path_label = QLabel()
        self.config_path_label.setObjectName("ConfigPath")
        self.config_path_label.setWordWrap(True)
        self.config_path_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        self.config_path_label.setFont(build_font(FONT_SIZES["body"]))
        self.config_path_label.setTextInteractionFlags(Qt.TextSelectableByMouse)
        self.config_path_label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
        path_layout.addWidget(self.config_path_label)

        self.config_note_label = QLabel()
        self.config_note_label.setObjectName("ConfigNote")
        self.config_note_label.setWordWrap(True)
        self.config_note_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        self.config_note_label.setFont(build_font(FONT_SIZES["secondary"]))
        self.config_note_label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
        path_layout.addWidget(self.config_note_label)

        self.config_content_layout.addWidget(self.config_path_panel, 1)

        actions = QWidget()
        actions_layout = QVBoxLayout(actions)
        actions_layout.setContentsMargins(0, 0, 0, 0)
        actions_layout.setSpacing(self._standard_layout_spacing())

        self.auto_cookie_button = self._create_review_button("自动获取 cookie 并保存")
        self.auto_cookie_button.clicked.connect(self.open_cookie_capture_dialog)
        actions_layout.addWidget(self.auto_cookie_button, 1)

        self.config_content_layout.addWidget(actions)

        return content

    def _build_review_content(self):
        """构建中差评卡片内容。"""
        content = QWidget()
        self.review_content_layout = QVBoxLayout(content)
        self.review_content_layout.setContentsMargins(0, 0, 0, 0)
        self.review_content_layout.setSpacing(self._standard_layout_spacing())

        days_row = QWidget()
        days_row_layout = QHBoxLayout(days_row)
        days_row_layout.setContentsMargins(0, 0, 0, 0)
        days_row_layout.setSpacing(self._standard_layout_spacing())

        self.review_days_label = QLabel("选择订单查询天数")
        self.review_days_label.setFont(build_font(FONT_SIZES["body"], bold=True))
        self.review_days_label.setStyleSheet(f"color: {APP_COLORS['blue_deep']};")
        days_row_layout.addWidget(self.review_days_label, 0, Qt.AlignVCenter)
        days_row_layout.addStretch(1)

        self.review_days_spin = QSpinBox()
        self.review_days_spin.setRange(1, 90)
        self.review_days_spin.setValue(DEFAULT_REVIEW_DAYS)
        self.review_days_spin.setSuffix(" 天")
        self.review_days_spin.setAlignment(Qt.AlignCenter)
        self.review_days_spin.setFixedWidth(scale_px(128, min_value=96))
        self.review_days_spin.setFixedHeight(scale_px(36, min_value=28))
        self.review_days_spin.setFont(build_font(FONT_SIZES["button"], bold=True))
        self.review_days_spin.setStyleSheet(
            f"""QSpinBox {{
                background: {APP_COLORS['surface']};
                color: {APP_COLORS['text']};
                border: 1px solid {APP_COLORS['border']};
                border-radius: {scale_px(12, min_value=8)}px;
                padding: {self._scaled_padding(6, 10)};
            }}
            QSpinBox::up-button, QSpinBox::down-button {{
                width: {scale_px(22, min_value=16)}px;
            }}"""
        )
        days_row_layout.addWidget(self.review_days_spin, 0, Qt.AlignVCenter)
        self.review_content_layout.addWidget(days_row)

        self.review_find_button = self._create_review_button("获取差评订单")
        self.review_find_button.clicked.connect(self.on_review_find_clicked)

        self.review_full_scan_button = self._create_review_button("完整补查订单")
        self.review_full_scan_button.clicked.connect(self.on_review_full_scan_clicked)

        self.quality_refund_button = self._create_review_button("获取品退订单")
        self.quality_refund_button.clicked.connect(self.on_quality_refund_clicked)

        self.order_cache_button = self._create_review_button("订单缓存管理")
        self.order_cache_button.clicked.connect(self.on_order_cache_manage_clicked)
        self.review_buttons = [
            self.auto_cookie_button,
            self.review_find_button,
            self.quality_refund_button,
            self.review_full_scan_button,
            self.order_cache_button,
        ]

        first_button_row = QWidget()
        first_button_row_layout = QHBoxLayout(first_button_row)
        first_button_row_layout.setContentsMargins(0, 0, 0, 0)
        first_button_row_layout.setSpacing(self._standard_layout_spacing())
        first_button_row_layout.addWidget(self.review_find_button, 1)
        first_button_row_layout.addWidget(self.quality_refund_button, 1)
        self.review_content_layout.addWidget(first_button_row)

        second_button_row = QWidget()
        second_button_row_layout = QHBoxLayout(second_button_row)
        second_button_row_layout.setContentsMargins(0, 0, 0, 0)
        second_button_row_layout.setSpacing(self._standard_layout_spacing())
        second_button_row_layout.addWidget(self.review_full_scan_button, 1)
        second_button_row_layout.addWidget(self.order_cache_button, 1)
        self.review_content_layout.addWidget(second_button_row)

        self.review_content_layout.addStretch(1)
        return content

    def _build_action_content(self):
        """构建执行控制卡片内容。"""
        content = QWidget()
        self.action_content_layout = QVBoxLayout(content)
        self.action_content_layout.setContentsMargins(0, 0, 0, 0)
        self.action_content_layout.setSpacing(self._standard_layout_spacing())

        self.start_button = QPushButton("开始批量处理")
        self.start_button.setObjectName("PrimaryButton")
        self.start_button.setCursor(Qt.PointingHandCursor)
        self.start_button.setFont(build_font(FONT_SIZES["button"], bold=True))
        self.start_button.setFixedHeight(scale_px(BUTTON_HEIGHT, min_value=44))
        self.start_button.setMinimumWidth(scale_px(140, min_value=120))
        self.start_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.start_button.clicked.connect(self.on_start_clicked)
        self.action_content_layout.addWidget(self.start_button)

        self.pause_button = QPushButton("暂停批量处理")
        self.pause_button.setObjectName("PauseButton")
        self.pause_button.setCursor(Qt.PointingHandCursor)
        self.pause_button.setFont(build_font(FONT_SIZES["button"], bold=True))
        self.pause_button.setFixedHeight(scale_px(BUTTON_HEIGHT, min_value=44))
        self.pause_button.setMinimumWidth(scale_px(140, min_value=120))
        self.pause_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.pause_button.clicked.connect(self.on_pause_clicked)
        self.action_content_layout.addWidget(self.pause_button)
        self.action_buttons = [self.start_button, self.pause_button]
        return content

    def _build_license_content(self):
        """构建激活状态卡片内容。"""
        content = QWidget()
        self.license_content_layout = QVBoxLayout(content)
        self.license_content_layout.setContentsMargins(0, 0, 0, 0)
        self.license_content_layout.setSpacing(self._standard_layout_spacing())

        body_wrap = QWidget()
        body_wrap.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        body_layout = QVBoxLayout(body_wrap)
        body_layout.setContentsMargins(0, 0, 0, 0)
        body_layout.setSpacing(self._standard_layout_spacing())

        info_panel = QFrame()
        info_panel.setObjectName("LicenseInfoPanel")
        panel_layout = QVBoxLayout(info_panel)
        panel_layout.setContentsMargins(
            scale_px(14, min_value=8),
            scale_px(14, min_value=8),
            scale_px(14, min_value=8),
            scale_px(14, min_value=8),
        )
        panel_layout.setSpacing(self._standard_layout_spacing())

        self.license_summary_label = QLabel()
        self.license_summary_label.setObjectName("LicenseSummary")
        self.license_summary_label.setFont(build_font(FONT_SIZES["badge"], bold=True))
        self.license_summary_label.setWordWrap(True)
        panel_layout.addWidget(self.license_summary_label)

        self.license_meta_label = QLabel()
        self.license_meta_label.setObjectName("LicenseMeta")
        self.license_meta_label.setFont(build_font(FONT_SIZES["secondary"]))
        self.license_meta_label.setWordWrap(True)
        panel_layout.addWidget(self.license_meta_label)

        body_layout.addWidget(info_panel)
        self.license_content_layout.addStretch(1)
        self.license_content_layout.addWidget(body_wrap)
        self.license_content_layout.addStretch(1)
        return content

    # -----------------------------------------------------------------------
    # 窗口尺寸 / 响应式
    # -----------------------------------------------------------------------

    def _calculate_editor_height(self, editor, visible_lines=10):
        """按指定可见行数计算输入框高度。"""
        line_height = editor.fontMetrics().lineSpacing()
        document_margin = int(editor.document().documentMargin() * 2)
        frame = editor.frameWidth() * 2
        padding = scale_px(INPUT_EDIT_PADDING, min_value=8) + 2
        return line_height * visible_lines + document_margin + frame + padding

    @staticmethod
    def _resolve_height_profile(viewport_height):
        """根据当前可用高度返回垂直紧凑模式。"""
        if viewport_height <= 620:
            return "dense"
        if viewport_height <= 720:
            return "compact"
        return "comfortable"

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
        """首次打开时确保窗口在屏幕内，不超出可用区域。"""
        default_w, default_h = self._resolve_initial_window_size()
        self.setMinimumSize(
            scale_px(MIN_WINDOW_WIDTH, min_value=640),
            scale_px(MIN_WINDOW_HEIGHT, min_value=560),
        )
        self.resize(default_w, default_h)

    @staticmethod
    def _resolve_layout_mode(width, height):
        """根据当前可用尺寸判定缩放模式，但不改变布局结构。"""
        if width >= WIDE_LAYOUT_MIN_WIDTH and height >= WIDE_LAYOUT_MIN_HEIGHT:
            return "wide"
        if width >= COMPACT_LAYOUT_MIN_WIDTH:
            return "compact"
        return "dense"

    def _resolve_ui_scale(self):
        """结合缩放模式和逻辑 DPI，计算当前窗口的紧凑缩放系数（基于屏幕可用区域）。"""
        screen = QApplication.primaryScreen()
        if screen is None:
            return 1.0
        available = screen.availableGeometry()
        return self._resolve_ui_scale_for_size(available.width(), available.height())

    def _resolve_ui_scale_for_size(self, width, height):
        """根据给定窗口尺寸与平台/DPI 计算缩放系数，用于首次打开时与默认窗口匹配。"""
        screen = QApplication.primaryScreen()
        if screen is None:
            return 1.0

        layout_mode = self._resolve_layout_mode(width, height)
        scale_map = {
            "wide": 1.0,
            "compact": 0.92,
            "dense": 0.86,
        }
        scale = scale_map.get(layout_mode, 1.0)

        if sys.platform.startswith("win"):
            logical_dpi = screen.logicalDotsPerInch()
            if logical_dpi >= VERY_HIGH_DPI_COMPACT_THRESHOLD:
                scale *= 0.97 if layout_mode == "wide" else 0.92
            elif logical_dpi >= HIGH_DPI_COMPACT_THRESHOLD:
                scale *= 0.985 if layout_mode == "wide" else 0.96
            else:
                # Windows 常见 100%~125% 显示缩放下，默认略降系数更友好
                scale *= 0.93
        else:
            logical_dpi = screen.logicalDotsPerInch()
            if logical_dpi >= VERY_HIGH_DPI_COMPACT_THRESHOLD and layout_mode != "wide":
                scale *= 0.94

        return max(MIN_UI_SCALE, min(MAX_UI_SCALE, scale))

    def _resolve_initial_window_size(self):
        """结合平台默认值与屏幕可用区域，计算首次打开时的窗口尺寸（逻辑像素，可缩放）。"""
        default_width, default_height = get_platform_default_window_size()
        screen = self.screen() or QApplication.primaryScreen()
        available = screen.availableGeometry() if screen is not None else None
        w, h = default_width, default_height
        if available is not None:
            if sys.platform.startswith("win"):
                max_w = int(available.width() * 0.96)
                max_h = int(available.height() * 0.97)
                target_w = max(default_width, int(available.width() * 0.55))
                w = min(target_w, max_w)
                h = min(default_height, max_h)
            else:
                max_w = int(available.width() * 0.92)
                max_h = int(available.height() * 0.92)
                w = min(w, max_w)
                h = min(h, max_h)
        return w, h

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

    def resizeEvent(self, event):
        """窗口尺寸变化时同步内部尺寸。"""
        self._sync_responsive_metrics()
        super().resizeEvent(event)

    def showEvent(self, event):
        """首次展示时按内容自然高度收口，避免底部出现大块空白。"""
        super().showEvent(event)
        self._sync_responsive_metrics()
        if self._initial_height_fit_applied:
            return
        self._fit_window_height_to_content()
        self._initial_height_fit_applied = True

    def _fit_window_height_to_content(self):
        """仅在窗口偏高时按当前内容自然高度收口。"""
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
        content_height = self.page_widget.sizeHint().height() + chrome_height
        target_height = max(self.minimumHeight(), content_height)

        screen = self.screen() or QApplication.primaryScreen()
        if screen is not None:
            available_height = screen.availableGeometry().height()
            max_ratio = 0.97 if sys.platform.startswith("win") else 0.92
            target_height = min(target_height, int(available_height * max_ratio))

        shrink_tolerance = scale_px(12, min_value=8)
        if self.height() > target_height + shrink_tolerance:
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
        """追加执行日志。"""
        self.log_view.appendPlainText(text)
        scrollbar = self.log_view.verticalScrollBar()
        scrollbar.setValue(scrollbar.maximum())

    def clear_result_log(self):
        """清空执行日志。"""
        self.log_view.clear()

    def _set_order_input_values(self, order_ids):
        """用给定订单号列表覆盖订单输入框。"""
        self.order_edit.blockSignals(True)
        try:
            self.order_edit.setPlainText("\n".join(order_ids))
        finally:
            self.order_edit.blockSignals(False)
        self.refresh_input_metrics()

    def _set_batch_input_values(self, order_ids, tracking_numbers):
        """同步覆盖订单号和物流单号输入框。"""
        self.order_edit.blockSignals(True)
        self.tracking_edit.blockSignals(True)
        try:
            self.order_edit.setPlainText("\n".join(order_ids))
            self.tracking_edit.setPlainText("\n".join(tracking_numbers))
        finally:
            self.order_edit.blockSignals(False)
            self.tracking_edit.blockSignals(False)
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

    def _style_message_box(self, dialog):
        """统一提示弹窗视觉。"""
        dialog.setStyleSheet(
            """
            QDialog#AppMessageDialog {
                background: """ + APP_COLORS["bg"] + """;
                border: 1px solid """ + APP_COLORS["border"] + """;
                border-radius: 14px;
            }
            QLabel#MessageTitle {
                color: """ + APP_COLORS["heading"] + """;
                font-size: 18px;
                font-weight: 700;
            }
            QLabel#MessageText {
                color: """ + APP_COLORS["text"] + """;
                font-size: 15px;
                line-height: 1.45;
            }
            QLabel#MessageInfo {
                color: """ + APP_COLORS["muted"] + """;
                font-size: 13px;
                line-height: 1.45;
            }
            QPushButton#MessagePrimary {
                background: """ + APP_COLORS["blue"] + """;
                color: #FFFFFF;
                border: 1px solid """ + APP_COLORS["blue_deep"] + """;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 700;
            }
            QPushButton#MessagePrimary:hover {
                background: """ + APP_COLORS["blue_deep"] + """;
            }
            QPushButton#MessagePrimary:pressed {
                background: """ + APP_COLORS["blue_deep"] + """;
            }
            QPushButton#MessageSecondary {
                background: """ + APP_COLORS["neutral_bg"] + """;
                color: """ + APP_COLORS["text"] + """;
                border: 1px solid """ + APP_COLORS["neutral_border"] + """;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 600;
            }
            QPushButton#MessageSecondary:hover {
                background: """ + APP_COLORS["border"] + """;
            }
            QPushButton#MessageSecondary:pressed {
                background: """ + APP_COLORS["border_strong"] + """;
            }
            """
        )
        return dialog

    def _message_icon_pixmap(self, level):
        """根据消息级别返回标准图标。"""
        icon_map = {
            QMessageBox.Information: QStyle.SP_MessageBoxInformation,
            QMessageBox.Warning: QStyle.SP_MessageBoxWarning,
            QMessageBox.Critical: QStyle.SP_MessageBoxCritical,
            QMessageBox.Question: QStyle.SP_MessageBoxQuestion,
        }
        icon_type = icon_map.get(level, QStyle.SP_MessageBoxInformation)
        return self.style().standardIcon(icon_type).pixmap(46, 46)

    def _create_message_dialog_base(self, level, title, text, informative_text="", *, min_width=560):
        """构建统一样式弹窗骨架，返回 (dialog, actions_layout)。"""
        dialog = QDialog(self)
        dialog.setObjectName("AppMessageDialog")
        dialog.setWindowTitle(title)
        dialog.setModal(True)
        dialog.setMinimumWidth(min_width)

        dialog.root_layout = QVBoxLayout(dialog)
        dialog.root_layout.setContentsMargins(*get_dialog_content_margins())
        dialog.root_layout.setSpacing(get_dialog_section_spacing())

        dialog.body_layout = QHBoxLayout()
        dialog.body_layout.setSpacing(get_dialog_section_spacing())

        icon_label = QLabel()
        icon_label.setPixmap(self._message_icon_pixmap(level))
        icon_label.setAlignment(Qt.AlignTop | Qt.AlignHCenter)
        icon_label.setFixedWidth(56)
        dialog.body_layout.addWidget(icon_label, 0, Qt.AlignTop)

        text_wrap = QWidget()
        dialog.text_layout = QVBoxLayout(text_wrap)
        dialog.text_layout.setContentsMargins(0, 0, 0, 0)
        dialog.text_layout.setSpacing(get_dialog_text_spacing())

        title_label = QLabel(title)
        title_label.setObjectName("MessageTitle")
        title_label.setWordWrap(True)
        dialog.text_layout.addWidget(title_label)

        text_label = QLabel(text)
        text_label.setObjectName("MessageText")
        text_label.setWordWrap(True)
        text_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        dialog.text_layout.addWidget(text_label)

        if informative_text:
            info_label = QLabel(informative_text)
            info_label.setObjectName("MessageInfo")
            info_label.setWordWrap(True)
            info_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
            dialog.text_layout.addWidget(info_label)

        dialog.body_layout.addWidget(text_wrap, 1)
        dialog.root_layout.addLayout(dialog.body_layout, 1)

        dialog.actions_layout = QHBoxLayout()
        dialog.actions_layout.setContentsMargins(0, 0, 0, 0)
        dialog.actions_layout.setSpacing(get_dialog_action_spacing())
        dialog.actions_layout.addStretch(1)
        dialog.root_layout.addLayout(dialog.actions_layout)

        self._style_message_box(dialog)
        return dialog, dialog.actions_layout

    def _add_message_action(self, actions_layout, text, object_name, callback):
        """向弹窗动作栏添加按钮。"""
        button = QPushButton(text)
        button.setObjectName(object_name)
        button.clicked.connect(callback)
        actions_layout.addWidget(button)
        return button

    def _build_message_dialog(self, level, title, text, informative_text=""):
        """构建普通提示弹窗（单确定按钮）。"""
        dialog, actions = self._create_message_dialog_base(
            level, title, text, informative_text, min_width=560,
        )
        self._add_message_action(actions, "确定", "MessagePrimary", dialog.accept)
        return dialog

    def show_message(self, level, title, text, informative_text=""):
        """显示统一样式的提示弹窗。"""
        dialog = self._build_message_dialog(level, title, text, informative_text)
        dialog.exec()

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

    # -----------------------------------------------------------------------
    # 按钮状态
    # -----------------------------------------------------------------------

    def refresh_action_buttons(self):
        """同步开始/暂停按钮状态。"""
        running = self.worker is not None
        self.order_edit.setReadOnly(running)
        self.tracking_edit.setReadOnly(running)
        if hasattr(self, "auto_cookie_button"):
            self.auto_cookie_button.setDisabled(running)
        self.pause_button.setDisabled((not running) or self.is_paused)
        self.start_button.setDisabled(running and not self.is_paused)
        self.start_button.setText("继续批量处理" if self.is_paused else "开始批量处理")
        self.pause_button.setText("已暂停" if self.is_paused else "暂停批量处理")

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

    def choose_config_dir(self):
        """选择配置文件所在目录并记住。"""
        start_dir = get_config_dir_cache() or get_saved_user_config_dir() or os.path.expanduser("~")
        selected_dir = QFileDialog.getExistingDirectory(self, "选择配置目录", start_dir)
        if not selected_dir:
            return

        resolved_files = resolve_config_files_in_dir(selected_dir)
        missing_files = []
        if not resolved_files or "cookie" not in resolved_files:
            missing_files.append("cookie(.txt)")
        else:
            try:
                cookie_data = read_cookie_data(resolved_files["cookie"])
            except Exception:  # noqa: BLE001
                missing_files.append("cookie(.txt) 内容不可读")
            else:
                magic_from_cookie = extract_biz_magic_from_cookie(cookie_data)
                if not magic_from_cookie:
                    missing_files.append("cookie 中 biz_magic 键")

        if missing_files:
            self.show_message(
                QMessageBox.Warning,
                "目录不完整",
                "所选目录缺少以下文件：" + "、".join(missing_files),
            )
            return

        save_user_config_dir(selected_dir)
        self.refresh_config_path_label()
        self.show_message(
            QMessageBox.Information,
            "配置目录已更新",
            f"后续将优先使用：\n{selected_dir}",
        )

    def open_cookie_capture_dialog(self):
        """打开网页登录窗口并自动抓取 Cookie；保存时选择目录并记住。"""
        # 延迟导入 QtWebEngine 相关模块，避免启动时加载
        try:
            from ..core.cookie_browser import CookieCaptureDialog, QTWEBENGINE_AVAILABLE, QTWEBENGINE_IMPORT_ERROR
        except ImportError as exc:
            self.show_message(
                QMessageBox.Warning,
                "当前环境缺少 QtWebEngine",
                "暂时无法打开内置网页登录窗口。",
                "请先安装支持 QtWebEngine 的 PySide6 组件后重试。\n"
                f"错误详情：{exc}",
            )
            return

        if not QTWEBENGINE_AVAILABLE:
            self.show_message(
                QMessageBox.Warning,
                "当前环境缺少 QtWebEngine",
                "暂时无法打开内置网页登录窗口。",
                "请先安装支持 QtWebEngine 的 PySide6 组件后重试。\n"
                f"错误详情：{QTWEBENGINE_IMPORT_ERROR or 'QtWebEngine 不可用'}",
            )
            return

        initial_dir = get_default_config_dir()
        self.append_result_log("正在打开内置网页登录窗口，登录成功后点击保存并选择目录即可。")
        try:
            dialog = CookieCaptureDialog(initial_dir, self)
        except Exception as exc:  # noqa: BLE001
            self.show_message(
                QMessageBox.Critical,
                "打开网页登录窗口失败",
                "QtWebEngine 已检测到，但初始化内置浏览器时失败。",
                str(exc),
            )
            return

        if dialog.exec() != QDialog.Accepted:
            self.append_result_log("已取消自动获取 Cookie。")
            return

        cookie_data = dialog.cookie_data
        magic_value = extract_biz_magic_from_cookie(cookie_data)
        if not magic_value:
            self.show_message(
                QMessageBox.Warning,
                "Cookie 尚未准备好",
                "当前未检测到 biz_magic，请在页面里完成登录后重试。",
            )
            return

        start_dir = get_config_dir_cache() or get_saved_user_config_dir() or os.path.expanduser("~")
        selected_dir = QFileDialog.getExistingDirectory(
            self,
            "选择 Cookie 保存位置",
            start_dir,
        )
        if not selected_dir:
            self.append_result_log("已取消选择目录，Cookie 未保存。")
            return

        try:
            cookie_path = save_cookie_data(cookie_data, config_dir=selected_dir, remember_dir=True)
        except Exception as exc:  # noqa: BLE001
            self.show_message(
                QMessageBox.Critical,
                "保存 Cookie 失败",
                "已抓取到登录态，但写入 cookie.txt 失败。",
                str(exc),
            )
            return

        self.refresh_config_path_label()
        self.append_result_log(f"Cookie 已保存到：{cookie_path}")
        self.append_result_log("已记住该路径，后续将从此目录读取 cookie.txt。")
        self.show_message(
            QMessageBox.Information,
            "Cookie 获取成功",
            f"已保存到：\n{cookie_path}",
            "已记住该配置目录，后续获取差评、获取品退和批量处理会直接使用此目录下的 cookie.txt。",
        )

    def show_missing_config_error(self, searched_dirs):
        """提示缺少配置文件，并允许用户直接选择目录。"""
        if isinstance(searched_dirs, str):
            details = searched_dirs
        else:
            details = "\n".join(str(item) for item in searched_dirs)
        info_text = f"错误详情:\n{details}"
        dialog, actions = self._create_message_dialog_base(
            QMessageBox.Warning,
            "缺少配置文件",
            "当前未找到可用配置目录，请先手动选择目录。",
            info_text,
            min_width=620,
        )
        self._add_message_action(actions, "关闭", "MessageSecondary", dialog.reject)
        self._add_message_action(actions, "选择配置目录", "MessagePrimary", dialog.accept)
        if dialog.exec() == QDialog.Accepted:
            self.choose_config_dir()

    # -----------------------------------------------------------------------
    # 授权
    # -----------------------------------------------------------------------

    def _refresh_license_state(self):
        """刷新本地授权状态。"""
        info, reason = check_stored_license()
        self._license_reason = reason
        self._license_info = info or {}
        self._sync_window_title_with_license(reason, info)
        return info, reason

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
            "expired": "已过期",
            "device_mismatch": "设备不符",
            "invalid": "状态异常",
            "not_found": "未激活",
        }
        status = status_map.get(reason, "未激活")
        self.setWindowTitle(WINDOW_TITLE)
        if hasattr(self, "license_status_badge"):
            self.license_status_badge.setText(status)
            self.license_status_badge.setStyleSheet(
                self._build_license_status_badge_style(reason == "ok")
            )
        expires_at = str(info.get("expires_at", ""))[:10]
        if hasattr(self, "license_summary_label"):
            if reason == "ok":
                self.license_summary_label.setText("软件已激活，可正常执行批量处理。")
            else:
                self.license_summary_label.setText(get_license_reason_text(reason))
        if hasattr(self, "license_meta_label"):
            if reason == "ok":
                meta_parts = []
                if expires_at:
                    meta_parts.append(f"有效期至：{expires_at}")
                device_id = str(info.get("device_id", "")).strip()
                if device_id:
                    meta_parts.append(f"设备尾号：{device_id[-6:]}")
                self.license_meta_label.setText(
                    "  |  ".join(meta_parts) or "授权信息已写入本地，可直接开始执行任务。"
                )
            elif reason == "expired" and expires_at:
                self.license_meta_label.setText(
                    f"当前授权已于 {expires_at} 到期，请联系微信 {AUTHOR_WECHAT} 获取新卡密。"
                )
            elif reason == "device_mismatch":
                self.license_meta_label.setText(
                    f"当前设备与原授权绑定设备不一致，请联系微信 {AUTHOR_WECHAT} 处理重绑。"
                )
            else:
                self.license_meta_label.setText(
                    f"联系微信 {AUTHOR_WECHAT} 获取卡密后，即可完成激活并开始使用。"
                )

    def _prompt_license_activation(self, reason=None):
        """弹出激活窗口，返回是否激活成功。"""
        if reason is None:
            _, reason = self._refresh_license_state()
        self.append_result_log(f"授权状态：{get_license_reason_text(reason)}")
        self.append_result_log("正在打开卡密激活窗口...")

        dialog = LicenseDialog(self, reason=reason)
        result = dialog.exec()
        if result == QDialog.Accepted and dialog.activated:
            info, refreshed_reason = self._refresh_license_state()
            if refreshed_reason == "ok":
                self.append_result_log("卡密激活成功，已解锁批量处理功能。")
                expires = str((info or {}).get("expires_at", ""))[:10]
                if expires:
                    self.append_result_log(f"授权有效期至：{expires}")
                return True

            self.append_result_log("卡密激活结果校验失败，请重试。")
            self.append_result_log(f"当前状态：{get_license_reason_text(refreshed_reason)}")
            return False

        _, refreshed_reason = self._refresh_license_state()
        self.append_result_log("卡密激活未完成。")
        self.append_result_log(f"当前状态：{get_license_reason_text(refreshed_reason)}")
        return False

    def prompt_license_on_startup(self):
        """启动后提示激活（仅在未激活时弹出）。"""
        info, reason = self._refresh_license_state()
        if reason == "ok":
            self.append_result_log("授权状态：已激活。")
            expires = str((info or {}).get("expires_at", ""))[:10]
            if expires:
                self.append_result_log(f"授权有效期至：{expires}")
            return True

        self.append_result_log("当前未激活，执行前需先输入卡密。")
        return self._prompt_license_activation(reason)

    # -----------------------------------------------------------------------
    # 批量处理
    # -----------------------------------------------------------------------

    def on_start_clicked(self):
        """开始或继续批量处理。"""
        if self.worker is not None and self.is_paused:
            self.worker.resume()
            self.is_paused = False
            self.refresh_action_buttons()
            self.append_result_log("已继续执行剩余任务。")
            return

        if self.worker is not None:
            return

        _, reason = self._refresh_license_state()
        if reason != "ok" and not self._prompt_license_activation(reason):
            self._show_activation_required("批量处理")
            return

        self.normalize_inputs()

        order_ids = parse_batch_input(self.order_edit.toPlainText())
        tracking_numbers = parse_batch_input(self.tracking_edit.toPlainText())

        if not order_ids or not tracking_numbers:
            self.show_message(QMessageBox.Information, "提示", "请输入订单号和新物流单号。")
            return

        if len(order_ids) != len(tracking_numbers):
            self.show_message(
                QMessageBox.Critical,
                "数量不匹配",
                f"订单号共 {len(order_ids)} 个，新物流单号共 {len(tracking_numbers)} 个。\n"
                "请确保一一对应后再执行。",
            )
            return

        if len(order_ids) > MAX_BATCH_SIZE:
            self.show_message(
                QMessageBox.Critical,
                "超出数量限制",
                f"一次最多处理 {MAX_BATCH_SIZE} 条，请拆分后再执行。",
            )
            return

        self._batch_rows = [
            {
                "order_id": order_id,
                "tracking_number": tracking_number,
                "succeeded": False,
            }
            for order_id, tracking_number in zip(order_ids, tracking_numbers)
        ]
        self.clear_result_log()
        self.append_result_log(f"开始执行：共 {len(order_ids)} 条。")
        self.set_submit_running(True)

        self.worker_thread = QThread(self)
        self.worker = BatchWorker(order_ids, tracking_numbers)
        self.worker.moveToThread(self.worker_thread)

        self.worker.started.connect(self._on_worker_started)
        self.worker.step_started.connect(self._on_worker_step_started)
        self.worker.step_succeeded.connect(self._on_worker_step_succeeded)
        self.worker.step_failed.connect(self._on_worker_step_failed)
        self.worker.fatal_error.connect(self._on_worker_fatal_error)
        self.worker.missing_config.connect(self.show_missing_config_error)
        self.worker.finished.connect(self._on_worker_finished)
        self.worker.finished.connect(self.worker_thread.quit)
        self._bind_thread_lifecycle(
            self.worker_thread,
            self.worker,
            self._clear_worker_refs,
        )
        self.worker_thread.start()
        self.refresh_action_buttons()

    def on_pause_clicked(self):
        """暂停后续批量任务。"""
        if self.worker is None or self.is_paused:
            return
        self.worker.pause()
        self.is_paused = True
        self.refresh_action_buttons()
        self.append_result_log("已暂停处理，当前单完成后将停止继续执行。")

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
        super().closeEvent(event)

    def _on_worker_started(self, total_count):
        """记录任务开始。"""
        self.append_result_log(f"任务已创建：共 {total_count} 条，准备顺序执行。")

    def _on_worker_step_started(self, index, total_count, order_id):
        """记录单条开始。"""
        self.append_result_log(f"[{index}/{total_count}] 开始处理订单 {order_id}")

    def _on_worker_step_succeeded(self, index, total_count, order_id, tracking_number, old_waybill):
        """记录单条成功。"""
        self._mark_batch_row_succeeded(index)
        self.append_result_log(
            f"[{index}/{total_count}] 订单 {order_id} 成功：{old_waybill} -> {tracking_number}"
        )

    def _on_worker_step_failed(self, index, total_count, order_id, tracking_number, error_message):
        """记录单条失败。"""
        self.append_result_log(
            f"[{index}/{total_count}] 订单 {order_id} -> {tracking_number} 失败：{error_message}"
        )

    def _on_worker_fatal_error(self, error_message):
        """记录批量中断。"""
        self.append_result_log(f"批量执行中断：{error_message}")

    def _on_worker_finished(self, success_count, failure_count, total_count, aborted):
        """恢复界面并汇总结果。"""
        self.set_submit_running(False)

        if aborted:
            return

        summary = (
            f"批量执行完成：共 {total_count} 条，成功 {success_count} 条，失败 {failure_count} 条。"
        )
        self.append_result_log(summary)

        if failure_count > 0:
            self.show_message(QMessageBox.Warning, "批量执行完成", summary)
        else:
            self.show_message(QMessageBox.Information, "批量执行完成", summary)

    # -----------------------------------------------------------------------
    # 中差评查找
    # -----------------------------------------------------------------------

    def _set_review_task_buttons(self, *, running, active_task=None):
        """同步中差评 / 品退按钮状态。"""
        self.review_find_button.setDisabled(running)
        self.review_full_scan_button.setDisabled(running)
        self.quality_refund_button.setDisabled(running)
        self.order_cache_button.setDisabled(running)
        self.review_find_button.setText(
            "正在获取..." if running and active_task == TASK_REVIEW_MATCH else "获取差评订单"
        )
        self.review_full_scan_button.setText(
            "正在完整补查..." if running and active_task == TASK_REVIEW_FULL_SCAN else "完整补查订单"
        )
        self.quality_refund_button.setText(
            "正在获取..." if running and active_task == TASK_QUALITY_REFUND else "获取品退订单"
        )
        self.order_cache_button.setText(
            "正在刷新缓存..."
            if running and active_task == TASK_CACHE_REFRESH
            else (
                "正在重建缓存..."
                if running and active_task == TASK_CACHE_REBUILD
                else "订单缓存管理"
            )
        )

    def _ensure_review_task_license(self, task_label):
        """校验查找类任务的激活状态。"""
        _, reason = self._refresh_license_state()
        if reason == "ok" or self._prompt_license_activation(reason):
            return True

        self._show_activation_required(task_label)
        return False

    def _show_order_cache_manage_dialog(self):
        """弹出订单缓存管理对话框，返回选中的任务类型。"""
        dialog, actions = self._create_message_dialog_base(
            QMessageBox.Question,
            "订单缓存管理",
            "请选择要执行的订单缓存任务。",
            (
                f"增量刷新会同步最近 {ORDER_CACHE_INCREMENTAL_DAYS} 天订单；"
                f"重建缓存会清空本地数据并重新抓取最近 {ORDER_CACHE_COVERAGE_DAYS} 天订单。"
            ),
            min_width=640,
        )
        selected_task = {"task_type": None}

        def _accept(task_type):
            selected_task["task_type"] = task_type
            dialog.accept()

        self._add_message_action(actions, "关闭", "MessageSecondary", dialog.reject)
        self._add_message_action(
            actions,
            f"重建最近 {ORDER_CACHE_COVERAGE_DAYS} 天",
            "MessageSecondary",
            lambda: _accept(TASK_CACHE_REBUILD),
        )
        self._add_message_action(
            actions,
            f"增量刷新最近 {ORDER_CACHE_INCREMENTAL_DAYS} 天",
            "MessagePrimary",
            lambda: _accept(TASK_CACHE_REFRESH),
        )
        if dialog.exec() != QDialog.Accepted:
            return None
        return selected_task["task_type"]

    def _start_review_worker(self, *, task_type, days, start_message, clear_order_input=True):
        """启动中差评 / 品退后台任务。"""
        self.review_task_type = task_type
        if clear_order_input:
            self._set_order_input_values([])
        self.clear_result_log()
        self.append_result_log(start_message)
        self._set_review_task_buttons(running=True, active_task=task_type)

        self.review_worker_thread = QThread(self)
        self.review_worker = ReviewMatcherWorker(days=days, task_type=task_type)
        self.review_worker.moveToThread(self.review_worker_thread)

        self.review_worker.progress.connect(self._on_review_progress)
        self.review_worker.order_ids_ready.connect(self._on_review_order_ids)
        self.review_worker.missing_config.connect(self.show_missing_config_error)
        self.review_worker.finished.connect(self._on_review_finished)
        self.review_worker.finished.connect(lambda *_: self.review_worker_thread.quit())
        self._bind_thread_lifecycle(
            self.review_worker_thread,
            self.review_worker,
            self._clear_review_worker_refs,
        )
        self.review_worker_thread.start()

    def on_review_find_clicked(self):
        """开始查找中差评订单。"""
        if self.review_worker is not None:
            return

        if not self._ensure_review_task_license("中差评查找"):
            return

        days = self.review_days_spin.value()
        self._start_review_worker(
            task_type=TASK_REVIEW_MATCH,
            days=days,
            start_message=f"开始查找最近 {days} 天的中差评订单...",
        )

    def on_quality_refund_clicked(self):
        """开始获取品质退款订单。"""
        if self.review_worker is not None:
            return

        if not self._ensure_review_task_license("品质退款订单获取"):
            return

        days = self.review_days_spin.value()
        self._start_review_worker(
            task_type=TASK_QUALITY_REFUND,
            days=days,
            start_message=f"开始获取最近 {days} 天的品质退款订单...",
        )

    def on_review_full_scan_clicked(self):
        """开始完整补查订单（最近 30 天用缓存，更早范围临时抓取）。"""
        if self.review_worker is not None:
            return

        if not self._ensure_review_task_license("完整补查订单"):
            return

        days = self.review_days_spin.value()
        self._start_review_worker(
            task_type=TASK_REVIEW_FULL_SCAN,
            days=days,
            start_message=f"开始完整补查最近 {days} 天差评订单（最近 30 天命中缓存，超出范围临时抓取）...",
        )

    def on_order_cache_manage_clicked(self):
        """打开订单缓存管理入口。"""
        if self.review_worker is not None:
            return

        if not self._ensure_review_task_license("订单缓存同步"):
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
        self.append_result_log(message)

    def _on_review_order_ids(self, order_ids):
        """将匹配到的订单号回填到订单输入框。"""
        self._set_order_input_values(order_ids)

    def _on_review_finished(self, status, message, matched_count, total_count):
        """中差评 / 品退查找完成。"""
        task_type = self.review_task_type
        self._set_review_task_buttons(running=False)
        warning_text = (message or "").strip()

        if status == TERMINAL_STATUS_CANCELLED:
            return

        if status == TERMINAL_STATUS_ERROR:
            self.append_result_log(f"❌ 错误: {message}")
            title = (
                "缓存任务失败"
                if task_type in (TASK_CACHE_REFRESH, TASK_CACHE_REBUILD)
                else "查找失败"
            )
            self.show_message(QMessageBox.Critical, title, message)
            return

        if task_type in (TASK_CACHE_REFRESH, TASK_CACHE_REBUILD):
            action_label = "订单缓存重建" if task_type == TASK_CACHE_REBUILD else "订单缓存刷新"
            summary = f"{action_label}完成：写入/更新 {matched_count} 个订单。"
            self.append_result_log(summary)
            if status == TERMINAL_STATUS_WARNING and warning_text:
                self.append_result_log(f"⚠️ 提醒: {warning_text}")
                self.show_message(
                    QMessageBox.Warning,
                    "缓存任务提醒",
                    f"{summary}\n\n{warning_text}",
                )
            else:
                self.show_message(QMessageBox.Information, "缓存任务完成", summary)
            return

        if task_type == TASK_QUALITY_REFUND:
            summary = (
                f"品退订单获取完成：共 {total_count} 个订单，"
                f"回填 {matched_count} 个订单号。"
            )
            if total_count > 0:
                self.append_result_log(summary)
                if status == TERMINAL_STATUS_WARNING and warning_text:
                    self.append_result_log(f"⚠️ 提醒: {warning_text}")
                    self.show_message(
                        QMessageBox.Warning,
                        "查找提醒",
                        f"{summary}\n\n{warning_text}",
                    )
                else:
                    self.show_message(QMessageBox.Information, "查找完成", summary)
            else:
                self.show_message(QMessageBox.Warning, "查找完成", "未找到品质退款订单。")
            return

        task_label = "完整补查" if task_type == TASK_REVIEW_FULL_SCAN else "中差评查找"
        if total_count > 0:
            summary = (
                f"{task_label}完成：共 {total_count} 条差评，"
                f"匹配到 {matched_count} 个订单。"
            )
            self.append_result_log(summary)
            if status == TERMINAL_STATUS_WARNING and warning_text:
                self.append_result_log(f"⚠️ 提醒: {warning_text}")
                self.show_message(
                    QMessageBox.Warning,
                    "查找提醒",
                    f"{summary}\n\n{warning_text}",
                )
            elif matched_count > 0:
                self.show_message(QMessageBox.Information, "查找完成", summary)
            else:
                self.show_message(QMessageBox.Warning, "查找完成", summary)

    def _clear_review_worker_refs(self):
        """清理中差评查找线程引用。"""
        self.review_worker = None
        self.review_worker_thread = None
        self.review_task_type = None
