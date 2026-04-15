# -*- coding: utf-8 -*-
"""统一消息弹窗辅助。"""

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QDialog, QHBoxLayout, QLabel, QMessageBox, QPushButton, QStyle, QVBoxLayout, QWidget

from settings import APP_COLORS
from ui.widgets import (
    get_dialog_action_spacing,
    get_dialog_content_margins,
    get_dialog_section_spacing,
    get_dialog_text_spacing,
)


class MessagePresenter:
    """统一管理消息弹窗与动作弹窗。"""

    def __init__(self, parent):
        self.parent = parent

    def style_message_box(self, dialog):
        dialog.setStyleSheet(
            """
            QDialog#AppMessageDialog {
                background: """ + APP_COLORS["bg"] + """;
                border: 1px solid """ + APP_COLORS["border"] + """;
                border-radius: 14px;
            }
            QLabel#MessageTitle {
                color: """ + APP_COLORS["heading"] + """;
                font-size: 18px;
                font-weight: 700;
            }
            QLabel#MessageText {
                color: """ + APP_COLORS["text"] + """;
                font-size: 15px;
                line-height: 1.45;
            }
            QLabel#MessageInfo {
                color: """ + APP_COLORS["muted"] + """;
                font-size: 13px;
                line-height: 1.45;
            }
            QPushButton#MessagePrimary {
                background: """ + APP_COLORS["blue"] + """;
                color: #FFFFFF;
                border: 1px solid """ + APP_COLORS["blue_deep"] + """;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 700;
            }
            QPushButton#MessagePrimary:hover {
                background: """ + APP_COLORS["blue_deep"] + """;
            }
            QPushButton#MessagePrimary:pressed {
                background: """ + APP_COLORS["blue_deep"] + """;
            }
            QPushButton#MessageSecondary {
                background: """ + APP_COLORS["neutral_bg"] + """;
                color: """ + APP_COLORS["text"] + """;
                border: 1px solid """ + APP_COLORS["neutral_border"] + """;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 600;
            }
            QPushButton#MessageSecondary:hover {
                background: """ + APP_COLORS["border"] + """;
            }
            QPushButton#MessageSecondary:pressed {
                background: """ + APP_COLORS["border_strong"] + """;
            }
            """
        )
        return dialog

    def message_icon_pixmap(self, level):
        icon_map = {
            QMessageBox.Information: QStyle.SP_MessageBoxInformation,
            QMessageBox.Warning: QStyle.SP_MessageBoxWarning,
            QMessageBox.Critical: QStyle.SP_MessageBoxCritical,
            QMessageBox.Question: QStyle.SP_MessageBoxQuestion,
        }
        icon_type = icon_map.get(level, QStyle.SP_MessageBoxInformation)
        return self.parent.style().standardIcon(icon_type).pixmap(46, 46)

    def create_message_dialog_base(self, level, title, text, informative_text="", *, min_width=560):
        dialog = QDialog(self.parent)
        dialog.setObjectName("AppMessageDialog")
        dialog.setWindowTitle(title)
        dialog.setModal(True)
        dialog.setMinimumWidth(min_width)

        dialog.root_layout = QVBoxLayout(dialog)
        dialog.root_layout.setContentsMargins(*get_dialog_content_margins())
        dialog.root_layout.setSpacing(get_dialog_section_spacing())

        dialog.body_layout = QHBoxLayout()
        dialog.body_layout.setSpacing(get_dialog_section_spacing())

        icon_label = QLabel()
        icon_label.setPixmap(self.message_icon_pixmap(level))
        icon_label.setAlignment(Qt.AlignTop | Qt.AlignHCenter)
        icon_label.setFixedWidth(56)
        dialog.body_layout.addWidget(icon_label)

        text_wrap = QWidget()
        dialog.text_layout = QVBoxLayout(text_wrap)
        dialog.text_layout.setContentsMargins(0, 0, 0, 0)
        dialog.text_layout.setSpacing(get_dialog_text_spacing())

        title_label = QLabel(title)
        title_label.setObjectName('MessageTitle')
        title_label.setWordWrap(True)
        dialog.text_layout.addWidget(title_label)

        text_label = QLabel(text)
        text_label.setObjectName('MessageText')
        text_label.setWordWrap(True)
        dialog.text_layout.addWidget(text_label)

        if informative_text:
            info_label = QLabel(informative_text)
            info_label.setObjectName('MessageInfo')
            info_label.setWordWrap(True)
            dialog.text_layout.addWidget(info_label)

        dialog.body_layout.addWidget(text_wrap, 1)
        dialog.root_layout.addLayout(dialog.body_layout, 1)

        dialog.actions_layout = QHBoxLayout()
        dialog.actions_layout.setContentsMargins(0, 0, 0, 0)
        dialog.actions_layout.setSpacing(get_dialog_action_spacing())
        dialog.actions_layout.addStretch(1)
        dialog.root_layout.addLayout(dialog.actions_layout)

        self.style_message_box(dialog)
        return dialog, dialog.actions_layout

    @staticmethod
    def add_message_action(actions_layout, text, object_name, callback):
        button = QPushButton(text)
        button.setObjectName(object_name)
        button.clicked.connect(callback)
        actions_layout.addWidget(button)
        return button

    def add_message_actions(self, actions_layout, action_specs):
        return [self.add_message_action(actions_layout, text, object_name, callback) for text, object_name, callback in action_specs]

    def show_action_dialog(self, level, title, text, informative_text="", *, min_width=560, action_specs=()):
        dialog, actions = self.create_message_dialog_base(level, title, text, informative_text, min_width=min_width)

        def _wrap_callback(callback):
            def _handler():
                result = callback()
                if result in (QDialog.Accepted, QDialog.Rejected):
                    dialog.done(result)
                else:
                    dialog.accept()
            return _handler

        wrapped_specs = [(button_text, object_name, _wrap_callback(callback)) for button_text, object_name, callback in action_specs]
        self.add_message_actions(actions, wrapped_specs)
        return dialog.exec()

    def build_message_dialog(self, level, title, text, informative_text=""):
        dialog, actions = self.create_message_dialog_base(level, title, text, informative_text, min_width=560)
        self.add_message_actions(actions, (("确定", "MessagePrimary", dialog.accept),))
        return dialog

    def show_message(self, level, title, text, informative_text=""):
        dialog = self.build_message_dialog(level, title, text, informative_text)
        dialog.exec()
