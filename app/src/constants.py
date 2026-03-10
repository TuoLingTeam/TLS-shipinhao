# -*- coding: utf-8 -*-
"""TLS-shipinhao 全局常量。"""

import sys

# ---------------------------------------------------------------------------
# 批量处理 & 网络
# ---------------------------------------------------------------------------
MAX_BATCH_SIZE = 100
REQUEST_TIMEOUT = 30

# ---------------------------------------------------------------------------
# 窗口 & 品牌
# ---------------------------------------------------------------------------
WINDOW_TITLE = "驼铃视频小店中差评处理"
AUTHOR_WECHAT = "TLS-801"
TUTORIAL_URL = "https://tuolingshe.feishu.cn/docx/BHiIdOUKxomqVgxIb1zcmIr8nLe"

DESIGN_WIDTH = 1240
DESIGN_HEIGHT = 980
MAC_DEFAULT_WINDOW_WIDTH = 1880
MAC_DEFAULT_WINDOW_HEIGHT = 1668
WINDOWS_DEFAULT_WINDOW_WIDTH = 1280
WINDOWS_DEFAULT_WINDOW_HEIGHT = 820

# ---------------------------------------------------------------------------
# 颜色方案
# ---------------------------------------------------------------------------
APP_COLORS = {
    "bg": "#E9EFF6",
    "bg_panel": "#DCE6F2",
    "surface": "#FFFFFF",
    "surface_soft": "#F5F8FC",
    "border": "#B8C6D9",
    "border_strong": "#90A5BF",
    "text": "#1A2A3E",
    "heading": "#0B1B32",
    "muted": "#4A5F79",
    "muted_soft": "#7A8FA9",
    "blue": "#0F5BD6",
    "blue_deep": "#0C3C8F",
    "blue_soft": "#D5E7FF",
    "blue_tint": "#CADDF7",
    "hero_tint": "#B7CCE6",
    "hero_tint_deep": "#9FBADF",
    "hero_border": "#7F9FC7",
    "orange": "#D97706",
    "orange_deep": "#A84E05",
    "orange_soft": "#FBE5C8",
    "orange_tint": "#F6DEC0",
    "orange_tint_deep": "#EDCC9D",
    "orange_border": "#D9A86E",
    "green": "#15803D",
    "green_soft": "#DCFCE7",
    "red": "#B91C1C",
    "red_soft": "#FEE2E2",
    "input_bg": "#FDFEFF",
    "input_border": "#AFC0D6",
    "input_border_focus": "#2B78F6",
    "log_bg": "#06254A",
    "log_surface": "#041D39",
    "log_fg": "#F0F6FF",
    "log_muted": "#B7CBE4",
}

# ---------------------------------------------------------------------------
# 配置文件
# ---------------------------------------------------------------------------
CONFIG_DIR_NAME = ".tls-shipinhao"
COOKIE_FILE_NAME = "cookie.txt"
MAGIC_FILE_NAME = "biz_magic.txt"
COOKIE_FILE_STEM = "cookie"
MAGIC_FILE_STEM = "biz_magic"
USER_CONFIG_POINTER = "selected_config_dir.txt"

# ---------------------------------------------------------------------------
# 微信小商店 API
# ---------------------------------------------------------------------------
ORDER_DETAIL_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
ORDER_DELIVERY_UPDATE_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateOrderDeliveryInfo"
DELIVERY_MISMATCH_MESSAGE = "快递单号与所选物流商不匹配"

# ---------------------------------------------------------------------------
# 卡密验证后端 API
# ---------------------------------------------------------------------------
LICENSE_API_BASE_URL = "https://api.199908.top"
LICENSE_ACTIVATE_URL = f"{LICENSE_API_BASE_URL}/api/activate"
LICENSE_VERIFY_URL = f"{LICENSE_API_BASE_URL}/api/verify"
LICENSE_API_TIMEOUT = 15


def get_platform_default_window_size():
    """按平台返回默认窗口尺寸。"""
    if sys.platform.startswith("win"):
        return WINDOWS_DEFAULT_WINDOW_WIDTH, WINDOWS_DEFAULT_WINDOW_HEIGHT
    return MAC_DEFAULT_WINDOW_WIDTH, MAC_DEFAULT_WINDOW_HEIGHT
