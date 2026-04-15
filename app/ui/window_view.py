# -*- coding: utf-8 -*-
"""MainWindow 视图构建与布局辅助。"""

import sys
from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QApplication,
    QFrame,
    QHBoxLayout,
    QLabel,
    QPlainTextEdit,
    QPushButton,
    QSizePolicy,
    QSpinBox,
    QVBoxLayout,
    QWidget,
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
    COMPACT_LAYOUT_MIN_WIDTH,
    CONFIG_PATH_MIN_HEIGHT,
    DEFAULT_REVIEW_DAYS,
    FONT_SIZES,
    HERO_PADDING_X,
    HERO_PADDING_Y,
    HERO_RADIUS,
    HIGH_DPI_COMPACT_THRESHOLD,
    INPUT_BADGE_HEIGHT,
    INPUT_BADGE_MIN_WIDTH,
    INPUT_BADGE_RADIUS,
    INPUT_EDIT_PADDING,
    INPUT_EDIT_RADIUS,
    INPUT_VISIBLE_LINES,
    LOG_EDIT_PADDING,
    LOG_EDIT_RADIUS,
    LOG_PANEL_MIN_HEIGHT,
    MAX_UI_SCALE,
    MIN_UI_SCALE,
    ROW_GAP,
    SETUP_SECTION_PADDING,
    VERY_HIGH_DPI_COMPACT_THRESHOLD,
    WIDE_LAYOUT_MIN_HEIGHT,
    WIDE_LAYOUT_MIN_WIDTH,
    get_platform_default_window_size,
    scale_px,
)
from ui.widgets import BatchInputEdit, build_fixed_font, build_font


@dataclass
class HeaderSectionRefs:
    header_card: QFrame
    header_box: QHBoxLayout
    title_box: QVBoxLayout
    author_badge: QLabel
    tutorial_badge: QLabel
    update_button: QPushButton


@dataclass
class SetupSectionRefs:
    config_card: QFrame
    config_badge: QLabel
    config_path_panel: QFrame
    config_path_label: QLabel
    config_note_label: QLabel
    auto_cookie_button: QWidget
    review_days_spin: QSpinBox
    review_find_button: QWidget
    review_full_scan_button: QWidget
    quality_refund_button: QWidget
    order_cache_button: QWidget
    setup_content_layout: QVBoxLayout
    config_content_layout: QVBoxLayout
    review_content_layout: QVBoxLayout


@dataclass
class BatchSectionRefs:
    order_count_badge: QLabel
    tracking_count_badge: QLabel
    order_edit: BatchInputEdit
    tracking_edit: BatchInputEdit
    order_card: QWidget
    tracking_card: QWidget
    start_button: QPushButton
    pause_button: QPushButton
    action_card: QWidget
    action_content_layout: QVBoxLayout
    log_hint_label: QLabel
    log_view: QPlainTextEdit
    log_card: QWidget


@dataclass
class LicenseSectionRefs:
    license_card: QFrame
    license_summary_label: QLabel
    license_meta_label: QLabel
    license_content_layout: QVBoxLayout


def scaled_padding(vertical, horizontal):
    return f"{scale_px(vertical, min_value=1)}px {scale_px(horizontal, min_value=1)}px"


def build_badge_style(background, text_color, border_color, *, radius=None, padding=None):
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


def standard_layout_spacing():
    return scale_px(ROW_GAP, min_value=8)


def build_main_window_stylesheet():
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
            QPushButton#PrimaryButton:hover {{ background: {c["orange_deep"]}; }}
            QPushButton#PrimaryButton:pressed {{ background: {c["orange_deep"]}; }}
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
            QPushButton#PauseButton:hover {{ background: {c["blue_tint"]}; }}
            QPushButton#PauseButton:pressed {{ background: {c["blue_soft"]}; }}
            QPushButton#PauseButton:disabled {{
                background: {c["neutral_bg"]};
                color: {c["neutral_text"]};
                border: 1px solid {c["neutral_border"]};
            }}
            QLabel#HeroTitle {{ color: {c["heading"]}; }}
            QLabel#HeroSubtitle {{ color: {c["muted"]}; }}
            QLabel#SectionTitle {{ color: {c["heading"]}; }}
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
            QLabel#LogTitle {{ color: {c["heading"]}; }}
            QLabel#LogHint {{ color: {c["muted"]}; }}
            QLabel#ConfigPath {{ color: {c["heading"]}; }}
            QLabel#ConfigNote {{ color: {c["muted"]}; }}
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
            QLabel#LicenseSummary {{ color: {c["heading"]}; }}
            QLabel#LicenseMeta {{ color: {c["muted"]}; }}
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
                padding: {scale_px(7, min_value=5)}px {scale_px(12, min_value=8)}px;
                font-weight: 700;
            }}
            QPushButton#SecondaryButton:hover {{
                background: {c["blue_tint"]};
                border-color: {c["border_strong"]};
            }}
            QPushButton#SecondaryButton:pressed {{ background: {c["blue_soft"]}; }}
            QScrollArea {{ border: none; background: transparent; }}
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
            QScrollBar::handle:vertical:hover {{ background: {c["muted_soft"]}; }}
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{ height: 0; }}
            QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{ background: transparent; }}
            """


def calculate_editor_height(editor, visible_lines=10):
    line_height = editor.fontMetrics().lineSpacing()
    document_margin = int(editor.document().documentMargin() * 2)
    frame = editor.frameWidth() * 2
    padding = scale_px(INPUT_EDIT_PADDING, min_value=8) + 2
    return line_height * visible_lines + document_margin + frame + padding


def resolve_height_profile(viewport_height):
    if viewport_height <= 620:
        return 'dense'
    if viewport_height <= 720:
        return 'compact'
    return 'comfortable'


def resolve_layout_mode(width, height):
    if width >= WIDE_LAYOUT_MIN_WIDTH and height >= WIDE_LAYOUT_MIN_HEIGHT:
        return 'wide'
    if width >= COMPACT_LAYOUT_MIN_WIDTH:
        return 'compact'
    return 'dense'


def resolve_ui_scale_for_size(width, height):
    screen = QApplication.primaryScreen()
    if screen is None:
        return 1.0
    layout_mode = resolve_layout_mode(width, height)
    scale_map = {'wide': 1.0, 'compact': 0.92, 'dense': 0.86}
    scale = scale_map.get(layout_mode, 1.0)
    logical_dpi = screen.logicalDotsPerInch()
    if sys.platform.startswith('win'):
        if logical_dpi >= VERY_HIGH_DPI_COMPACT_THRESHOLD:
            scale *= 0.97 if layout_mode == 'wide' else 0.92
        elif logical_dpi >= HIGH_DPI_COMPACT_THRESHOLD:
            scale *= 0.985 if layout_mode == 'wide' else 0.96
        else:
            scale *= 0.93
    else:
        if logical_dpi >= VERY_HIGH_DPI_COMPACT_THRESHOLD and layout_mode != 'wide':
            scale *= 0.94
    return max(MIN_UI_SCALE, min(MAX_UI_SCALE, scale))


def resolve_initial_window_size(widget):
    default_width, default_height = get_platform_default_window_size()
    screen = widget.screen() or QApplication.primaryScreen()
    available = screen.availableGeometry() if screen is not None else None
    w, h = default_width, default_height
    if available is not None:
        if sys.platform.startswith('win'):
            max_w = int(available.width() * 0.92)
            max_h = int(available.height() * 0.95)
            w = min(default_width, max_w)
            h = min(default_height, max_h)
        else:
            max_w = int(available.width() * 0.92)
            max_h = int(available.height() * 0.92)
            w = min(w, max_w)
            h = min(h, max_h)
    return w, h


def create_card(window, title, title_right, content, object_name):
    card = QFrame()
    card.setObjectName(object_name)
    card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
    card_layout = QVBoxLayout(card)
    card_layout.setContentsMargins(*(scale_px(CARD_PADDING, min_value=6) for _ in range(4)))
    card_layout.setSpacing(scale_px(CARD_HEADER_GAP, min_value=3))
    card.title_label = None
    if title or title_right is not None:
        header = QFrame()
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(max(scale_px(8, min_value=4), scale_px(ROW_GAP, min_value=6) // 2))
        header_height = scale_px(CARD_HEADER_HEIGHT, min_value=22)
        if title_right is not None:
            header_height = max(header_height, title_right.sizeHint().height())
        header.setMinimumHeight(header_height)
        if title:
            title_label = QLabel(title)
            title_label.setObjectName('LogTitle' if object_name == 'LogCard' else 'SectionTitle')
            title_label.setFont(build_font(FONT_SIZES['section_log'] if object_name == 'LogCard' else FONT_SIZES['section'], bold=True))
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


def create_count_badge():
    badge = QLabel()
    badge.setObjectName('MetricChip')
    badge.setAlignment(Qt.AlignCenter)
    badge.setMinimumWidth(scale_px(INPUT_BADGE_MIN_WIDTH, min_value=52))
    badge.setFixedHeight(scale_px(INPUT_BADGE_HEIGHT, min_value=24))
    badge.setFont(build_fixed_font(11))
    badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    return badge


def create_config_badge(window):
    badge = QLabel('未配置')
    badge.setObjectName('StatusBadge')
    badge.setAlignment(Qt.AlignCenter)
    badge.setMinimumWidth(scale_px(INPUT_BADGE_MIN_WIDTH, min_value=52))
    badge.setFixedHeight(scale_px(INPUT_BADGE_HEIGHT, min_value=24))
    badge.setFont(build_font(FONT_SIZES['secondary'], bold=True))
    badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    badge.setStyleSheet(window._build_badge_style(APP_COLORS['red_soft'], APP_COLORS['red'], APP_COLORS['red']))
    return badge


def create_input_editor(window, placeholder):
    editor = BatchInputEdit(placeholder)
    editor.setMinimumHeight(window._calculate_editor_height(editor, max(4, scale_px(INPUT_VISIBLE_LINES - 4, min_value=4))))
    editor.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
    return editor


def create_review_button(window, text):
    button = QPushButton(text)
    button.setObjectName('ReviewButton')
    button.setCursor(Qt.PointingHandCursor)
    button.setFont(build_font(FONT_SIZES['button'], bold=True))
    button.setFixedHeight(scale_px(BADGE_HEIGHT, min_value=28))
    button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
    button.setStyleSheet(
        f"""QPushButton#ReviewButton {{
                background: {APP_COLORS['blue']};
                color: white;
                border: 1px solid {APP_COLORS['blue_deep']};
                border-radius: {scale_px(12, min_value=8)}px;
                padding: {window._scaled_padding(10, 18)};
                font-weight: 700;
            }}
            QPushButton#ReviewButton:hover {{ background: {APP_COLORS['blue_deep']}; }}
            QPushButton#ReviewButton:pressed {{ background: {APP_COLORS['blue_deep']}; }}
            QPushButton#ReviewButton:disabled {{
                background: {APP_COLORS['neutral_bg']};
                color: {APP_COLORS['neutral_text']};
                border: 1px solid {APP_COLORS['neutral_border']};
            }}"""
    )
    return button


def create_setup_section_label(text):
    label = QLabel(text)
    label.setObjectName('SetupSectionTitle')
    label.setFont(build_font(FONT_SIZES['secondary'], bold=True))
    label.setAlignment(Qt.AlignCenter)
    label.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    return label


def build_setup_section_card(title, content):
    card = QFrame()
    card.setObjectName('SetupSectionCard')
    card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
    layout = QVBoxLayout(card)
    layout.setContentsMargins(*(scale_px(SETUP_SECTION_PADDING, min_value=8) for _ in range(4)))
    layout.setSpacing(standard_layout_spacing())
    if title:
        layout.addWidget(create_setup_section_label(title), 0, Qt.AlignLeft)
    layout.addWidget(content)
    return card


def set_tutorial_badge_link(label, url):
    tutorial_url = (url or '').strip()
    if tutorial_url:
        label.setOpenExternalLinks(True)
        label.setText(f'<a href="{tutorial_url}" style="color: {APP_COLORS["blue_deep"]}; text-decoration: none;">查看使用教程</a>')
    else:
        label.setOpenExternalLinks(False)
        label.setText(f'<span style="color: {APP_COLORS["blue_deep"]};">查看使用教程</span>')


def build_header_section(window):
    header_card = QFrame()
    header_card.setObjectName('HeroCard')
    header_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
    header_box = QHBoxLayout(header_card)
    header_box.setContentsMargins(
        scale_px(HERO_PADDING_X, min_value=14), scale_px(HERO_PADDING_Y, min_value=6),
        scale_px(HERO_PADDING_X, min_value=14), scale_px(HERO_PADDING_Y, min_value=6),
    )
    header_box.setSpacing(scale_px(12, min_value=6))
    title_wrap = QWidget()
    title_box = QVBoxLayout(title_wrap)
    title_box.setContentsMargins(0, 0, 0, 0)
    title_box.setSpacing(scale_px(4, min_value=2))
    title_label = QLabel('驼铃·视频小店差评处理')
    title_label.setObjectName('HeroTitle')
    title_label.setFont(build_font(FONT_SIZES['title'], bold=True))
    title_box.addWidget(title_label)
    title_description_label = QLabel('软件实现自动化批量处理中差评、品质退款订单的功能。')
    title_description_label.setObjectName('HeroSubtitle')
    title_description_label.setWordWrap(True)
    title_description_label.setFont(build_font(FONT_SIZES['badge']))
    title_box.addWidget(title_description_label)
    header_box.addWidget(title_wrap, 1)
    badge_wrap = QWidget()
    badge_layout = QHBoxLayout(badge_wrap)
    badge_layout.setContentsMargins(0, 0, 0, 0)
    badge_layout.setSpacing(scale_px(12, min_value=6))
    author_badge = QLabel(f'微信：{AUTHOR_WECHAT}')
    author_badge.setAlignment(Qt.AlignCenter)
    author_badge.setFont(build_font(FONT_SIZES['badge'], bold=True))
    author_badge.setMinimumWidth(scale_px(BADGE_MIN_WIDTH, min_value=64))
    author_badge.setFixedHeight(scale_px(BADGE_HEIGHT, min_value=28))
    author_badge.setStyleSheet(window._build_badge_style(APP_COLORS['blue_soft'], APP_COLORS['blue_deep'], APP_COLORS['blue_tint'], radius=scale_px(BADGE_RADIUS, min_value=10), padding=window._scaled_padding(7, 12)))
    author_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    badge_layout.addWidget(author_badge, 0, Qt.AlignVCenter)
    tutorial_badge = QLabel()
    tutorial_badge.setAlignment(Qt.AlignCenter)
    tutorial_badge.setTextFormat(Qt.RichText)
    tutorial_badge.setTextInteractionFlags(Qt.TextBrowserInteraction)
    tutorial_badge.setOpenExternalLinks(True)
    tutorial_badge.setCursor(Qt.PointingHandCursor)
    tutorial_badge.setFont(build_font(FONT_SIZES['badge'], bold=True))
    tutorial_badge.setMinimumWidth(scale_px(BADGE_MIN_WIDTH, min_value=64))
    tutorial_badge.setFixedHeight(scale_px(BADGE_HEIGHT, min_value=28))
    set_tutorial_badge_link(tutorial_badge, '')
    tutorial_badge.setStyleSheet(window._build_badge_style(APP_COLORS['surface_soft'], APP_COLORS['blue_deep'], APP_COLORS['border'], radius=scale_px(BADGE_RADIUS, min_value=10), padding=window._scaled_padding(7, 12)))
    tutorial_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    badge_layout.addWidget(tutorial_badge, 0, Qt.AlignVCenter)
    update_button = QPushButton('检查更新')
    update_button.setObjectName('SecondaryButton')
    update_button.setCursor(Qt.PointingHandCursor)
    update_button.setFont(build_font(FONT_SIZES['badge'], bold=True))
    update_button.setMinimumWidth(scale_px(BADGE_MIN_WIDTH, min_value=64))
    update_button.setFixedHeight(scale_px(BADGE_HEIGHT, min_value=28))
    update_button.clicked.connect(lambda: window.trigger_background_update_check(manual=True))
    update_button.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    badge_layout.addWidget(update_button, 0, Qt.AlignVCenter)
    header_box.addWidget(badge_wrap, 0, Qt.AlignVCenter | Qt.AlignRight)
    return HeaderSectionRefs(header_card, header_box, title_box, author_badge, tutorial_badge, update_button)


def build_setup_section(window):
    content = QWidget()
    setup_content_layout = QVBoxLayout(content)
    setup_content_layout.setContentsMargins(0, 0, 0, 0)
    setup_content_layout.setSpacing(standard_layout_spacing())

    config_content = QWidget()
    config_content_layout = QVBoxLayout(config_content)
    config_content_layout.setContentsMargins(0, 0, 0, 0)
    config_content_layout.setSpacing(standard_layout_spacing())

    config_path_panel = QFrame()
    config_path_panel.setObjectName('ConfigPathPanel')
    config_path_panel.setMinimumHeight(scale_px(CONFIG_PATH_MIN_HEIGHT, min_value=48))
    config_path_panel.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
    path_layout = QVBoxLayout(config_path_panel)
    path_layout.setContentsMargins(scale_px(12, min_value=8), scale_px(12, min_value=8), scale_px(12, min_value=8), scale_px(12, min_value=8))
    path_layout.setSpacing(standard_layout_spacing())

    config_path_label = QLabel()
    config_path_label.setObjectName('ConfigPath')
    config_path_label.setWordWrap(True)
    config_path_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
    config_path_label.setFont(build_font(FONT_SIZES['body']))
    config_path_label.setTextInteractionFlags(Qt.TextSelectableByMouse)
    config_path_label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
    path_layout.addWidget(config_path_label)

    config_note_label = QLabel()
    config_note_label.setObjectName('ConfigNote')
    config_note_label.setWordWrap(True)
    config_note_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
    config_note_label.setFont(build_font(FONT_SIZES['secondary']))
    config_note_label.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
    path_layout.addWidget(config_note_label)

    config_content_layout.addWidget(config_path_panel, 1)
    actions = QWidget()
    actions_layout = QVBoxLayout(actions)
    actions_layout.setContentsMargins(0, 0, 0, 0)
    actions_layout.setSpacing(standard_layout_spacing())
    auto_cookie_button = create_review_button(window, '自动获取 cookie 并保存')
    auto_cookie_button.clicked.connect(window.open_cookie_capture_dialog)
    actions_layout.addWidget(auto_cookie_button, 1)
    config_content_layout.addWidget(actions)

    review_content = QWidget()
    review_content_layout = QVBoxLayout(review_content)
    review_content_layout.setContentsMargins(0, 0, 0, 0)
    review_content_layout.setSpacing(standard_layout_spacing())
    days_row = QWidget()
    days_row_layout = QHBoxLayout(days_row)
    days_row_layout.setContentsMargins(0, 0, 0, 0)
    days_row_layout.setSpacing(standard_layout_spacing())
    review_days_label = QLabel('1.2. 选择订单查询天数')
    review_days_label.setFont(build_font(FONT_SIZES['body'], bold=True))
    review_days_label.setStyleSheet(f"color: {APP_COLORS['blue_deep']};")
    days_row_layout.addWidget(review_days_label, 0, Qt.AlignVCenter)
    days_row_layout.addStretch(1)
    review_days_spin = QSpinBox()
    review_days_spin.setRange(1, 90)
    review_days_spin.setValue(DEFAULT_REVIEW_DAYS)
    review_days_spin.setSuffix(' 天')
    review_days_spin.setAlignment(Qt.AlignCenter)
    review_days_spin.setFixedWidth(scale_px(128, min_value=96))
    review_days_spin.setFixedHeight(scale_px(36, min_value=28))
    review_days_spin.setFont(build_font(FONT_SIZES['button'], bold=True))
    review_days_spin.setStyleSheet(
        f"""QSpinBox {{
                background: {APP_COLORS['surface']};
                color: {APP_COLORS['text']};
                border: 1px solid {APP_COLORS['border']};
                border-radius: {scale_px(12, min_value=8)}px;
                padding: {window._scaled_padding(6, 10)};
            }}
            QSpinBox::up-button, QSpinBox::down-button {{ width: {scale_px(22, min_value=16)}px; }}"""
    )
    days_row_layout.addWidget(review_days_spin, 0, Qt.AlignVCenter)
    review_content_layout.addWidget(days_row)
    review_find_button = create_review_button(window, '获取差评订单')
    review_find_button.clicked.connect(window.on_review_find_clicked)
    review_full_scan_button = create_review_button(window, '完整补查订单')
    review_full_scan_button.clicked.connect(window.on_review_full_scan_clicked)
    quality_refund_button = create_review_button(window, '获取品退订单')
    quality_refund_button.clicked.connect(window.on_quality_refund_clicked)
    order_cache_button = create_review_button(window, '订单缓存管理')
    order_cache_button.clicked.connect(window.on_order_cache_manage_clicked)
    first_button_row = QWidget()
    first_button_row_layout = QHBoxLayout(first_button_row)
    first_button_row_layout.setContentsMargins(0, 0, 0, 0)
    first_button_row_layout.setSpacing(standard_layout_spacing())
    first_button_row_layout.addWidget(review_find_button, 1)
    first_button_row_layout.addWidget(quality_refund_button, 1)
    review_content_layout.addWidget(first_button_row)
    second_button_row = QWidget()
    second_button_row_layout = QHBoxLayout(second_button_row)
    second_button_row_layout.setContentsMargins(0, 0, 0, 0)
    second_button_row_layout.setSpacing(standard_layout_spacing())
    second_button_row_layout.addWidget(review_full_scan_button, 1)
    second_button_row_layout.addWidget(order_cache_button, 1)
    review_content_layout.addWidget(second_button_row)
    review_content_layout.addStretch(1)
    config_badge = create_config_badge(window)
    config_card = create_card(window, '1. 系统配置与订单获取', config_badge, content, 'ConfigCard')
    config_card.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
    setup_content_layout.addWidget(build_setup_section_card('1.1. 系统配置', config_content))
    setup_content_layout.addWidget(build_setup_section_card(None, review_content))
    return SetupSectionRefs(config_card, config_badge, config_path_panel, config_path_label, config_note_label, auto_cookie_button, review_days_spin, review_find_button, review_full_scan_button, quality_refund_button, order_cache_button, setup_content_layout, config_content_layout, review_content_layout)


def build_batch_section(window):
    order_count_badge = create_count_badge()
    tracking_count_badge = create_count_badge()
    order_edit = BatchInputEdit('请用英文逗号、换行分隔，最多100个')
    order_edit.setMinimumHeight(window._calculate_editor_height(order_edit, max(4, scale_px(INPUT_VISIBLE_LINES - 4, min_value=4))))
    order_edit.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
    tracking_edit = BatchInputEdit('请用英文逗号、换行分隔，最多100个')
    tracking_edit.setMinimumHeight(window._calculate_editor_height(tracking_edit, max(4, scale_px(INPUT_VISIBLE_LINES - 4, min_value=4))))
    tracking_edit.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
    order_card = create_card(window, '2. 填写订单号', order_count_badge, order_edit, 'OrderCard')
    tracking_card = create_card(window, '3. 填写物流单号', tracking_count_badge, tracking_edit, 'TrackingCard')
    action_content = QWidget()
    action_content_layout = QVBoxLayout(action_content)
    action_content_layout.setContentsMargins(0, 0, 0, 0)
    action_content_layout.setSpacing(standard_layout_spacing())
    start_button = QPushButton('开始批量处理')
    start_button.setObjectName('PrimaryButton')
    start_button.setCursor(Qt.PointingHandCursor)
    start_button.setFont(build_font(FONT_SIZES['button'], bold=True))
    start_button.setFixedHeight(scale_px(BADGE_HEIGHT, min_value=28))
    start_button.setMinimumWidth(scale_px(140, min_value=120))
    start_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
    start_button.clicked.connect(window.on_start_clicked)
    pause_button = QPushButton('暂停批量处理')
    pause_button.setObjectName('PauseButton')
    pause_button.setCursor(Qt.PointingHandCursor)
    pause_button.setFont(build_font(FONT_SIZES['button'], bold=True))
    pause_button.setFixedHeight(scale_px(BADGE_HEIGHT, min_value=28))
    pause_button.setMinimumWidth(scale_px(140, min_value=120))
    pause_button.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
    pause_button.clicked.connect(window.on_pause_clicked)
    action_row = QWidget()
    action_row_layout = QHBoxLayout(action_row)
    action_row_layout.setContentsMargins(0, 0, 0, 0)
    action_row_layout.setSpacing(standard_layout_spacing())
    action_row_layout.addWidget(start_button, 1)
    action_row_layout.addWidget(pause_button, 1)
    action_content_layout.addWidget(action_row)
    action_card = create_card(window, '4. 执行批量处理', None, action_content, 'ActionCard')
    log_hint_label = QLabel('按时间顺序滚动')
    log_hint_label.setObjectName('LogHintPill')
    log_hint_label.setFont(build_font(FONT_SIZES['hint'], bold=True))
    log_hint_label.setAlignment(Qt.AlignCenter)
    log_hint_label.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Fixed)
    log_hint_label.setToolTip('最近执行记录会按时间顺序滚动显示')
    log_view = QPlainTextEdit()
    log_view.setObjectName('LogEdit')
    log_view.setReadOnly(True)
    log_view.setFont(build_fixed_font(11))
    log_view.setMinimumHeight(scale_px(LOG_PANEL_MIN_HEIGHT, min_value=128))
    log_view.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
    log_view.setPlaceholderText('运行日志会显示在这里')
    log_card = create_card(window, '运行日志', log_hint_label, log_view, 'LogCard')
    return BatchSectionRefs(order_count_badge, tracking_count_badge, order_edit, tracking_edit, order_card, tracking_card, start_button, pause_button, action_card, action_content_layout, log_hint_label, log_view, log_card)


def build_license_section(window):
    content = QWidget()
    license_content_layout = QVBoxLayout(content)
    license_content_layout.setContentsMargins(0, 0, 0, 0)
    license_content_layout.setSpacing(standard_layout_spacing())
    body_wrap = QWidget()
    body_wrap.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Maximum)
    body_layout = QVBoxLayout(body_wrap)
    body_layout.setContentsMargins(0, 0, 0, 0)
    body_layout.setSpacing(standard_layout_spacing())
    info_panel = QFrame()
    info_panel.setObjectName('LicenseInfoPanel')
    panel_layout = QVBoxLayout(info_panel)
    panel_layout.setContentsMargins(scale_px(14, min_value=8), scale_px(14, min_value=8), scale_px(14, min_value=8), scale_px(14, min_value=8))
    panel_layout.setSpacing(standard_layout_spacing())
    license_summary_label = QLabel()
    license_summary_label.setObjectName('LicenseSummary')
    license_summary_label.setFont(build_font(FONT_SIZES['badge'], bold=True))
    license_summary_label.setWordWrap(True)
    license_summary_label.setAlignment(Qt.AlignCenter)
    panel_layout.addWidget(license_summary_label)
    license_meta_label = QLabel()
    license_meta_label.setObjectName('LicenseMeta')
    license_meta_label.setFont(build_font(FONT_SIZES['secondary']))
    license_meta_label.setWordWrap(True)
    license_meta_label.setAlignment(Qt.AlignCenter)
    panel_layout.addWidget(license_meta_label)
    body_layout.addWidget(info_panel)
    license_content_layout.addStretch(1)
    license_content_layout.addWidget(body_wrap)
    license_content_layout.addStretch(1)
    license_card = create_card(window, None, None, content, 'LicenseCard')
    return LicenseSectionRefs(license_card, license_summary_label, license_meta_label, license_content_layout)
