# -*- coding: utf-8 -*-
"""TLS-shipinhao 全局配置与常量。"""

from __future__ import annotations

import os
import sys
from pathlib import Path

# =============================================================================
# 配置文件 / 环境
# =============================================================================

CONFIG_DIR_NAME = ".tls-shipinhao"
APP_DATA_DIR_NAME = "TLS-shipinhao"
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

APP_VERSION = "4.3.0"
WINDOW_TITLE = f"驼铃·视频小店差评处理 {APP_VERSION}"
UPDATE_VERSION_URL = "https://gitee.com/tuolingshe/tuoling-shipinhao/raw/master/version.json"
UPDATE_CHECK_DELAY_MS = 1200
AUTHOR_WECHAT = "TLS-801"
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
PAGE_GAP = 14
ROW_GAP = 16
LEFT_COLUMN_GAP = 24
CARD_PADDING = 16
CARD_HEADER_HEIGHT = 28
CARD_HEADER_GAP = 8
SETUP_SECTION_PADDING = 16
SETUP_SECTION_SPACING = 14
CARD_RADIUS = 16
HERO_RADIUS = 20

# =============================================================================
# 组件尺寸
# =============================================================================

HERO_PADDING_X = 26
HERO_PADDING_Y = 24
BADGE_MIN_WIDTH = 88
BADGE_HEIGHT = 40
BADGE_RADIUS = 12
INPUT_BADGE_MIN_WIDTH = 58
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
ORDER_CACHE_DIR_NAME = "cache"
TASK_HISTORY_FILE_NAME = "task_history.json"
TASK_HISTORY_MAX_ENTRIES = 50

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

import re

_CONFIG_DIR_CACHE: str | None = None


def get_home_config_dir() -> Path:
    return Path.home() / CONFIG_DIR_NAME


def get_app_runtime_dir() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent


def get_user_data_dir() -> Path:
    if sys.platform == "darwin":
        base = Path.home() / "Library" / "Application Support"
    elif os.name == "nt":
        local_appdata = os.environ.get("LOCALAPPDATA") or os.environ.get("APPDATA")
        base = Path(local_appdata) if local_appdata else (Path.home() / "AppData" / "Local")
    else:
        xdg = os.environ.get("XDG_DATA_HOME")
        base = Path(xdg) if xdg else (Path.home() / ".local" / "share")
    data_dir = base / APP_DATA_DIR_NAME
    data_dir.mkdir(parents=True, exist_ok=True)
    return data_dir


def get_order_cache_dir() -> Path:
    cache_dir = get_user_data_dir() / ORDER_CACHE_DIR_NAME
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir


def get_internal_order_cache_dir() -> Path:
    cache_dir = get_app_runtime_dir() / ORDER_CACHE_DIR_NAME
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir


class ConfigNotFoundError(FileNotFoundError):
    def __init__(self, searched_dirs):
        self.searched_dirs = [str(item) for item in searched_dirs]
        super().__init__("未找到可用的配置目录")



def get_default_config_dir() -> str:
    path = get_home_config_dir()
    path.mkdir(parents=True, exist_ok=True)
    return str(path)


def get_config_dir_cache() -> str | None:
    return _CONFIG_DIR_CACHE


def get_saved_user_config_dir() -> str | None:
    pointer = get_home_config_dir() / USER_CONFIG_POINTER
    if not pointer.exists():
        return None
    try:
        value = pointer.read_text(encoding="utf-8").strip()
    except Exception:
        return None
    return value or None


def save_user_config_dir(config_dir: str) -> str:
    global _CONFIG_DIR_CACHE
    target = Path(config_dir).expanduser().resolve()
    target.mkdir(parents=True, exist_ok=True)
    home_dir = get_home_config_dir()
    home_dir.mkdir(parents=True, exist_ok=True)
    (home_dir / USER_CONFIG_POINTER).write_text(str(target), encoding="utf-8")
    _CONFIG_DIR_CACHE = str(target)
    return str(target)


def _candidate_config_dirs() -> list[Path]:
    dirs: list[Path] = []
    if _CONFIG_DIR_CACHE:
        dirs.append(Path(_CONFIG_DIR_CACHE).expanduser())
    saved = get_saved_user_config_dir()
    if saved:
        dirs.append(Path(saved).expanduser())
    dirs.append(get_home_config_dir())

    unique: list[Path] = []
    seen: set[str] = set()
    for item in dirs:
        normalized = str(item.resolve()) if item.exists() else str(item)
        if normalized in seen:
            continue
        seen.add(normalized)
        unique.append(item)
    return unique


def normalize_batch_text(text: str) -> str:
    items = parse_batch_input(text)
    return "\n".join(items)


def parse_batch_input(text: str) -> list[str]:
    if not text:
        return []
    parts = re.split(r"[\s,，;；]+", text.strip())
    return [item for item in parts if item]


def serialize_cookie_data(cookie_data) -> str:
    if isinstance(cookie_data, dict):
        return "; ".join(f"{key}={value}" for key, value in cookie_data.items() if key)
    return str(cookie_data or "")


def read_cookie_data(cookie_path: str | os.PathLike[str]):
    raw = Path(cookie_path).read_text(encoding="utf-8").strip()
    cookie_map = {}
    for chunk in raw.split(';'):
        part = chunk.strip()
        if not part or '=' not in part:
            continue
        key, value = part.split('=', 1)
        key = key.strip()
        if key:
            cookie_map[key] = value.strip()
    return cookie_map or raw


def save_cookie_data(cookie_data, config_dir: str | os.PathLike[str] | None = None, remember_dir: bool = False) -> str:
    target_dir = Path(config_dir or get_default_config_dir()).expanduser().resolve()
    target_dir.mkdir(parents=True, exist_ok=True)
    cookie_path = target_dir / COOKIE_FILE_NAME
    cookie_path.write_text(serialize_cookie_data(cookie_data), encoding="utf-8")
    if remember_dir:
        save_user_config_dir(str(target_dir))
    return str(cookie_path)


def extract_biz_magic_from_cookie(cookie_data) -> str:
    if isinstance(cookie_data, dict):
        return str(cookie_data.get('biz_magic') or cookie_data.get('magic') or '').strip()
    raw = serialize_cookie_data(cookie_data)
    for pattern in (r'biz_magic=([^;\s]+)', r'magic=([^;\s]+)'):
        match = re.search(pattern, raw)
        if match:
            return match.group(1).strip()
    return ''


def get_magic(cookie_data) -> str:
    return extract_biz_magic_from_cookie(cookie_data)


def resolve_config_files_in_dir(config_dir: str | os.PathLike[str]) -> dict[str, str]:
    target = Path(config_dir).expanduser().resolve()
    result: dict[str, str] = {}
    cookie_file = target / COOKIE_FILE_NAME
    if cookie_file.exists():
        result['cookie'] = str(cookie_file)
    return result


def resolve_config_dir() -> str:
    searched = []
    for cfg_dir in _candidate_config_dirs():
        searched.append(cfg_dir)
        resolved_files = resolve_config_files_in_dir(cfg_dir)
        if 'cookie' in resolved_files:
            return str(Path(cfg_dir).expanduser().resolve())
    raise ConfigNotFoundError(searched)


def get_cookie():
    resolved_dir = resolve_config_dir()
    cookie_path = resolve_config_files_in_dir(resolved_dir).get('cookie')
    if not cookie_path:
        raise ConfigNotFoundError([resolved_dir])
    return read_cookie_data(cookie_path)

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
