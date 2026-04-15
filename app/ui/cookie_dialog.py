# -*- coding: utf-8 -*-
"""基于 QtWebEngine 的 Cookie 自动获取对话框。"""

import os

from PySide6.QtCore import Qt, QUrl
from PySide6.QtWidgets import (
    QDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)

from settings import extract_biz_magic_from_cookie
from settings import APP_COLORS, BUTTON_HEIGHT
from ui.widgets import build_font

try:
    from PySide6.QtWebEngineCore import QWebEnginePage, QWebEngineProfile
    from PySide6.QtWebEngineWidgets import QWebEngineView
except Exception as exc:  # noqa: BLE001
    QTWEBENGINE_AVAILABLE = False
    QTWEBENGINE_IMPORT_ERROR = f"{type(exc).__name__}: {exc}"
    QWebEnginePage = None
    QWebEngineProfile = None
    QWebEngineView = None
else:
    QTWEBENGINE_AVAILABLE = True
    QTWEBENGINE_IMPORT_ERROR = ""


WECHAT_STORE_HOME_URL = "https://store.weixin.qq.com/"
TARGET_COOKIE_DOMAIN_KEYWORD = "weixin.qq.com"


def _decode_cookie_value(value):
    """将 Qt 的 cookie 字节字段转换为字符串。"""
    try:
        return bytes(value).decode("utf-8", errors="ignore").strip()
    except Exception:  # noqa: BLE001
        return str(value).strip()


class CookieCaptureDialog(QDialog):
    """打开内置网页登录页并实时捕获 Cookie。"""

    def __init__(self, config_dir, parent=None):
        if not QTWEBENGINE_AVAILABLE:
            raise RuntimeError(QTWEBENGINE_IMPORT_ERROR or "QtWebEngine 不可用。")

        super().__init__(parent)
        self._config_dir = os.path.abspath(config_dir)
        self._cookies = {}
        self._status_suffix = ""

        self.setWindowTitle("自动获取 Cookie")
        self.setModal(True)
        self.resize(980, 760)
        self.setMinimumSize(860, 620)
        self.setStyleSheet(self._build_stylesheet())

        self._build_ui()
        self._build_browser()
        self._refresh_cookie_state()
        self._load_home_page()

    @property
    def cookie_data(self):
        """返回当前已捕获的 Cookie 字典。"""
        return dict(self._cookies)

    def _build_stylesheet(self):
        """构建对话框样式。"""
        return f"""
            QDialog {{
                background: {APP_COLORS["bg"]};
            }}
            QLabel#CookieTitle {{
                color: {APP_COLORS["heading"]};
            }}
            QFrame#CookiePromptCard {{
                background: {APP_COLORS["surface"]};
                border: 1px solid {APP_COLORS["border_strong"]};
                border-radius: 12px;
            }}
            QLabel#CookieHint {{
                color: {APP_COLORS["muted"]};
            }}
            QLabel#CookieStatus {{
                color: {APP_COLORS["blue_deep"]};
            }}
            QPushButton#CookieSecondary {{
                background: {APP_COLORS["surface"]};
                color: {APP_COLORS["blue_deep"]};
                border: 1px solid {APP_COLORS["blue_tint"]};
                border-radius: 12px;
                padding: 10px 16px;
                font-weight: 700;
            }}
            QPushButton#CookieSecondary:hover {{
                background: {APP_COLORS["blue_soft"]};
                border-color: {APP_COLORS["border_strong"]};
            }}
            QPushButton#CookiePrimary {{
                background: {APP_COLORS["orange"]};
                color: white;
                border: 1px solid {APP_COLORS["orange_deep"]};
                border-radius: 12px;
                padding: 10px 18px;
                font-weight: 700;
            }}
            QPushButton#CookiePrimary:hover {{
                background: {APP_COLORS["orange_deep"]};
            }}
            QPushButton#CookiePrimary:disabled {{
                background: {APP_COLORS["neutral_bg"]};
                color: {APP_COLORS["neutral_text"]};
                border: 1px solid {APP_COLORS["neutral_border"]};
            }}
        """

    def _build_ui(self):
        """创建对话框骨架。"""
        root = QVBoxLayout(self)
        root.setContentsMargins(16, 16, 16, 16)
        root.setSpacing(10)

        header = QWidget()
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(10)

        title_label = QLabel("自动获取微信小店 Cookie")
        title_label.setObjectName("CookieTitle")
        title_label.setFont(build_font(18, bold=True))
        header_layout.addWidget(title_label, 1)

        self.cancel_button = self._create_button("关闭", "CookieSecondary")
        self.cancel_button.clicked.connect(self.reject)
        header_layout.addWidget(self.cancel_button)

        self.save_button = self._create_button("保存 Cookie", "CookiePrimary")
        self.save_button.clicked.connect(self._accept_with_cookie)
        self.save_button.setEnabled(False)
        self.save_button.setToolTip(f"保存到：{self._config_dir}/cookie.txt")
        header_layout.addWidget(self.save_button)

        root.addWidget(header)

        prompt_card = QFrame()
        prompt_card.setObjectName("CookiePromptCard")
        card_layout = QVBoxLayout(prompt_card)
        card_layout.setContentsMargins(16, 12, 16, 12)
        card_layout.setSpacing(8)

        hint_label = QLabel(
            "在下方页面完成扫码登录；点击「保存 Cookie」后将选择保存目录，软件会从该目录读取 cookie.txt。"
        )
        hint_label.setObjectName("CookieHint")
        hint_label.setWordWrap(True)
        hint_label.setFont(build_font(11))
        card_layout.addWidget(hint_label)

        self.status_label = QLabel()
        self.status_label.setObjectName("CookieStatus")
        self.status_label.setWordWrap(True)
        self.status_label.setFont(build_font(11))
        card_layout.addWidget(self.status_label)

        root.addWidget(prompt_card)

        self.browser_container = QWidget()
        self.browser_container.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
        self.browser_layout = QVBoxLayout(self.browser_container)
        self.browser_layout.setContentsMargins(0, 0, 0, 0)
        self.browser_layout.setSpacing(0)
        root.addWidget(self.browser_container, 1)

    def _build_browser(self):
        """初始化 QWebEngine 视图和 Cookie 监听。"""
        self.profile = QWebEngineProfile(self)
        self.page = QWebEnginePage(self.profile, self)
        self.web_view = QWebEngineView(self)
        self.web_view.setPage(self.page)
        self.web_view.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Expanding)
        self.browser_layout.addWidget(self.web_view, 1)

        self.profile.cookieStore().cookieAdded.connect(self._on_cookie_added)
        self.profile.cookieStore().cookieRemoved.connect(self._on_cookie_removed)
        self.web_view.loadStarted.connect(self._on_load_started)
        self.web_view.loadFinished.connect(self._on_load_finished)

    def _create_button(self, text, object_name):
        """创建对话框按钮。"""
        button = QPushButton(text)
        button.setObjectName(object_name)
        button.setCursor(Qt.PointingHandCursor)
        button.setFont(build_font(11, bold=True))
        button.setFixedHeight(BUTTON_HEIGHT)
        return button

    def _load_home_page(self):
        """打开微信小店首页。"""
        self.web_view.setUrl(QUrl(WECHAT_STORE_HOME_URL))

    def _has_login_cookie(self):
        """当前是否已抓到可用登录态。"""
        return bool(extract_biz_magic_from_cookie(self._cookies))

    def _accept_with_cookie(self):
        """确认使用当前抓到的 Cookie。"""
        if not self._has_login_cookie():
            return
        self.accept()

    def _set_status_suffix(self, suffix):
        """统一更新页面状态附加文案。"""
        self._status_suffix = suffix
        self._refresh_cookie_state()

    def _on_load_started(self):
        """页面开始加载时刷新状态。"""
        self._set_status_suffix("页面加载中，请稍候…")

    def _on_load_finished(self, ok):
        """页面加载完成后刷新状态。"""
        if ok:
            self._set_status_suffix("")
            return
        self._set_status_suffix("页面加载失败，请检查网络后重试。")

    def _update_cookie_store(self, cookie, *, remove=False):
        """同步更新目标域名下的 Cookie 缓存。"""
        if not self._is_target_cookie(cookie):
            return

        name = _decode_cookie_value(cookie.name())
        if not name:
            return

        if remove:
            self._cookies.pop(name, None)
        else:
            self._cookies[name] = _decode_cookie_value(cookie.value())
        self._refresh_cookie_state()

    def _on_cookie_added(self, cookie):
        """记录目标域名下新增的 Cookie。"""
        self._update_cookie_store(cookie)

    def _on_cookie_removed(self, cookie):
        """同步删除被移除的 Cookie。"""
        self._update_cookie_store(cookie, remove=True)

    def _is_target_cookie(self, cookie):
        """仅保留微信小店相关域名的 Cookie。"""
        domain = _decode_cookie_value(cookie.domain()).lstrip(".").lower()
        if not domain:
            return False
        return (
            domain == TARGET_COOKIE_DOMAIN_KEYWORD
            or domain.endswith(f".{TARGET_COOKIE_DOMAIN_KEYWORD}")
        )

    def _refresh_cookie_state(self):
        """刷新 Cookie 捕获状态文案。"""
        if self._has_login_cookie():
            base_text = "已检测到登录态，点击右上角「保存 Cookie」后选择保存目录即可。"
            self.save_button.setEnabled(True)
        else:
            base_text = "请先完成扫码或确认登录。"
            self.save_button.setEnabled(False)

        if self._status_suffix:
            self.status_label.setText(f"{base_text} {self._status_suffix}")
        else:
            self.status_label.setText(base_text)
