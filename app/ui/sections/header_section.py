# -*- coding: utf-8 -*-
"""顶部 Hero 区域构建。"""

from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QFrame, QHBoxLayout, QLabel, QSizePolicy, QVBoxLayout, QWidget, QPushButton

from settings import APP_COLORS, AUTHOR_WECHAT, BADGE_HEIGHT, BADGE_MIN_WIDTH, BADGE_RADIUS, FONT_SIZES, HERO_PADDING_X, HERO_PADDING_Y, scale_px
from ui.widgets import build_font


@dataclass
class HeaderSectionRefs:
    header_card: QFrame
    header_box: QHBoxLayout
    title_box: QVBoxLayout
    author_badge: QLabel
    tutorial_badge: QLabel
    update_button: QPushButton


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
        scale_px(HERO_PADDING_X, min_value=14),
        scale_px(HERO_PADDING_Y, min_value=6),
        scale_px(HERO_PADDING_X, min_value=14),
        scale_px(HERO_PADDING_Y, min_value=6),
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
