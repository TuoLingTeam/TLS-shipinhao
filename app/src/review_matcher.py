# -*- coding: utf-8 -*-
"""TLS-shipinhao 中差评订单查找器（核心逻辑）。"""

import time
from datetime import datetime, timedelta

import requests

from .constants import (
    EVALUATION_SEARCH_URL,
    ORDER_SEARCH_URL,
    REQUEST_TIMEOUT,
)


class BadReviewOrderFinder:
    """中差评订单查找器。

    通过微信小商店 API 获取差评数据和订单数据，
    使用多属性评分算法将差评匹配到对应订单。
    """

    def __init__(self, cookie: str, magic: str):
        self.cookie = cookie
        self.magic = magic
        self._stopped = False

    def stop(self):
        """请求终止（安全退出）。"""
        self._stopped = True

    # -------------------------------------------------------------------
    # HTTP 请求
    # -------------------------------------------------------------------

    def _build_headers(self, referer="https://store.weixin.qq.com/shop/evaluate/home"):
        """构建 HTTP 请求头。"""
        return {
            "Accept": "application/json, text/plain, */*",
            "Accept-Encoding": "gzip, deflate, br, zstd",
            "Accept-Language": "zh-CN,zh;q=0.9",
            "Connection": "keep-alive",
            "Content-Type": "application/json",
            "Host": "store.weixin.qq.com",
            "Origin": "https://store.weixin.qq.com",
            "Referer": referer,
            "Sec-Fetch-Dest": "empty",
            "Sec-Fetch-Mode": "cors",
            "Sec-Fetch-Site": "same-origin",
            "User-Agent": (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                "AppleWebKit/537.36 (KHTML, like Gecko) "
                "Chrome/144.0.0.0 Safari/537.36"
            ),
            "sec-ch-ua": '"Not(A:Brand";v="8", "Chromium";v="144", "Google Chrome";v="144"',
            "sec-ch-ua-mobile": "?0",
            "sec-ch-ua-platform": '"macOS"',
            "Cookie": self.cookie,
            "biz_magic": self.magic,
            "potter-scene": "weixinShop",
        }

    # -------------------------------------------------------------------
    # 获取差评
    # -------------------------------------------------------------------

    def get_bad_evaluations(self, days=30, on_progress=None):
        """获取差评数据。

        Args:
            days: 查询最近多少天的差评。
            on_progress: 可选回调 ``on_progress(message)``。

        Returns:
            差评评价列表。
        """
        end_time = int(time.time())
        start_time = int((datetime.now() - timedelta(days=days)).timestamp())

        all_bad_reviews = []
        page = 1
        max_pages = 10

        while page <= max_pages:
            if self._stopped:
                break

            if on_progress:
                on_progress(f"正在获取第 {page} 页评价...")

            params = {"token": "", "lang": "zh_CN"}
            data = {
                "orderId": "",
                "productId": "",
                "productEvaluationId": "",
                "buyerEvaluationTimeStart": start_time,
                "buyerEvaluationTimeEnd": end_time,
                "page": page,
                "status": 2,
                "visibleType": 0,
            }

            try:
                response = requests.post(
                    EVALUATION_SEARCH_URL,
                    params=params,
                    json=data,
                    headers=self._build_headers(),
                    timeout=REQUEST_TIMEOUT,
                )
            except Exception as exc:
                raise RuntimeError(f"差评请求异常: {exc}") from exc

            if response.status_code not in (200, 201):
                raise RuntimeError(f"差评请求失败: HTTP {response.status_code}")

            result = response.json()
            if result.get("code") != 0:
                raise RuntimeError(f"差评API错误: {result}")

            evaluations = result.get("finderProductEvaluationInfoList", [])

            page_bad_reviews = []
            for evaluation in evaluations:
                operation_info = evaluation.get("operationInfo", {})
                attitude_name = operation_info.get("attitudeName", "")
                can_reply_expire_time = operation_info.get("canReplyExpireTime", 0)

                if attitude_name == "不够好":
                    expire_date = datetime.fromtimestamp(can_reply_expire_time)
                    days_until_expire = (expire_date - datetime.now()).days
                    if days_until_expire >= -30:
                        page_bad_reviews.append(evaluation)

            all_bad_reviews.extend(page_bad_reviews)

            if on_progress:
                on_progress(
                    f"第 {page} 页获取到 {len(page_bad_reviews)} 条差评"
                    f"（累计: {len(all_bad_reviews)}）"
                )

            if len(evaluations) < 10:
                break

            page += 1
            time.sleep(1)

        return all_bad_reviews

    # -------------------------------------------------------------------
    # 获取订单
    # -------------------------------------------------------------------

    def get_orders(self, max_pages=None, on_progress=None):
        """获取订单数据。

        Args:
            max_pages: 最大页数限制，``None`` 表示获取全部。
            on_progress: 可选回调 ``on_progress(message)``。

        Returns:
            订单列表。
        """
        all_orders = []
        next_key = ""
        page = 1

        headers = self._build_headers("https://store.weixin.qq.com/shop/order/list")

        while True:
            if self._stopped:
                break

            if on_progress:
                on_progress(f"正在获取第 {page} 页订单...")

            params = {"token": "", "lang": "zh_CN"}
            data = {
                "pageSize": 100,
                "nextKey": next_key,
                "orderStatus": "",
                "searchType": 0,
            }

            try:
                response = requests.post(
                    ORDER_SEARCH_URL,
                    params=params,
                    json=data,
                    headers=headers,
                    timeout=REQUEST_TIMEOUT,
                )
            except Exception as exc:
                raise RuntimeError(f"订单请求异常: {exc}") from exc

            if response.status_code not in (200, 201):
                raise RuntimeError(f"订单请求失败: HTTP {response.status_code}")

            result = response.json()
            if result.get("code") != 0:
                raise RuntimeError(f"订单API错误: {result}")

            orders = result.get("orderList", [])
            next_key = result.get("nextKey", "")

            all_orders.extend(orders)

            if on_progress:
                on_progress(
                    f"第 {page} 页获取到 {len(orders)} 个订单"
                    f"（累计: {len(all_orders)}）"
                )

            if not next_key or not orders:
                break

            if max_pages and page >= max_pages:
                break

            page += 1

        return all_orders

    # -------------------------------------------------------------------
    # 昵称标准化
    # -------------------------------------------------------------------

    @staticmethod
    def normalize_nickname(nickname):
        """标准化昵称，移除 emoji 和特殊字符。"""
        if not nickname:
            return ""

        emoji_chars = [
            "🌈", "⭐", "💎", "🔥", "✨", "🎉", "🎊", "💫", "🌟",
            "❤️", "💕", "💖", "💗", "💘", "💙", "💚", "💛", "💜",
            "🧡", "🖤", "🤍", "🤎", "💯", "💢", "💥", "💫", "💦",
            "💨", "🕳️", "💣", "💬", "👁️‍🗨️", "🗨️", "🗯️", "💭", "💤",
        ]

        result = nickname
        for emoji in emoji_chars:
            result = result.replace(emoji, "")

        return result.strip()

    # -------------------------------------------------------------------
    # 匹配算法
    # -------------------------------------------------------------------

    def match_orders_with_evaluations(self, bad_evaluations, orders, on_progress=None):
        """使用多属性评分算法匹配差评到订单。

        匹配策略: 商品匹配 + 时间窗口匹配 + 买家特征匹配。

        Args:
            bad_evaluations: 差评列表。
            orders: 订单列表。
            on_progress: 可选回调 ``on_progress(message)``。

        Returns:
            匹配结果列表。
        """
        if on_progress:
            on_progress("正在构建索引...")

        # 构建多层索引 —— 按 productId + skuId 分组
        product_sku_index = {}

        for order in orders:
            order_id = order.get("commonInfo", {}).get("orderId")
            buyer_nickname = order.get("buyerInfo", {}).get("nickName", "")
            normalized_buyer_nickname = self.normalize_nickname(buyer_nickname)
            create_time = order.get("commonInfo", {}).get("createTime", 0)

            confirm_receipt_time = order.get("acceptInfo", {}).get("confirmReceiptTime", "")
            confirm_receipt_timestamp = 0
            if confirm_receipt_time and str(confirm_receipt_time).isdigit():
                confirm_receipt_timestamp = int(confirm_receipt_time)

            order_status = order.get("commonInfo", {}).get("status", 0)
            openid = order.get("commonInfo", {}).get("openid", "")

            order_products = order.get("orderProductInfo", [])
            for product in order_products:
                product_id = product.get("productId")
                sku_id = product.get("skuId")
                sale_params = product.get("saleParam", [])
                sale_param_str = "|".join(sale_params) if sale_params else ""

                if product_id and sku_id:
                    order_data = {
                        "orderId": order_id,
                        "productId": product_id,
                        "skuId": sku_id,
                        "saleParam": sale_param_str,
                        "buyerNickname": buyer_nickname,
                        "normalizedNickname": normalized_buyer_nickname,
                        "createTime": create_time,
                        "confirmReceiptTime": confirm_receipt_timestamp,
                        "orderStatus": order_status,
                        "openid": openid,
                        "orderData": order,
                    }

                    product_sku_key = f"{product_id}_{sku_id}"
                    if product_sku_key not in product_sku_index:
                        product_sku_index[product_sku_key] = []
                    product_sku_index[product_sku_key].append(order_data)

        if on_progress:
            on_progress(f"索引构建完成: {len(product_sku_index)} 个商品+SKU 组合")

        # 匹配评价
        results = []
        matched_count = 0
        total = len(bad_evaluations)

        for i, evaluation in enumerate(bad_evaluations, 1):
            if self._stopped:
                break

            eval_info = evaluation.get("evaluationInfo", {})
            product_info = evaluation.get("productInfo", {})
            operation_info = evaluation.get("operationInfo", {})

            evaluation_id = evaluation.get("productEvaluationId")
            product_id = product_info.get("productId")
            sku_id = product_info.get("skuId")
            sku_name = product_info.get("skuName", "")

            buyer_nickname = (
                eval_info.get("buyer", {}).get("identity", {}).get("nickname", "")
            )
            normalized_buyer_nickname = self.normalize_nickname(buyer_nickname)
            eval_time = (
                eval_info.get("firstEvaluationInfo", {})
                .get("buyerEvaluationInfo", {})
                .get("createTime", 0)
            )

            if on_progress:
                on_progress(f"[{i}/{total}] 匹配评价: {buyer_nickname}")

            matched_order = None
            match_strategy = None
            match_score = 0

            if product_id and sku_id:
                product_sku_key = f"{product_id}_{sku_id}"
                candidate_orders = product_sku_index.get(product_sku_key, [])

                if candidate_orders:
                    best_matches = []

                    for order_data in candidate_orders:
                        score = 0
                        reasons = []

                        # 1. 有效评价时间窗口 (最高优先级 - 35分)
                        reference_time = (
                            order_data["confirmReceiptTime"]
                            if order_data["confirmReceiptTime"] > 0
                            else order_data["createTime"]
                        )

                        if eval_time > 0 and reference_time > 0:
                            if eval_time > reference_time:
                                time_diff_hours = (eval_time - reference_time) / 3600
                                time_diff_days = time_diff_hours / 24

                                if time_diff_days <= 30:
                                    if time_diff_days <= 1:
                                        score += 35
                                        reasons.append(f"有效评价期-极及时({time_diff_days:.1f}天)")
                                    elif time_diff_days <= 7:
                                        score += 30
                                        reasons.append(f"有效评价期-很及时({time_diff_days:.1f}天)")
                                    elif time_diff_days <= 15:
                                        score += 25
                                        reasons.append(f"有效评价期-及时({time_diff_days:.1f}天)")
                                    else:
                                        score += 20
                                        reasons.append(f"有效评价期-正常({time_diff_days:.1f}天)")
                                else:
                                    continue
                            else:
                                continue
                        else:
                            continue

                        # 2. 买家身份匹配 (第二优先级 - 30分)
                        if normalized_buyer_nickname and order_data["normalizedNickname"]:
                            if normalized_buyer_nickname == order_data["normalizedNickname"]:
                                score += 30
                                reasons.append("买家昵称完全匹配")
                            elif (
                                len(normalized_buyer_nickname) >= 2
                                and len(order_data["normalizedNickname"]) >= 2
                            ):
                                if (
                                    normalized_buyer_nickname in order_data["normalizedNickname"]
                                    or order_data["normalizedNickname"] in normalized_buyer_nickname
                                ):
                                    score += 15
                                    reasons.append("买家昵称部分匹配")

                        # 3. 基础商品匹配 (第三优先级 - 20分，必须条件)
                        if sku_name in order_data["saleParam"]:
                            score += 20
                            reasons.append("商品规格完全匹配")
                        else:
                            continue

                        # 4. 订单完成状态 (第四优先级 - 10分)
                        if order_data["orderStatus"] >= 100:
                            score += 10
                            reasons.append("订单已完成")
                        elif order_data["orderStatus"] >= 60:
                            score += 5
                            reasons.append("订单已发货")
                        else:
                            reasons.append("订单未完成")

                        # 5. 评价及时性 (第五优先级 - 5分)
                        if eval_time > 0 and order_data["confirmReceiptTime"] > 0:
                            confirm_diff = abs(eval_time - order_data["confirmReceiptTime"])
                            confirm_diff_hours = confirm_diff / 3600
                            confirm_diff_days = confirm_diff_hours / 24

                            if confirm_diff < 3600:
                                score += 5
                                reasons.append(f"收货后立即评价({confirm_diff}秒)")
                            elif confirm_diff_days <= 1:
                                score += 4
                                reasons.append(f"收货后当天评价({confirm_diff_hours:.1f}小时)")
                            elif confirm_diff_days <= 7:
                                score += 3
                                reasons.append(f"收货后一周内评价({confirm_diff_days:.1f}天)")
                            else:
                                score += 2
                                reasons.append(f"收货后月内评价({confirm_diff_days:.1f}天)")

                        if score >= 40:
                            best_matches.append({
                                "order_data": order_data,
                                "score": score,
                                "reasons": reasons,
                                "time_diff": (
                                    abs(eval_time - order_data["createTime"])
                                    if eval_time > 0 and order_data["createTime"] > 0
                                    else float("inf")
                                ),
                                "confirm_diff": (
                                    abs(eval_time - order_data["confirmReceiptTime"])
                                    if eval_time > 0 and order_data["confirmReceiptTime"] > 0
                                    else float("inf")
                                ),
                            })

                    # 选择最佳匹配
                    if best_matches:
                        best_matches.sort(
                            key=lambda x: (-x["score"], x["confirm_diff"], x["time_diff"])
                        )
                        best_match = best_matches[0]

                        matched_order = best_match["order_data"]
                        match_score = best_match["score"]

                        if match_score >= 80:
                            match_strategy = "exact_match"
                        elif match_score >= 50:
                            match_strategy = "time_window"
                        elif match_score >= 30:
                            match_strategy = "buyer_feature"
                        else:
                            match_strategy = "fallback"

                        matched_count += 1

                        if on_progress:
                            on_progress(
                                f"  ✅ 匹配成功 (得分: {match_score}, "
                                f"策略: {match_strategy}) → 订单 {matched_order['orderId']}"
                            )

            if not matched_order and on_progress:
                on_progress(f"  ❌ 未找到匹配订单")

            # 构建结果
            result = {
                "evaluationId": evaluation_id,
                "orderId": matched_order["orderId"] if matched_order else None,
                "productId": product_id,
                "skuId": sku_id,
                "skuName": sku_name,
                "saleParam": matched_order["saleParam"] if matched_order else "",
                "buyerNickname": buyer_nickname,
                "orderBuyerNickname": matched_order["buyerNickname"] if matched_order else "",
                "matchStrategy": match_strategy if matched_order else None,
                "matchScore": match_score if matched_order else 0,
                "timeDiffHours": (
                    (eval_time - matched_order["createTime"]) / 3600
                    if matched_order and eval_time > 0 and matched_order["createTime"] > 0
                    else None
                ),
                "confirmDiffHours": (
                    (eval_time - matched_order["confirmReceiptTime"]) / 3600
                    if matched_order and eval_time > 0 and matched_order["confirmReceiptTime"] > 0
                    else None
                ),
                "attitudeName": operation_info.get("attitudeName", ""),
                "evaluationContent": (
                    eval_info.get("firstEvaluationInfo", {})
                    .get("buyerEvaluationInfo", {})
                    .get("content", "")
                ),
                "defaultContent": (
                    eval_info.get("firstEvaluationInfo", {})
                    .get("buyerEvaluationInfo", {})
                    .get("defaultContent", "")
                ),
                "evaluationStar": eval_info.get("evaluationStar", 0),
                "productName": product_info.get("spuName", ""),
                "canReplyExpireTime": operation_info.get("canReplyExpireTime", 0),
                "matched": matched_order is not None,
            }

            results.append(result)

        return results
