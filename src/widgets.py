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

from .config import normalize_batch_text
from .constants import AUTHOR_WECHAT

try:
    from .license import activate_license
except ImportError:  # pragma: no cover
    from license import activate_license


# ---------------------------------------------------------------------------
# 字体工具
# ---------------------------------------------------------------------------


@lru_cache(maxsize=64)
def build_font(size, bold=False):
    """获取通用字体（带缓存，避免重复创建）。"""
    font = QFontDatabase.systemFont(QFontDatabase.GeneralFont)
    font.setPointSize(size)
    font.setBold(bold)
    return font


@lru_cache(maxsize=32)
def build_fixed_font(size):
    """获取等宽字体（带缓存，避免重复创建）。"""
    font = QFontDatabase.systemFont(QFontDatabase.FixedFont)
    font.setPointSize(size)
    return font


# ---------------------------------------------------------------------------
# 许可证状态文案
# ---------------------------------------------------------------------------


def get_license_reason_text(reason):
    """将许可证状态码映射为可读提示。"""
    reason_map = {
        "expired": "当前授权已到期，请输入新卡密继续使用。",
        "device_mismatch": "当前设备与授权设备不一致，请重新激活。",
        "invalid": "本地授权文件异常，请重新输入卡密激活。",
        "not_found": "尚未激活，请输入卡密开始使用。",
    }
    return reason_map.get(reason, "授权状态未知，请重新输入卡密激活。")


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
                background: #0A1C36;
                border: 1px solid #1E3A8A;
                border-radius: 14px;
            }
            QLabel#LicenseTitle {
                color: #F8FAFC;
                font-size: 19px;
                font-weight: 700;
            }
            QLabel#LicenseDesc {
                color: #CFE0F5;
                font-size: 14px;
                line-height: 1.45;
            }
            QLabel#LicenseHint {
                color: #22D3EE;
                font-size: 14px;
                font-weight: 700;
            }
            QLineEdit#LicenseInput {
                background: #0F2748;
                color: #F8FAFC;
                border: 1px solid #3B82F6;
                border-radius: 10px;
                padding: 10px 12px;
                font-size: 15px;
            }
            QLineEdit#LicenseInput:focus {
                border: 2px solid #60A5FA;
            }
            QLabel#LicenseMessage {
                font-size: 13px;
            }
            QPushButton#LicensePrimary {
                background: #1D4ED8;
                color: #F8FAFC;
                border: 1px solid #3B82F6;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 110px;
                font-weight: 700;
            }
            QPushButton#LicensePrimary:hover {
                background: #2563EB;
            }
            QPushButton#LicenseSecondary {
                background: rgba(148, 163, 184, 0.18);
                color: #EAF2FC;
                border: 1px solid #64748B;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 110px;
                font-weight: 600;
            }
            QPushButton#LicenseSecondary:hover {
                background: rgba(148, 163, 184, 0.28);
            }
            """
        )

        root = QVBoxLayout(self)
        root.setContentsMargins(22, 18, 22, 18)
        root.setSpacing(14)

        title = QLabel("请输入卡密激活软件")
        title.setObjectName("LicenseTitle")
        root.addWidget(title)

        desc = QLabel(get_license_reason_text(self.reason))
        desc.setObjectName("LicenseDesc")
        desc.setWordWrap(True)
        root.addWidget(desc)

        self.wechat_label = QLabel(f"联系作者微信：{AUTHOR_WECHAT}（点击复制）")
        self.wechat_label.setObjectName("LicenseHint")
        self.wechat_label.setCursor(Qt.PointingHandCursor)
        self.wechat_label.mousePressEvent = self._copy_wechat
        root.addWidget(self.wechat_label)

        self.key_input = QLineEdit()
        self.key_input.setObjectName("LicenseInput")
        self.key_input.setPlaceholderText("例如：TLS-XXXX-XXXX-XXXX-XXXX")
        self.key_input.returnPressed.connect(self._on_activate_clicked)
        root.addWidget(self.key_input)

        self.message_label = QLabel("")
        self.message_label.setObjectName("LicenseMessage")
        self.message_label.setWordWrap(True)
        self.message_label.setStyleSheet("color: #FCA5A5;")
        root.addWidget(self.message_label)

        action_row = QHBoxLayout()
        action_row.setContentsMargins(0, 0, 0, 0)
        action_row.setSpacing(10)
        action_row.addStretch(1)
        root.addLayout(action_row)

        cancel_button = QPushButton("取消")
        cancel_button.setObjectName("LicenseSecondary")
        cancel_button.clicked.connect(self.reject)
        action_row.addWidget(cancel_button)

        activate_button = QPushButton("激活")
        activate_button.setObjectName("LicensePrimary")
        activate_button.clicked.connect(self._on_activate_clicked)
        action_row.addWidget(activate_button)

        self.key_input.setFocus()

    def _copy_wechat(self, _event):
        clipboard = QApplication.clipboard()
        clipboard.setText(AUTHOR_WECHAT)
        self.message_label.setText(f"已复制作者微信：{AUTHOR_WECHAT}")
        self.message_label.setStyleSheet("color: #86EFAC;")

    def _on_activate_clicked(self):
        key = self.key_input.text().strip()
        if not key:
            self.message_label.setText("请输入卡密。")
            self.message_label.setStyleSheet("color: #FCA5A5;")
            return
        try:
            info = activate_license(key)
        except ValueError as exc:
            self.message_label.setText(str(exc))
            self.message_label.setStyleSheet("color: #FCA5A5;")
            return
        except Exception as exc:  # noqa: BLE001
            self.message_label.setText(f"激活失败：{exc}")
            self.message_label.setStyleSheet("color: #FCA5A5;")
            return

        self.activated = True
        expires = str(info.get("expires_at", ""))[:19]
        self.message_label.setText(f"激活成功，有效期至：{expires}")
        self.message_label.setStyleSheet("color: #86EFAC;")
        self.accept()
