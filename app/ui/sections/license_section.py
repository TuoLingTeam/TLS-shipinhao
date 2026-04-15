# -*- coding: utf-8 -*-
"""授权状态卡片构建。"""

from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QFrame, QLabel, QSizePolicy, QVBoxLayout, QWidget

from settings import FONT_SIZES, scale_px
from ui.sections.shared_widgets import create_card, standard_layout_spacing
from ui.widgets import build_font


@dataclass
class LicenseSectionRefs:
    license_card: QFrame
    license_summary_label: QLabel
    license_meta_label: QLabel
    license_content_layout: QVBoxLayout


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
