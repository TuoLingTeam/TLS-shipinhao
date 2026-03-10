# -*- coding: utf-8 -*-
"""TLS-shipinhao 主窗口。"""

import os

from PySide6.QtCore import Qt, QThread
from PySide6.QtWidgets import (
    QApplication,
    QDialog,
    QFileDialog,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPlainTextEdit,
    QPushButton,
    QScrollArea,
    QSizePolicy,
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

from .license import check_stored_license


class MainWindow(QWidget):
    """主窗口。"""

    def __init__(self, license_reason="ok"):
        super().__init__()
        self.worker_thread = None
        self.worker = None
        self.is_paused = False
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
            QWidget#LogHeader {{
                background: transparent;
            }}
            QFrame#Card {{
                background: {c["surface"]};
                border: 1px solid {c["border"]};
                border-radius: 20px;
            }}
            QFrame#HeroCard {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 1,
                    stop: 0 {c["hero_tint"]},
                    stop: 1 {c["hero_tint_deep"]}
                );
                border: 1px solid {c["hero_border"]};
                border-radius: 22px;
            }}
            QFrame#InputCardBlue,
            QFrame#InputCardBlue2,
            QFrame#ConfigCard {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 1,
                    stop: 0 {c["blue_tint"]},
                    stop: 1 #B9D2F0
                );
                border: 1px solid #9EB7D7;
                border-radius: 20px;
            }}
            QFrame#LogCard {{
                background: {c["log_bg"]};
                border: 1px solid #205187;
                border-radius: 20px;
            }}
            QFrame#InputShell {{
                background: transparent;
                border: none;
            }}
            QPlainTextEdit#InputEdit {{
                background: {c["input_bg"]};
                color: {c["text"]};
                border: 1px solid {c["input_border"]};
                border-radius: 16px;
                padding: 14px;
                selection-background-color: {c["blue"]};
            }}
            QPlainTextEdit#InputEdit:focus {{
                border: 2px solid {c["input_border_focus"]};
                background: #FFFFFF;
            }}
            QPlainTextEdit#LogEdit {{
                background: {c["log_surface"]};
                color: {c["log_fg"]};
                border: 1px solid #2D5D94;
                border-radius: 16px;
                padding: 14px;
                selection-background-color: {c["blue"]};
            }}
            QPushButton#PrimaryButton {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 0,
                    stop: 0 {c["orange"]},
                    stop: 1 {c["orange_deep"]}
                );
                color: white;
                border: 1px solid #8A3D03;
                border-radius: 16px;
                padding: 12px 18px;
                font-weight: 700;
            }}
            QPushButton#PrimaryButton:hover {{
                background: #C86805;
            }}
            QPushButton#PrimaryButton:pressed {{
                padding-top: 13px;
                padding-bottom: 11px;
            }}
            QPushButton#PrimaryButton:disabled {{
                background: #F1F5F9;
                color: #94A3B8;
                border: 1px solid #ADC0D8;
            }}
            QPushButton#PauseButton {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 0,
                    stop: 0 #2E4662,
                    stop: 1 #1A2C44
                );
                color: #EAF2FC;
                border: 1px solid #5A7598;
                border-radius: 16px;
                padding: 12px 18px;
                font-weight: 700;
            }}
            QPushButton#PauseButton:hover {{
                background: #203651;
            }}
            QPushButton#PauseButton:pressed {{
                background: #182B42;
            }}
            QPushButton#PauseButton:disabled {{
                background: #D7E2F0;
                color: #94A3B8;
                border: 1px solid #ADC0D8;
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
            QLabel#LogTitle {{
                color: {c["log_fg"]};
            }}
            QLabel#LogHint {{
                color: {c["log_muted"]};
            }}
            QLabel#ConfigPath {{
                color: {c["muted"]};
                background: rgba(255, 255, 255, 0.82);
                border: 1px solid #AEC0D8;
                border-radius: 12px;
                padding: 10px 12px;
            }}
            QPushButton#SecondaryButton {{
                background: #FFFFFF;
                color: {c["blue_deep"]};
                border: 1px solid #96ACCA;
                border-radius: 12px;
                padding: 10px 14px;
                font-weight: 700;
            }}
            QPushButton#SecondaryButton:hover {{
                background: #EEF4FF;
                border-color: #9FB5D1;
            }}
            QPushButton#SecondaryButton:pressed {{
                background: #E4EDFA;
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
        self._build_input_section()
        self._build_action_section()
        self._build_log_section()

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
        self.title_description_label.setWordWrap(False)
        self.title_description_label.setFont(build_font(12))
        title_box.addWidget(self.title_description_label)

        header_box.addWidget(title_wrap, 1)

        badge_wrap = QWidget()
        badge_layout = QHBoxLayout(badge_wrap)
        badge_layout.setContentsMargins(0, 0, 0, 0)
        badge_layout.setSpacing(10)

        self.author_badge = QLabel(f"微信：{AUTHOR_WECHAT}")
        self.author_badge.setAlignment(Qt.AlignCenter)
        self.author_badge.setFont(build_font(12, bold=True))
        self.author_badge.setStyleSheet(
            f"background: {APP_COLORS['blue_soft']};"
            f"color: {APP_COLORS['blue_deep']};"
            "border: 1px solid #9FC0F0;"
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
            f"background: {APP_COLORS['blue_soft']};"
            f"color: {APP_COLORS['blue_deep']};"
            "border: 1px solid #9FC0F0;"
            "border-radius: 14px;"
            "padding: 10px 14px;"
        )
        self.tutorial_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_layout.addWidget(self.tutorial_badge, 0, Qt.AlignVCenter)

        header_box.addWidget(badge_wrap, 0, Qt.AlignVCenter | Qt.AlignRight)
        header_layout.addWidget(header_body)

    def _build_input_section(self):
        """创建三列输入区域（订单号、物流单号、配置目录）。"""
        self.input_container = QWidget()
        self.input_container.setObjectName("InputContainer")
        self.input_container.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.input_grid = QGridLayout(self.input_container)
        self.input_grid.setContentsMargins(0, 0, 0, 0)
        self.input_grid.setHorizontalSpacing(16)
        self.input_grid.setVerticalSpacing(12)

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

        self.page_layout.addWidget(self.input_container, 0, Qt.AlignTop)
        self.input_grid.addWidget(self.order_card, 0, 0, Qt.AlignTop)
        self.input_grid.addWidget(self.tracking_card, 0, 1, Qt.AlignTop)
        self.input_grid.addWidget(self.config_card, 0, 2, Qt.AlignTop)
        self.input_grid.setColumnStretch(0, 2)
        self.input_grid.setColumnStretch(1, 2)
        self.input_grid.setColumnStretch(2, 2)

    def _build_action_section(self):
        """创建开始/暂停操作区。"""
        self.action_row = QWidget()
        self.action_row.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.action_layout = QHBoxLayout(self.action_row)
        self.action_layout.setContentsMargins(0, 0, 0, 0)
        self.action_layout.setSpacing(12)

        self.pause_button = QPushButton("暂停批量处理")
        self.pause_button.setObjectName("PauseButton")
        self.pause_button.setCursor(Qt.PointingHandCursor)
        self.pause_button.setFont(build_font(15, bold=True))
        self.pause_button.setMinimumHeight(52)
        self.pause_button.clicked.connect(self.on_pause_clicked)
        self.action_layout.addWidget(self.pause_button, 1)

        self.start_button = QPushButton("开始批量处理")
        self.start_button.setObjectName("PrimaryButton")
        self.start_button.setCursor(Qt.PointingHandCursor)
        self.start_button.setFont(build_font(16, bold=True))
        self.start_button.setMinimumHeight(52)
        self.start_button.clicked.connect(self.on_start_clicked)
        self.action_layout.addWidget(self.start_button, 1)
        self.page_layout.addWidget(self.action_row)

    def _build_log_section(self):
        """创建日志展示区。"""
        self.log_card = self._create_card(self.page_layout, stretch=1, object_name="LogCard")
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
        self.log_view.setMinimumHeight(300)
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
        header_layout.setSpacing(0)

        # 标题+badge 绑定为一组，整体居中
        title_group = QWidget()
        title_group.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        title_group_layout = QHBoxLayout(title_group)
        title_group_layout.setContentsMargins(0, 0, 0, 0)
        title_group_layout.setSpacing(8)

        title_label = QLabel(title)
        title_label.setObjectName("SectionTitle")
        title_label.setFont(build_font(15, bold=True))
        title_label.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        card.title_label = title_label
        title_group_layout.addWidget(title_label)
        title_group_layout.addWidget(badge)

        header_layout.addStretch(1)
        header_layout.addWidget(title_group, 0, Qt.AlignCenter)
        header_layout.addStretch(1)
        body_layout.addWidget(header)

        body_layout.addWidget(editor)
        card_layout.addWidget(body)
        return card

    def _create_config_card(self):
        """创建配置目录卡片，复用输入卡片骨架以确保三列对齐。"""
        badge_placeholder = QLabel("目录")
        badge_placeholder.setAlignment(Qt.AlignCenter)
        badge_placeholder.setMinimumWidth(60)
        badge_placeholder.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_placeholder.setStyleSheet(
            f"background: {APP_COLORS['blue_soft']};"
            f"color: {APP_COLORS['blue']};"
            "border: 1px solid #9FC0F0;"
            "border-radius: 8px;"
            "padding: 4px 8px;"
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
        button.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        button.clicked.connect(self.choose_config_dir)
        shell_layout.addWidget(button)
        self.config_button = button

        card = self._create_input_card(
            "第三步：选择配置目录",
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
            (self.author_badge,               12, 11, True,  False),
            (self.tutorial_badge,             12, 11, True,  False),
            (self.order_count_badge,          10,  9, True,  False),
            (self.tracking_count_badge,       10,  9, True,  False),
            (self.order_card.title_label,     15, 13, True,  False),
            (self.tracking_card.title_label,  15, 13, True,  False),
            (self.config_title_label,         15, 13, True,  False),
            (self.config_badge,               10,  9, True,  False),
            (self.config_path_label,          13, 11, False, False),
            (self.config_button,              11, 10, True,  False),
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
        button_h = sc(52, 46, s)
        header_h = sc(120, 100, s)
        log_h = sc(300, 190, s)
        input_h = self._calculate_editor_height(self.order_edit, 10)

        self.author_badge.setFixedSize(author_w, author_h)
        self.tutorial_badge.setFixedSize(author_w, author_h)
        for badge in (self.order_count_badge, self.tracking_count_badge, self.config_badge):
            badge.setFixedHeight(badge_h)
        self.start_button.setFixedHeight(button_h)
        self.pause_button.setFixedHeight(button_h)
        self.header_card.setMinimumHeight(header_h)
        self.order_edit.setFixedHeight(input_h)
        self.tracking_edit.setFixedHeight(input_h)
        self.log_view.setMinimumHeight(log_h)
        self.config_button.setFixedHeight(sc(44, 40, s))

        # -- 三列卡片等高对齐 --
        card_target_height = max(
            self.order_card.sizeHint().height(),
            self.tracking_card.sizeHint().height(),
        )
        config_non_path_height = self.config_card.sizeHint().height() - self.config_path_label.sizeHint().height()
        self.config_path_label.setFixedHeight(max(56, card_target_height - config_non_path_height))
        for card in (self.order_card, self.tracking_card, self.config_card):
            card.setFixedHeight(card_target_height)

        # -- 间距 --
        self.page_layout.setContentsMargins(sc(24, 14, s), sc(22, 12, s), sc(24, 14, s), sc(22, 12, s))
        self.page_layout.setSpacing(sc(22, 12, s))
        self.input_grid.setHorizontalSpacing(sc(18, 11, s))
        self.input_grid.setVerticalSpacing(sc(14, 10, s))

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
        elif saved_dir:
            text = (
                "已记录目录：\n"
                f"{saved_dir}\n\n"
                "但是未找到可用的 cookie.txt 文件。"
            )
        else:
            text = (
                "当前未指定目录。\n\n"
                "请点击下方按钮手动选择配置目录。"
            )
        self.config_path_label.setText(text)

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
        super().closeEvent(event)

    def _on_worker_started(self, total_count):
        """记录任务开始。"""
        self.append_result_log(f"任务已创建：共 {total_count} 条，准备顺序执行。")

    def _on_worker_step_started(self, index, total_count, order_id):
        """记录单条开始。"""
        self.append_result_log(f"[{index}/{total_count}] 开始处理订单 {order_id}")

    def _on_worker_step_succeeded(self, index, total_count, order_id, tracking_number, old_waybill):
        """记录单条成功。"""
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
