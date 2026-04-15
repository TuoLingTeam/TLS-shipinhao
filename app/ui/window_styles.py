# -*- coding: utf-8 -*-
"""MainWindow 样式辅助。"""

from settings import (
    APP_COLORS,
    CARD_RADIUS,
    CARD_PADDING,
    CARD_HEADER_GAP,
    HERO_RADIUS,
    INPUT_BADGE_RADIUS,
    INPUT_EDIT_PADDING,
    INPUT_EDIT_RADIUS,
    LOG_EDIT_PADDING,
    LOG_EDIT_RADIUS,
    SETUP_SECTION_PADDING,
    scale_px,
)


def scaled_padding(vertical, horizontal):
    """按当前 UI 缩放系数返回统一 padding 字符串。"""
    return f"{scale_px(vertical, min_value=1)}px {scale_px(horizontal, min_value=1)}px"


def build_badge_style(background, text_color, border_color, *, radius=None, padding=None):
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


def build_main_window_stylesheet():
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
                padding: {scale_px(7, min_value=5)}px {scale_px(12, min_value=8)}px;
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
