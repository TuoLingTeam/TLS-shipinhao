# -*- coding: utf-8 -*-
"""右侧输入区、执行区和日志区构建。"""

from dataclasses import dataclass

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QHBoxLayout, QLabel, QPlainTextEdit, QPushButton, QSizePolicy, QVBoxLayout, QWidget

from settings import BADGE_HEIGHT, FONT_SIZES, INPUT_VISIBLE_LINES, LOG_PANEL_MIN_HEIGHT, scale_px
from ui.sections.shared_widgets import create_card, create_count_badge, standard_layout_spacing
from ui.widgets import build_fixed_font, build_font, BatchInputEdit


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

    return BatchSectionRefs(
        order_count_badge, tracking_count_badge, order_edit, tracking_edit,
        order_card, tracking_card, start_button, pause_button, action_card, action_content_layout,
        log_hint_label, log_view, log_card,
    )
