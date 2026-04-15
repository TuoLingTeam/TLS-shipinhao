# -*- coding: utf-8 -*-
"""订单字段与文本归一化的纯函数工具。"""

from __future__ import annotations

import re
from typing import Any, Mapping, Sequence

TRAILING_DIGIT_CHARS = "0-9０-９⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉"


def first_non_empty(data: Mapping[str, Any], keys: Sequence[str], default: Any = "") -> Any:
    """从多个候选字段中取第一个非空值。"""
    for key in keys:
        value = data.get(key)
        if isinstance(value, str):
            stripped = value.strip()
            if stripped:
                return stripped
            continue
        if value not in (None, [], {}):
            return value
    return default


def normalize_sale_param(raw_value: Any) -> str:
    """将 saleParam / skuName 等规格字段统一为纯文本。"""
    if isinstance(raw_value, list):
        return "|".join(str(v).strip() for v in raw_value if str(v).strip())
    if raw_value is None:
        return ""
    return str(raw_value).strip()


def parse_confirm_receipt_timestamp(confirm_receipt_time: Any) -> int:
    """解析 confirmReceiptTime（字符串秒级时间戳）为 int。"""
    if confirm_receipt_time and str(confirm_receipt_time).isdigit():
        return int(confirm_receipt_time)
    return 0


def parse_timestamp(raw_value: Any) -> int:
    """将原始时间值转为秒级时间戳，自动处理毫秒。"""
    if raw_value is None:
        return 0
    raw_text = str(raw_value).strip()
    if not raw_text.isdigit():
        return 0
    timestamp = int(raw_text)
    if timestamp > 9_999_999_999:
        timestamp //= 1000
    return timestamp


def normalize_product_text(text: str | None) -> str:
    """标准化商品字段值，便于跨字段名比较。"""
    if not text:
        return ""
    return re.sub(r"[\s，,、/\-_|（）()]+", "", str(text)).lower()


def split_sku_tokens(raw_text: str) -> list[str]:
    """按常见分隔符拆分规格文本。"""
    if not raw_text:
        return []
    return [token.strip() for token in re.split(r"[，,、/\-_ |]+", raw_text) if token.strip()]
