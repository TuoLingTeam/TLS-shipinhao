# -*- coding: utf-8 -*-
"""TLS-shipinhao工具。"""

import json
import os
import re
import sys
import threading

import requests
from PySide6.QtCore import QObject, Qt, QThread, Signal
from PySide6.QtGui import QFont, QFontDatabase
from PySide6.QtWidgets import (
    QApplication,
    QFileDialog,
    QDialog,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPlainTextEdit,
    QPushButton,
    QScrollArea,
    QSizePolicy,
    QStyle,
    QVBoxLayout,
    QWidget,
)


MAX_BATCH_SIZE = 100
REQUEST_TIMEOUT = 30
WINDOW_TITLE = "驼铃视频小店中差评处理"
TUTORIAL_URL = "https://tuolingshe.feishu.cn/docx/BHiIdOUKxomqVgxIb1zcmIr8nLe"
DESIGN_WIDTH = 1240
DESIGN_HEIGHT = 980
MAC_DEFAULT_WINDOW_WIDTH = 1880
MAC_DEFAULT_WINDOW_HEIGHT = 1668
WINDOWS_DEFAULT_WINDOW_WIDTH = 1520
WINDOWS_DEFAULT_WINDOW_HEIGHT = 980

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

CONFIG_DIR_NAME = ".tls-shipinhao"
COOKIE_FILE_NAME = "cookie.txt"
MAGIC_FILE_NAME = "biz_magic.txt"
USER_CONFIG_POINTER = "selected_config_dir.txt"
_CONFIG_DIR_CACHE = None

ORDER_DETAIL_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
ORDER_DELIVERY_UPDATE_URL = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateOrderDeliveryInfo"
DELIVERY_MISMATCH_MESSAGE = "快递单号与所选物流商不匹配"


class ConfigNotFoundError(FileNotFoundError):
    """配置文件缺失时抛出更明确的错误。"""

    def __init__(self, searched_dirs):
        self.searched_dirs = searched_dirs
        super().__init__("未找到配置文件。")


def get_platform_default_window_size():
    """按平台返回默认窗口尺寸。"""
    if sys.platform.startswith("win"):
        return WINDOWS_DEFAULT_WINDOW_WIDTH, WINDOWS_DEFAULT_WINDOW_HEIGHT
    return MAC_DEFAULT_WINDOW_WIDTH, MAC_DEFAULT_WINDOW_HEIGHT


def get_app_dir():
    """获取 .app 同级目录或源码项目根目录。"""
    if getattr(sys, "frozen", False):
        exe_dir = os.path.abspath(os.path.dirname(sys.executable))
        if sys.platform == "darwin":
            bundle_root = os.path.abspath(os.path.join(exe_dir, "..", ".."))
            if bundle_root.endswith(".app"):
                return os.path.dirname(bundle_root)
        return exe_dir
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def get_home_config_dir():
    """获取用户主目录下的固定配置目录。"""
    return os.path.join(os.path.expanduser("~"), CONFIG_DIR_NAME)


def get_user_config_pointer_path():
    """记录用户指定配置目录的指针文件。"""
    return os.path.join(get_home_config_dir(), USER_CONFIG_POINTER)


def get_saved_user_config_dir():
    """读取用户上次选择的配置目录。"""
    pointer_path = get_user_config_pointer_path()
    if not os.path.exists(pointer_path):
        return None
    with open(pointer_path, "r", encoding="utf-8") as file:
        selected_dir = file.read().strip()
    if selected_dir and os.path.isdir(selected_dir):
        return selected_dir
    return None


def save_user_config_dir(config_dir):
    """保存用户指定的配置目录。"""
    global _CONFIG_DIR_CACHE
    config_dir = os.path.abspath(config_dir)
    os.makedirs(get_home_config_dir(), exist_ok=True)
    with open(get_user_config_pointer_path(), "w", encoding="utf-8") as file:
        file.write(config_dir)
    _CONFIG_DIR_CACHE = config_dir


def get_config_search_dirs():
    """按优先级返回配置目录搜索链路。"""
    search_dirs = []
    for candidate in (get_app_dir(), _CONFIG_DIR_CACHE or get_saved_user_config_dir(), get_home_config_dir()):
        if not candidate:
            continue
        normalized = os.path.abspath(candidate)
        if normalized not in search_dirs:
            search_dirs.append(normalized)
    return search_dirs


def resolve_config_dir():
    """解析实际可用的配置目录。"""
    global _CONFIG_DIR_CACHE
    if _CONFIG_DIR_CACHE:
        cookie_path = os.path.join(_CONFIG_DIR_CACHE, COOKIE_FILE_NAME)
        magic_path = os.path.join(_CONFIG_DIR_CACHE, MAGIC_FILE_NAME)
        if os.path.exists(cookie_path) and os.path.exists(magic_path):
            return _CONFIG_DIR_CACHE

    search_dirs = get_config_search_dirs()
    for config_dir in search_dirs:
        cookie_path = os.path.join(config_dir, COOKIE_FILE_NAME)
        magic_path = os.path.join(config_dir, MAGIC_FILE_NAME)
        if os.path.exists(cookie_path) and os.path.exists(magic_path):
            _CONFIG_DIR_CACHE = config_dir
            return config_dir

    raise ConfigNotFoundError(search_dirs)


def getCookie():
    """从 cookie.txt 文件读取 Cookie 信息。"""
    path = os.path.join(resolve_config_dir(), COOKIE_FILE_NAME)
    with open(path, "r", encoding="utf-8") as file:
        content = file.read().strip()

    pairs = content.split(";")
    data = {}
    for pair in pairs:
        if "=" in pair:
            key, value = pair.strip().split("=", 1)
            data[key.strip()] = value.strip()

    return data


def getMagic():
    """从 biz_magic.txt 文件读取 magic 值。"""
    path = os.path.join(resolve_config_dir(), MAGIC_FILE_NAME)
    with open(path, "r", encoding="utf-8") as file:
        return file.read().strip()


def build_headers(magic):
    """根据 magic 构建 HTTP 请求头。"""
    return {
        "Accept": "application/json, text/plain, */*",
        "Accept-Language": "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7",
        "Cache-Control": "no-cache",
        "Connection": "keep-alive",
        "Content-Type": "application/json",
        "Origin": "https://store.weixin.qq.com",
        "Pragma": "no-cache",
        "Sec-Fetch-Dest": "empty",
        "Sec-Fetch-Mode": "cors",
        "Sec-Fetch-Site": "same-origin",
        "User-Agent": (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
        ),
        "biz_magic": magic,
        "mcn_magic": "",
        "potter-scene": "weixinShop",
        "sec-ch-ua": '"Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144"',
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": '"Windows"',
        "supplier_magic": "",
        "talent_magic": "",
        "wecom_magic": "",
    }


def get_response_error(response):
    """尽量从接口响应里提取可读错误信息。"""
    try:
        payload = response.json()
    except ValueError:
        text = response.text.strip()
        if text:
            return f"HTTP {response.status_code}: {text[:200]}"
        return f"HTTP {response.status_code}"

    if not isinstance(payload, dict):
        return f"HTTP {response.status_code}"

    for key in ("errmsg", "message", "msg"):
        value = payload.get(key)
        if value:
            return str(value)

    errcode = payload.get("errcode")
    if errcode not in (None, 0):
        return f"错误码 {errcode}"

    return f"HTTP {response.status_code}"


def normalize_product_infos(delivery_product_info):
    """保留订单详情里的商品信息。"""
    product_infos = []
    for item in delivery_product_info.get("productInfos") or []:
        product_id = item.get("productId")
        sku_id = item.get("skuId")
        if product_id is None or sku_id is None:
            continue
        product_infos.append(
            {
                "productId": product_id,
                "skuId": sku_id,
                "productCnt": item.get("productCnt", 1),
            }
        )
    return product_infos


def get_payload_error(payload, default_message):
    """从业务响应里提取更具体的错误信息。"""
    if not isinstance(payload, dict):
        return default_message

    for key in ("errmsg", "message", "msg"):
        value = payload.get(key)
        if value:
            return str(value)

    for key in ("code", "errcode", "ret"):
        value = payload.get(key)
        if value not in (None, 0):
            return f"{default_message}（错误码 {value}）"

    return default_message


def parse_batch_input(raw_text):
    """解析批量输入，支持空格、英文逗号、中文逗号和换行。"""
    return [
        item.strip()
        for item in re.split(r"[\s,，]+", raw_text.strip())
        if item.strip()
    ]


def normalize_batch_text(raw_text):
    """将批量输入规范化为一行一个值。"""
    return "\n".join(parse_batch_input(raw_text))


def create_session():
    """创建复用连接的会话。"""
    cookies = getCookie()
    magic = getMagic()
    session = requests.Session()
    session.headers.update(build_headers(magic))
    session.cookies.update(cookies)
    return session


def fetch_order_detail_payload(order_id, session):
    """拉取完整订单详情响应。"""
    params = {"token": "", "lang": "zh_CN"}
    data = json.dumps({"id": str(order_id)}, separators=(",", ":"))

    try:
        response = session.post(ORDER_DETAIL_URL, params=params, data=data, timeout=REQUEST_TIMEOUT)
    except requests.RequestException as exc:
        raise RuntimeError(f"获取订单详情失败：{exc}") from exc

    if response.status_code != 200:
        raise RuntimeError(f"获取订单详情失败：{get_response_error(response)}")

    try:
        detail_payload = response.json()
    except ValueError as exc:
        raise RuntimeError("获取订单详情失败：接口返回了非 JSON 响应。") from exc

    if detail_payload.get("success") is False:
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(detail_payload, '订单详情接口返回失败。')}"
        )

    if detail_payload.get("code") not in (None, 0):
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(detail_payload, '订单详情接口返回失败。')}"
        )

    return detail_payload


def fetch_delivery_product_info(order_id, session):
    """查询单个订单详情并返回物流产品信息。"""
    detail_payload = fetch_order_detail_payload(order_id, session)

    delivery_product_list = (
        detail_payload.get("expressInfo", {}).get("deliveryProductInfo") or []
    )
    if not delivery_product_list:
        raise RuntimeError("获取订单详情失败：订单详情中没有可更新的物流信息。")

    delivery_product_info = delivery_product_list[0]
    delivery_id = delivery_product_info.get("deliveryId")
    if delivery_id in (None, ""):
        raise RuntimeError("获取订单详情失败：订单详情缺少承运商信息（deliveryId）。")

    product_infos = normalize_product_infos(delivery_product_info)
    if not product_infos:
        raise RuntimeError("获取订单详情失败：订单详情缺少商品信息，无法更新物流。")

    return delivery_product_info


def build_delivery_candidates(order_id, tracking_number, delivery_product_info, session):
    """构建当前单号的 deliveryId 候选列表。

    当前策略与既有程序保持一致：
    1. 优先使用新物流单号前两位作为 deliveryId（主路径）。
    2. 失败后回退到订单原始 deliveryId（兜底）。
    """
    del order_id, session
    candidates = []
    seen_keys = set()

    def add_candidate(delivery_id, delivery_name):
        if delivery_id in (None, ""):
            return
        key = (str(delivery_id), str(delivery_name or ""))
        if key in seen_keys:
            return
        seen_keys.add(key)
        candidates.append({"deliveryId": str(delivery_id), "deliveryName": str(delivery_name or "")})

    tracking_prefix = str(tracking_number).strip()[:2]
    add_candidate(tracking_prefix, delivery_product_info.get("deliveryName"))
    add_candidate(delivery_product_info.get("deliveryId"), delivery_product_info.get("deliveryName"))
    return candidates


def update_delivery_info(order_id, tracking_number, delivery_product_info, session, delivery_override=None):
    """提交单个订单的物流更新。"""
    params = {"token": "", "lang": "zh_CN"}
    selected_delivery_id = (
        delivery_override.get("deliveryId") if delivery_override else delivery_product_info.get("deliveryId")
    )
    selected_delivery_name = (
        delivery_override.get("deliveryName") if delivery_override else delivery_product_info.get("deliveryName")
    )

    delivery_item = {
        "waybillId": str(tracking_number),
        "deliveryId": selected_delivery_id,
        "productInfos": normalize_product_infos(delivery_product_info),
        "isAllProduct": delivery_product_info.get("isAllProduct", False),
        "deliverType": delivery_product_info.get("deliverType", 1),
        "waybillStatus": delivery_product_info.get("waybillStatus", 2),
    }
    if selected_delivery_name not in (None, ""):
        delivery_item["deliveryName"] = selected_delivery_name
    if delivery_override is None:
        delivery_time = delivery_product_info.get("deliveryTime")
        if delivery_time not in (None, ""):
            delivery_item["deliveryTime"] = delivery_time

    payload = json.dumps(
        {
            "orderId": str(order_id),
            "deliveryInfo": {
                "deliverType": delivery_product_info.get("deliverType", 1),
                "deliveryProductInfo": [delivery_item],
            },
        },
        separators=(",", ":"),
    )

    try:
        response = session.post(ORDER_DELIVERY_UPDATE_URL, params=params, data=payload, timeout=REQUEST_TIMEOUT)
    except requests.RequestException as exc:
        raise RuntimeError(f"更新物流信息失败：{exc}") from exc

    if response.status_code != 200:
        raise RuntimeError(f"更新物流信息失败：{get_response_error(response)}")

    try:
        result = response.json()
    except ValueError as exc:
        raise RuntimeError("更新物流信息失败：接口返回了非 JSON 响应。") from exc

    if result.get("success") is True:
        return

    if result.get("ret") == 0 and result.get("code") in (None, 0):
        return

    raise RuntimeError(f"更新物流信息失败：{get_payload_error(result, '物流信息修改失败。')}")


def update_single_order(order_id, tracking_number, session):
    """顺序执行单个订单更新。"""
    delivery_product_info = fetch_delivery_product_info(order_id, session)
    old_waybill = delivery_product_info.get("waybillId", "")
    last_error = None

    for delivery_option in build_delivery_candidates(order_id, tracking_number, delivery_product_info, session):
        try:
            override = None
            current_delivery_id = str(delivery_product_info.get("deliveryId") or "")
            if delivery_option.get("deliveryId") != current_delivery_id:
                override = delivery_option
            update_delivery_info(order_id, tracking_number, delivery_product_info, session, override)
            return old_waybill
        except RuntimeError as exc:
            last_error = exc
            if DELIVERY_MISMATCH_MESSAGE in str(exc):
                continue
            raise

    if last_error is not None:
        raise last_error
    raise RuntimeError("更新物流信息失败：未识别到可用的物流公司映射。")


def build_font(size, bold=False):
    """获取通用字体。"""
    font = QFontDatabase.systemFont(QFontDatabase.GeneralFont)
    font.setPointSize(size)
    font.setBold(bold)
    return font


def build_fixed_font(size):
    """获取等宽字体。"""
    font = QFontDatabase.systemFont(QFontDatabase.FixedFont)
    font.setPointSize(size)
    return font


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


class BatchWorker(QObject):
    """后台批量执行器。"""

    started = Signal(int)
    step_started = Signal(int, int, str)
    step_succeeded = Signal(int, int, str, str, str)
    step_failed = Signal(int, int, str, str, str)
    fatal_error = Signal(str)
    missing_config = Signal(str)
    finished = Signal(int, int, int, bool)

    def __init__(self, order_ids, tracking_numbers):
        super().__init__()
        self.order_ids = order_ids
        self.tracking_numbers = tracking_numbers
        self._resume_event = threading.Event()
        self._resume_event.set()

    def pause(self):
        """暂停后续任务。"""
        self._resume_event.clear()

    def resume(self):
        """恢复任务。"""
        self._resume_event.set()

    def run(self):
        """后台线程执行入口。"""
        success_count = 0
        failure_count = 0
        total_count = len(self.order_ids)
        self.started.emit(total_count)

        try:
            session = create_session()
        except ConfigNotFoundError as exc:
            self.missing_config.emit("\n".join(exc.searched_dirs))
            self.finished.emit(0, 0, total_count, True)
            return

        try:
            with session:
                for index, (order_id, tracking_number) in enumerate(
                    zip(self.order_ids, self.tracking_numbers), start=1
                ):
                    self._resume_event.wait()
                    self.step_started.emit(index, total_count, order_id)
                    try:
                        old_waybill = update_single_order(order_id, tracking_number, session)
                    except Exception as exc:  # noqa: BLE001
                        failure_count += 1
                        self.step_failed.emit(
                            index,
                            total_count,
                            order_id,
                            tracking_number,
                            str(exc),
                        )
                        continue

                    success_count += 1
                    self.step_succeeded.emit(
                        index,
                        total_count,
                        order_id,
                        tracking_number,
                        old_waybill or "无原物流单号",
                    )
        except Exception as exc:  # noqa: BLE001
            failure_count += total_count - success_count - failure_count
            self.fatal_error.emit(str(exc))
        finally:
            self.finished.emit(success_count, failure_count, total_count, False)


class MainWindow(QWidget):
    """主窗口。"""

    def __init__(self):
        super().__init__()
        self.worker_thread = None
        self.worker = None
        self.is_paused = False

        self.setWindowTitle(WINDOW_TITLE)
        self.setObjectName("AppRoot")
        fixed_width, fixed_height = self._resolve_fixed_window_size()
        self.setFixedSize(fixed_width, fixed_height)
        self.setStyleSheet(
            f"""
            QWidget#AppRoot {{
                background: {APP_COLORS["bg"]};
            }}
            QWidget {{
                color: {APP_COLORS["text"]};
            }}
            QLabel {{
                background: transparent;
            }}
            QWidget#PageWidget,
            QWidget#HeaderBody,
            QWidget#TitleWrap,
            QWidget#InputContainer,
            QWidget#InputCardBody,
            QWidget#CardHeader,
            QWidget#LogBody,
            QWidget#LogHeader {{
                background: transparent;
            }}
            QFrame#Card {{
                background: {APP_COLORS["surface"]};
                border: 1px solid {APP_COLORS["border"]};
                border-radius: 20px;
            }}
            QFrame#HeroCard {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 1,
                    stop: 0 {APP_COLORS["hero_tint"]},
                    stop: 1 {APP_COLORS["hero_tint_deep"]}
                );
                border: 1px solid {APP_COLORS["hero_border"]};
                border-radius: 22px;
            }}
            QFrame#InputCardBlue {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 1,
                    stop: 0 {APP_COLORS["blue_tint"]},
                    stop: 1 #B9D2F0
                );
                border: 1px solid #9EB7D7;
                border-radius: 20px;
            }}
            QFrame#InputCardOrange {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 1,
                    stop: 0 {APP_COLORS["orange_tint"]},
                    stop: 1 {APP_COLORS["orange_tint_deep"]}
                );
                border: 1px solid {APP_COLORS["orange_border"]};
                border-radius: 20px;
            }}
            QFrame#ConfigCard {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 1,
                    stop: 0 #E2EAF5,
                    stop: 1 #CEDBEC
                );
                border: 1px solid #AEBFD6;
                border-radius: 20px;
            }}
            QFrame#LogCard {{
                background: {APP_COLORS["log_bg"]};
                border: 1px solid #205187;
                border-radius: 20px;
            }}
            QFrame#InputShell {{
                background: transparent;
                border: none;
            }}
            QPlainTextEdit#InputEdit {{
                background: {APP_COLORS["input_bg"]};
                color: {APP_COLORS["text"]};
                border: 1px solid {APP_COLORS["input_border"]};
                border-radius: 16px;
                padding: 14px;
                selection-background-color: {APP_COLORS["blue"]};
            }}
            QPlainTextEdit#InputEdit:focus {{
                border: 2px solid {APP_COLORS["input_border_focus"]};
                background: #FFFFFF;
            }}
            QPlainTextEdit#LogEdit {{
                background: {APP_COLORS["log_surface"]};
                color: {APP_COLORS["log_fg"]};
                border: 1px solid #2D5D94;
                border-radius: 16px;
                padding: 14px;
                selection-background-color: {APP_COLORS["blue"]};
            }}
            QPushButton#PrimaryButton {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 0,
                    stop: 0 {APP_COLORS["orange"]},
                    stop: 1 {APP_COLORS["orange_deep"]}
                );
                color: white;
                border: 1px solid #8A3D03;
                border-radius: 16px;
                padding: 16px 20px;
                font-weight: 700;
            }}
            QPushButton#PrimaryButton:hover {{
                background: #C86805;
            }}
            QPushButton#PrimaryButton:pressed {{
                padding-top: 17px;
                padding-bottom: 15px;
            }}
            QPushButton#PrimaryButton:disabled {{
                background: #F1F5F9;
                color: #94A3B8;
                border: 1px solid #ADC0D8;
            }}
            QPushButton#PauseButton {{
                background: qlineargradient(
                    x1: 0, y1: 0, x2: 1, y2: 0,
                    stop: 0 #2E4662,
                    stop: 1 #1A2C44
                );
                color: #EAF2FC;
                border: 1px solid #5A7598;
                border-radius: 16px;
                padding: 16px 18px;
                font-weight: 700;
            }}
            QPushButton#PauseButton:hover {{
                background: #203651;
            }}
            QPushButton#PauseButton:pressed {{
                background: #182B42;
            }}
            QPushButton#PauseButton:disabled {{
                background: #D7E2F0;
                color: #94A3B8;
                border: 1px solid #ADC0D8;
            }}
            QLabel#HeroTitle {{
                color: {APP_COLORS["heading"]};
            }}
            QLabel#HeroSubtitle {{
                color: {APP_COLORS["muted"]};
            }}
            QLabel#SectionTitle {{
                color: {APP_COLORS["heading"]};
            }}
            QLabel#SectionHint {{
                color: {APP_COLORS["muted"]};
            }}
            QLabel#LogTitle {{
                color: {APP_COLORS["log_fg"]};
            }}
            QLabel#LogHint {{
                color: {APP_COLORS["log_muted"]};
            }}
            QLabel#ConfigPath {{
                color: {APP_COLORS["muted"]};
                background: rgba(255, 255, 255, 0.82);
                border: 1px solid #AEC0D8;
                border-radius: 12px;
                padding: 10px 12px;
            }}
            QPushButton#SecondaryButton {{
                background: #FFFFFF;
                color: {APP_COLORS["blue_deep"]};
                border: 1px solid #96ACCA;
                border-radius: 12px;
                padding: 10px 14px;
                font-weight: 700;
            }}
            QPushButton#SecondaryButton:hover {{
                background: #EEF4FF;
                border-color: #9FB5D1;
            }}
            QPushButton#SecondaryButton:pressed {{
                background: #E4EDFA;
            }}
            QScrollArea {{
                border: none;
                background: transparent;
            }}
            QScrollBar:vertical {{
                background: #D8E2EE;
                width: 12px;
                margin: 8px 4px 8px 0;
                border-radius: 6px;
            }}
            QScrollBar::handle:vertical {{
                background: #8EA5C1;
                min-height: 36px;
                border-radius: 6px;
            }}
            QScrollBar::handle:vertical:hover {{
                background: #738FB2;
            }}
            QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
                height: 0;
            }}
            QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{
                background: transparent;
            }}
            """
        )

        self._build_ui()
        self.refresh_config_path_label()
        self._fit_window_to_screen()
        self.refresh_input_metrics()
        self._sync_responsive_metrics()
        self.refresh_action_buttons()

    def _build_ui(self):
        """构建主界面骨架。"""
        self._build_root_container()
        self._build_header_card()
        self._build_input_section()
        self._build_action_section()
        self._build_log_section()

    def _build_root_container(self):
        """创建根容器与可滚动页面。"""
        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self.scroll_area = QScrollArea()
        self.scroll_area.setWidgetResizable(True)
        self.scroll_area.setFrameShape(QFrame.NoFrame)
        self.scroll_area.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.scroll_area.setStyleSheet("QScrollArea { border: none; background: transparent; }")
        root_layout.addWidget(self.scroll_area)

        self.page_widget = QWidget()
        self.page_widget.setObjectName("PageWidget")
        self.scroll_area.setWidget(self.page_widget)

        self.page_layout = QVBoxLayout(self.page_widget)
        self.page_layout.setContentsMargins(24, 22, 24, 22)
        self.page_layout.setSpacing(16)
        self.page_layout.setAlignment(Qt.AlignTop)

    def _build_header_card(self):
        """创建顶部标题卡片。"""
        self.header_card = self._create_card(self.page_layout, object_name="HeroCard")
        header_layout = QVBoxLayout(self.header_card)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(0)

        header_body = QWidget()
        header_body.setObjectName("HeaderBody")
        header_box = QHBoxLayout(header_body)
        header_box.setContentsMargins(22, 18, 22, 18)
        header_box.setSpacing(14)

        title_wrap = QWidget()
        title_wrap.setObjectName("TitleWrap")
        title_box = QVBoxLayout(title_wrap)
        title_box.setContentsMargins(0, 0, 0, 0)
        title_box.setSpacing(6)

        title_label = QLabel("驼铃视频小店中差评处理")
        title_label.setObjectName("HeroTitle")
        title_label.setFont(build_font(22, bold=True))
        self.hero_title_label = title_label
        title_box.addWidget(title_label)

        self.title_description_label = QLabel(
            "批量处理中差评、品质退款订单。"
        )
        self.title_description_label.setObjectName("HeroSubtitle")
        self.title_description_label.setWordWrap(False)
        self.title_description_label.setFont(build_font(12))
        title_box.addWidget(self.title_description_label)

        header_box.addWidget(title_wrap, 1)

        badge_wrap = QWidget()
        badge_layout = QHBoxLayout(badge_wrap)
        badge_layout.setContentsMargins(0, 0, 0, 0)
        badge_layout.setSpacing(10)

        self.author_badge = QLabel("作者微信：TLS-801")
        self.author_badge.setAlignment(Qt.AlignCenter)
        self.author_badge.setFont(build_font(12, bold=True))
        self.author_badge.setStyleSheet(
            f"background: {APP_COLORS['blue_soft']};"
            f"color: {APP_COLORS['blue_deep']};"
            "border: 1px solid #9FC0F0;"
            "border-radius: 14px;"
            "padding: 12px 18px;"
        )
        self.author_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_layout.addWidget(self.author_badge, 0, Qt.AlignVCenter)

        self.tutorial_badge = QLabel()
        self.tutorial_badge.setAlignment(Qt.AlignCenter)
        self.tutorial_badge.setTextFormat(Qt.RichText)
        self.tutorial_badge.setTextInteractionFlags(Qt.TextBrowserInteraction)
        self.tutorial_badge.setOpenExternalLinks(True)
        self.tutorial_badge.setCursor(Qt.PointingHandCursor)
        self.tutorial_badge.setFont(build_font(12, bold=True))
        self.tutorial_badge.setText(
            f'<a href="{TUTORIAL_URL}" style="color: {APP_COLORS["blue_deep"]}; text-decoration: none;">查看使用教程</a>'
        )
        self.tutorial_badge.setStyleSheet(
            f"background: {APP_COLORS['blue_soft']};"
            f"color: {APP_COLORS['blue_deep']};"
            "border: 1px solid #9FC0F0;"
            "border-radius: 14px;"
            "padding: 12px 18px;"
        )
        self.tutorial_badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_layout.addWidget(self.tutorial_badge, 0, Qt.AlignVCenter)

        header_box.addWidget(badge_wrap, 0, Qt.AlignVCenter | Qt.AlignRight)
        header_layout.addWidget(header_body)

    def _create_count_badge(self, text_color, bg_color, border_color):
        """创建输入数量徽标。"""
        badge = QLabel()
        badge.setAlignment(Qt.AlignCenter)
        badge.setMinimumWidth(72)
        badge.setFont(build_font(10, bold=True))
        badge.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge.setStyleSheet(
            f"background: {bg_color};"
            f"color: {text_color};"
            f"border: 1px solid {border_color};"
            "border-radius: 10px;"
            "padding: 8px 10px;"
        )
        return badge

    def _build_input_section(self):
        """创建三列输入区域（订单号、物流单号、配置目录）。"""
        self.input_container = QWidget()
        self.input_container.setObjectName("InputContainer")
        self.input_container.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.input_grid = QGridLayout(self.input_container)
        self.input_grid.setContentsMargins(0, 0, 0, 0)
        self.input_grid.setHorizontalSpacing(16)
        self.input_grid.setVerticalSpacing(12)

        self.order_count_badge = self._create_count_badge(
            text_color=APP_COLORS["blue"],
            bg_color=APP_COLORS["blue_soft"],
            border_color="#9FC0F0",
        )

        self.tracking_count_badge = self._create_count_badge(
            text_color=APP_COLORS["orange"],
            bg_color=APP_COLORS["orange_soft"],
            border_color="#E4B57E",
        )

        self.order_edit = BatchInputEdit("每行一个订单号，最多 100 条。")
        self.tracking_edit = BatchInputEdit("每行一个物流单号，最多 100 条。")

        self.order_edit.textChanged.connect(self.refresh_input_metrics)
        self.tracking_edit.textChanged.connect(self.refresh_input_metrics)
        self.order_edit.normalized.connect(self.refresh_input_metrics)
        self.tracking_edit.normalized.connect(self.refresh_input_metrics)

        self.order_card = self._create_input_card(
            "第一步：填写订单号",
            "多个订单号请用英文逗号、换行分隔。",
            self.order_count_badge,
            self.order_edit,
            APP_COLORS["blue"],
            "InputCardBlue",
        )
        self.tracking_card = self._create_input_card(
            "第二步：填写物流单号",
            "多个物流单号请用英文逗号、换行分隔。",
            self.tracking_count_badge,
            self.tracking_edit,
            APP_COLORS["orange"],
            "InputCardOrange",
        )
        self.config_card = self._create_config_card()

        self.page_layout.addWidget(self.input_container, 0, Qt.AlignTop)
        self.input_grid.addWidget(self.order_card, 0, 0, Qt.AlignTop)
        self.input_grid.addWidget(self.tracking_card, 0, 1, Qt.AlignTop)
        self.input_grid.addWidget(self.config_card, 0, 2, Qt.AlignTop)
        self.input_grid.setColumnStretch(0, 1)
        self.input_grid.setColumnStretch(1, 1)
        self.input_grid.setColumnStretch(2, 1)

    def _build_action_section(self):
        """创建开始/暂停操作区。"""
        self.action_row = QWidget()
        self.action_row.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        self.action_layout = QHBoxLayout(self.action_row)
        self.action_layout.setContentsMargins(0, 0, 0, 0)
        self.action_layout.setSpacing(12)

        self.pause_button = QPushButton("暂停批量处理")
        self.pause_button.setObjectName("PauseButton")
        self.pause_button.setCursor(Qt.PointingHandCursor)
        self.pause_button.setFont(build_font(16, bold=True))
        self.pause_button.setMinimumHeight(56)
        self.pause_button.clicked.connect(self.on_pause_clicked)
        self.action_layout.addWidget(self.pause_button, 1)

        self.start_button = QPushButton("开始批量处理")
        self.start_button.setObjectName("PrimaryButton")
        self.start_button.setCursor(Qt.PointingHandCursor)
        self.start_button.setFont(build_font(17, bold=True))
        self.start_button.setMinimumHeight(56)
        self.start_button.clicked.connect(self.on_start_clicked)
        self.action_layout.addWidget(self.start_button, 1)
        self.page_layout.addWidget(self.action_row)

    def _build_log_section(self):
        """创建日志展示区。"""
        self.log_card = self._create_card(self.page_layout, stretch=1, object_name="LogCard")
        log_layout = QVBoxLayout(self.log_card)
        log_layout.setContentsMargins(0, 0, 0, 0)
        log_layout.setSpacing(0)

        log_body = QWidget()
        log_body.setObjectName("LogBody")
        log_box = QVBoxLayout(log_body)
        log_box.setContentsMargins(16, 14, 16, 14)
        log_box.setSpacing(10)

        log_header = QWidget()
        log_header.setObjectName("LogHeader")
        log_header_box = QHBoxLayout(log_header)
        log_header_box.setContentsMargins(0, 0, 0, 0)
        log_header_box.setSpacing(10)

        log_title = QLabel("执行日志")
        log_title.setObjectName("LogTitle")
        log_title.setFont(build_font(15, bold=True))
        self.log_title_label = log_title
        log_header_box.addWidget(log_title)

        self.log_hint_label = QLabel("最近执行记录会按时间顺序滚动显示。")
        self.log_hint_label.setObjectName("LogHint")
        self.log_hint_label.setWordWrap(False)
        self.log_hint_label.setFont(build_font(10))
        self.log_hint_label.setAlignment(Qt.AlignRight | Qt.AlignVCenter)
        log_header_box.addWidget(self.log_hint_label, 1)
        log_box.addWidget(log_header)

        self.log_view = QPlainTextEdit()
        self.log_view.setObjectName("LogEdit")
        self.log_card.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Expanding)
        self.log_view.setReadOnly(True)
        self.log_view.setFont(build_fixed_font(11))
        self.log_view.setMinimumHeight(300)
        log_box.addWidget(self.log_view, 1)

        log_layout.addWidget(log_body)

    def _create_card(self, parent_layout, stretch=0, object_name="Card"):
        """创建卡片容器。"""
        card = QFrame()
        card.setObjectName(object_name)
        if stretch:
            parent_layout.addWidget(card, stretch)
        else:
            parent_layout.addWidget(card)
        return card

    def _create_input_card(self, title, hint, badge, editor, accent, object_name):
        """创建输入卡片。"""
        card = QFrame()
        card.setObjectName(object_name)
        card_layout = QVBoxLayout(card)
        card_layout.setContentsMargins(0, 0, 0, 0)
        card_layout.setSpacing(0)

        body = QWidget()
        body.setObjectName("InputCardBody")
        body_layout = QVBoxLayout(body)
        body_layout.setContentsMargins(16, 14, 16, 14)
        body_layout.setSpacing(10)

        header = QWidget()
        header.setObjectName("CardHeader")
        header.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(10)

        title_label = QLabel(title)
        title_label.setObjectName("SectionTitle")
        title_label.setFont(build_font(15, bold=True))
        title_label.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        card.title_label = title_label
        header_layout.addWidget(title_label)
        header_layout.addWidget(badge, 0, Qt.AlignRight)
        body_layout.addWidget(header)

        hint_label = QLabel(hint)
        hint_label.setObjectName("SectionHint")
        hint_label.setWordWrap(False)
        hint_label.setFont(build_font(10))
        hint_label.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        body_layout.addWidget(hint_label)
        card.hint_label = hint_label

        body_layout.addWidget(editor)
        card_layout.addWidget(body)
        return card

    def _create_config_card(self):
        """创建配置目录卡片，复用输入卡片骨架以确保三列对齐。"""
        badge_placeholder = QLabel("目录")
        badge_placeholder.setAlignment(Qt.AlignCenter)
        badge_placeholder.setMinimumWidth(72)
        badge_placeholder.setSizePolicy(QSizePolicy.Maximum, QSizePolicy.Maximum)
        badge_placeholder.setStyleSheet(
            "background: #D7E2F0;"
            "color: #375271;"
            "border: 1px solid #ADC0D8;"
            "border-radius: 10px;"
            "padding: 8px 10px;"
        )
        self.config_badge = badge_placeholder

        shell = QWidget()
        shell.setObjectName("InputShell")
        shell_layout = QVBoxLayout(shell)
        shell_layout.setContentsMargins(0, 0, 0, 0)
        shell_layout.setSpacing(10)

        path_label = QLabel()
        path_label.setObjectName("ConfigPath")
        path_label.setWordWrap(True)
        path_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        path_label.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Preferred)
        shell_layout.addWidget(path_label, 1)
        self.config_path_label = path_label

        button = QPushButton("选择配置目录")
        button.setObjectName("SecondaryButton")
        button.setCursor(Qt.PointingHandCursor)
        button.setSizePolicy(QSizePolicy.Preferred, QSizePolicy.Maximum)
        button.clicked.connect(self.choose_config_dir)
        shell_layout.addWidget(button)
        self.config_button = button

        card = self._create_input_card(
            "第三步：选择配置目录",
            "选择 cookie.txt 与 biz_magic.txt 所在目录。",
            self.config_badge,
            shell,
            APP_COLORS["blue"],
            "ConfigCard",
        )
        self.config_title_label = card.title_label
        self.config_hint_label = card.hint_label
        return card

    def _calculate_editor_height(self, editor, visible_lines=10):
        """按指定可见行数计算输入框高度。"""
        line_height = editor.fontMetrics().lineSpacing()
        document_margin = int(editor.document().documentMargin() * 2)
        frame = editor.frameWidth() * 2
        padding = 18
        return line_height * visible_lines + document_margin + frame + padding

    def _fit_window_to_screen(self):
        """按屏幕缩放比例锁定固定窗口尺寸。"""
        fixed_width, fixed_height = self._resolve_fixed_window_size()
        self.setFixedSize(fixed_width, fixed_height)

    def _resolve_fixed_window_size(self):
        """结合平台默认值与屏幕信息，计算最终窗口固定尺寸。"""
        default_width, default_height = get_platform_default_window_size()
        screen = self.screen() or QApplication.primaryScreen()
        scale = max(1.0, screen.devicePixelRatio()) if screen is not None else 1.0
        available = screen.availableGeometry() if screen is not None else None
        fixed_width = int(round(default_width / scale))
        fixed_height = int(round(default_height / scale))
        if available is not None:
            fixed_width = min(fixed_width, available.width())
            fixed_height = min(fixed_height, available.height())
        return fixed_width, fixed_height

    def _sync_responsive_metrics(self):
        """窗口变化时同步紧凑布局尺寸。"""
        viewport = self.scroll_area.viewport().size()
        if not viewport.width() or not viewport.height():
            return

        width_scale = viewport.width() / DESIGN_WIDTH
        height_scale = viewport.height() / DESIGN_HEIGHT
        scale = max(0.78, min(1.0, width_scale, height_scale))

        page_margin_x = max(12, int(18 * scale))
        page_margin_y = max(10, int(16 * scale))
        page_spacing = max(8, int(14 * scale))
        button_height = max(48, int(56 * scale))
        header_height = max(100, int(120 * scale))
        log_editor_height = max(190, int(300 * scale))

        self.hero_title_label.setFont(build_font(max(18, int(22 * scale)), bold=True))
        self.title_description_label.setFont(build_font(max(10, int(12 * scale))))
        self.author_badge.setFont(build_font(max(11, int(12 * scale)), bold=True))
        self.tutorial_badge.setFont(build_font(max(11, int(12 * scale)), bold=True))
        self.order_count_badge.setFont(build_font(max(9, int(10 * scale)), bold=True))
        self.tracking_count_badge.setFont(build_font(max(9, int(10 * scale)), bold=True))
        self.order_card.title_label.setFont(build_font(max(13, int(15 * scale)), bold=True))
        self.tracking_card.title_label.setFont(build_font(max(13, int(15 * scale)), bold=True))
        self.order_card.hint_label.setFont(build_font(max(9, int(10 * scale))))
        self.tracking_card.hint_label.setFont(build_font(max(9, int(10 * scale))))
        self.config_title_label.setFont(build_font(max(13, int(15 * scale)), bold=True))
        self.config_badge.setFont(build_font(max(9, int(10 * scale)), bold=True))
        self.config_hint_label.setFont(build_font(max(9, int(10 * scale))))
        self.config_path_label.setFont(build_font(max(9, int(10 * scale))))
        self.config_button.setFont(build_font(max(10, int(11 * scale)), bold=True))
        self.log_title_label.setFont(build_font(max(13, int(15 * scale)), bold=True))
        self.log_hint_label.setFont(build_font(max(9, int(10 * scale))))
        self.start_button.setFont(build_font(max(14, int(17 * scale)), bold=True))
        self.pause_button.setFont(build_font(max(13, int(15 * scale)), bold=True))
        self.order_edit.setFont(build_fixed_font(max(11, int(13 * scale))))
        self.tracking_edit.setFont(build_fixed_font(max(11, int(13 * scale))))
        self.log_view.setFont(build_fixed_font(max(9, int(11 * scale))))

        badge_height = max(36, int(40 * scale))
        author_badge_height = max(44, int(50 * scale))
        author_badge_width = max(180, int(210 * scale))
        input_editor_height = self._calculate_editor_height(self.order_edit, 10)

        self.author_badge.setFixedHeight(author_badge_height)
        self.author_badge.setFixedWidth(author_badge_width)
        self.tutorial_badge.setFixedHeight(author_badge_height)
        self.tutorial_badge.setFixedWidth(author_badge_width)
        self.order_count_badge.setFixedHeight(badge_height)
        self.tracking_count_badge.setFixedHeight(badge_height)
        self.config_badge.setFixedHeight(badge_height)
        self.start_button.setFixedHeight(button_height)
        self.pause_button.setFixedHeight(button_height)
        self.header_card.setMinimumHeight(header_height)
        self.order_edit.setFixedHeight(input_editor_height)
        self.tracking_edit.setFixedHeight(input_editor_height)
        self.log_view.setMinimumHeight(log_editor_height)
        self.config_button.setFixedHeight(max(40, int(44 * scale)))

        card_target_height = max(
            self.order_card.sizeHint().height(),
            self.tracking_card.sizeHint().height(),
        )
        config_non_path_height = self.config_card.sizeHint().height() - self.config_path_label.sizeHint().height()
        config_path_height = max(56, card_target_height - config_non_path_height)
        self.config_path_label.setFixedHeight(config_path_height)
        self.order_card.setFixedHeight(card_target_height)
        self.tracking_card.setFixedHeight(card_target_height)
        self.config_card.setFixedHeight(card_target_height)

        self.page_layout.setContentsMargins(page_margin_x, page_margin_y, page_margin_x, page_margin_y)
        self.page_layout.setSpacing(page_spacing)
        self.input_grid.setHorizontalSpacing(max(10, int(14 * scale)))
        self.input_grid.setVerticalSpacing(max(8, int(10 * scale)))

    def resizeEvent(self, event):
        """窗口尺寸变化时同步内部尺寸。"""
        self._sync_responsive_metrics()
        super().resizeEvent(event)

    def refresh_input_metrics(self):
        """刷新两个输入框的数量徽标。"""
        order_count = len(parse_batch_input(self.order_edit.toPlainText()))
        tracking_count = len(parse_batch_input(self.tracking_edit.toPlainText()))
        self.order_count_badge.setText(f"{order_count}/{MAX_BATCH_SIZE}")
        self.tracking_count_badge.setText(f"{tracking_count}/{MAX_BATCH_SIZE}")

    def normalize_inputs(self):
        """整理两个输入框内容。"""
        self.order_edit.normalize_content()
        self.tracking_edit.normalize_content()
        self.refresh_input_metrics()

    def append_result_log(self, text):
        """追加执行日志。"""
        self.log_view.appendPlainText(text)
        scrollbar = self.log_view.verticalScrollBar()
        scrollbar.setValue(scrollbar.maximum())

    def clear_result_log(self):
        """清空执行日志。"""
        self.log_view.clear()

    def _style_message_box(self, dialog):
        """统一提示弹窗视觉。"""
        dialog.setStyleSheet(
            """
            QDialog#AppMessageDialog {
                background: #0A1C36;
                border: 1px solid #1E3A8A;
                border-radius: 14px;
            }
            QLabel#MessageTitle {
                color: #F8FAFC;
                font-size: 18px;
                font-weight: 700;
            }
            QLabel#MessageText {
                color: #EAF2FC;
                font-size: 15px;
                line-height: 1.45;
            }
            QLabel#MessageInfo {
                color: #BFD0E5;
                font-size: 13px;
                line-height: 1.45;
            }
            QPushButton#MessagePrimary {
                background: #1D4ED8;
                color: #F8FAFC;
                border: 1px solid #3B82F6;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 700;
            }
            QPushButton#MessagePrimary:hover {
                background: #2563EB;
            }
            QPushButton#MessagePrimary:pressed {
                background: #1E40AF;
            }
            QPushButton#MessageSecondary {
                background: rgba(148, 163, 184, 0.18);
                color: #EAF2FC;
                border: 1px solid #64748B;
                border-radius: 10px;
                padding: 9px 18px;
                min-width: 112px;
                font-weight: 600;
            }
            QPushButton#MessageSecondary:hover {
                background: rgba(148, 163, 184, 0.28);
            }
            QPushButton#MessageSecondary:pressed {
                background: rgba(100, 116, 139, 0.35);
            }
            """
        )
        return dialog

    def _message_icon_pixmap(self, level):
        """根据消息级别返回标准图标。"""
        icon_map = {
            QMessageBox.Information: QStyle.SP_MessageBoxInformation,
            QMessageBox.Warning: QStyle.SP_MessageBoxWarning,
            QMessageBox.Critical: QStyle.SP_MessageBoxCritical,
            QMessageBox.Question: QStyle.SP_MessageBoxQuestion,
        }
        icon_type = icon_map.get(level, QStyle.SP_MessageBoxInformation)
        return self.style().standardIcon(icon_type).pixmap(46, 46)

    def _create_message_dialog_base(self, level, title, text, informative_text="", *, min_width=560):
        """构建统一样式弹窗骨架，返回 (dialog, actions_layout)。"""
        dialog = QDialog(self)
        dialog.setObjectName("AppMessageDialog")
        dialog.setWindowTitle(title)
        dialog.setModal(True)
        dialog.setMinimumWidth(min_width)

        root = QVBoxLayout(dialog)
        root.setContentsMargins(22, 18, 22, 18)
        root.setSpacing(16)

        body = QHBoxLayout()
        body.setSpacing(14)

        icon_label = QLabel()
        icon_label.setPixmap(self._message_icon_pixmap(level))
        icon_label.setAlignment(Qt.AlignTop | Qt.AlignHCenter)
        icon_label.setFixedWidth(56)
        body.addWidget(icon_label, 0, Qt.AlignTop)

        text_wrap = QWidget()
        text_layout = QVBoxLayout(text_wrap)
        text_layout.setContentsMargins(0, 0, 0, 0)
        text_layout.setSpacing(8)

        title_label = QLabel(title)
        title_label.setObjectName("MessageTitle")
        title_label.setWordWrap(True)
        text_layout.addWidget(title_label)

        text_label = QLabel(text)
        text_label.setObjectName("MessageText")
        text_label.setWordWrap(True)
        text_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        text_layout.addWidget(text_label)

        if informative_text:
            info_label = QLabel(informative_text)
            info_label.setObjectName("MessageInfo")
            info_label.setWordWrap(True)
            info_label.setAlignment(Qt.AlignLeft | Qt.AlignTop)
            text_layout.addWidget(info_label)

        body.addWidget(text_wrap, 1)
        root.addLayout(body, 1)

        actions = QHBoxLayout()
        actions.setContentsMargins(0, 0, 0, 0)
        actions.setSpacing(10)
        actions.addStretch(1)
        root.addLayout(actions)

        self._style_message_box(dialog)
        return dialog, actions

    def _add_message_action(self, actions_layout, text, object_name, callback):
        """向弹窗动作栏添加按钮。"""
        button = QPushButton(text)
        button.setObjectName(object_name)
        button.clicked.connect(callback)
        actions_layout.addWidget(button)
        return button

    def _build_message_dialog(self, level, title, text, informative_text=""):
        """构建普通提示弹窗（单确定按钮）。"""
        dialog, actions = self._create_message_dialog_base(
            level,
            title,
            text,
            informative_text,
            min_width=560,
        )
        self._add_message_action(actions, "确定", "MessagePrimary", dialog.accept)
        return dialog

    def show_message(self, level, title, text, informative_text=""):
        """显示统一样式的提示弹窗。"""
        dialog = self._build_message_dialog(level, title, text, informative_text)
        dialog.exec()

    def refresh_action_buttons(self):
        """同步开始/暂停按钮状态。"""
        running = self.worker is not None
        self.order_edit.setReadOnly(running)
        self.tracking_edit.setReadOnly(running)
        self.config_button.setDisabled(running)
        self.pause_button.setDisabled((not running) or self.is_paused)
        self.start_button.setDisabled(running and not self.is_paused)
        self.start_button.setText("继续批量处理" if self.is_paused else "开始批量处理")
        self.pause_button.setText("已暂停" if self.is_paused else "暂停批量处理")

    def set_submit_running(self, is_running):
        """切换按钮和输入框状态。"""
        if not is_running:
            self.is_paused = False
        self.refresh_action_buttons()

    def refresh_config_path_label(self):
        """刷新配置目录卡片文案。"""
        saved_dir = _CONFIG_DIR_CACHE or get_saved_user_config_dir()
        try:
            resolved_dir = resolve_config_dir()
        except ConfigNotFoundError:
            resolved_dir = None

        if resolved_dir:
            text = (
                "当前已生效目录：\n"
                f"{resolved_dir}\n\n"
                "程序会优先读取这里的 cookie.txt 与 biz_magic.txt。"
            )
        elif saved_dir:
            text = (
                "已记录目录：\n"
                f"{saved_dir}\n\n"
                "但这里暂未同时找到 cookie.txt 与 biz_magic.txt。"
            )
        else:
            text = (
                "当前未指定目录。\n\n"
                "程序会依次在 .app 同级目录、你手动选择的目录、"
                "主目录固定配置目录 ~/.tls-shipinhao 中查找。"
            )
        self.config_path_label.setText(text)

    def choose_config_dir(self):
        """选择配置文件所在目录并记住。"""
        start_dir = _CONFIG_DIR_CACHE or get_saved_user_config_dir() or get_app_dir()
        selected_dir = QFileDialog.getExistingDirectory(self, "选择配置目录", start_dir)
        if not selected_dir:
            return

        cookie_path = os.path.join(selected_dir, COOKIE_FILE_NAME)
        magic_path = os.path.join(selected_dir, MAGIC_FILE_NAME)
        missing_files = []
        if not os.path.exists(cookie_path):
            missing_files.append(COOKIE_FILE_NAME)
        if not os.path.exists(magic_path):
            missing_files.append(MAGIC_FILE_NAME)

        if missing_files:
            self.show_message(
                QMessageBox.Warning,
                "目录不完整",
                "所选目录缺少以下文件：\n"
                + "\n".join(missing_files)
                + "\n\n请选择同时包含 cookie.txt 与 biz_magic.txt 的目录。",
            )
            return

        save_user_config_dir(selected_dir)
        self.refresh_config_path_label()
        self.show_message(
            QMessageBox.Information,
            "配置目录已更新",
            f"后续将优先使用：\n{selected_dir}",
        )

    def show_missing_config_error(self, searched_dirs):
        """提示缺少配置文件，并允许用户直接选择目录。"""
        info_text = (
            "程序会按以下顺序查找配置目录:\n"
            "1. .app 同级目录\n"
            "2. 你手动选择并记住的目录\n"
            "3. 主目录固定配置目录 ~/.tls-shipinhao\n\n"
            f"本次已检查:\n{searched_dirs}"
        )
        dialog, actions = self._create_message_dialog_base(
            QMessageBox.Warning,
            "缺少配置文件",
            "未找到配置文件 cookie.txt 或 biz_magic.txt。",
            info_text,
            min_width=620,
        )
        self._add_message_action(actions, "关闭", "MessageSecondary", dialog.reject)
        self._add_message_action(actions, "选择配置目录", "MessagePrimary", dialog.accept)
        if dialog.exec() == QDialog.Accepted:
            self.choose_config_dir()

    def on_start_clicked(self):
        """开始或继续批量处理。"""
        if self.worker is not None and self.is_paused:
            self.worker.resume()
            self.is_paused = False
            self.refresh_action_buttons()
            self.append_result_log("已继续执行剩余任务。")
            return

        if self.worker is not None:
            return

        self.normalize_inputs()

        order_ids = parse_batch_input(self.order_edit.toPlainText())
        tracking_numbers = parse_batch_input(self.tracking_edit.toPlainText())

        if not order_ids or not tracking_numbers:
            self.show_message(QMessageBox.Information, "提示", "请输入订单号和新物流单号。")
            return

        if len(order_ids) != len(tracking_numbers):
            self.show_message(
                QMessageBox.Critical,
                "数量不匹配",
                f"订单号共 {len(order_ids)} 个，新物流单号共 {len(tracking_numbers)} 个。\n"
                "请确保一一对应后再执行。",
            )
            return

        if len(order_ids) > MAX_BATCH_SIZE:
            self.show_message(
                QMessageBox.Critical,
                "超出数量限制",
                f"一次最多处理 {MAX_BATCH_SIZE} 条，请拆分后再执行。",
            )
            return

        self.clear_result_log()
        self.append_result_log(
            f"开始执行：共 {len(order_ids)} 条。输入支持空格、英文逗号、中文逗号或换行分隔。"
        )
        self.set_submit_running(True)

        self.worker_thread = QThread(self)
        self.worker = BatchWorker(order_ids, tracking_numbers)
        self.worker.moveToThread(self.worker_thread)

        self.worker_thread.started.connect(self.worker.run)
        self.worker.started.connect(self._on_worker_started)
        self.worker.step_started.connect(self._on_worker_step_started)
        self.worker.step_succeeded.connect(self._on_worker_step_succeeded)
        self.worker.step_failed.connect(self._on_worker_step_failed)
        self.worker.fatal_error.connect(self._on_worker_fatal_error)
        self.worker.missing_config.connect(self.show_missing_config_error)
        self.worker.finished.connect(self._on_worker_finished)
        self.worker.finished.connect(self.worker_thread.quit)
        self.worker_thread.finished.connect(self.worker.deleteLater)
        self.worker_thread.finished.connect(self.worker_thread.deleteLater)
        self.worker_thread.finished.connect(self._clear_worker_refs)
        self.worker_thread.start()
        self.refresh_action_buttons()

    def on_pause_clicked(self):
        """暂停后续批量任务。"""
        if self.worker is None or self.is_paused:
            return
        self.worker.pause()
        self.is_paused = True
        self.refresh_action_buttons()
        self.append_result_log("已暂停处理，当前单完成后将停止继续执行。")

    def _clear_worker_refs(self):
        """清理线程引用。"""
        self.worker = None
        self.worker_thread = None
        self.is_paused = False
        self.refresh_action_buttons()

    def _on_worker_started(self, total_count):
        """记录任务开始。"""
        self.append_result_log(f"任务已创建：共 {total_count} 条，准备顺序执行。")

    def _on_worker_step_started(self, index, total_count, order_id):
        """记录单条开始。"""
        self.append_result_log(f"[{index}/{total_count}] 开始处理订单 {order_id}")

    def _on_worker_step_succeeded(self, index, total_count, order_id, tracking_number, old_waybill):
        """记录单条成功。"""
        self.append_result_log(
            f"[{index}/{total_count}] 订单 {order_id} 成功：{old_waybill} -> {tracking_number}"
        )

    def _on_worker_step_failed(self, index, total_count, order_id, tracking_number, error_message):
        """记录单条失败。"""
        self.append_result_log(
            f"[{index}/{total_count}] 订单 {order_id} -> {tracking_number} 失败：{error_message}"
        )

    def _on_worker_fatal_error(self, error_message):
        """记录批量中断。"""
        self.append_result_log(f"批量执行中断：{error_message}")

    def _on_worker_finished(self, success_count, failure_count, total_count, aborted):
        """恢复界面并汇总结果。"""
        self.set_submit_running(False)

        if aborted:
            return

        summary = (
            f"批量执行完成：共 {total_count} 条，成功 {success_count} 条，失败 {failure_count} 条。"
        )
        self.append_result_log(summary)

        if failure_count > 0:
            self.show_message(QMessageBox.Warning, "批量执行完成", summary)
        else:
            self.show_message(QMessageBox.Information, "批量执行完成", summary)


def main():
    """程序入口。"""
    app = QApplication(sys.argv)
    app.setStyle("Fusion")
    window = MainWindow()
    window.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
