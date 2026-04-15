# -*- coding: utf-8 -*-
"""订单匹配评分规则。

规则：
1. 买家昵称按原样参与匹配，不做清洗。
2. 商品信息同时使用标题、商品 ID、SKU ID。
3. 昵称和商品信息均完全一致时，得分 100。
4. 一边完全一致、另一边相似时，按相似度档位扣分。
5. 两边都仅部分相似时，取两侧扣分均值，最低 50 分。
"""

from __future__ import annotations

import re
from difflib import SequenceMatcher

from services.order_field_utils import normalize_product_text

SIMILARITY_PENALTY_BANDS: tuple[tuple[int, int], ...] = (
    (100, 0),
    (90, 5),
    (80, 10),
    (70, 15),
    (60, 20),
    (50, 25),
    (40, 30),
    (30, 35),
    (20, 40),
    (10, 45),
    (0, 50),
)

PRODUCT_ID_WEIGHT = 40
PRODUCT_SKU_WEIGHT = 40
PRODUCT_TITLE_WEIGHT = 20
PRODUCT_SIMILARITY_WEIGHT_TOTAL = PRODUCT_ID_WEIGHT + PRODUCT_SKU_WEIGHT + PRODUCT_TITLE_WEIGHT

MIN_MATCH_SCORE = 50
TRAILING_DIGIT_CHARS = "0-9０-９⁰¹²³⁴⁵⁶⁷⁸⁹₀₁₂₃₄₅₆₇₈₉"


def clamp_percent(value: float | int) -> int:
    """将相似度裁剪到 0~100。"""
    try:
        numeric = round(float(value))
    except (TypeError, ValueError):
        return 0
    return max(0, min(100, numeric))


def _sequence_similarity(left: str, right: str) -> int:
    """基于 SequenceMatcher 计算百分比相似度。"""
    return clamp_percent(SequenceMatcher(None, left, right).ratio() * 100)


def similarity_percent(left: str | None, right: str | None) -> int:
    """昵称相似度（0~100）。"""
    left_text = "" if left is None else str(left)
    right_text = "" if right is None else str(right)
    if left_text == right_text:
        return 100
    if not left_text or not right_text:
        return 0

    left_trimmed = left_text.strip()
    right_trimmed = right_text.strip()
    if left_trimmed and left_trimmed == right_trimmed:
        return 95

    stripped_similarity = _nickname_similarity_by_rename_patterns(left_trimmed, right_trimmed)
    if stripped_similarity is not None:
        return stripped_similarity

    return _sequence_similarity(left_text, right_text)


def _strip_trailing_digit_tail(text: str) -> str:
    """移除昵称尾部常见数字尾巴，仅用于相似度识别。"""
    if not text:
        return ""
    return re.sub(rf"[{TRAILING_DIGIT_CHARS}\s]+$", "", text).strip()


def _is_subsequence(shorter: str, longer: str) -> bool:
    """判断 shorter 是否按顺序出现在 longer 中。"""
    if not shorter:
        return False
    pos = 0
    for char in longer:
        if pos < len(shorter) and char == shorter[pos]:
            pos += 1
            if pos == len(shorter):
                return True
    return False


def _single_char_containment_similarity(longer: str) -> int:
    """单字命中时使用保守相似度，避免被误判为高相似改名。"""
    normalized_length = max(len(longer or ""), 3)
    return clamp_percent(100 / normalized_length)


def _subsequence_similarity_by_length(text: str) -> int | None:
    """按较短文本长度返回子序列改名场景相似度。"""
    length = len(text)
    if length >= 4:
        return 85
    if length == 3:
        return 80
    if length == 2:
        return 70
    return None


def _nickname_similarity_by_rename_patterns(left: str, right: str) -> int | None:
    """针对改昵称场景的特化相似度规则。"""
    if not left or not right:
        return None

    left_core = _strip_trailing_digit_tail(left)
    right_core = _strip_trailing_digit_tail(right)

    if left_core and right_core and left_core == right_core and left != right:
        if len(left_core) >= 2:
            return 95
        return 80

    shorter, longer = (left, right) if len(left) <= len(right) else (right, left)
    shorter_core, longer_core = (left_core, right_core) if len(left_core) <= len(right_core) else (right_core, left_core)

    if shorter and shorter in longer:
        if len(shorter) >= 3:
            return 90
        if len(shorter) == 2:
            return 80
        return _single_char_containment_similarity(longer)

    if shorter_core and shorter_core in longer_core:
        if len(shorter_core) >= 3:
            return 90
        if len(shorter_core) == 2:
            return 80
        return _single_char_containment_similarity(longer_core)

    direct_similarity = _subsequence_similarity_by_length(shorter)
    if direct_similarity is not None and _is_subsequence(shorter, longer):
        return direct_similarity

    core_similarity = _subsequence_similarity_by_length(shorter_core)
    if core_similarity is not None and _is_subsequence(shorter_core, longer_core):
        return core_similarity

    return None


def normalize_product_title_for_similarity(title: str | None) -> str:
    """仅用于商品标题相似度计算的轻量规范化。"""
    return normalize_product_text(title)


def title_similarity_percent(left: str | None, right: str | None) -> int:
    """商品标题相似度。"""
    left_text = "" if left is None else str(left)
    right_text = "" if right is None else str(right)
    if left_text == right_text:
        return 100
    left_norm = normalize_product_title_for_similarity(left_text)
    right_norm = normalize_product_title_for_similarity(right_text)
    if not left_norm or not right_norm:
        return 0
    if left_norm == right_norm:
        return 100
    return _sequence_similarity(left_norm, right_norm)


def penalty_from_similarity(similarity: int) -> int:
    """根据相似度返回扣分。"""
    value = clamp_percent(similarity)
    for minimum, penalty in SIMILARITY_PENALTY_BANDS:
        if value >= minimum:
            return penalty
    return 50


def _weighted_product_similarity(product_id_similarity: int, sku_id_similarity: int, title_similarity: int) -> int:
    """按约定权重计算商品综合相似度。"""
    weighted = (
        product_id_similarity * PRODUCT_ID_WEIGHT
        + sku_id_similarity * PRODUCT_SKU_WEIGHT
        + title_similarity * PRODUCT_TITLE_WEIGHT
    ) / PRODUCT_SIMILARITY_WEIGHT_TOTAL
    return clamp_percent(weighted)


def compute_product_similarity(
    *,
    evaluation_product_id: str | None,
    evaluation_sku_id: str | None,
    evaluation_title: str | None,
    order_product_id: str | None,
    order_sku_id: str | None,
    order_title: str | None,
) -> dict[str, int | bool]:
    """计算商品综合相似度。"""
    eval_product_id = "" if evaluation_product_id is None else str(evaluation_product_id)
    eval_sku_id = "" if evaluation_sku_id is None else str(evaluation_sku_id)
    eval_title = "" if evaluation_title is None else str(evaluation_title)
    order_product_id = "" if order_product_id is None else str(order_product_id)
    order_sku_id = "" if order_sku_id is None else str(order_sku_id)
    order_title = "" if order_title is None else str(order_title)

    product_id_exact = bool(eval_product_id) and eval_product_id == order_product_id
    sku_id_exact = bool(eval_sku_id) and eval_sku_id == order_sku_id
    title_exact = bool(eval_title) and eval_title == order_title

    product_id_similarity = 100 if product_id_exact else 0
    sku_id_similarity = 100 if sku_id_exact else 0
    title_similarity = title_similarity_percent(eval_title, order_title)

    weighted_similarity = _weighted_product_similarity(
        product_id_similarity,
        sku_id_similarity,
        title_similarity,
    )

    product_exact = product_id_exact and sku_id_exact and title_exact
    product_similarity = 100 if product_exact else min(99, weighted_similarity)

    return {
        "productExact": product_exact,
        "productIdExact": product_id_exact,
        "skuIdExact": sku_id_exact,
        "titleExact": title_exact,
        "titleSimilarity": title_similarity,
        "productSimilarity": product_similarity,
    }


def compute_match_score(
    *,
    evaluation_buyer_nickname: str | None,
    evaluation_product_id: str | None,
    evaluation_sku_id: str | None,
    evaluation_title: str | None,
    order_buyer_nickname: str | None,
    order_product_id: str | None,
    order_sku_id: str | None,
    order_title: str | None,
) -> dict[str, int | bool]:
    """计算买家昵称 + 商品信息匹配得分。"""
    eval_buyer = "" if evaluation_buyer_nickname is None else str(evaluation_buyer_nickname)
    order_buyer = "" if order_buyer_nickname is None else str(order_buyer_nickname)

    buyer_nickname_exact = bool(eval_buyer) and eval_buyer == order_buyer
    buyer_nickname_similarity = similarity_percent(eval_buyer, order_buyer)

    product_result = compute_product_similarity(
        evaluation_product_id=evaluation_product_id,
        evaluation_sku_id=evaluation_sku_id,
        evaluation_title=evaluation_title,
        order_product_id=order_product_id,
        order_sku_id=order_sku_id,
        order_title=order_title,
    )

    buyer_penalty = penalty_from_similarity(buyer_nickname_similarity)
    product_penalty = penalty_from_similarity(int(product_result["productSimilarity"]))

    if buyer_nickname_exact and bool(product_result["productExact"]):
        score = 100
    elif bool(product_result["productExact"]):
        score = max(MIN_MATCH_SCORE, 100 - buyer_penalty)
    elif buyer_nickname_exact:
        score = max(MIN_MATCH_SCORE, 100 - product_penalty)
    else:
        score = max(MIN_MATCH_SCORE, 100 - round((buyer_penalty + product_penalty) / 2))

    return {
        "buyerNicknameExact": buyer_nickname_exact,
        "buyerNicknameSimilarity": buyer_nickname_similarity,
        "buyerNicknamePenalty": buyer_penalty,
        "productPenalty": product_penalty,
        "score": score,
        **product_result,
    }
