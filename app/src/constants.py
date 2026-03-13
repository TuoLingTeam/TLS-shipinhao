# -*- coding: utf-8 -*-
"""TLS-shipinhao 全局常量与设计规范。"""

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

# ---------------------------------------------------------------------------
# 设计规范：默认窗口尺寸（程序首次打开时的宽高）
# 宽度为设计宽度的 3/4，高度收紧避免底部留白；响应式会限制不超过屏幕 92%
# 窗口可缩放，最小尺寸保证三栏等内容可读。
# ---------------------------------------------------------------------------
DEFAULT_WINDOW_WIDTH = 850   # 默认宽度收紧，减少右侧留白
DEFAULT_WINDOW_HEIGHT = 780  # 默认高度收紧，减少底部留白
DESIGN_WIDTH = 1280
DESIGN_HEIGHT = 900
MIN_WINDOW_WIDTH = 800
MIN_WINDOW_HEIGHT = 700
MAC_DEFAULT_WINDOW_WIDTH = DEFAULT_WINDOW_WIDTH
MAC_DEFAULT_WINDOW_HEIGHT = DEFAULT_WINDOW_HEIGHT
WINDOWS_DEFAULT_WINDOW_WIDTH = DEFAULT_WINDOW_WIDTH
WINDOWS_DEFAULT_WINDOW_HEIGHT = DEFAULT_WINDOW_HEIGHT

# ---------------------------------------------------------------------------
# 设计规范：组件尺寸（px）
# ---------------------------------------------------------------------------
DESIGN_SIZES = {
    # 窗口与页面
    "page_margin_x": 20,
    "page_margin_y": 20,
    "page_spacing": 16,
    "workspace_gap": 16,
    "column_gap": 16,
    "input_pair_gap": 16,
    # 顶部 Hero 区
    "hero_card_height": 80,
    "hero_padding_x": 24,
    "hero_padding_y": 14,
    "hero_title_font_size": 24,
    "hero_subtitle_font_size": 13,
    # 徽标（已激活、微信、教程）
    "badge_height": 40,
    "badge_min_width": 88,
    "badge_padding_x": 16,
    "badge_padding_y": 10,
    "badge_radius": 12,
    "badge_gap": 12,
    # 卡片标题上边距（统一缩小，避免标题上方空白过大）
    "card_header_top_padding": 8,
    # 侧边栏
    "sidebar_width": 320,
    "sidebar_button_height": 50,
    "sidebar_card_padding_x": 20,
    "sidebar_card_padding_y": 16,
    "sidebar_card_radius": 16,
    "sidebar_card_gap": 10,
    # 输入卡片（订单号/物流单号）
    "input_card_padding_x": 20,
    "input_card_padding_y": 16,
    "input_card_radius": 16,
    "input_card_header_gap": 10,
    "input_edit_radius": 14,
    "input_edit_padding": 16,
    "input_edit_border_width": 1,
    "input_visible_lines": 11,
    # 计数徽标（0/100）
    "count_badge_min_width": 64,
    "count_badge_height": 32,
    "count_badge_radius": 10,
    "count_badge_font_size": 11,
    # 配置目录卡片
    "config_path_min_height": 80,
    "config_button_height": 50,
    # 日志区
    "log_card_radius": 16,
    "log_card_padding_x": 20,
    "log_card_padding_y": 16,
    "log_edit_radius": 14,
    "log_edit_padding": 14,
    "log_panel_min_height": 220,
    "log_title_font_size": 16,
    "log_hint_font_size": 11,
    # 圆角统一
    "radius_sm": 8,
    "radius_md": 12,
    "radius_lg": 16,
    "radius_xl": 20,
    # 最小高度（用于响应式分配）
    "workspace_top_min_height": 320,
    "workspace_bottom_min_height": 300,
}

# ---------------------------------------------------------------------------
# 设计规范：颜色方案（基于 Design System：SaaS + 翠绿主色 + 橙色 CTA）
# 每个组件颜色见下方 APP_COLORS 与注释
# ---------------------------------------------------------------------------
APP_COLORS = {
    # 背景
    "bg": "#ECFDF5",                    # 窗口根背景（浅翠绿）
    "bg_panel": "#D1FAE5",              # 面板次级背景（翠绿 100）
    "surface": "#FFFFFF",               # 卡片/输入框表面
    "surface_soft": "#F0FDF4",          # Hero 等柔和表面（翠绿 50）
    # 边框
    "border": "#A7F3D0",                # 默认边框（翠绿 200）
    "border_strong": "#6EE7B7",         # 强调边框（翠绿 300）
    "input_border": "#A7F3D0",
    "input_border_focus": "#059669",    # 输入框聚焦（Primary）
    # 文字
    "text": "#064E3B",                  # 正文（翠绿 800）
    "heading": "#022C22",               # 标题（翠绿 900）
    "muted": "#047857",                 # 次要文案（翠绿 700）
    "muted_soft": "#059669",            # 更弱次要（翠绿 600）
    # 主色（Primary）- 获取差评/品退、链接、强调
    "blue": "#059669",
    "blue_deep": "#047857",
    "blue_soft": "#D1FAE5",
    "blue_tint": "#A7F3D0",
    "hero_tint": "#D1FAE5",
    "hero_tint_deep": "#A7F3D0",
    "hero_border": "#6EE7B7",
    # CTA 主按钮（开始批量处理）
    "orange": "#F97316",                # CTA 橙
    "orange_deep": "#EA580C",
    "orange_soft": "#FFEDD5",
    "orange_tint": "#FED7AA",
    "orange_tint_deep": "#FDBA74",
    "orange_border": "#F97316",
    # 成功/已激活
    "green": "#059669",
    "green_soft": "#D1FAE5",
    # 错误/未配置/警告
    "red": "#B91C1C",
    "red_soft": "#FEE2E2",
    # 中性（暂停、禁用）
    "neutral_bg": "#F1F5F9",
    "neutral_text": "#64748B",
    "neutral_border": "#CBD5E1",
    # 输入与日志
    "input_bg": "#FFFFFF",
    "log_bg": "#022C22",
    "log_surface": "#064E3B",
    "log_fg": "#ECFDF5",
    "log_muted": "#6EE7B7",
}

# ---------------------------------------------------------------------------
# 配置文件
# ---------------------------------------------------------------------------
CONFIG_DIR_NAME = ".tls-shipinhao"
COOKIE_FILE_NAME = "cookie.txt"
COOKIE_FILE_STEM = "cookie"
USER_CONFIG_POINTER = "selected_config_dir.txt"

# ---------------------------------------------------------------------------
# 微信小商店 API
# ---------------------------------------------------------------------------
ORDER_DETAIL_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
ORDER_DELIVERY_UPDATE_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateOrderDeliveryInfo"
DELIVERY_MISMATCH_MESSAGE = "快递单号与所选物流商不匹配"

# 中差评查找 API
EVALUATION_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search"
ORDER_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch"
QUALITY_REFUND_ORDER_URL = "https://store.weixin.qq.com/shop-faas/statistic/dsr/product/refund/order"
DEFAULT_REVIEW_DAYS = 30

# ---------------------------------------------------------------------------
# 卡密验证后端 API
# ---------------------------------------------------------------------------
LICENSE_API_BASE_URL = "https://sphapi.199908.top"
LICENSE_ACTIVATE_URL = f"{LICENSE_API_BASE_URL}/api/activate"
LICENSE_VERIFY_URL = f"{LICENSE_API_BASE_URL}/api/verify"
LICENSE_API_TIMEOUT = 15


def get_platform_default_window_size():
    """按平台返回默认窗口尺寸。"""
    if sys.platform.startswith("win"):
        return WINDOWS_DEFAULT_WINDOW_WIDTH, WINDOWS_DEFAULT_WINDOW_HEIGHT
    return MAC_DEFAULT_WINDOW_WIDTH, MAC_DEFAULT_WINDOW_HEIGHT
