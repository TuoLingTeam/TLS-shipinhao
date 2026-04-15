# -*- coding: utf-8 -*-
"""MainWindow 布局与尺寸辅助。"""

import sys

from PySide6.QtWidgets import QApplication

from settings import (
    COMPACT_LAYOUT_MIN_WIDTH,
    HIGH_DPI_COMPACT_THRESHOLD,
    INPUT_EDIT_PADDING,
    MAX_UI_SCALE,
    MIN_UI_SCALE,
    VERY_HIGH_DPI_COMPACT_THRESHOLD,
    WIDE_LAYOUT_MIN_HEIGHT,
    WIDE_LAYOUT_MIN_WIDTH,
    get_platform_default_window_size,
    scale_px,
)


def calculate_editor_height(editor, visible_lines=10):
    """按指定可见行数计算输入框高度。"""
    line_height = editor.fontMetrics().lineSpacing()
    document_margin = int(editor.document().documentMargin() * 2)
    frame = editor.frameWidth() * 2
    padding = scale_px(INPUT_EDIT_PADDING, min_value=8) + 2
    return line_height * visible_lines + document_margin + frame + padding


def resolve_height_profile(viewport_height):
    """根据当前可用高度返回垂直紧凑模式。"""
    if viewport_height <= 620:
        return 'dense'
    if viewport_height <= 720:
        return 'compact'
    return 'comfortable'


def resolve_layout_mode(width, height):
    """根据当前可用尺寸判定缩放模式，但不改变布局结构。"""
    if width >= WIDE_LAYOUT_MIN_WIDTH and height >= WIDE_LAYOUT_MIN_HEIGHT:
        return 'wide'
    if width >= COMPACT_LAYOUT_MIN_WIDTH:
        return 'compact'
    return 'dense'


def resolve_ui_scale_for_size(width, height):
    """根据给定窗口尺寸与平台/DPI 计算缩放系数。"""
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
    """结合平台默认值与屏幕可用区域，计算首次打开时的窗口尺寸。"""
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
