# -*- coding: utf-8 -*-
"""订单字段提取与归一化纯函数。"""

from __future__ import annotations

import re


def first_non_empty(data, keys):
    for key in keys:
        value = data.get(key)
        if isinstance(value, str):
            value = value.strip()
            if value:
                return value
            continue
        if value not in (None, [], {}):
            return value
    return ""


def normalize_sale_param(raw_value):
    if isinstance(raw_value, list):
        return "|".join(str(v).strip() for v in raw_value if str(v).strip())
    if raw_value is None:
        return ""
    return str(raw_value).strip()


def parse_confirm_receipt_timestamp(value) -> int:
    if value is None:
        return 0
    text = str(value).strip()
    return int(text) if text.isdigit() else 0


def parse_timestamp(value) -> int:
    if value in (None, ""):
        return 0
    text = str(value).strip()
    if not text.isdigit():
        return 0
    parsed = int(text)
    if parsed > 10**12:
        parsed //= 1000
    return parsed


def normalize_product_text(value) -> str:
    text = str(value or "").strip().lower()
    text = re.sub(r"[\s\-_/|,，、]+", "", text)
    return text


def split_sku_tokens(value) -> list[str]:
    tokens = re.split(r"[|/,，、]+", str(value or ""))
    return [token.strip() for token in tokens if token.strip()]
