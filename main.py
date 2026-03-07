# -*- coding: utf-8 -*-
"""
订单物流信息更新工具
用于更新微信小商店订单的物流信息
"""
import json
import os
import re
import sys
import threading
from tkinter import messagebox, scrolledtext
import tkinter as tk
from tkinter import ttk
import requests

MAX_BATCH_SIZE = 10
REQUEST_TIMEOUT = 30
APP_COLORS = {
    "bg": "#EDF4FF",
    "surface": "#FFFFFF",
    "surface_soft": "#F8FBFF",
    "border": "#D7E3F4",
    "text": "#0F172A",
    "muted": "#64748B",
    "blue": "#2563EB",
    "blue_soft": "#DBEAFE",
    "orange": "#F97316",
    "orange_soft": "#FFEDD5",
    "green": "#16A34A",
    "green_soft": "#DCFCE7",
    "red": "#DC2626",
    "red_soft": "#FEE2E2",
    "slate_soft": "#E2E8F0",
    "input_bg": "#F8FAFC",
    "input_border": "#CBD5E1",
    "log_bg": "#0F172A",
    "log_fg": "#E2E8F0",
    "log_muted": "#94A3B8"
}
APP_FONTS = {}
match_value_label = None
status_badge_label = None
btn_normalize = None
btn_clear = None
progress_var = None
progress_meta_var = None
progress_note_var = None
status_var = None
status_badge_var = None


def get_app_dir():
    """获取配置文件的查找目录。
    开发时：脚本所在目录。
    打包后 Windows：可执行文件同目录。
    打包后 macOS .app：与 .app 包同路径的目录（即 .app 所在目录）。
    """
    if getattr(sys, "frozen", False):
        exe_dir = os.path.abspath(os.path.dirname(sys.executable))
        if sys.platform == "darwin":
            # 可执行文件在 xxx.app/Contents/MacOS/ 内，上两级即 .app 包根目录
            bundle_root = os.path.abspath(os.path.join(exe_dir, "..", ".."))
            if bundle_root.endswith(".app"):
                # 与 .app 同路径 = 包所在目录
                return os.path.dirname(bundle_root)
        return exe_dir
    return os.path.dirname(os.path.abspath(__file__))


def getCookie():
    """从 cookie.txt 文件读取 Cookie 信息"""
    path = os.path.join(get_app_dir(), "cookie.txt")
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
    """从 biz_magic.txt 文件读取 magic 值"""
    path = os.path.join(get_app_dir(), "biz_magic.txt")
    with open(path, "r", encoding="utf-8") as file:
        content = file.read().strip()
    return content


def build_headers(magic):
    """根据 magic 构建 HTTP 请求头。

    浏览器开发者工具里复制出来的 header 值常带有包裹引号，
    直接原样发给 requests 会导致微信接口长时间无响应。
    """
    return {
        'Accept': 'application/json, text/plain, */*',
        'Accept-Language': 'zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
        'Content-Type': 'application/json',
        'Origin': 'https://store.weixin.qq.com',
        'Pragma': 'no-cache',
        'Sec-Fetch-Dest': 'empty',
        'Sec-Fetch-Mode': 'cors',
        'Sec-Fetch-Site': 'same-origin',
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36',
        'biz_magic': magic,
        'mcn_magic': '',
        'potter-scene': 'weixinShop',
        'sec-ch-ua': '"Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144"',
        'sec-ch-ua-mobile': '?0',
        'sec-ch-ua-platform': '"Windows"',
        'supplier_magic': '',
        'talent_magic': '',
        'wecom_magic': ''
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
    """保留订单详情里的商品信息，避免更新请求丢失商品数量。"""
    product_infos = []
    for item in delivery_product_info.get("productInfos") or []:
        product_id = item.get("productId")
        sku_id = item.get("skuId")
        if product_id is None or sku_id is None:
            continue

        product_infos.append({
            "productId": product_id,
            "skuId": sku_id,
            "productCnt": item.get("productCnt", 1)
        })

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
    """解析批量输入，支持空格、英文逗号和换行分隔。"""
    return [
        item.strip()
        for item in re.split(r"[\s,，]+", raw_text.strip())
        if item.strip()
    ]


def normalize_batch_text(raw_text):
    """将批量输入规范化为一行一个值，自动去掉多余空格和逗号。"""
    return "\n".join(parse_batch_input(raw_text))


def normalize_text_widget(widget):
    """就地清洗输入框内容，删除空白行和多余分隔符。"""
    if str(widget.cget("state")) == tk.DISABLED:
        return

    normalized_text = normalize_batch_text(widget.get("1.0", tk.END))
    current_text = widget.get("1.0", tk.END).strip()
    if normalized_text == current_text:
        return

    widget.delete("1.0", tk.END)
    if normalized_text:
        widget.insert("1.0", normalized_text)


def schedule_normalize_widget(widget):
    """在粘贴结束后再清洗输入框，避免打断默认粘贴行为。"""
    root.after_idle(lambda widget=widget: normalize_and_refresh_widget(widget))


def get_font_tokens():
    """根据系统返回一组桌面端更稳妥的字体配置。"""
    if sys.platform == "darwin":
        heading_family = "SF Pro Display"
        body_family = "SF Pro Text"
        mono_family = "Menlo"
    elif sys.platform.startswith("win"):
        heading_family = "Segoe UI"
        body_family = "Segoe UI"
        mono_family = "Consolas"
    else:
        heading_family = "DejaVu Sans"
        body_family = "DejaVu Sans"
        mono_family = "DejaVu Sans Mono"

    return {
        "eyebrow": (body_family, 10, "bold"),
        "hero": (heading_family, 22, "bold"),
        "title": (heading_family, 13, "bold"),
        "stat": (heading_family, 18, "bold"),
        "body": (body_family, 12),
        "small": (body_family, 10),
        "button": (body_family, 12, "bold"),
        "mono": (mono_family, 13),
        "log": (mono_family, 11)
    }


def get_tone_palette(tone):
    """统一管理标签和状态色。"""
    tone_map = {
        "blue": (APP_COLORS["blue_soft"], APP_COLORS["blue"]),
        "orange": (APP_COLORS["orange_soft"], APP_COLORS["orange"]),
        "green": (APP_COLORS["green_soft"], APP_COLORS["green"]),
        "red": (APP_COLORS["red_soft"], APP_COLORS["red"]),
        "slate": (APP_COLORS["slate_soft"], APP_COLORS["muted"])
    }
    return tone_map.get(tone, tone_map["slate"])


def configure_app_styles():
    """配置按钮与进度条样式。"""
    style = ttk.Style()
    try:
        style.theme_use("clam")
    except tk.TclError:
        pass

    style.configure(
        "Primary.TButton",
        font=APP_FONTS["button"],
        background=APP_COLORS["orange"],
        foreground="#FFFFFF",
        borderwidth=0,
        focusthickness=0,
        padding=(20, 12)
    )
    style.map(
        "Primary.TButton",
        background=[
            ("active", "#EA580C"),
            ("disabled", "#FDBA74")
        ],
        foreground=[("disabled", "#FFF7ED")]
    )

    style.configure(
        "Ghost.TButton",
        font=APP_FONTS["button"],
        background=APP_COLORS["surface"],
        foreground=APP_COLORS["text"],
        borderwidth=1,
        focusthickness=0,
        padding=(16, 11)
    )
    style.map(
        "Ghost.TButton",
        background=[
            ("active", APP_COLORS["surface_soft"]),
            ("disabled", "#F8FAFC")
        ],
        foreground=[("disabled", "#94A3B8")]
    )

    style.configure(
        "Blue.Horizontal.TProgressbar",
        troughcolor=APP_COLORS["surface_soft"],
        background=APP_COLORS["blue"],
        lightcolor=APP_COLORS["blue"],
        darkcolor=APP_COLORS["blue"],
        bordercolor=APP_COLORS["surface_soft"]
    )


def apply_badge_tone(widget, tone):
    """给徽标类标签套用统一色盘。"""
    if widget is None:
        return
    bg_color, fg_color = get_tone_palette(tone)
    widget.configure(bg=bg_color, fg=fg_color)


def apply_stat_tone(widget, tone):
    """给统计数字切换强调色。"""
    if widget is None:
        return
    _, fg_color = get_tone_palette(tone)
    widget.configure(fg=fg_color)


def set_text_shell_tone(shell, tone="default", accent=None):
    """切换输入框外层描边状态。"""
    if tone == "focus" and accent:
        border_color = accent
    else:
        border_color = APP_COLORS["input_border"]
    shell.configure(bg=border_color)


def configure_text_editor(widget):
    """统一配置输入框和日志框外观。"""
    widget.configure(
        relief=tk.FLAT,
        bd=0,
        bg=APP_COLORS["input_bg"],
        fg=APP_COLORS["text"],
        insertbackground=APP_COLORS["blue"],
        selectbackground=APP_COLORS["blue"],
        selectforeground="#FFFFFF",
        highlightthickness=0,
        padx=14,
        pady=14,
        spacing1=1,
        spacing3=5,
        undo=True
    )

    try:
        widget.vbar.configure(
            bd=0,
            relief=tk.FLAT,
            width=12,
            bg=APP_COLORS["surface_soft"],
            activebackground=APP_COLORS["blue_soft"],
            troughcolor=APP_COLORS["bg"],
            highlightthickness=0
        )
    except tk.TclError:
        pass


def scroll_canvas_with_mousewheel(event, canvas):
    """让整页在内容溢出时支持鼠标滚轮滚动。"""
    if getattr(event, "num", None) == 4:
        canvas.yview_scroll(-1, "units")
        return "break"

    if getattr(event, "num", None) == 5:
        canvas.yview_scroll(1, "units")
        return "break"

    if not getattr(event, "delta", 0):
        return None

    if sys.platform == "darwin":
        step = -1 if event.delta > 0 else 1
    else:
        step = -int(event.delta / 120)
        if step == 0:
            step = -1 if event.delta > 0 else 1

    canvas.yview_scroll(step, "units")
    return "break"


def bind_canvas_scroll_support(widget, canvas):
    """为非文本组件补充页面级滚动支持。"""
    if not isinstance(widget, tk.Text):
        widget.bind(
            "<MouseWheel>",
            lambda event, canvas=canvas: scroll_canvas_with_mousewheel(event, canvas),
            add="+"
        )
        widget.bind(
            "<Button-4>",
            lambda event, canvas=canvas: scroll_canvas_with_mousewheel(event, canvas),
            add="+"
        )
        widget.bind(
            "<Button-5>",
            lambda event, canvas=canvas: scroll_canvas_with_mousewheel(event, canvas),
            add="+"
        )

    for child in widget.winfo_children():
        bind_canvas_scroll_support(child, canvas)


def normalize_and_refresh_widget(widget):
    """整理单个输入框后刷新批量统计。"""
    normalize_text_widget(widget)
    refresh_input_metrics()


def create_session():
    """创建复用连接的会话，批量执行时顺序请求。"""
    cookies = getCookie()
    magic = getMagic()
    session = requests.Session()
    session.headers.update(build_headers(magic))
    session.cookies.update(cookies)
    return session


def fetch_delivery_product_info(order_id, session):
    """查询单个订单详情并返回物流产品信息。"""
    url = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/detail/cgi/orderDetail"
    params = {'token': "", 'lang': "zh_CN"}
    data = json.dumps({"id": str(order_id)}, separators=(',', ':'))

    try:
        response = session.post(url, params=params, data=data, timeout=REQUEST_TIMEOUT)
    except requests.RequestException as e:
        raise RuntimeError(f"获取订单详情失败：{e}") from e

    if response.status_code != 200:
        raise RuntimeError(f"获取订单详情失败：{get_response_error(response)}")

    try:
        detail_payload = response.json()
    except ValueError as e:
        raise RuntimeError("获取订单详情失败：接口返回了非 JSON 响应。") from e

    if detail_payload.get("success") is False:
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(detail_payload, '订单详情接口返回失败。')}"
        )

    if detail_payload.get("code") not in (None, 0):
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(detail_payload, '订单详情接口返回失败。')}"
        )

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


def update_delivery_info(order_id, tracking_number, delivery_product_info, session):
    """提交单个订单的物流更新。"""
    url = "https://store.weixin.qq.com/shop-faas/mmchannelstradeorder/ship/cgi/updateOrderDeliveryInfo"
    params = {'token': "", 'lang': "zh_CN"}
    delivery_item = {
        'waybillId': str(tracking_number),
        'deliveryId': delivery_product_info.get('deliveryId'),
        'productInfos': normalize_product_infos(delivery_product_info),
        'isAllProduct': delivery_product_info.get('isAllProduct', False),
        'deliverType': delivery_product_info.get('deliverType', 1),
        'waybillStatus': delivery_product_info.get('waybillStatus', 2)
    }
    for optional_key in ('deliveryName', 'deliveryTime'):
        optional_value = delivery_product_info.get(optional_key)
        if optional_value not in (None, ""):
            delivery_item[optional_key] = optional_value

    data = {
        'orderId': str(order_id),
        'deliveryInfo': {
            'deliverType': delivery_product_info.get('deliverType', 1),
            'deliveryProductInfo': [delivery_item]
        }
    }
    data = json.dumps(data, separators=(',', ':'))

    try:
        response = session.post(url, params=params, data=data, timeout=REQUEST_TIMEOUT)
    except requests.RequestException as e:
        raise RuntimeError(f"更新物流信息失败：{e}") from e

    if response.status_code != 200:
        raise RuntimeError(f"更新物流信息失败：{get_response_error(response)}")

    try:
        result = response.json()
    except ValueError as e:
        raise RuntimeError("更新物流信息失败：接口返回了非 JSON 响应。") from e

    if result.get("success") is True:
        return

    if result.get("ret") == 0 and result.get("code") in (None, 0):
        return

    raise RuntimeError(
        f"更新物流信息失败：{get_payload_error(result, '物流信息修改失败。')}"
    )


def update_single_order(order_id, tracking_number, session):
    """顺序执行单个订单更新，返回原物流单号用于展示。"""
    delivery_product_info = fetch_delivery_product_info(order_id, session)
    old_waybill = delivery_product_info.get("waybillId", "")
    update_delivery_info(order_id, tracking_number, delivery_product_info, session)
    return old_waybill


def refresh_input_metrics():
    """刷新输入计数与匹配状态。"""
    order_count = len(parse_batch_input(text_order.get("1.0", tk.END)))
    tracking_count = len(parse_batch_input(text_tracking.get("1.0", tk.END)))

    order_count_var.set(f"{order_count}/{MAX_BATCH_SIZE}")
    tracking_count_var.set(f"{tracking_count}/{MAX_BATCH_SIZE}")

    if order_count == 0 and tracking_count == 0:
        match_state_var.set("等待输入")
        match_note_var.set("粘贴 1-10 组数据后，系统会自动整理并准备执行。")
        apply_stat_tone(match_value_label, "slate")
        return

    if order_count == tracking_count:
        match_state_var.set("数量匹配")
        match_note_var.set(f"已准备好 {order_count} 组映射，将按顺序逐条处理。")
        apply_stat_tone(match_value_label, "green")
        return

    match_state_var.set("需要修正")
    match_note_var.set(
        f"订单号 {order_count} 条，物流单号 {tracking_count} 条，请补齐后再执行。"
    )
    apply_stat_tone(match_value_label, "red")


def set_submit_running(is_running):
    """切换按钮状态，避免重复提交。"""
    widget_state = tk.DISABLED if is_running else tk.NORMAL
    for widget in (text_order, text_tracking):
        widget.configure(state=widget_state)

    if btn_normalize is not None:
        btn_normalize.configure(state=tk.DISABLED if is_running else tk.NORMAL)

    if btn_clear is not None:
        btn_clear.configure(state=tk.DISABLED if is_running else tk.NORMAL)

    btn_submit.configure(
        state=tk.DISABLED if is_running else tk.NORMAL,
        text="执行中..." if is_running else "点击开始批量处理"
    )


def set_status(text, tone="slate", badge_text=None):
    """更新界面状态栏。"""
    if status_var is not None:
        status_var.set(text)

    if status_badge_var is not None:
        status_badge_var.set(
            badge_text or {
                "blue": "执行中",
                "green": "完成",
                "orange": "提示",
                "red": "异常",
                "slate": "待执行"
            }.get(tone, "状态")
        )

    apply_badge_tone(status_badge_label, tone)


def set_progress(completed, total):
    """更新批量执行进度。"""
    if progress_var is None or progress_meta_var is None or progress_note_var is None:
        return

    if total <= 0:
        progress_var.set(0)
        progress_meta_var.set("0/0")
        progress_note_var.set("尚未开始执行")
        return

    progress_var.set((completed / total) * 100)
    progress_meta_var.set(f"{completed}/{total}")
    progress_note_var.set(f"已完成 {completed} 条，共 {total} 条")


def clear_result_log():
    """清空执行日志。"""
    text_result.configure(state=tk.NORMAL)
    text_result.delete("1.0", tk.END)
    text_result.configure(state=tk.DISABLED)


def append_result_log(text):
    """追加执行日志。"""
    text_result.configure(state=tk.NORMAL)
    text_result.insert(tk.END, text + "\n")
    text_result.see(tk.END)
    text_result.configure(state=tk.DISABLED)


def show_missing_config_error():
    """提示缺少配置文件。"""
    config_dir = get_app_dir()
    messagebox.showerror(
        "缺少配置文件",
        "未找到配置文件 cookie.txt 或 biz_magic.txt。\n\n"
        f"请将这两个文件放在以下目录（与 .app 同路径）：\n{config_dir}"
    )


def normalize_inputs():
    """手动整理两个输入框内容。"""
    normalize_and_refresh_widget(text_order)
    normalize_and_refresh_widget(text_tracking)
    set_status("输入内容已整理，可直接开始执行。", tone="blue", badge_text="已整理")


def clear_inputs():
    """清空输入与日志。"""
    for widget in (text_order, text_tracking):
        widget.configure(state=tk.NORMAL)
        widget.delete("1.0", tk.END)

    clear_result_log()
    set_progress(0, 0)
    set_status(
        "粘贴 1-10 条订单号与物流单号，系统会自动整理格式并顺序执行。",
        tone="slate",
        badge_text="待执行"
    )
    refresh_input_metrics()
    text_order.focus_set()


def announce_batch_step(index, total_count, order_id):
    """更新当前执行进度显示。"""
    set_progress(index - 1, total_count)
    set_status(
        f"执行中 {index}/{total_count}：正在处理订单 {order_id}",
        tone="blue",
        badge_text="执行中"
    )


def record_batch_success(index, total_count, order_id, tracking_number, old_waybill):
    """记录单条成功结果。"""
    set_progress(index, total_count)
    append_result_log(
        f"[{index}/{total_count}] 订单 {order_id} 成功："
        f"{old_waybill or '无原物流单号'} -> {tracking_number}"
    )


def record_batch_failure(index, total_count, order_id, tracking_number, error_message):
    """记录单条失败结果。"""
    set_progress(index, total_count)
    append_result_log(
        f"[{index}/{total_count}] 订单 {order_id} -> {tracking_number} 失败："
        f"{error_message}"
    )
    set_status(
        f"第 {index}/{total_count} 条失败，继续执行剩余任务。",
        tone="orange",
        badge_text="执行中"
    )


def finish_batch(success_count, failure_count, total_count, aborted=False):
    """恢复界面并汇总批量结果。"""
    set_submit_running(False)

    if aborted:
        set_progress(0, 0)
        return

    set_progress(total_count, total_count)
    summary = (
        f"批量执行完成：共 {total_count} 条，成功 {success_count} 条，"
        f"失败 {failure_count} 条。"
    )
    append_result_log(summary)
    set_status(
        summary,
        tone="green" if failure_count == 0 else "orange",
        badge_text="已完成"
    )

    if failure_count > 0:
        messagebox.showwarning("批量执行完成", summary)
    else:
        messagebox.showinfo("批量执行完成", summary)


def run_batch_updates(order_ids, tracking_numbers):
    """后台线程：按顺序逐条执行物流更新。"""
    success_count = 0
    failure_count = 0
    total_count = len(order_ids)

    try:
        session = create_session()
    except FileNotFoundError:
        root.after(0, show_missing_config_error)
        root.after(
            0,
            lambda: set_status(
                "执行已中止：缺少配置文件。",
                tone="red",
                badge_text="异常"
            )
        )
        root.after(0, lambda: finish_batch(0, 0, total_count, aborted=True))
        return

    try:
        with session:
            for index, (order_id, tracking_number) in enumerate(
                zip(order_ids, tracking_numbers), start=1
            ):
                root.after(
                    0,
                    lambda index=index, total_count=total_count, order_id=order_id:
                    announce_batch_step(index, total_count, order_id)
                )

                try:
                    old_waybill = update_single_order(order_id, tracking_number, session)
                except Exception as e:
                    failure_count += 1
                    root.after(
                        0,
                        lambda index=index,
                        total_count=total_count,
                        order_id=order_id,
                        tracking_number=tracking_number,
                        error_message=str(e):
                        record_batch_failure(
                            index, total_count, order_id, tracking_number, error_message
                        )
                    )
                    continue

                success_count += 1
                root.after(
                    0,
                    lambda index=index,
                    total_count=total_count,
                    order_id=order_id,
                    tracking_number=tracking_number,
                    old_waybill=old_waybill:
                    record_batch_success(
                        index, total_count, order_id, tracking_number, old_waybill
                    )
                )
    except Exception as e:
        failure_count += (total_count - success_count - failure_count)
        root.after(
            0,
            lambda error_message=str(e): (
                append_result_log(f"批量执行中断：{error_message}"),
                set_status(
                    f"批量执行中断：{error_message}",
                    tone="red",
                    badge_text="异常"
                )
            )
        )
    finally:
        root.after(
            0,
            lambda: finish_batch(success_count, failure_count, total_count)
        )


def on_submit():
    """按钮点击事件处理函数 - 批量顺序更新物流信息。"""
    normalize_inputs()

    order_ids = parse_batch_input(text_order.get("1.0", tk.END))
    tracking_numbers = parse_batch_input(text_tracking.get("1.0", tk.END))

    if not order_ids or not tracking_numbers:
        set_status("请先填写订单号和新物流单号。", tone="orange", badge_text="待补充")
        messagebox.showinfo("提示", "请输入订单号和新物流单号！")
        return

    if len(order_ids) != len(tracking_numbers):
        set_status("输入数量不一致，请先修正再执行。", tone="red", badge_text="需修正")
        messagebox.showerror(
            "数量不匹配",
            f"订单号共 {len(order_ids)} 个，新物流单号共 {len(tracking_numbers)} 个。\n"
            "请确保一一对应后再执行。"
        )
        return

    if len(order_ids) > MAX_BATCH_SIZE:
        set_status("超出单次处理上限，请拆分后再执行。", tone="red", badge_text="超出上限")
        messagebox.showerror(
            "超出数量限制",
            f"一次最多处理 {MAX_BATCH_SIZE} 条，请拆分后再执行。"
        )
        return

    clear_result_log()
    set_progress(0, len(order_ids))
    append_result_log(
        f"开始执行：共 {len(order_ids)} 条。输入支持空格、英文逗号或换行分隔。"
    )
    set_status(
        f"任务已创建：共 {len(order_ids)} 条，准备顺序执行。",
        tone="blue",
        badge_text="执行中"
    )
    set_submit_running(True)

    worker = threading.Thread(
        target=run_batch_updates,
        args=(order_ids, tracking_numbers),
        daemon=True
    )
    worker.start()


if __name__ == "__main__":
    root = tk.Tk()
    root.title("驼铃视频小店中差评处理")
    root.geometry("1240x860")
    root.minsize(880, 680)
    root.resizable(True, True)
    root.configure(bg=APP_COLORS["bg"])

    APP_FONTS = get_font_tokens()
    configure_app_styles()

    main_frame = tk.Frame(root, bg=APP_COLORS["bg"], padx=24, pady=22)
    main_frame.pack(fill=tk.BOTH, expand=True)
    main_frame.columnconfigure(0, weight=1)
    main_frame.rowconfigure(1, weight=3)
    main_frame.rowconfigure(3, weight=2)

    header_card = tk.Frame(
        main_frame,
        bg=APP_COLORS["surface"],
        highlightthickness=1,
        highlightbackground=APP_COLORS["border"]
    )
    header_card.grid(row=0, column=0, sticky="ew")
    tk.Frame(header_card, bg=APP_COLORS["blue"], height=5).pack(fill=tk.X)

    header_body = tk.Frame(header_card, bg=APP_COLORS["surface"], padx=22, pady=18)
    header_body.pack(fill=tk.X)
    header_body.columnconfigure(0, weight=1)

    title_wrap = tk.Frame(header_body, bg=APP_COLORS["surface"])
    title_wrap.grid(row=0, column=0, sticky="w")
    tk.Label(
        title_wrap,
        text="驼铃视频小店中差评处理",
        font=APP_FONTS["hero"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["text"]
    ).pack(anchor="w")
    title_description_label = tk.Label(
        title_wrap,
        text="批量填写订单号，自动化批量处理中差评。",
        font=APP_FONTS["body"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["muted"],
        justify=tk.LEFT
    )
    title_description_label.pack(anchor="w", pady=(6, 0))

    meta_wrap = tk.Frame(header_body, bg=APP_COLORS["surface"])
    meta_wrap.grid(row=0, column=1, sticky="e")
    author_badge = tk.Label(
        meta_wrap,
        text="作者微信：TLS-801",
        font=APP_FONTS["small"],
        bg=APP_COLORS["blue_soft"],
        fg=APP_COLORS["blue"],
        padx=10,
        pady=6
    )
    author_badge.pack()

    order_count_var = tk.StringVar(value=f"0/{MAX_BATCH_SIZE}")
    tracking_count_var = tk.StringVar(value=f"0/{MAX_BATCH_SIZE}")
    match_state_var = tk.StringVar(value="等待输入")
    match_note_var = tk.StringVar(value="粘贴 1-10 组数据后，系统会自动整理并准备执行。")
    input_frame = tk.Frame(main_frame, bg=APP_COLORS["bg"])
    input_frame.grid(row=1, column=0, sticky="nsew", pady=(16, 0))
    input_frame.columnconfigure(0, weight=1, uniform="input")
    input_frame.columnconfigure(1, weight=1, uniform="input")
    input_frame.rowconfigure(0, weight=1)

    order_card = tk.Frame(
        input_frame,
        bg=APP_COLORS["surface"],
        highlightthickness=1,
        highlightbackground=APP_COLORS["border"]
    )
    order_card.grid(row=0, column=0, sticky="nsew", padx=(0, 10))
    tk.Frame(order_card, bg=APP_COLORS["blue"], height=4).pack(fill=tk.X)
    order_body = tk.Frame(order_card, bg=APP_COLORS["surface"], padx=18, pady=16)
    order_body.pack(fill=tk.BOTH, expand=True)
    order_header = tk.Frame(order_body, bg=APP_COLORS["surface"])
    order_header.pack(fill=tk.X)
    tk.Label(
        order_header,
        text="填写订单号",
        font=APP_FONTS["title"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["text"]
    ).pack(side=tk.LEFT)
    order_badge = tk.Label(
        order_header,
        textvariable=order_count_var,
        font=APP_FONTS["small"],
        bg=APP_COLORS["blue_soft"],
        fg=APP_COLORS["blue"],
        padx=10,
        pady=5
    )
    order_badge.pack(side=tk.RIGHT)
    order_hint_label = tk.Label(
        order_body,
        text="支持空格、英文逗号、换行分隔；建议一行一个，便于核对。",
        font=APP_FONTS["small"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["muted"],
        justify=tk.LEFT
    )
    order_hint_label.pack(anchor="w", pady=(8, 12))
    order_shell = tk.Frame(order_body, bg=APP_COLORS["input_border"])
    order_shell.pack(fill=tk.BOTH, expand=True)
    text_order = scrolledtext.ScrolledText(order_shell, wrap=tk.CHAR, height=10, width=1)
    configure_text_editor(text_order)
    text_order.configure(font=APP_FONTS["mono"])
    text_order.pack(fill=tk.BOTH, expand=True)

    tracking_card = tk.Frame(
        input_frame,
        bg=APP_COLORS["surface"],
        highlightthickness=1,
        highlightbackground=APP_COLORS["border"]
    )
    tracking_card.grid(row=0, column=1, sticky="nsew", padx=(10, 0))
    tk.Frame(tracking_card, bg=APP_COLORS["orange"], height=4).pack(fill=tk.X)
    tracking_body = tk.Frame(tracking_card, bg=APP_COLORS["surface"], padx=18, pady=16)
    tracking_body.pack(fill=tk.BOTH, expand=True)
    tracking_header = tk.Frame(tracking_body, bg=APP_COLORS["surface"])
    tracking_header.pack(fill=tk.X)
    tk.Label(
        tracking_header,
        text="填写物流单号",
        font=APP_FONTS["title"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["text"]
    ).pack(side=tk.LEFT)
    tracking_badge = tk.Label(
        tracking_header,
        textvariable=tracking_count_var,
        font=APP_FONTS["small"],
        bg=APP_COLORS["orange_soft"],
        fg=APP_COLORS["orange"],
        padx=10,
        pady=5
    )
    tracking_badge.pack(side=tk.RIGHT)
    tracking_hint_label = tk.Label(
        tracking_body,
        text="支持空格、英文逗号、换行分隔；建议一行一个，便于核对。",
        font=APP_FONTS["small"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["muted"],
        justify=tk.LEFT
    )
    tracking_hint_label.pack(anchor="w", pady=(8, 12))
    tracking_shell = tk.Frame(tracking_body, bg=APP_COLORS["input_border"])
    tracking_shell.pack(fill=tk.BOTH, expand=True)
    text_tracking = scrolledtext.ScrolledText(tracking_shell, wrap=tk.CHAR, height=10, width=1)
    configure_text_editor(text_tracking)
    text_tracking.configure(font=APP_FONTS["mono"])
    text_tracking.pack(fill=tk.BOTH, expand=True)

    set_input_cards_layout(input_frame, order_card, tracking_card, is_stacked=False)

    status_badge_var = tk.StringVar(value="待执行")
    status_var = tk.StringVar(value="粘贴 1-10 条订单号与物流单号，系统会自动整理格式并顺序执行。")
    progress_meta_var = tk.StringVar(value="0/0")
    progress_note_var = tk.StringVar(value="尚未开始执行")
    progress_var = tk.DoubleVar(value=0)

    button_row = tk.Frame(main_frame, bg=APP_COLORS["bg"])
    button_row.grid(row=2, column=0, sticky="ew", pady=(16, 0))
    button_row.columnconfigure(0, weight=1)

    btn_submit = ttk.Button(
        button_row,
        text="点击开始批量处理",
        command=on_submit,
        style="Primary.TButton"
    )
    btn_submit.grid(row=0, column=0, sticky="ew", ipady=4)

    console_card = tk.Frame(
        main_frame,
        bg=APP_COLORS["surface"],
        highlightthickness=1,
        highlightbackground=APP_COLORS["border"]
    )
    console_card.grid(row=3, column=0, sticky="nsew", pady=(16, 0))
    tk.Frame(console_card, bg=APP_COLORS["blue"], height=4).pack(fill=tk.X)
    console_body = tk.Frame(console_card, bg=APP_COLORS["surface"], padx=18, pady=16)
    console_body.pack(fill=tk.BOTH, expand=True)
    console_header = tk.Frame(console_body, bg=APP_COLORS["surface"])
    console_header.pack(fill=tk.X)
    tk.Label(
        console_header,
        text="执行日志",
        font=APP_FONTS["title"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["text"]
    ).pack(side=tk.LEFT)
    console_hint_label = tk.Label(
        console_header,
        text="最近执行记录会按时间顺序滚动显示。",
        font=APP_FONTS["small"],
        bg=APP_COLORS["surface"],
        fg=APP_COLORS["muted"],
        justify=tk.RIGHT
    )
    console_hint_label.pack(side=tk.RIGHT)
    log_shell = tk.Frame(console_body, bg=APP_COLORS["log_bg"])
    log_shell.pack(fill=tk.BOTH, expand=True, pady=(12, 0))
    text_result = scrolledtext.ScrolledText(
        log_shell,
        width=1,
        height=13,
        wrap=tk.WORD,
        state=tk.DISABLED
    )
    text_result.configure(
        relief=tk.FLAT,
        bd=0,
        bg=APP_COLORS["log_bg"],
        fg=APP_COLORS["log_fg"],
        insertbackground=APP_COLORS["log_fg"],
        selectbackground=APP_COLORS["blue"],
        selectforeground="#FFFFFF",
        highlightthickness=0,
        padx=14,
        pady=14,
        spacing3=4,
        font=APP_FONTS["log"]
    )
    try:
        text_result.vbar.configure(
            bd=0,
            relief=tk.FLAT,
            width=12,
            bg="#1E293B",
            activebackground="#334155",
            troughcolor=APP_COLORS["log_bg"],
            highlightthickness=0
        )
    except tk.TclError:
        pass
    text_result.pack(fill=tk.BOTH, expand=True)

    for widget, shell, accent in (
        (text_order, order_shell, APP_COLORS["blue"]),
        (text_tracking, tracking_shell, APP_COLORS["orange"])
    ):
        widget.bind(
            "<FocusIn>",
            lambda event, shell=shell, accent=accent: set_text_shell_tone(shell, "focus", accent)
        )
        widget.bind(
            "<FocusOut>",
            lambda event, widget=widget, shell=shell: (
                normalize_and_refresh_widget(widget),
                set_text_shell_tone(shell)
            )
        )
        widget.bind(
            "<<Paste>>",
            lambda event, widget=widget: schedule_normalize_widget(widget)
        )
        widget.bind("<KeyRelease>", lambda event: refresh_input_metrics())

    current_layout = {"stacked": False}

    def refresh_responsive_layout(event=None):
        content_width = max(main_frame.winfo_width() - 48, 720)
        should_stack = root.winfo_width() < 1080

        if should_stack != current_layout["stacked"]:
            set_input_cards_layout(input_frame, order_card, tracking_card, should_stack)
            current_layout["stacked"] = should_stack

        column_width = content_width - 36 if should_stack else max((content_width - 20) // 2, 320)
        title_description_label.configure(wraplength=max(420, content_width - 240))
        order_hint_label.configure(wraplength=max(300, column_width - 64))
        tracking_hint_label.configure(wraplength=max(300, column_width - 64))
        console_hint_label.configure(wraplength=max(260, min(420, content_width // 3)))

    root.bind("<Configure>", refresh_responsive_layout)
    root.after_idle(refresh_responsive_layout)

    apply_badge_tone(status_badge_label, "slate")
    refresh_input_metrics()
    set_progress(0, 0)
    text_order.focus_set()
    root.mainloop()
