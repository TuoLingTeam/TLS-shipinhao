# -*- coding: utf-8 -*-
"""TLS-shipinhao 自定义控件与字体工具。"""

from functools import lru_cache

from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFontDatabase
from PySide6.QtWidgets import (
    QApplication,
    QDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QPushButton,
    QVBoxLayout,
)

from settings import (
    APP_COLORS,
    AUTHOR_WECHAT,
    get_ui_scale,
    normalize_batch_text,
    scale_px,
)
from core.license import activate_license


# ---------------------------------------------------------------------------
# 字体工具
# ---------------------------------------------------------------------------


@lru_cache(maxsize=64)
def build_font(size, bold=False):
    """获取通用字体（带缓存，避免重复创建）。"""
    font = QFontDatabase.systemFont(QFontDatabase.GeneralFont)
    font.setPointSize(_scale_font_size(size))
    font.setBold(bold)
    return font


@lru_cache(maxsize=32)
def build_fixed_font(size):
    """获取等宽字体（带缓存，避免重复创建）。"""
    font = QFontDatabase.systemFont(QFontDatabase.FixedFont)
    font.setPointSize(_scale_font_size(size))
    return font


def _scale_font_size(size):
    """按当前 UI 缩放系数返回字体大小。"""
    return max(9, int(round(size * get_ui_scale())))


def reset_font_caches():
    """缩放系数变化后清理字体缓存。"""
    build_font.cache_clear()
    build_fixed_font.cache_clear()


def get_dialog_content_margins():
    """统一弹窗内容区外边距。"""
    return (
        scale_px(22, min_value=16),
        scale_px(18, min_value=14),
        scale_px(22, min_value=16),
        scale_px(18, min_value=14),
    )


def _scaled_dialog_spacing(base: int, *, min_value: int) -> int:
    """统一弹窗间距换算。"""
    return scale_px(base, min_value=min_value)


def get_dialog_section_spacing():
    """统一弹窗主内容分组间距。"""
    return _scaled_dialog_spacing(14, min_value=10)


def get_dialog_text_spacing():
    """统一弹窗文案堆叠间距。"""
    return _scaled_dialog_spacing(8, min_value=6)


def get_dialog_action_spacing():
    """统一弹窗按钮区间距。"""
    return _scaled_dialog_spacing(10, min_value=8)


# ---------------------------------------------------------------------------
# 许可证状态文案
# ---------------------------------------------------------------------------


LICENSE_REASON_TEXTS = {
    "expired": "授权已到期，请购买卡密激活使用。",
    "device_mismatch": "当前设备与激活授权设备不一致！",
    "invalid": "本地授权异常，请重新输入卡密激活。",
    "not_found": "未激活，请购买卡密激活使用。",
    "reactivation_required": "授权协议升级！请重新用卡密激活。",
    "online_refresh_required": "当前设备需要联网刷新授权票据后才能继续执行核心任务。",
    "revoked": "当前卡密已被吊销，请联系作者处理。",
}


def get_license_reason_text(reason):
    """将许可证状态码映射为可读提示。"""
    return LICENSE_REASON_TEXTS.get(reason, "授权状态未知，请输入卡密激活。")


# ---------------------------------------------------------------------------
# 批量输入框
# ---------------------------------------------------------------------------


class BatchInputEdit(QPlainTextEdit):
    """批量输入框。"""

    normalized = Signal()

    def __init__(self, placeholder, parent=None):
        super().__init__(parent)
        self.setPlaceholderText(placeholder)
        self.setTabChangesFocus(True)
        self.setObjectName("InputEdit")
        self.setFont(build_fixed_font(13))

    def normalize_content(self):
        """整理输入框内容。"""
        normalized_text = normalize_batch_text(self.toPlainText())
        current_text = self.toPlainText().strip()
        if normalized_text == current_text:
            return
        self.blockSignals(True)
        self.setPlainText(normalized_text)
        self.blockSignals(False)
        self.normalized.emit()

    def focusOutEvent(self, event):
        """失焦时自动清理多余空格和空白行。"""
        self.normalize_content()
        super().focusOutEvent(event)


# ---------------------------------------------------------------------------
# 卡密激活弹窗
# ---------------------------------------------------------------------------


class LicenseDialog(QDialog):
    """离线卡密激活弹窗。"""

    def __init__(self, parent=None, reason="not_found"):
        super().__init__(parent)
        self.activated = False
        self.reason = reason
        self._build_ui()

    def _build_ui(self):
        self.setWindowTitle("卡密激活")
        self.setModal(True)
        self.setObjectName("LicenseDialog")
        self.setMinimumWidth(560)
        self.setStyleSheet(
            """
            QDialog#LicenseDialog {
                background: """ + APP_COLORS["bg"] + """;
                border: 1px solid """ + APP_COLORS["border"] + """;
                border-radius: 14px;
            }
            QLabel#LicenseTitle {
                color: """ + APP_COLORS["heading"] + """;
                font-size: 19px;
                font-weight: 700;
            }
            QLabel#LicenseDesc {
                color: """ + APP_COLORS["muted"] + """;
                font-size: 14px;
                line-height: 1.45;
            }
            QLabel#LicenseHint {
                color: """ + APP_COLORS["blue"] + """;
                font-size: 14px;
                font-weight: 700;
            }
            QLineEdit#LicenseInput {
                background: """ + APP_COLORS["input_bg"] + """;
                color: """ + APP_COLORS["text"] + """;
                border: 1px solid """ + APP_COLORS["input_border"] + """;
                border-radius: 10px;
                padding: 10px 12px;
                font-size: 15px;
            }
            QLineEdit#LicenseInput:focus {
                border: 2px solid """ + APP_COLORS["input_border_focus"] + """;
            }
            QLabel#LicenseMessage {
                font-size: 13px;
            }
            QPushButton#LicensePrimary {
                background: """ + APP_COLORS["blue"] + """;
                color: #FFFFFF;
                border: 1px solid """ + APP_COLORS["blue_deep"] + """;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 110px;
                font-weight: 700;
            }
            QPushButton#LicensePrimary:hover {
                background: """ + APP_COLORS["blue_deep"] + """;
            }
            QPushButton#LicenseSecondary {
                background: """ + APP_COLORS["neutral_bg"] + """;
                color: """ + APP_COLORS["text"] + """;
                border: 1px solid """ + APP_COLORS["neutral_border"] + """;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 110px;
                font-weight: 600;
            }
            QPushButton#LicenseSecondary:hover {
                background: """ + APP_COLORS["border"] + """;
            }
            """
        )

        self.root_layout = QVBoxLayout(self)
        self.root_layout.setContentsMargins(*get_dialog_content_margins())
        self.root_layout.setSpacing(get_dialog_section_spacing())

        title = QLabel("请输入卡密激活软件")
        title.setObjectName("LicenseTitle")
        self.root_layout.addWidget(title)

        desc = QLabel(get_license_reason_text(self.reason))
        desc.setObjectName("LicenseDesc")
        desc.setWordWrap(True)
        self.root_layout.addWidget(desc)

        self.wechat_label = QLabel(f"联系作者微信：{AUTHOR_WECHAT}（点击复制）")
        self.wechat_label.setObjectName("LicenseHint")
        self.wechat_label.setCursor(Qt.PointingHandCursor)
        self.wechat_label.mousePressEvent = self._copy_wechat
        self.root_layout.addWidget(self.wechat_label)

        self.key_input = QLineEdit()
        self.key_input.setObjectName("LicenseInput")
        self.key_input.setPlaceholderText("例如：TLS-XXXX-XXXX-XXXX-XXXX")
        self.key_input.returnPressed.connect(self._on_activate_clicked)
        self.root_layout.addWidget(self.key_input)

        self.message_label = QLabel("")
        self.message_label.setObjectName("LicenseMessage")
        self.message_label.setWordWrap(True)
        self.message_label.setStyleSheet("color: " + APP_COLORS["red"] + ";")
        self.root_layout.addWidget(self.message_label)

        self.action_row_layout = QHBoxLayout()
        self.action_row_layout.setContentsMargins(0, 0, 0, 0)
        self.action_row_layout.setSpacing(get_dialog_action_spacing())
        self.action_row_layout.addStretch(1)
        self.root_layout.addLayout(self.action_row_layout)

        cancel_button = QPushButton("取消")
        cancel_button.setObjectName("LicenseSecondary")
        cancel_button.clicked.connect(self.reject)
        self.action_row_layout.addWidget(cancel_button)

        activate_button = QPushButton("激活")
        activate_button.setObjectName("LicensePrimary")
        activate_button.clicked.connect(self._on_activate_clicked)
        self.action_row_layout.addWidget(activate_button)

        self.key_input.setFocus()

    def _copy_wechat(self, _event):
        clipboard = QApplication.clipboard()
        clipboard.setText(AUTHOR_WECHAT)
        self._set_message(f"已复制作者微信：{AUTHOR_WECHAT}", APP_COLORS["green"])

    def _set_message(self, text, color):
        """更新弹窗提示文案与颜色。"""
        self.message_label.setText(text)
        self.message_label.setStyleSheet("color: " + color + ";")

    def _on_activate_clicked(self):
        key = self.key_input.text().strip()
        if not key:
            self._set_message("请输入卡密。", APP_COLORS["red"])
            return
        try:
            info = activate_license(key)
        except ValueError as exc:
            self._set_message(str(exc), APP_COLORS["red"])
            return
        except Exception as exc:  # noqa: BLE001
            self._set_message(f"激活失败：{exc}", APP_COLORS["red"])
            return

        self.activated = True
        expires = str(info.get("expires_at", ""))[:19]
        self._set_message(f"激活成功，有效期至：{expires}", APP_COLORS["green"])
        self.accept()
