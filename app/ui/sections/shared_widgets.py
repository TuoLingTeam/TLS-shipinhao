# -*- coding: utf-8 -*-
"""MainWindow 共享控件工厂。"""

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QPushButton, QSizePolicy, QVBoxLayout

from settings import (
    APP_COLORS,
    BADGE_HEIGHT,
    CARD_HEADER_GAP,
    CARD_HEADER_HEIGHT,
    CARD_PADDING,
    FONT_SIZES,
    INPUT_BADGE_HEIGHT,
    INPUT_BADGE_MIN_WIDTH,
    INPUT_VISIBLE_LINES,
    ROW_GAP,
    SETUP_SECTION_PADDING,
    scale_px,
)
from ui.widgets import BatchInputEdit, build_fixed_font, build_font


def standard_layout_spacing():
    return scale_px(ROW_GAP, min_value=8)


def create_card(window, title, title_right, content, object_name):
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
    layout.setContentsMargins(
        scale_px(SETUP_SECTION_PADDING, min_value=8),
        scale_px(SETUP_SECTION_PADDING, min_value=8),
        scale_px(SETUP_SECTION_PADDING, min_value=8),
        scale_px(SETUP_SECTION_PADDING, min_value=8),
    )
    layout.setSpacing(standard_layout_spacing())
    if title:
        layout.addWidget(create_setup_section_label(title), 0, Qt.AlignLeft)
    layout.addWidget(content)
    return card
