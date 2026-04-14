# -*- coding: utf-8 -*-
"""TLS-shipinhao 全局配置与常量。"""

from __future__ import annotations

import os
from pathlib import Path

# =============================================================================
# 配置文件 / 环境
# =============================================================================

CONFIG_DIR_NAME = ".tls-shipinhao"
COOKIE_FILE_NAME = "cookie.txt"
COOKIE_FILE_STEM = "cookie"
USER_CONFIG_POINTER = "selected_config_dir.txt"

# =============================================================================
# 批量处理 & 网络
# =============================================================================

MAX_BATCH_SIZE = 100
REQUEST_TIMEOUT = 30

# =============================================================================
# 窗口 & 品牌
# =============================================================================

APP_VERSION = "4.2"
WINDOW_TITLE = f"驼铃·视频小店差评处理 {APP_VERSION}"
AUTHOR_WECHAT = "TLS-801"
TUTORIAL_URL = "https://tuolingshe.feishu.cn/docx/BHiIdOUKxomqVgxIb1zcmIr8nLe"
DEFAULT_WINDOW_WIDTH = 880
DEFAULT_WINDOW_HEIGHT = 830
MIN_WINDOW_WIDTH = 800
MIN_WINDOW_HEIGHT = 700
MIN_UI_SCALE = 0.82
MAX_UI_SCALE = 1.0
WIDE_LAYOUT_MIN_WIDTH = 1320
WIDE_LAYOUT_MIN_HEIGHT = 780
COMPACT_LAYOUT_MIN_WIDTH = 860
HIGH_DPI_COMPACT_THRESHOLD = 120
VERY_HIGH_DPI_COMPACT_THRESHOLD = 140

# =============================================================================
# 布局
# =============================================================================

PAGE_MARGIN = 16
PAGE_GAP = 10
ROW_GAP = 12
LEFT_COLUMN_GAP = 20
CARD_PADDING = 12
CARD_HEADER_HEIGHT = 28
CARD_HEADER_GAP = 6
SETUP_SECTION_PADDING = 14
SETUP_SECTION_SPACING = 12
CARD_RADIUS = 16
HERO_RADIUS = 20

# =============================================================================
# 组件尺寸
# =============================================================================

HERO_PADDING_X = 24
HERO_PADDING_Y = 22
BADGE_MIN_WIDTH = 88
BADGE_HEIGHT = 36
BADGE_RADIUS = 12
INPUT_BADGE_MIN_WIDTH = 72
INPUT_BADGE_HEIGHT = 34
INPUT_BADGE_RADIUS = 10
BUTTON_HEIGHT = 40
CONFIG_PATH_MIN_HEIGHT = 76
INPUT_VISIBLE_LINES = 20
INPUT_EDIT_RADIUS = 14
INPUT_EDIT_PADDING = 16
LOG_EDIT_RADIUS = 14
LOG_EDIT_PADDING = 14
LOG_PANEL_MIN_HEIGHT = 180
DEFAULT_REVIEW_DAYS = 30

# =============================================================================
# 差评匹配算法配置
# =============================================================================

MATCH_MIN_SCORE = 50
AUTO_FILL_SCORE_THRESHOLD = 100
EVALUATION_MAX_DAYS = 30
EDUCATION_ORDER_MAX_DAYS = 60

# =============================================================================
# API 请求参数
# =============================================================================

FETCH_PAGE_INTERVAL_SECONDS = 0.3
ORDER_PAGE_SIZE = 100
EVALUATION_PAGE_SIZE = 10
EVALUATION_MAX_PAGES = 10
RATE_LIMIT_RETRY_COUNT = 3
ORDER_WINDOW_WORKERS = 3
ORDER_RISK_WINDOW_WORKERS = 1
ORDER_RISK_PAGE_INTERVAL_SECONDS = 2.0
ORDER_CACHE_SCOPE = "orders_30d"
ORDER_CACHE_COVERAGE_DAYS = 30
ORDER_CACHE_INCREMENTAL_DAYS = 3
ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS = 1
ORDER_CACHE_DB_NAME = "order_cache.sqlite3"

# =============================================================================
# 字体方案
# =============================================================================

FONT_SIZES = {
    "title": 24,
    "section": 15,
    "section_log": 16,
    "badge": 13,
    "body": 12,
    "button": 13,
    "secondary": 11,
    "hint": 10,
}

# =============================================================================
# 颜色方案
# =============================================================================

APP_COLORS = {
    "window_base": "#3A3D38",
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
    "section_title": "#064E3B",
    "body_text": "#064E3B",
}

# =============================================================================
# 微信小商店 API
# =============================================================================

ORDER_DETAIL_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
ORDER_INIT_SHIP_DATA_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/initShipData"
ORDER_DELIVERY_UPDATE_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateDeliveryInfo"
DELIVERY_MISMATCH_MESSAGE = "快递单号与所选物流商不匹配"
EVALUATION_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search"
ORDER_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch"
QUALITY_REFUND_ORDER_URL = "https://store.weixin.qq.com/shop-faas/statistic/dsr/product/refund/order"

# =============================================================================
# 卡密验证后端 API
# =============================================================================

LICENSE_API_BASE_URLS = [
    "https://sphapi.199908.top",
    "https://sphapi.tuoling.ccwu.cc",
    "https://sphapi.tuoling.us.ci",
    "https://sphapi.tuoling.eu.cc",
]
LICENSE_API_TIMEOUT = 10
LICENSE_STATUS_CACHE_TTL_SECONDS = 60


# =============================================================================
# 配置读取 / Cookie 工具
# =============================================================================

class ConfigNotFoundError(FileNotFoundError):
    def __init__(self, searched_dirs):
        self.searched_dirs = list(searched_dirs)
        super().__init__("未找到可用的配置目录")


def get_home_config_dir() -> Path:
    return Path.home() / CONFIG_DIR_NAME


def _candidate_config_dirs():
    dirs = []
    pointer = get_home_config_dir() / USER_CONFIG_POINTER
    if pointer.exists():
        try:
            selected = pointer.read_text(encoding='utf-8').strip()
            if selected:
                dirs.append(Path(selected).expanduser())
        except Exception:
            pass
    dirs.append(get_home_config_dir())
    return dirs


def get_cookie() -> str:
    searched = []
    for cfg_dir in _candidate_config_dirs():
        searched.append(str(cfg_dir))
        cookie_file = cfg_dir / COOKIE_FILE_NAME
        if cookie_file.exists():
            return cookie_file.read_text(encoding='utf-8').strip()
    raise ConfigNotFoundError(searched)


def serialize_cookie_data(cookie_data):
    if isinstance(cookie_data, dict):
        return cookie_data.get('cookie', '') or cookie_data.get('cookie_str', '') or ''
    return str(cookie_data or '')


def get_magic(cookie_data: str) -> str:
    # 兼容旧逻辑：从 cookie 文本中提取 biz_magic / magic。
    import re
    for pattern in (r'biz_magic=([^;\s]+)', r'magic=([^;\s]+)'):
        match = re.search(pattern, cookie_data or '')
        if match:
            return match.group(1)
    return ''

# =============================================================================
# 运行时 UI 缩放
# =============================================================================

_UI_SCALE = 1.0


def clamp_ui_scale(scale):
    return max(MIN_UI_SCALE, min(MAX_UI_SCALE, float(scale)))


def set_ui_scale(scale):
    global _UI_SCALE
    _UI_SCALE = clamp_ui_scale(scale)


def get_ui_scale():
    return _UI_SCALE


def scale_px(value, *, min_value=0):
    return max(min_value, int(round(value * _UI_SCALE)))


def get_platform_default_window_size():
    return DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT


def get_home_config_dir() -> Path:
    return Path.home() / CONFIG_DIR_NAME
