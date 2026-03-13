# -*- coding: utf-8 -*-
"""TLS-shipinhao 全局常量。"""

import sys

# 批量处理 & 网络
MAX_BATCH_SIZE = 100
REQUEST_TIMEOUT = 30

# 窗口 & 品牌
WINDOW_TITLE = "驼铃视频小店中差评处理"
AUTHOR_WECHAT = "TLS-801"
TUTORIAL_URL = "https://tuolingshe.feishu.cn/docx/BHiIdOUKxomqVgxIb1zcmIr8nLe"
DEFAULT_WINDOW_WIDTH = 900
DEFAULT_WINDOW_HEIGHT = 850
WINDOWS_DEFAULT_WINDOW_WIDTH = 1120
WINDOWS_DEFAULT_WINDOW_HEIGHT = 780
MIN_WINDOW_WIDTH = 800
MIN_WINDOW_HEIGHT = 700
MIN_UI_SCALE = 0.82
MAX_UI_SCALE = 1.0
WIDE_LAYOUT_MIN_WIDTH = 1320
WIDE_LAYOUT_MIN_HEIGHT = 780
COMPACT_LAYOUT_MIN_WIDTH = 980
HIGH_DPI_COMPACT_THRESHOLD = 120
VERY_HIGH_DPI_COMPACT_THRESHOLD = 140

# 布局
PAGE_MARGIN = 16
PAGE_GAP = 10
ROW_GAP = 12
CARD_PADDING = 12
CARD_HEADER_HEIGHT = 28
CARD_HEADER_GAP = 6
CARD_RADIUS = 16
HERO_RADIUS = 20

# 组件尺寸
HERO_PADDING_X = 24
HERO_PADDING_Y = 10
BADGE_MIN_WIDTH = 88
BADGE_HEIGHT = 36
BADGE_RADIUS = 12
INPUT_BADGE_MIN_WIDTH = 72
INPUT_BADGE_HEIGHT = 34
INPUT_BADGE_RADIUS = 10
BUTTON_HEIGHT = 40
CONFIG_PATH_MIN_HEIGHT = 68
INPUT_VISIBLE_LINES = 9
INPUT_EDIT_RADIUS = 14
INPUT_EDIT_PADDING = 16
LOG_EDIT_RADIUS = 14
LOG_EDIT_PADDING = 14
LOG_PANEL_MIN_HEIGHT = 180
DEFAULT_REVIEW_DAYS = 30

# 颜色方案
APP_COLORS = {
    "bg": "#ECFDF5",
    "surface": "#FFFFFF",
    "surface_soft": "#F0FDF4",
    "border": "#A7F3D0",
    "border_strong": "#6EE7B7",
    "input_border": "#A7F3D0",
    "input_border_focus": "#059669",
    "text": "#064E3B",
    "heading": "#022C22",
    "muted": "#047857",
    "muted_soft": "#059669",
    "blue": "#059669",
    "blue_deep": "#047857",
    "blue_soft": "#D1FAE5",
    "blue_tint": "#A7F3D0",
    "hero_border": "#6EE7B7",
    "orange": "#F97316",
    "orange_deep": "#EA580C",
    "green": "#059669",
    "green_soft": "#D1FAE5",
    "red": "#B91C1C",
    "red_soft": "#FEE2E2",
    "neutral_bg": "#F1F5F9",
    "neutral_text": "#64748B",
    "neutral_border": "#CBD5E1",
    "input_bg": "#FFFFFF",
}

# 配置文件
CONFIG_DIR_NAME = ".tls-shipinhao"
COOKIE_FILE_NAME = "cookie.txt"
COOKIE_FILE_STEM = "cookie"
USER_CONFIG_POINTER = "selected_config_dir.txt"

# 微信小商店 API
ORDER_DETAIL_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
ORDER_DELIVERY_UPDATE_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateOrderDeliveryInfo"
DELIVERY_MISMATCH_MESSAGE = "快递单号与所选物流商不匹配"
EVALUATION_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search"
ORDER_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch"
QUALITY_REFUND_ORDER_URL = "https://store.weixin.qq.com/shop-faas/statistic/dsr/product/refund/order"

# 卡密验证后端 API
LICENSE_API_BASE_URL = "https://sphapi.199908.top"
LICENSE_ACTIVATE_URL = f"{LICENSE_API_BASE_URL}/api/activate"
LICENSE_VERIFY_URL = f"{LICENSE_API_BASE_URL}/api/verify"
LICENSE_API_TIMEOUT = 15


_UI_SCALE = 1.0


def clamp_ui_scale(scale):
    """限制 UI 缩放系数，避免过小或过大。"""
    return max(MIN_UI_SCALE, min(MAX_UI_SCALE, float(scale)))


def set_ui_scale(scale):
    """设置全局 UI 缩放系数。"""
    global _UI_SCALE
    _UI_SCALE = clamp_ui_scale(scale)


def get_ui_scale():
    """获取全局 UI 缩放系数。"""
    return _UI_SCALE


def scale_px(value, *, min_value=0):
    """按当前 UI 缩放系数返回像素值。"""
    return max(min_value, int(round(value * _UI_SCALE)))


def get_platform_default_window_size():
    """按平台返回默认窗口尺寸。"""
    if sys.platform.startswith("win"):
        return WINDOWS_DEFAULT_WINDOW_WIDTH, WINDOWS_DEFAULT_WINDOW_HEIGHT
    return DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT
