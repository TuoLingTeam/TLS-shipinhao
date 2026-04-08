# -*- coding: utf-8 -*-
"""TLS-shipinhao 全局常量。"""

import sys

# 批量处理 & 网络
MAX_BATCH_SIZE = 100              # 批量处理最大订单数
REQUEST_TIMEOUT = 30              # 网络请求超时时间（秒）

# 窗口 & 品牌
APP_VERSION = "3.0"                # 应用版本号（Mac 简介/Windows 属性及窗口标题）
WINDOW_TITLE = f"驼铃视频小店中差评处理 {APP_VERSION}"  # 窗口标题
AUTHOR_WECHAT = "TLS-801"         # 作者微信（用于授权联系）
TUTORIAL_URL = "https://tuolingshe.feishu.cn/docx/BHiIdOUKxomqVgxIb1zcmIr8nLe"  # 教程链接
DEFAULT_WINDOW_WIDTH = 810        # 非 Windows 系统默认窗口宽度，略收紧
DEFAULT_WINDOW_HEIGHT = 800       # 非 Windows 系统默认窗口高度，适度增加
WINDOWS_DEFAULT_WINDOW_WIDTH = 930  # Windows 系统默认窗口宽度，略收紧
WINDOWS_DEFAULT_WINDOW_HEIGHT = 850  # Windows 系统默认窗口高度，适度增加
MIN_WINDOW_WIDTH = 800            # 窗口最小宽度
MIN_WINDOW_HEIGHT = 700           # 窗口最小高度
MIN_UI_SCALE = 0.82               # UI 最小缩放系数
MAX_UI_SCALE = 1.0                # UI 最大缩放系数
WIDE_LAYOUT_MIN_WIDTH = 1320      # 宽屏布局最小宽度
WIDE_LAYOUT_MIN_HEIGHT = 780      # 宽屏布局最小高度
COMPACT_LAYOUT_MIN_WIDTH = 980    # 紧凑布局最小宽度
HIGH_DPI_COMPACT_THRESHOLD = 120  # 高 DPI 紧凑布局阈值
VERY_HIGH_DPI_COMPACT_THRESHOLD = 140  # 极高 DPI 紧凑布局阈值

# 布局
PAGE_MARGIN = 16                  # 页面外边距
PAGE_GAP = 10                      # 页面元素间距
ROW_GAP = 12                       # 行间距
LEFT_COLUMN_GAP = 20               # 左侧配置区卡片间距，略大于 ROW_GAP 以减轻拥挤感
CARD_PADDING = 12                 # 卡片内边距
CARD_HEADER_HEIGHT = 28           # 卡片标题高度
CARD_HEADER_GAP = 6                # 卡片标题与内容间距
SETUP_SECTION_PADDING = 14         # 配置区内小节（如配置目录、订单查询）内边距
SETUP_SECTION_SPACING = 12         # 配置区内小节标题与内容间距
CARD_RADIUS = 16                   # 卡片圆角半径
HERO_RADIUS = 20                  # 标题卡片圆角半径

# 组件尺寸
HERO_PADDING_X = 24               # 标题卡片水平内边距
HERO_PADDING_Y = 22               # 标题卡片垂直内边距
BADGE_MIN_WIDTH = 88              # 徽标最小宽度
BADGE_HEIGHT = 36                 # 徽标高度
BADGE_RADIUS = 12                  # 徽标圆角半径
INPUT_BADGE_MIN_WIDTH = 72         # 输入框徽标最小宽度
INPUT_BADGE_HEIGHT = 34           # 输入框徽标高度
INPUT_BADGE_RADIUS = 10            # 输入框徽标圆角半径
BUTTON_HEIGHT = 40                # 按钮高度
CONFIG_PATH_MIN_HEIGHT = 76        # 路径显示区最小高度，略增以减轻拥挤感
INPUT_VISIBLE_LINES = 20           # 输入框默认可见行数，收紧顶部输入区高度以贴齐左列
INPUT_EDIT_RADIUS = 14             # 输入框圆角半径
INPUT_EDIT_PADDING = 16            # 输入框内边距
LOG_EDIT_RADIUS = 14               # 日志区域圆角半径
LOG_EDIT_PADDING = 14              # 日志区域内边距
LOG_PANEL_MIN_HEIGHT = 180         # 日志面板最小高度
DEFAULT_REVIEW_DAYS = 30          # 默认查询天数

# ============================================================================
# 差评匹配算法配置
# ============================================================================

# 评分权重（总分 100 分）
SCORE_WEIGHTS = {
    "nickname": 10,        # 买家昵称在评价阶段可能已被用户修改，只作为辅助维度
    "sku": 30,             # 规格信息一致性（主维度）
    "reference_time": 35,  # 评价时间与收货/签收时间贴合度（主维度）
    "create_time": 20,     # 评价时间与下单时间合理性（主维度）
    "order_status": 5,     # 订单状态可靠性
}

# 匹配阈值
MATCH_MIN_SCORE = 52              # 达到该分数才认为"可匹配"
AUTO_FILL_SCORE_THRESHOLD = 80    # 达到该分数才自动填入订单号，低于该分数需要人工核对

# 多候选订单竞争扣分参数
MULTI_ORDER_PENALTY_FACTOR = 1.5       # 多单竞争时效劣势扣分系数
MULTI_ORDER_PENALTY_MAX = 12            # 多单竞争最大扣分

# 评价匹配时间窗口（天）
EVALUATION_MAX_DAYS = 30                # 普通订单评价有效天数
EDUCATION_ORDER_MAX_DAYS = 60           # 教育订单评价有效天数

# API 请求参数
FETCH_PAGE_INTERVAL_SECONDS = 0.3       # 翻页请求间隔（秒）
ORDER_PAGE_SIZE = 100                    # 订单每页大小
EVALUATION_MAX_PAGES = 10               # 差评搜索最大页数
RATE_LIMIT_RETRY_COUNT = 3              # 频率限制重试次数
ORDER_WINDOW_WORKERS = 3                # 订单时间分片并发 worker 数
ORDER_RISK_WINDOW_WORKERS = 1           # 风控降级模式下的窗口并发数
ORDER_RISK_PAGE_INTERVAL_SECONDS = 2.0  # 风控降级模式下的翻页间隔（秒）
ORDER_CACHE_SCOPE = "orders_30d"        # 最近 30 天持久缓存作用域
ORDER_CACHE_COVERAGE_DAYS = 30          # 本地订单缓存覆盖天数
ORDER_CACHE_INCREMENTAL_DAYS = 3        # 本地缓存默认增量刷新天数
ORDER_CACHE_INCREMENTAL_OVERLAP_DAYS = 1  # 增量刷新安全重叠天数
ORDER_CACHE_DB_NAME = "order_cache.sqlite3"  # 本地订单缓存数据库文件名

# 字体方案（统一管理各场景字体大小）
FONT_SIZES = {
    "title": 24,           # 主标题（窗口标题）
    "section": 15,         # 卡片标题（配置目录、订单获取等）
    "section_log": 16,     # 日志卡片标题
    "badge": 13,           # 徽标文字（授权状态、配置状态）
    "body": 12,            # 正文/路径显示
    "button": 13,          # 按钮文字
    "secondary": 11,       # 副文本/说明/帮助
    "hint": 10,            # 提示文字
}

# 颜色方案
APP_COLORS = {
    "window_base": "#3A3D38",     # 主窗口最底层背景色（深灰绿）
    "bg": "#ECFDF5",              # 页面背景色（淡绿）
    "surface": "#FFFFFF",         # 卡片/表面背景色（白色）
    "surface_soft": "#F0FDF4",    # 柔和表面背景色（淡绿白）
    "border": "#A7F3D0",          # 普通边框色（淡绿）
    "border_strong": "#6EE7B7",   # 强调边框色（亮绿）
    "input_border": "#A7F3D0",    # 输入框边框色
    "input_border_focus": "#059669",  # 输入框聚焦边框色（深绿）
    "text": "#064E3B",            # 主文字颜色（深绿）
    "heading": "#022C22",        # 标题文字颜色（最深绿）
    "muted": "#047857",           # 辅助文字颜色（中绿）
    "muted_soft": "#059669",     # 柔和辅助文字色
    "blue": "#059669",           # 主蓝色（实际为绿色，用于按钮等）
    "blue_deep": "#047857",      # 深蓝色（悬停状态）
    "blue_soft": "#D1FAE5",     # 浅蓝色（背景）
    "blue_tint": "#A7F3D0",     # 淡蓝色（标签背景）
    "hero_border": "#6EE7B7",    # 标题卡片边框色
    "orange": "#F97316",         # 橙色（警告/进行中）
    "orange_deep": "#EA580C",    # 深橙色
    "green": "#059669",          # 成功绿色（与 blue 相同）
    "green_soft": "#D1FAE5",    # 浅成功绿（背景）
    "red": "#B91C1C",            # 错误/失败红色
    "red_soft": "#FEE2E2",      # 浅红色（背景）
    "neutral_bg": "#F1F5F9",     # 中性背景色（灰白）
    "neutral_text": "#64748B",   # 中性文字色（灰）
    "neutral_border": "#CBD5E1", # 中性边框色（灰蓝）
    "input_bg": "#FFFFFF",       # 输入框背景色（同 surface）
    # 语义化颜色（用于统一引用）
    "section_title": "#064E3B",  # 卡片标题颜色（同 text）
    "body_text": "#064E3B",      # 正文颜色（同 text）
}

# 配置文件
CONFIG_DIR_NAME = ".tls-shipinhao"      # 配置目录名称（隐藏文件夹）
COOKIE_FILE_NAME = "cookie.txt"         # Cookie 文件名
COOKIE_FILE_STEM = "cookie"             # Cookie 文件词干（用于日志等）
USER_CONFIG_POINTER = "selected_config_dir.txt"  # 用户配置的目录指针文件

# 微信小商店 API
ORDER_DETAIL_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"  # 订单详情接口
ORDER_DELIVERY_UPDATE_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateOrderDeliveryInfo"  # 物流更新接口
DELIVERY_MISMATCH_MESSAGE = "快递单号与所选物流商不匹配"  # 物流不匹配错误提示
EVALUATION_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeevaluation/cgi/search"  # 评价搜索接口
ORDER_SEARCH_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/list/cgi/orderSearch"  # 订单搜索接口
QUALITY_REFUND_ORDER_URL = "https://store.weixin.qq.com/shop-faas/statistic/dsr/product/refund/order"  # 品质退款订单接口

# 卡密验证后端 API
LICENSE_API_BASE_URL = "https://sphapi.199908.top"  # 授权 API 基础地址
LICENSE_ACTIVATE_URL = f"{LICENSE_API_BASE_URL}/api/activate"  # 激活接口
LICENSE_VERIFY_URL = f"{LICENSE_API_BASE_URL}/api/verify"     # 验证接口
LICENSE_API_TIMEOUT = 15                                              # 授权 API 超时时间（秒）


_UI_SCALE = 1.0                      # 全局 UI 缩放系数（运行时）


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
    """按当前 UI 缩放系数返回像素值。

    Args:
        value: 设计稿像素值
        min_value: 最小返回值（确保在小屏幕上不至于过小）

    Returns:
        缩放后的像素值
    """
    return max(min_value, int(round(value * _UI_SCALE)))


def get_platform_default_window_size():
    """按平台返回默认窗口尺寸。

    Returns:
        (宽度, 高度) 元组
    """
    if sys.platform.startswith("win"):
        return WINDOWS_DEFAULT_WINDOW_WIDTH, WINDOWS_DEFAULT_WINDOW_HEIGHT
    return DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT
