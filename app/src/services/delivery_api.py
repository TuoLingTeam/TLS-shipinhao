# -*- coding: utf-8 -*-
"""TLS-shipinhao 微信小商店 API 交互（物流更新）。"""

import copy
import json
import logging

import requests

from ..config import get_cookie, get_magic
from ..constants import (
    DELIVERY_MISMATCH_MESSAGE,
    ORDER_DELIVERY_UPDATE_URL,
    ORDER_DETAIL_URL,
    ORDER_INIT_SHIP_DATA_URL,
    REQUEST_TIMEOUT,
)
from ..core.http_utils import (
    build_headers,
    build_request_params,
    get_payload_error,
    get_response_error,
)

ORDER_LIST_REFERER = "https://store.weixin.qq.com/shop/order/list"
_SNAPSHOT_MISSING_MARKERS = (
    "没有可更新的物流信息",
    "缺少承运商信息",
    "缺少商品信息",
)
_DELIVERY_MISMATCH_MARKERS = (
    DELIVERY_MISMATCH_MESSAGE,
    "快递单号有误",
)


def _post_session_json_payload(
    session,
    url,
    payload,
    error_prefix,
    *,
    order_id="",
    tracking_number="",
    log_label="",
):
    """使用现有会话发送 JSON 字符串并返回解析后的响应。"""
    params = build_request_params()
    data = json.dumps(payload, separators=(",", ":"))

    try:
        response = session.post(url, params=params, data=data, timeout=REQUEST_TIMEOUT)
    except requests.RequestException as exc:
        raise RuntimeError(f"{error_prefix}：{exc}") from exc

    if response.status_code != 200:
        if log_label:
            logging.error(
                "%s http_error status=%s order_id=%s tracking=%s payload=%s body=%s",
                log_label,
                response.status_code,
                order_id,
                tracking_number,
                json.dumps(payload, ensure_ascii=False)[:1000],
                response.text[:1000],
            )
        raise RuntimeError(f"{error_prefix}：{get_response_error(response)}")

    try:
        return response.json()
    except ValueError as exc:
        if log_label:
            logging.error(
                "%s parse_error order_id=%s tracking=%s payload=%s body=%s",
                log_label,
                order_id,
                tracking_number,
                json.dumps(payload, ensure_ascii=False)[:1000],
                response.text[:1000],
            )
        raise RuntimeError(f"{error_prefix}：接口返回了非 JSON 响应。") from exc


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
                "productId": str(product_id),
                "skuId": str(sku_id),
                "productCnt": item.get("productCnt", 1),
            }
        )
    return product_infos


def create_session():
    """创建复用连接的会话。"""
    cookies = get_cookie()
    magic = get_magic(cookies)
    session = requests.Session()
    session.headers.update(build_headers(magic, referer=ORDER_LIST_REFERER))
    session.cookies.update(cookies)
    return session


def fetch_init_ship_data_payload(order_id, session):
    """拉取发货初始化数据响应。"""
    payload = _post_session_json_payload(
        session,
        ORDER_INIT_SHIP_DATA_URL,
        {"id": str(order_id)},
        "获取订单详情失败",
        order_id=str(order_id),
        log_label="init ship data",
    )

    if payload.get("success") is False:
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(payload, '发货初始化接口返回失败。')}"
        )

    if payload.get("code") not in (None, 0):
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(payload, '发货初始化接口返回失败。')}"
        )

    return payload


def fetch_order_detail_payload(order_id, session):
    """拉取完整订单详情响应。"""
    detail_payload = _post_session_json_payload(
        session,
        ORDER_DETAIL_URL,
        {"id": str(order_id)},
        "获取订单详情失败",
        order_id=str(order_id),
        log_label="order detail",
    )

    if detail_payload.get("success") is False:
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(detail_payload, '订单详情接口返回失败。')}"
        )

    if detail_payload.get("code") not in (None, 0):
        raise RuntimeError(
            f"获取订单详情失败：{get_payload_error(detail_payload, '订单详情接口返回失败。')}"
        )

    return detail_payload


def _extract_delivery_snapshot(delivery_product_info):
    """把接口里的物流产品信息标准化为统一快照。"""
    delivery_id = delivery_product_info.get("deliveryId")
    if delivery_id in (None, ""):
        raise RuntimeError("获取订单详情失败：订单详情缺少承运商信息（deliveryId）。")

    product_infos = normalize_product_infos(delivery_product_info)
    if not product_infos:
        raise RuntimeError("获取订单详情失败：订单详情缺少商品信息，无法更新物流。")

    return {
        "deliveryId": str(delivery_id),
        "deliveryName": str(delivery_product_info.get("deliveryName") or ""),
        "waybillId": str(delivery_product_info.get("waybillId") or ""),
        "productInfos": product_infos,
    }


def _extract_raw_delivery_product_info_from_init_ship_data(payload):
    """从 initShipData 响应中提取原始物流对象。"""
    delivery_product_list = (
        payload.get("orderDetail", {})
        .get("expressInfo", {})
        .get("deliveryProductInfo")
        or []
    )
    if not delivery_product_list:
        raise RuntimeError("获取订单详情失败：订单详情中没有可更新的物流信息。")
    return delivery_product_list[0]


def _extract_raw_delivery_product_info_from_order_detail(payload):
    """从 orderDetail 响应中提取原始物流对象。"""
    delivery_product_list = payload.get("expressInfo", {}).get("deliveryProductInfo") or []
    if not delivery_product_list:
        raise RuntimeError("获取订单详情失败：订单详情中没有可更新的物流信息。")
    return delivery_product_list[0]


def _build_delivery_context(delivery_product_info):
    """构建原始物流对象与标准快照的组合上下文。"""
    return {
        "raw": copy.deepcopy(delivery_product_info),
        "snapshot": _extract_delivery_snapshot(delivery_product_info),
    }


def extract_delivery_snapshot_from_init_ship_data(payload):
    """从 initShipData 响应中提取旧物流快照。"""
    return _extract_delivery_snapshot(_extract_raw_delivery_product_info_from_init_ship_data(payload))


def extract_delivery_snapshot_from_order_detail(payload):
    """从 orderDetail 响应中提取旧物流快照。"""
    return _extract_delivery_snapshot(_extract_raw_delivery_product_info_from_order_detail(payload))


def _is_missing_snapshot_error(exc):
    return any(marker in str(exc) for marker in _SNAPSHOT_MISSING_MARKERS)


def _is_delivery_mismatch_error(exc):
    """判断是否为快递公司不匹配类错误（可通过切换 deliveryId 重试）。"""
    return any(marker in str(exc) for marker in _DELIVERY_MISMATCH_MARKERS)


def fetch_current_delivery_context(order_id, session):
    """优先通过 initShipData 获取旧物流上下文，失败后回退到 orderDetail。"""
    init_error = None

    try:
        raw_info = _extract_raw_delivery_product_info_from_init_ship_data(
            fetch_init_ship_data_payload(order_id, session)
        )
        return _build_delivery_context(raw_info)
    except RuntimeError as exc:
        init_error = exc

    try:
        raw_info = _extract_raw_delivery_product_info_from_order_detail(
            fetch_order_detail_payload(order_id, session)
        )
        return _build_delivery_context(raw_info)
    except RuntimeError as detail_exc:
        if init_error and _is_missing_snapshot_error(init_error) and _is_missing_snapshot_error(detail_exc):
            raise RuntimeError("获取订单详情失败：订单详情中没有可更新的物流信息。") from detail_exc
        raise detail_exc


def fetch_current_delivery_snapshot(order_id, session):
    """优先通过 initShipData 获取旧物流快照，失败后回退到 orderDetail。"""
    return fetch_current_delivery_context(order_id, session)["snapshot"]


def fetch_delivery_product_info(order_id, session):
    """兼容旧调用名称，返回原始物流对象。"""
    return fetch_current_delivery_context(order_id, session)["raw"]


def build_delivery_candidates(order_id, tracking_number, delivery_product_info, session):
    """构建当前单号的 deliveryId 候选列表。

    默认先沿用订单原始 deliveryId，只有物流不匹配时再回退到单号前缀推导值。
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

    add_candidate(delivery_product_info.get("deliveryId"), delivery_product_info.get("deliveryName"))
    tracking_prefix = str(tracking_number).strip()[:2]
    add_candidate(tracking_prefix, delivery_product_info.get("deliveryName"))
    return candidates


def build_update_delivery_payload(order_id, tracking_number, old_delivery_product_info, delivery_override=None):
    """组装物流修改请求体（与 exe 行为对齐，仅修改 waybillId）。"""
    old_info = copy.deepcopy(old_delivery_product_info)
    new_info = copy.deepcopy(old_delivery_product_info)
    new_info["waybillId"] = str(tracking_number).strip()

    if delivery_override and delivery_override.get("deliveryId") not in (None, ""):
        new_info["deliveryId"] = str(delivery_override["deliveryId"])
    if delivery_override and delivery_override.get("deliveryName") not in (None, ""):
        new_info["deliveryName"] = str(delivery_override["deliveryName"])

    return {
        "orderId": str(order_id).strip(),
        "changeInfo": [{"old": old_info, "new": new_info}],
    }


def update_delivery_info(order_id, tracking_number, delivery_product_info, session, delivery_override=None):
    """提交单个订单的物流更新。"""
    result = _post_session_json_payload(
        session,
        ORDER_DELIVERY_UPDATE_URL,
        build_update_delivery_payload(
            order_id,
            tracking_number,
            delivery_product_info,
            delivery_override=delivery_override,
        ),
        "更新物流信息失败",
        order_id=str(order_id),
        tracking_number=str(tracking_number).strip(),
        log_label="update delivery",
    )

    if result.get("success") is True:
        return

    if result.get("code") == 0 and result.get("errcode") == 0:
        return

    if result.get("errcode") is None and result.get("ret") == 0 and result.get("code") in (None, 0):
        return

    for key in ("errmsg", "message", "msg"):
        value = result.get(key)
        if value:
            raise RuntimeError(f"更新物流信息失败：{value}")
    raise RuntimeError(f"更新物流信息失败：物流更新失败：{result}")


def update_single_order(order_id, tracking_number, session):
    """顺序执行单个订单更新。

    使用 orderDetail 获取原始物流信息后只修改 waybillId，
    不主动改 deliveryId/deliveryName（与平台手动操作行为一致）。
    仅当服务器明确返回快递公司不匹配错误时，才用单号前缀重试。
    """
    detail = fetch_order_detail_payload(order_id, session)
    info_list = detail.get("expressInfo", {}).get("deliveryProductInfo") or []
    if not info_list:
        raise RuntimeError("获取订单详情失败：订单详情中没有可更新的物流信息。")
    delivery_product_info = info_list[0]
    old_waybill = str(delivery_product_info.get("waybillId") or "")

    try:
        update_delivery_info(order_id, tracking_number, delivery_product_info, session)
        return old_waybill
    except RuntimeError as exc:
        if not _is_delivery_mismatch_error(exc):
            raise

    tracking_prefix = str(tracking_number).strip()[:2]
    current_delivery_id = str(delivery_product_info.get("deliveryId") or "")
    if tracking_prefix and tracking_prefix != current_delivery_id:
        override = {"deliveryId": tracking_prefix, "deliveryName": ""}
        update_delivery_info(
            order_id, tracking_number, delivery_product_info, session, override,
        )
        return old_waybill

    raise RuntimeError("更新物流信息失败：快递单号与物流商不匹配，且无法自动映射。")
