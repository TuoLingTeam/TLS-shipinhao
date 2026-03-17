# -*- coding: utf-8 -*-
"""TLS-shipinhao 微信小商店 API 交互（物流更新）。"""

import json

import requests

from ..config import get_cookie, get_magic
from ..constants import (
    DELIVERY_MISMATCH_MESSAGE,
    ORDER_DELIVERY_UPDATE_URL,
    ORDER_DETAIL_URL,
    REQUEST_TIMEOUT,
)
from .http_utils import (
    build_headers,
    build_request_params,
    get_payload_error,
    get_response_error,
)


def _post_session_json_payload(session, url, payload, error_prefix):
    """使用现有会话发送 JSON 字符串并返回解析后的响应。"""
    params = build_request_params()
    data = json.dumps(payload, separators=(",", ":"))

    try:
        response = session.post(url, params=params, data=data, timeout=REQUEST_TIMEOUT)
    except requests.RequestException as exc:
        raise RuntimeError(f"{error_prefix}：{exc}") from exc

    if response.status_code != 200:
        raise RuntimeError(f"{error_prefix}：{get_response_error(response)}")

    try:
        return response.json()
    except ValueError as exc:
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
                "productId": product_id,
                "skuId": sku_id,
                "productCnt": item.get("productCnt", 1),
            }
        )
    return product_infos


def create_session():
    """创建复用连接的会话。"""
    cookies = get_cookie()
    magic = get_magic(cookies)
    session = requests.Session()
    session.headers.update(build_headers(magic))
    session.cookies.update(cookies)
    return session


def fetch_order_detail_payload(order_id, session):
    """拉取完整订单详情响应。"""
    detail_payload = _post_session_json_payload(
        session,
        ORDER_DETAIL_URL,
        {"id": str(order_id)},
        "获取订单详情失败",
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

    result = _post_session_json_payload(
        session,
        ORDER_DELIVERY_UPDATE_URL,
        {
            "orderId": str(order_id),
            "deliveryInfo": {
                "deliverType": delivery_product_info.get("deliverType", 1),
                "deliveryProductInfo": [delivery_item],
            },
        },
        "更新物流信息失败",
    )

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
