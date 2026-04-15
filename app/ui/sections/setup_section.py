# -*- coding: utf-8 -*-
"""系统配置与订单获取区域构建。"""

from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QSizePolicy, QSpinBox, QVBoxLayout, QWidget

from settings import APP_COLORS, CONFIG_PATH_MIN_HEIGHT, DEFAULT_REVIEW_DAYS, FONT_SIZES, scale_px
from ui.sections.shared_widgets import (
    build_setup_section_card,
    create_card,
    create_config_badge,
    create_review_button,
    standard_layout_spacing,
)
from ui.widgets import build_font


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
            QSpinBox::up-button, QSpinBox::down-button {{
                width: {scale_px(22, min_value=16)}px;
            }}"""
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

    return SetupSectionRefs(
        config_card, config_badge, config_path_panel, config_path_label, config_note_label,
        auto_cookie_button, review_days_spin, review_find_button, review_full_scan_button,
        quality_refund_button, order_cache_button, setup_content_layout, config_content_layout,
        review_content_layout,
    )
