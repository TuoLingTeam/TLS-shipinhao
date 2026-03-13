# -*- coding: utf-8 -*-
"""TLS-shipinhao 主窗口。"""

import os

from PySide6.QtCore import Qt, QThread
from PySide6.QtWidgets import (
    QApplication,
    QBoxLayout,
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

from .config import (
    ConfigNotFoundError,
    extract_biz_magic_from_cookie,
    get_config_dir_cache,
    get_saved_user_config_dir,
    parse_batch_input,
    read_cookie_data,
    resolve_config_dir,
    resolve_config_files_in_dir,
    save_user_config_dir,
)
from .constants import (
    APP_COLORS,
    AUTHOR_WECHAT,
    DEFAULT_REVIEW_DAYS,
    DESIGN_HEIGHT,
    DESIGN_WIDTH,
    MAX_BATCH_SIZE,
    TUTORIAL_URL,
    WINDOW_TITLE,
    get_platform_default_window_size,
)
from .widgets import (
    BatchInputEdit,
    LicenseDialog,
    build_fixed_font,
    build_font,
    get_license_reason_text,
)
from .worker import BatchWorker
from .review_worker import (
    ReviewMatcherWorker,
    TASK_QUALITY_REFUND,
    TASK_REVIEW_MATCH,
)

from .license import check_stored_license


SIDEBAR_BUTTON_HEIGHT = {
    "design": 48,
    "minimum": 44,
}

INPUT_PANE_VISIBLE_LINES = {
    "design": 11,
    "minimum": 9,
}

TOP_PANEL_STRETCH = {
    "config": 42,
    "inputs": 58,
}

TOP_INPUT_PAIR_STRETCH = {
    "order": 3,
    "tracking": 3,
}

BOTTOM_PANEL_STRETCH = {
    "left_column": 42,
    "right_column": 58,
}

BOTTOM_LEFT_PANEL_HEIGHT = {
    "review": 6,
    "action": 4,
}

WORKSPACE_PANEL_MIN_HEIGHT = {
    "top": {
        "design": 300,
        "minimum": 270,
    },
    "bottom": {
        "design": 340,
        "minimum": 280,
    },
}

WORKSPACE_PANEL_STRETCH = {
    "top": 5,
    "bottom": 6,
}

LOG_PANEL_MIN_HEIGHT = {
    "design": 170,
    "minimum": 140,
}


class MainWindow(QWidget):
    """主窗口。"""

    def __init__(self, license_reason="ok"):
        super().__init__()
        self.worker_thread = None
        self.worker = None
        self.is_paused = False
        self.review_worker_thread = None
        self.review_worker = None
        self.review_task_type = None
        self._batch_rows = []
        self._license_reason = license_reason

        self._sync_window_title_with_license(self._license_reason)
        self.setObjectName("AppRoot")
        fixed_width, fixed_height = self._resolve_fixed_window_size()
        self.setFixedSize(fixed_width, fixed_height)
        self.setStyleSheet(self._build_stylesheet())

        self._build_ui()
        self.refresh_config_path_label()
        self._fit_window_to_screen()
        self.refresh_input_metrics()
        self._sync_responsive_metrics()
        self.refresh_action_buttons()

    @staticmethod
    def _build_stylesheet():
        """构建全局 QSS 样式表。"""
        c = APP_COLORS
        return f"""
            QWidget#AppRoot {{
                background: {c["bg"]};
            }}
            QWidget {{
                color: {c["text"]};
            }}
            QLabel {{
                background: transparent;
            }}
            QWidget#PageWidget,
            QWidget#HeaderBody,
            QWidget#TitleWrap,
            QWidget#InputContainer,
            QWidget#InputCardBody,
            QWidget#CardHeader,
            QWidget#LogBody,
            QWidget#LogHeader,
            QWidget#WorkspacePanel,
            QWidget#PrimaryColumn,
            QWidget#SidebarColumn,
            QWidget#ActionButtons,
            QWidget#ReviewToolbar {{
                background: transparent;
            }}
            QFrame#Card {{
                background: #F7FAFC;
                border: 1px solid #CCD8E5;
                border-radius: 22px;
            }}
            QFrame#HeroCard {{
                background: #F2F7FB;
                border: 1px solid #C9D7E4;
                border-radius: 26px;
            }}
            QFrame#InputCardBlue,
            QFrame#InputCardBlue2,
            QFrame#ConfigCard {{
                background: #F8FBFF;
                border: 1px solid #C8D6E5;
                border-radius: 22px;
            }}
            QFrame#ReviewCard {{
                background: #F8FBFF;
                border: 1px solid #C8D6E5;
                border-radius: 22px;
            }}
            QFrame#ControlCard {{
                background: #F8FBFF;
                border: 1px solid #C8D6E5;
                border-radius: 22px;
            }}
            QFrame#LogCard {{
                background: #F8FBFF;
                border: 1px solid #C8D6E5;
                border-radius: 20px;
            }}
            QFrame#InputShell {{
                background: transparent;
                border: none;
            }}
            QPlainTextEdit#InputEdit {{
                background: #FFFFFF;
                color: #17314A;
                border: 1px solid #BDD0E2;
                border-radius: 18px;
                padding: 16px;
                selection-background-color: {c["blue"]};
            }}
            QPlainTextEdit#InputEdit:focus {{
                border: 2px solid #1D7AF2;
                background: #FFFFFF;
            }}
            QPlainTextEdit#LogEdit {{
                background: #FFFFFF;
                color: #17314A;
                border: 1px solid #BDD0E2;
                border-radius: 18px;
                padding: 14px;
                selection-background-color: {c["blue"]};
            }}
            QPushButton#PrimaryButton {{
                background: #D97706;
                color: white;
                border: 1px solid #B85F04;
                border-radius: 16px;
                padding: 12px 18px;
                font-weight: 700;
            }}
            QPushButton#PrimaryButton:hover {{
                background: #C56C05;
            }}
            QPushButton#PrimaryButton:pressed {{
                background: #B45E04;
            }}
            QPushButton#PrimaryButton:disabled {{
                background: #F1F5F9;
                color: #94A3B8;
                border: 1px solid #ADC0D8;
            }}
            QPushButton#PauseButton {{
                background: #EEF5FF;
                color: #0C3C8F;
                border: 1px solid #B6CAE2;
                border-radius: 16px;
                padding: 12px 18px;
                font-weight: 700;
            }}
            QPushButton#PauseButton:hover {{
                background: #E1ECFB;
            }}
            QPushButton#PauseButton:pressed {{
                background: #D6E5F8;
            }}
            QPushButton#PauseButton:disabled {{
                background: #F1F5F9;
                color: #94A3B8;
                border: 1px solid #ADC0D8;
            }}
            QLabel#HeroTitle {{
                color: #10243B;
            }}
            QLabel#HeroSubtitle {{
                color: #597089;
            }}
            QLabel#SectionTitle {{
                color: #10243B;
            }}
            QLabel#ControlTitle {{
                color: #10243B;
            }}
            QLabel#ControlDesc {{
                color: #597089;
            }}
            QLabel#MetricChip {{
                background: rgba(148, 163, 184, 0.12);
                color: #E2E8F0;
                border: 1px solid #334155;
                border-radius: 11px;
                padding: 6px 10px;
            }}
            QLabel#StatusBadge {{
                border-radius: 12px;
                padding: 7px 12px;
                font-weight: 700;
            }}
            QLabel#LogTitle {{
                color: #10243B;
            }}
            QLabel#LogHint {{
                color: #597089;
            }}
            QLabel#ConfigPath {{
                color: #4F6680;
                background: #FFFFFF;
                border: 1px solid #D3DFEB;
                border-radius: 14px;
                padding: 12px 14px;
            }}
            QPushButton#SecondaryButton {{
                background: #EEF5FF;
                color: #0C3C8F;
                border: 1px solid #B6CAE2;
                border-radius: 14px;
                padding: 10px 14px;
                font-weight: 700;
            }}
            QPushButton#SecondaryButton:hover {{
                background: #E1ECFB;
                border-color: #A5BDD8;
            }}
            QPushButton#SecondaryButton:pressed {{
                background: #D6E5F8;
            }}
            QScrollArea {{
                border: none;
                background: transparent;
            }}
            QScrollBar:vertical {{
                background: #D8E2EE;
                width: 12px;
                margin: 8px 4px 8px 0;
                border-radius: 6px;
            }}
            QScrollBar::handle:vertical {{
                background: #8EA5C1;
                min-height: 36px;
                border-radius: 6px;
            }}
            QScrollBar::handle:vertical:hover {{
                background: #738FB2;
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
        self._build_review_finder_section()
        self._build_input_section()
        self._build_action_section()
        self._build_log_section()
        self._build_workspace_section()

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
        root_layout.addWidget(self.scroll_area)

        self.page_widget = QWidget()
        self.page_widget.setObjectName("PageWidget")
        self.scroll_area.setWidget(self.page_widget)

        self.page_layout = QVBoxLayout(self.page_widget)
        self.page_layout.setContentsMargins(24, 22, 24, 22)
        self.page_layout.setSpacing(22)
        self.page_layout.setAlignment(Qt.AlignTop)

    def _build_header_card(self):
        """创建顶部标题卡片。"""
        self.header_card = self._create_card(self.page_layout, object_name="HeroCard")
        header_layout = QVBoxLayout(self.header_card)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(0)

        header_body = QWidget()
        header_body.setObjectName("HeaderBody")
        header_box = QHBoxLayout(header_body)
        header_box.setContentsMargins(22, 14, 22, 14)
        header_box.setSpacing(14)

        title_wrap = QWidget()
        title_wrap.setObjectName("TitleWrap")
        title_box = QVBoxLayout(title_wrap)
        title_box.setContentsMargins(0, 0, 0, 0)
        title_box.setSpacing(0)

        title_label = QLabel("驼铃视频小店中差评处理")
        title_label.setObjectName("HeroTitle")
        title_label.setFont(build_font(22, bold=True))
        self.hero_title_label = title_label
        title_box.addWidget(title_label)

        self.title_description_label = QLabel(
            "软件实现自动化批量处理中差评、品质退款订单的功能。"
        )
        self.title_description_label.setObjectName("HeroSubtitle")
        self.title_description_label.setWordWrap(True)
        self.title_description_label.setFont(build_font(12))
        title_box.addWidget(self.title_description_label)

        header_box.addWidget(title_wrap, 1)

        badge_wrap = QWidget()
        badge_layout = QHBoxLayout(badge_wrap)
        badge_layout.setContentsMargins(0, 0, 0, 0)
        badge_layout.setSpacing(10)

        self.license_status_badge = QLabel()
        self.license_status_badge.setObjectName("StatusBadge")
        self.license_status_badge.setAlignment(Qt.AlignCenter)
        self.license_status_badge.setFont(build_font(11, bold=True))
        self.license_status_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_layout.addWidget(self.license_status_badge, 0, Qt.AlignVCenter)

        self.author_badge = QLabel(f"微信：{AUTHOR_WECHAT}")
        self.author_badge.setAlignment(Qt.AlignCenter)
        self.author_badge.setFont(build_font(12, bold=True))
        self.author_badge.setStyleSheet(
            "background: #EAF3FF;"
            "color: #0C3C8F;"
            "border: 1px solid #BED0E6;"
            "border-radius: 14px;"
            "padding: 10px 14px;"
        )
        self.author_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_layout.addWidget(self.author_badge, 0, Qt.AlignVCenter)

        self.tutorial_badge = QLabel()
        self.tutorial_badge.setAlignment(Qt.AlignCenter)
        self.tutorial_badge.setTextFormat(Qt.RichText)
        self.tutorial_badge.setTextInteractionFlags(Qt.TextBrowserInteraction)
        self.tutorial_badge.setOpenExternalLinks(True)
        self.tutorial_badge.setCursor(Qt.PointingHandCursor)
        self.tutorial_badge.setFont(build_font(12, bold=True))
        self.tutorial_badge.setText(
            f'<a href="{TUTORIAL_URL}" style="color: {APP_COLORS["blue_deep"]}; text-decoration: none;">查看使用教程</a>'
        )
        self.tutorial_badge.setStyleSheet(
            "background: #F4F7FB;"
            "color: #0C3C8F;"
            "border: 1px solid #C8D4E2;"
            "border-radius: 14px;"
            "padding: 10px 14px;"
        )
        self.tutorial_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_layout.addWidget(self.tutorial_badge, 0, Qt.AlignVCenter)

        header_box.addWidget(badge_wrap, 0, Qt.AlignVCenter | Qt.AlignRight)
        header_layout.addWidget(header_body)
        self._sync_window_title_with_license(self._license_reason)

    def _build_review_finder_section(self):
        """创建中差评查找操作卡片。"""
        c = APP_COLORS

        self.review_card = QFrame()
        self.review_card.setObjectName("ReviewCard")
        self.review_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Expanding)

        card_layout = QVBoxLayout(self.review_card)
        card_layout.setContentsMargins(18, 16, 18, 16)
        card_layout.setSpacing(14)

        self.review_title_label = QLabel("一键获取中差评 / 品退订单")
        self.review_title_label.setObjectName("ReviewTitle")
        self.review_title_label.setFont(build_font(16, bold=True))
        self.review_title_label.setStyleSheet(
            f"color: {c['blue_deep']}; background: transparent;"
        )

        card_layout.addWidget(self.review_title_label)

        toolbar = QWidget()
        toolbar.setObjectName("ReviewToolbar")
        action_box = QVBoxLayout(toolbar)
        action_box.setContentsMargins(0, 0, 0, 0)
        action_box.setSpacing(10)

        days_row = QWidget()
        days_row_layout = QHBoxLayout(days_row)
        days_row_layout.setContentsMargins(0, 0, 0, 0)
        days_row_layout.setSpacing(10)

        days_label = QLabel("查询天数")
        days_label.setFont(build_font(12, bold=True))
        days_label.setStyleSheet(
            f"color: {c['blue_deep']}; background: transparent;"
        )
        self.review_days_label = days_label
        days_row_layout.addWidget(days_label, 0, Qt.AlignVCenter)
        days_row_layout.addStretch(1)

        self.review_days_spin = QSpinBox()
        self.review_days_spin.setRange(1, 90)
        self.review_days_spin.setValue(DEFAULT_REVIEW_DAYS)
        self.review_days_spin.setSuffix(" 天")
        self.review_days_spin.setFixedWidth(110)
        self.review_days_spin.setFixedHeight(38)
        self.review_days_spin.setStyleSheet(
            f"""QSpinBox {{
                background: #FFFFFF;
                color: {c['text']};
                border: 1px solid #B6CAE2;
                border-radius: 12px;
                padding: 4px 8px;
                font-size: 13px;
                font-weight: 600;
            }}
            QSpinBox::up-button, QSpinBox::down-button {{
                width: 18px;
            }}"""
        )
        days_row_layout.addWidget(self.review_days_spin, 0, Qt.AlignVCenter)
        action_box.addWidget(days_row)

        button_row = QWidget()
        button_row.setObjectName("ActionButtons")
        button_row_layout = QVBoxLayout(button_row)
        button_row_layout.setContentsMargins(0, 0, 0, 0)
        button_row_layout.setSpacing(10)

        self.review_find_button = QPushButton("获取差评订单")
        self.review_find_button.setObjectName("ReviewButton")
        self.review_find_button.setCursor(Qt.PointingHandCursor)
        self.review_find_button.setFont(build_font(14, bold=True))
        self.review_find_button.setFixedHeight(SIDEBAR_BUTTON_HEIGHT["design"])
        self.review_find_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.review_find_button.setStyleSheet(
            f"""QPushButton#ReviewButton {{
                background: #2563EB;
                color: white;
                border: 1px solid #1D4ED8;
                border-radius: 14px;
                padding: 10px 18px;
                font-weight: 700;
            }}
            QPushButton#ReviewButton:hover {{
                background: #1D4ED8;
            }}
            QPushButton#ReviewButton:pressed {{
                background: #1E40AF;
            }}
            QPushButton#ReviewButton:disabled {{
                background: #E2E8F0;
                color: #94A3B8;
                border: 1px solid #CBD5E1;
            }}"""
        )
        self.review_find_button.clicked.connect(self.on_review_find_clicked)
        button_row_layout.addWidget(self.review_find_button)

        self.quality_refund_button = QPushButton("获取品退订单")
        self.quality_refund_button.setObjectName("ReviewButton")
        self.quality_refund_button.setCursor(Qt.PointingHandCursor)
        self.quality_refund_button.setFont(build_font(14, bold=True))
        self.quality_refund_button.setFixedHeight(SIDEBAR_BUTTON_HEIGHT["design"])
        self.quality_refund_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.quality_refund_button.setStyleSheet(self.review_find_button.styleSheet())
        self.quality_refund_button.clicked.connect(self.on_quality_refund_clicked)
        button_row_layout.addWidget(self.quality_refund_button)

        action_box.addWidget(button_row)
        card_layout.addWidget(toolbar)

    def _build_input_section(self):
        """创建顶部三列输入容器。"""
        self.input_container = QWidget()
        self.input_container.setObjectName("InputContainer")
        self.input_container.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.input_layout = QBoxLayout(QBoxLayout.LeftToRight, self.input_container)
        self.input_layout.setContentsMargins(0, 0, 0, 0)
        self.input_layout.setSpacing(16)

        self.order_count_badge = self._create_count_badge(
            text_color=APP_COLORS["blue"],
            bg_color=APP_COLORS["blue_soft"],
            border_color="#9FC0F0",
        )
        self.tracking_count_badge = self._create_count_badge(
            text_color=APP_COLORS["blue"],
            bg_color=APP_COLORS["blue_soft"],
            border_color="#9FC0F0",
        )

        self.order_edit = BatchInputEdit("多个请用英文逗号、换行分隔，最多100 条")
        self.tracking_edit = BatchInputEdit("多个请用英文逗号、换行分隔，最多100 条")

        self.order_edit.textChanged.connect(self.refresh_input_metrics)
        self.tracking_edit.textChanged.connect(self.refresh_input_metrics)
        self.order_edit.normalized.connect(self.refresh_input_metrics)
        self.tracking_edit.normalized.connect(self.refresh_input_metrics)

        self.order_card = self._create_input_card(
            "第一步：填写订单号",
            self.order_count_badge,
            self.order_edit,
            "InputCardBlue",
        )
        self.tracking_card = self._create_input_card(
            "第二步：填写物流单号",
            self.tracking_count_badge,
            self.tracking_edit,
            "InputCardBlue2",
        )
        self.config_card = self._create_config_card()

        self.top_input_pair_panel = QWidget()
        self.top_input_pair_panel.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.top_input_pair_layout = QHBoxLayout(self.top_input_pair_panel)
        self.top_input_pair_layout.setContentsMargins(0, 0, 0, 0)
        self.top_input_pair_layout.setSpacing(16)
        self.top_input_pair_layout.addWidget(self.order_card, TOP_INPUT_PAIR_STRETCH["order"])
        self.top_input_pair_layout.addWidget(
            self.tracking_card,
            TOP_INPUT_PAIR_STRETCH["tracking"],
        )

        self.input_layout.addWidget(self.config_card, TOP_PANEL_STRETCH["config"])
        self.input_layout.addWidget(self.top_input_pair_panel, TOP_PANEL_STRETCH["inputs"])

    def _build_action_section(self):
        """创建执行控制卡片。"""
        self.action_card = QFrame()
        self.action_card.setObjectName("ControlCard")
        self.action_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Expanding)

        card_layout = QVBoxLayout(self.action_card)
        card_layout.setContentsMargins(18, 16, 18, 16)
        card_layout.setSpacing(14)

        self.action_title_label = QLabel("第四步：执行批量处理")
        self.action_title_label.setObjectName("ControlTitle")
        self.action_title_label.setFont(build_font(16, bold=True))
        card_layout.addWidget(self.action_title_label)

        self.action_row = QWidget()
        self.action_row.setObjectName("ActionButtons")
        self.action_row.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.action_layout = QVBoxLayout(self.action_row)
        self.action_layout.setContentsMargins(0, 0, 0, 0)
        self.action_layout.setSpacing(10)

        self.start_button = QPushButton("开始批量处理")
        self.start_button.setObjectName("PrimaryButton")
        self.start_button.setCursor(Qt.PointingHandCursor)
        self.start_button.setFont(build_font(16, bold=True))
        self.start_button.setFixedHeight(SIDEBAR_BUTTON_HEIGHT["design"])
        self.start_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.start_button.clicked.connect(self.on_start_clicked)
        self.action_layout.addWidget(self.start_button)

        self.pause_button = QPushButton("暂停批量处理")
        self.pause_button.setObjectName("PauseButton")
        self.pause_button.setCursor(Qt.PointingHandCursor)
        self.pause_button.setFont(build_font(15, bold=True))
        self.pause_button.setFixedHeight(SIDEBAR_BUTTON_HEIGHT["design"])
        self.pause_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        self.pause_button.clicked.connect(self.on_pause_clicked)
        self.action_layout.addWidget(self.pause_button)
        card_layout.addWidget(self.action_row)

    def _build_workspace_section(self):
        """创建标题下方的两层主容器。"""
        self.workspace_panel = QWidget()
        self.workspace_panel.setObjectName("WorkspacePanel")
        self.workspace_layout = QVBoxLayout(self.workspace_panel)
        self.workspace_panel.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.workspace_layout.setContentsMargins(0, 0, 0, 0)
        self.workspace_layout.setSpacing(16)

        self.bottom_panel = QWidget()
        self.bottom_panel.setObjectName("PrimaryColumn")
        self.bottom_panel.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.bottom_panel_layout = QHBoxLayout(self.bottom_panel)
        self.bottom_panel_layout.setContentsMargins(0, 0, 0, 0)
        self.bottom_panel_layout.setSpacing(18)

        self.sidebar_panel = QWidget()
        self.sidebar_panel.setObjectName("SidebarColumn")
        self.sidebar_panel.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        self.sidebar_layout = QVBoxLayout(self.sidebar_panel)
        self.sidebar_layout.setContentsMargins(0, 0, 0, 0)
        self.sidebar_layout.setSpacing(16)

        self.config_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.review_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.action_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.log_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Expanding)

        self.sidebar_layout.addWidget(self.review_card)
        self.sidebar_layout.addWidget(self.action_card)

        self.bottom_panel_layout.addWidget(
            self.sidebar_panel,
            BOTTOM_PANEL_STRETCH["left_column"],
        )
        self.bottom_panel_layout.addWidget(
            self.log_card,
            BOTTOM_PANEL_STRETCH["right_column"],
        )

        self.workspace_layout.setContentsMargins(0, 0, 0, 0)
        self.workspace_layout.addWidget(self.input_container)
        self.workspace_layout.addWidget(self.bottom_panel)
        self.page_layout.addWidget(self.workspace_panel)

    def _build_log_section(self):
        """创建日志展示区。"""
        self.log_card = QFrame()
        self.log_card.setObjectName("LogCard")
        log_layout = QVBoxLayout(self.log_card)
        log_layout.setContentsMargins(0, 0, 0, 0)
        log_layout.setSpacing(0)

        log_body = QWidget()
        log_body.setObjectName("LogBody")
        log_box = QVBoxLayout(log_body)
        log_box.setContentsMargins(16, 14, 16, 14)
        log_box.setSpacing(10)

        log_header = QWidget()
        log_header.setObjectName("LogHeader")
        log_header_box = QHBoxLayout(log_header)
        log_header_box.setContentsMargins(0, 0, 0, 0)
        log_header_box.setSpacing(10)

        log_title = QLabel("执行日志")
        log_title.setObjectName("LogTitle")
        log_title.setFont(build_font(15, bold=True))
        self.log_title_label = log_title
        log_header_box.addWidget(log_title)

        self.log_hint_label = QLabel("（最近执行记录会按时间顺序滚动显示）")
        self.log_hint_label.setObjectName("LogHint")
        self.log_hint_label.setWordWrap(False)
        self.log_hint_label.setFont(build_font(10))
        self.log_hint_label.setAlignment(Qt.AlignLeft | Qt.AlignVCenter)
        log_header_box.addWidget(self.log_hint_label, 0)
        log_header_box.addStretch(1)
        log_box.addWidget(log_header)

        self.log_view = QPlainTextEdit()
        self.log_view.setObjectName("LogEdit")
        self.log_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Expanding)
        self.log_view.setReadOnly(True)
        self.log_view.setFont(build_fixed_font(11))
        self.log_view.setMinimumHeight(LOG_PANEL_MIN_HEIGHT["design"])
        log_box.addWidget(self.log_view, 1)

        log_layout.addWidget(log_body)

    # -----------------------------------------------------------------------
    # 卡片 / 输入框工厂
    # -----------------------------------------------------------------------

    def _create_card(self, parent_layout, stretch=0, object_name="Card"):
        """创建卡片容器。"""
        card = QFrame()
        card.setObjectName(object_name)
        if stretch:
            parent_layout.addWidget(card, stretch)
        else:
            parent_layout.addWidget(card)
        return card

    def _create_count_badge(self, text_color, bg_color, border_color):
        """创建输入数量徽标。"""
        badge = QLabel()
        badge.setAlignment(Qt.AlignCenter)
        badge.setMinimumWidth(60)
        badge.setFont(build_font(10, bold=True))
        badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge.setStyleSheet(
            f"background: {bg_color};"
            f"color: {text_color};"
            f"border: 1px solid {border_color};"
            "border-radius: 8px;"
            "padding: 4px 8px;"
        )
        return badge

    def _create_input_card(self, title, badge, editor, object_name):
        """创建输入卡片。"""
        card = QFrame()
        card.setObjectName(object_name)
        card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
        card_layout = QVBoxLayout(card)
        card_layout.setContentsMargins(0, 0, 0, 0)
        card_layout.setSpacing(0)

        body = QWidget()
        body.setObjectName("InputCardBody")
        body_layout = QVBoxLayout(body)
        body_layout.setContentsMargins(16, 14, 16, 14)
        body_layout.setSpacing(10)

        header = QWidget()
        header.setObjectName("CardHeader")
        header.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(10)

        title_label = QLabel(title)
        title_label.setObjectName("SectionTitle")
        title_label.setFont(build_font(15, bold=True))
        title_label.setWordWrap(True)
        title_label.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        card.title_label = title_label
        header_layout.addWidget(title_label, 1, Qt.AlignVCenter)
        header_layout.addWidget(badge, 0, Qt.AlignRight | Qt.AlignVCenter)
        body_layout.addWidget(header)

        body_layout.addWidget(editor, 1)
        card_layout.addWidget(body)
        return card

    def _create_config_card(self):
        """创建配置目录卡片。"""
        badge_placeholder = QLabel("未配置")
        badge_placeholder.setAlignment(Qt.AlignCenter)
        badge_placeholder.setMinimumWidth(72)
        badge_placeholder.setObjectName("StatusBadge")
        badge_placeholder.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_placeholder.setStyleSheet(
            "background: #FEE2E2;"
            "color: #B91C1C;"
            "border: 1px solid #FCA5A5;"
        )
        self.config_badge = badge_placeholder

        shell = QWidget()
        shell.setObjectName("InputShell")
        shell_layout = QVBoxLayout(shell)
        shell_layout.setContentsMargins(0, 0, 0, 0)
        shell_layout.setSpacing(6)

        path_label = QLabel()
        path_label.setObjectName("ConfigPath")
        path_label.setWordWrap(True)
        path_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        path_label.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Preferred)
        shell_layout.addWidget(path_label, 1)
        self.config_path_label = path_label

        button = QPushButton("选择配置目录")
        button.setObjectName("SecondaryButton")
        button.setCursor(Qt.PointingHandCursor)
        button.setFixedHeight(SIDEBAR_BUTTON_HEIGHT["design"])
        button.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        button.clicked.connect(self.choose_config_dir)
        shell_layout.addWidget(button)
        self.config_button = button

        card = self._create_input_card(
            "系统配置目录",
            self.config_badge,
            shell,
            "ConfigCard",
        )
        self.config_title_label = card.title_label
        return card

    # -----------------------------------------------------------------------
    # 窗口尺寸 / 响应式
    # -----------------------------------------------------------------------

    def _calculate_editor_height(self, editor, visible_lines=10):
        """按指定可见行数计算输入框高度。"""
        line_height = editor.fontMetrics().lineSpacing()
        document_margin = int(editor.document().documentMargin() * 2)
        frame = editor.frameWidth() * 2
        padding = 18
        return line_height * visible_lines + document_margin + frame + padding

    def _fit_window_to_screen(self):
        """按屏幕缩放比例锁定固定窗口尺寸。"""
        fixed_width, fixed_height = self._resolve_fixed_window_size()
        self.setFixedSize(fixed_width, fixed_height)

    def _resolve_fixed_window_size(self):
        """结合平台默认值与屏幕信息，计算最终窗口固定尺寸。"""
        default_width, default_height = get_platform_default_window_size()
        screen = self.screen() or QApplication.primaryScreen()
        scale = max(1.0, screen.devicePixelRatio()) if screen is not None else 1.0
        available = screen.availableGeometry() if screen is not None else None
        fixed_width = int(round(default_width / scale))
        fixed_height = int(round(default_height / scale))
        if available is not None:
            # 限制不超过可用屏幕面积的 92%，避免小屏笔记本上窗口贴满边缘
            max_w = int(available.width() * 0.92)
            max_h = int(available.height() * 0.92)
            fixed_width = min(fixed_width, max_w)
            fixed_height = min(fixed_height, max_h)
        return fixed_width, fixed_height

    @staticmethod
    def _scaled(design, minimum, scale):
        """按缩放比计算值，不低于下限。"""
        return max(minimum, int(design * scale))

    @staticmethod
    def _allocate_proportional_heights(total_height, weights, minimums):
        """按权重分配高度，同时保证每项不低于最小内容高度。"""
        if total_height <= 0:
            return list(minimums)

        total_weight = max(1, sum(weights))
        heights = [int(round(total_height * weight / total_weight)) for weight in weights]
        heights[-1] += total_height - sum(heights)

        if sum(minimums) > total_height:
            return list(minimums)

        for index, minimum in enumerate(minimums):
            if heights[index] >= minimum:
                continue

            deficit = minimum - heights[index]
            heights[index] = minimum

            while deficit > 0:
                donor = max(
                    range(len(heights)),
                    key=lambda idx: heights[idx] - minimums[idx],
                )
                spare = heights[donor] - minimums[donor]
                if spare <= 0:
                    break
                taken = min(spare, deficit)
                heights[donor] -= taken
                deficit -= taken

        return heights

    def _sync_responsive_metrics(self):
        """窗口变化时同步紧凑布局尺寸。"""
        viewport = self.scroll_area.viewport().size()
        if not viewport.width() or not viewport.height():
            return

        width_scale = viewport.width() / DESIGN_WIDTH
        height_scale = viewport.height() / DESIGN_HEIGHT
        s = max(0.78, min(1.0, width_scale, height_scale))
        sc = self._scaled
        # -- 字体：(widget, design_size, min_size, bold, fixed) --
        _font_rules = [
            (self.hero_title_label,           22, 18, True,  False),
            (self.title_description_label,    12, 10, False, False),
            (self.license_status_badge,       11, 10, True,  False),
            (self.author_badge,               12, 11, True,  False),
            (self.tutorial_badge,             12, 11, True,  False),
            (self.review_title_label,         16, 14, True,  False),
            (self.review_days_label,          12, 10, True,  False),
            (self.review_find_button,         14, 12, True,  False),
            (self.quality_refund_button,      14, 12, True,  False),
            (self.order_count_badge,          10,  9, True,  False),
            (self.tracking_count_badge,       10,  9, True,  False),
            (self.order_card.title_label,     15, 13, True,  False),
            (self.tracking_card.title_label,  15, 13, True,  False),
            (self.config_title_label,         15, 13, True,  False),
            (self.config_badge,               10,  9, True,  False),
            (self.config_path_label,          13, 11, False, False),
            (self.config_button,              11, 10, True,  False),
            (self.action_title_label,         16, 14, True,  False),
            (self.log_title_label,            15, 13, True,  False),
            (self.log_hint_label,             10,  9, False, False),
            (self.start_button,               16, 13, True,  False),
            (self.pause_button,               15, 12, True,  False),
            (self.order_edit,                 13, 11, False, True),
            (self.tracking_edit,              13, 11, False, True),
            (self.log_view,                   11,  9, False, True),
        ]
        for widget, design, minimum, bold, fixed in _font_rules:
            size = sc(design, minimum, s)
            widget.setFont(build_fixed_font(size) if fixed else build_font(size, bold=bold))

        # -- 尺寸 --
        badge_h = sc(30, 26, s)
        author_h = sc(50, 44, s)
        author_w = sc(170, 150, s)
        sidebar_button_h = sc(
            SIDEBAR_BUTTON_HEIGHT["design"],
            SIDEBAR_BUTTON_HEIGHT["minimum"],
            s,
        )
        header_h = sc(124, 104, s)
        log_h = sc(
            LOG_PANEL_MIN_HEIGHT["design"],
            LOG_PANEL_MIN_HEIGHT["minimum"],
            s,
        )
        input_lines = (
            INPUT_PANE_VISIBLE_LINES["design"]
            if s >= 0.9
            else INPUT_PANE_VISIBLE_LINES["minimum"]
        )
        input_h = self._calculate_editor_height(self.order_edit, input_lines)

        self.author_badge.setFixedSize(author_w, author_h)
        self.tutorial_badge.setFixedSize(author_w, author_h)
        for badge in (self.order_count_badge, self.tracking_count_badge, self.config_badge):
            badge.setFixedHeight(badge_h)
        self.license_status_badge.setFixedHeight(sc(34, 30, s))
        self.review_find_button.setFixedHeight(sidebar_button_h)
        self.quality_refund_button.setFixedHeight(sidebar_button_h)
        self.review_days_spin.setFixedHeight(sc(40, 36, s))
        self.start_button.setFixedHeight(sidebar_button_h)
        self.pause_button.setFixedHeight(sidebar_button_h)
        header_card_height = max(header_h, self.header_card.sizeHint().height())
        self.header_card.setFixedHeight(header_card_height)
        self.order_edit.setFixedHeight(input_h)
        self.tracking_edit.setFixedHeight(input_h)
        self.log_view.setMinimumHeight(log_h)
        self.config_button.setFixedHeight(sidebar_button_h)
        self.config_path_label.setMinimumHeight(sc(136, 110, s))

        page_margin_x = sc(24, 14, s)
        page_margin_y = sc(22, 12, s)
        page_spacing = sc(22, 12, s)
        workspace_gap = sc(16, 12, s)
        shared_column_gap = sc(18, 12, s)
        input_pair_gap = sc(16, 10, s)
        available_content_width = max(0, viewport.width() - page_margin_x * 2)
        sidebar_ratio = (
            BOTTOM_PANEL_STRETCH["left_column"]
            / (BOTTOM_PANEL_STRETCH["left_column"] + BOTTOM_PANEL_STRETCH["right_column"])
        )
        sidebar_target_width = max(
            sc(320, 280, s),
            int(round(max(0, available_content_width - shared_column_gap) * sidebar_ratio)),
        )

        self.page_layout.setContentsMargins(page_margin_x, page_margin_y, page_margin_x, page_margin_y)
        self.page_layout.setSpacing(page_spacing)
        self.workspace_layout.setSpacing(workspace_gap)
        self.input_layout.setDirection(QBoxLayout.LeftToRight)
        self.input_layout.setSpacing(shared_column_gap)
        self.top_input_pair_layout.setSpacing(input_pair_gap)
        self.bottom_panel_layout.setSpacing(shared_column_gap)
        self.sidebar_layout.setSpacing(workspace_gap)
        self.action_layout.setSpacing(sc(10, 8, s))

        minimum_top_height = max(
            sc(
                WORKSPACE_PANEL_MIN_HEIGHT["top"]["design"],
                WORKSPACE_PANEL_MIN_HEIGHT["top"]["minimum"],
                s,
            ),
            self.config_card.sizeHint().height(),
            self.order_card.sizeHint().height(),
            self.tracking_card.sizeHint().height(),
        )
        minimum_bottom_height = sc(
            WORKSPACE_PANEL_MIN_HEIGHT["bottom"]["design"],
            WORKSPACE_PANEL_MIN_HEIGHT["bottom"]["minimum"],
            s,
        )
        minimum_workspace_height = minimum_top_height + minimum_bottom_height + workspace_gap
        available_workspace_height = max(
            minimum_workspace_height,
            viewport.height()
            - page_margin_y * 2
            - header_card_height
            - page_spacing,
        )
        top_panel_height, bottom_panel_height = self._allocate_proportional_heights(
            max(available_workspace_height - workspace_gap, 0),
            [
                WORKSPACE_PANEL_STRETCH["top"],
                WORKSPACE_PANEL_STRETCH["bottom"],
            ],
            [
                minimum_top_height,
                minimum_bottom_height,
            ],
        )

        self.config_card.setFixedWidth(sidebar_target_width)
        self.sidebar_panel.setFixedWidth(sidebar_target_width)
        for card in (self.config_card, self.order_card, self.tracking_card):
            card.setFixedHeight(top_panel_height)
        self.top_input_pair_panel.setFixedHeight(top_panel_height)
        self.input_container.setFixedHeight(top_panel_height)

        sidebar_gap = self.sidebar_layout.spacing()
        available_sidebar_height = max(bottom_panel_height - sidebar_gap, 0)
        review_height, action_height = self._allocate_proportional_heights(
            available_sidebar_height,
            [
                BOTTOM_LEFT_PANEL_HEIGHT["review"],
                BOTTOM_LEFT_PANEL_HEIGHT["action"],
            ],
            [
                self.review_card.sizeHint().height(),
                self.action_card.sizeHint().height(),
            ],
        )
        bottom_panel_height = review_height + action_height + sidebar_gap

        self.review_card.setFixedHeight(review_height)
        self.action_card.setFixedHeight(action_height)
        self.sidebar_panel.setFixedHeight(bottom_panel_height)
        self.log_card.setFixedHeight(bottom_panel_height)
        self.bottom_panel.setFixedHeight(bottom_panel_height)
        self.workspace_panel.setFixedHeight(
            top_panel_height + bottom_panel_height + workspace_gap
        )

    def resizeEvent(self, event):
        """窗口尺寸变化时同步内部尺寸。"""
        self._sync_responsive_metrics()
        super().resizeEvent(event)

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
                background: #0A1C36;
                border: 1px solid #1E3A8A;
                border-radius: 14px;
            }
            QLabel#MessageTitle {
                color: #F8FAFC;
                font-size: 18px;
                font-weight: 700;
            }
            QLabel#MessageText {
                color: #EAF2FC;
                font-size: 15px;
                line-height: 1.45;
            }
            QLabel#MessageInfo {
                color: #BFD0E5;
                font-size: 13px;
                line-height: 1.45;
            }
            QPushButton#MessagePrimary {
                background: #1D4ED8;
                color: #F8FAFC;
                border: 1px solid #3B82F6;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 700;
            }
            QPushButton#MessagePrimary:hover {
                background: #2563EB;
            }
            QPushButton#MessagePrimary:pressed {
                background: #1E40AF;
            }
            QPushButton#MessageSecondary {
                background: rgba(148, 163, 184, 0.18);
                color: #EAF2FC;
                border: 1px solid #64748B;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 600;
            }
            QPushButton#MessageSecondary:hover {
                background: rgba(148, 163, 184, 0.28);
            }
            QPushButton#MessageSecondary:pressed {
                background: rgba(100, 116, 139, 0.35);
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

        root = QVBoxLayout(dialog)
        root.setContentsMargins(22, 18, 22, 18)
        root.setSpacing(16)

        body = QHBoxLayout()
        body.setSpacing(14)

        icon_label = QLabel()
        icon_label.setPixmap(self._message_icon_pixmap(level))
        icon_label.setAlignment(Qt.AlignTop | Qt.AlignHCenter)
        icon_label.setFixedWidth(56)
        body.addWidget(icon_label, 0, Qt.AlignTop)

        text_wrap = QWidget()
        text_layout = QVBoxLayout(text_wrap)
        text_layout.setContentsMargins(0, 0, 0, 0)
        text_layout.setSpacing(8)

        title_label = QLabel(title)
        title_label.setObjectName("MessageTitle")
        title_label.setWordWrap(True)
        text_layout.addWidget(title_label)

        text_label = QLabel(text)
        text_label.setObjectName("MessageText")
        text_label.setWordWrap(True)
        text_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        text_layout.addWidget(text_label)

        if informative_text:
            info_label = QLabel(informative_text)
            info_label.setObjectName("MessageInfo")
            info_label.setWordWrap(True)
            info_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
            text_layout.addWidget(info_label)

        body.addWidget(text_wrap, 1)
        root.addLayout(body, 1)

        actions = QHBoxLayout()
        actions.setContentsMargins(0, 0, 0, 0)
        actions.setSpacing(10)
        actions.addStretch(1)
        root.addLayout(actions)

        self._style_message_box(dialog)
        return dialog, actions

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

    # -----------------------------------------------------------------------
    # 按钮状态
    # -----------------------------------------------------------------------

    def refresh_action_buttons(self):
        """同步开始/暂停按钮状态。"""
        running = self.worker is not None
        self.order_edit.setReadOnly(running)
        self.tracking_edit.setReadOnly(running)
        self.config_button.setDisabled(running)
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

        if resolved_dir:
            text = (
                "当前已生效目录：\n"
                f"{resolved_dir}\n\n"
                "程序会使用这里的 cookie 文件。"
            )
            badge_text = "已连接"
            badge_style = (
                "background: #DCFCE7;"
                "color: #166534;"
                "border: 1px solid #86EFAC;"
            )
        elif saved_dir:
            text = (
                "已记录目录：\n"
                f"{saved_dir}\n\n"
                "但是未找到可用的 cookie.txt 文件。"
            )
            badge_text = "待修复"
            badge_style = (
                "background: #FEF3C7;"
                "color: #B45309;"
                "border: 1px solid #FCD34D;"
            )
        else:
            text = (
                "当前未指定目录。\n\n"
                "请点击下方按钮手动选择配置目录。"
            )
            badge_text = "未配置"
            badge_style = (
                "background: #FEE2E2;"
                "color: #B91C1C;"
                "border: 1px solid #FCA5A5;"
            )
        self.config_path_label.setText(text)
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
        self._sync_window_title_with_license(reason)
        return info, reason

    def _sync_window_title_with_license(self, license_reason=None):
        """同步窗口标题中的授权状态。"""
        reason = self._license_reason if license_reason is None else license_reason
        status = "已激活" if reason == "ok" else "未激活"
        self.setWindowTitle(f"{WINDOW_TITLE}（{status}）")
        if hasattr(self, "license_status_badge"):
            self.license_status_badge.setText(status)
            if reason == "ok":
                self.license_status_badge.setStyleSheet(
                    "background: #DCFCE7;"
                    "color: #166534;"
                    "border: 1px solid #86EFAC;"
                )
            else:
                self.license_status_badge.setStyleSheet(
                    "background: #FEE2E2;"
                    "color: #B91C1C;"
                    "border: 1px solid #FCA5A5;"
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
            self.show_message(
                QMessageBox.Warning,
                "未激活",
                "软件尚未激活，无法执行批量处理。\n请先输入有效卡密完成激活。",
            )
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

        self.worker_thread.started.connect(self.worker.run)
        self.worker.started.connect(self._on_worker_started)
        self.worker.step_started.connect(self._on_worker_step_started)
        self.worker.step_succeeded.connect(self._on_worker_step_succeeded)
        self.worker.step_failed.connect(self._on_worker_step_failed)
        self.worker.fatal_error.connect(self._on_worker_fatal_error)
        self.worker.missing_config.connect(self.show_missing_config_error)
        self.worker.finished.connect(self._on_worker_finished)
        self.worker.finished.connect(self.worker_thread.quit)
        self.worker_thread.finished.connect(self.worker.deleteLater)
        self.worker_thread.finished.connect(self.worker_thread.deleteLater)
        self.worker_thread.finished.connect(self._clear_worker_refs)
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
        if self.worker_thread is not None and self.worker_thread.isRunning():
            self.worker_thread.quit()
            if not self.worker_thread.wait(3000):
                self.worker_thread.terminate()
                self.worker_thread.wait(1000)
        if self.review_worker is not None:
            self.review_worker.stop()
        if self.review_worker_thread is not None and self.review_worker_thread.isRunning():
            self.review_worker_thread.quit()
            if not self.review_worker_thread.wait(3000):
                self.review_worker_thread.terminate()
                self.review_worker_thread.wait(1000)
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
        self.quality_refund_button.setDisabled(running)
        self.review_find_button.setText(
            "正在获取..." if running and active_task == TASK_REVIEW_MATCH else "获取差评订单"
        )
        self.quality_refund_button.setText(
            "正在获取..." if running and active_task == TASK_QUALITY_REFUND else "获取品退订单"
        )

    def _ensure_review_task_license(self, task_label):
        """校验查找类任务的激活状态。"""
        _, reason = self._refresh_license_state()
        if reason == "ok" or self._prompt_license_activation(reason):
            return True

        self.show_message(
            QMessageBox.Warning,
            "未激活",
            f"软件尚未激活，无法执行{task_label}。\n请先输入有效卡密完成激活。",
        )
        return False

    def _start_review_worker(self, *, task_type, days, start_message):
        """启动中差评 / 品退后台任务。"""
        self.review_task_type = task_type
        self._set_order_input_values([])
        self.clear_result_log()
        self.append_result_log(start_message)
        self._set_review_task_buttons(running=True, active_task=task_type)

        self.review_worker_thread = QThread(self)
        self.review_worker = ReviewMatcherWorker(days=days, task_type=task_type)
        self.review_worker.moveToThread(self.review_worker_thread)

        self.review_worker_thread.started.connect(self.review_worker.run)
        self.review_worker.progress.connect(self._on_review_progress)
        self.review_worker.order_ids_ready.connect(self._on_review_order_ids)
        self.review_worker.error.connect(self._on_review_error)
        self.review_worker.missing_config.connect(self.show_missing_config_error)
        self.review_worker.finished.connect(self._on_review_finished)
        self.review_worker.finished.connect(self.review_worker_thread.quit)
        self.review_worker_thread.finished.connect(self.review_worker.deleteLater)
        self.review_worker_thread.finished.connect(self.review_worker_thread.deleteLater)
        self.review_worker_thread.finished.connect(self._clear_review_worker_refs)
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

    def _on_review_progress(self, message):
        """追加中差评查找进度日志。"""
        self.append_result_log(message)

    def _on_review_order_ids(self, order_ids):
        """将匹配到的订单号回填到订单输入框。"""
        self._set_order_input_values(order_ids)

    def _on_review_error(self, message):
        """处理中差评查找错误。"""
        self.append_result_log(f"❌ 错误: {message}")
        self.show_message(QMessageBox.Critical, "查找失败", message)

    def _on_review_finished(self, matched_count, total_count):
        """中差评 / 品退查找完成。"""
        task_type = self.review_task_type
        self._set_review_task_buttons(running=False)

        if task_type == TASK_QUALITY_REFUND:
            summary = (
                f"品退订单获取完成：共 {total_count} 个订单，"
                f"回填 {matched_count} 个订单号。"
            )
            self.append_result_log(summary)
            if total_count > 0:
                self.show_message(QMessageBox.Information, "查找完成", summary)
            else:
                self.show_message(QMessageBox.Warning, "查找完成", "未找到品质退款订单。")
            return

        if total_count > 0:
            summary = (
                f"中差评查找完成：共 {total_count} 条差评，"
                f"匹配到 {matched_count} 个订单。"
            )
            self.append_result_log(summary)
            if matched_count > 0:
                self.show_message(QMessageBox.Information, "查找完成", summary)
            else:
                self.show_message(QMessageBox.Warning, "查找完成", summary)

    def _clear_review_worker_refs(self):
        """清理中差评查找线程引用。"""
        self.review_worker = None
        self.review_worker_thread = None
        self.review_task_type = None
