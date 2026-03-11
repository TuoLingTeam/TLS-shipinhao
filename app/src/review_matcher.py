# -*- coding: utf-8 -*-
"""TLS-shipinhao 中差评订单查找器（核心逻辑）。"""

import re
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
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
            time.sleep(0.3)

        return all_bad_reviews

    # -------------------------------------------------------------------
    # 获取订单
    # -------------------------------------------------------------------

    def get_orders(
        self,
        max_pages=None,
        earliest_time=0,
        create_time_start=0,
        create_time_end=0,
        on_progress=None,
    ):
        """获取订单数据（单线程）。

        Args:
            max_pages: 最大页数限制，``None`` 表示获取全部。
            earliest_time: 订单时间下限（Unix 时间戳）。当某页中所有订单
                的 ``createTime`` 均早于此值时，提前终止翻页。设为 0 则不启用。
            create_time_start: 筛选订单创建时间起始（Unix 时间戳），0 = 不限。
            create_time_end: 筛选订单创建时间截止（Unix 时间戳），0 = 不限。
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
            if create_time_start > 0:
                data["createTimeStart"] = create_time_start
            if create_time_end > 0:
                data["createTimeEnd"] = create_time_end

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

            # 429 频率限制自动重试（指数退避）
            if response.status_code == 429:
                for retry in range(3):
                    wait = 2 ** (retry + 1)  # 2, 4, 8 秒
                    if on_progress:
                        on_progress(f"触发频率限制，等待 {wait} 秒后重试...")
                    time.sleep(wait)
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
                    if response.status_code != 429:
                        break
                else:
                    raise RuntimeError("订单API持续频率限制，请稍后再试")

            if response.status_code not in (200, 201):
                raise RuntimeError(f"订单请求失败: HTTP {response.status_code}")

            result = response.json()
            if result.get("code") == 429:
                for retry in range(3):
                    wait = 2 ** (retry + 1)
                    if on_progress:
                        on_progress(f"触发频率限制，等待 {wait} 秒后重试...")
                    time.sleep(wait)
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
                    result = response.json()
                    if result.get("code") != 429:
                        break
                else:
                    raise RuntimeError("订单API持续频率限制，请稍后再试")

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

            # 时间窗口早停：该页全部订单均早于阈值 → 停止翻页
            if earliest_time > 0 and orders:
                latest_in_page = max(
                    o.get("commonInfo", {}).get("createTime", 0) for o in orders
                )
                if latest_in_page < earliest_time:
                    if on_progress:
                        on_progress(
                            f"后续订单已超出时间窗口，提前结束（已获取 {len(all_orders)} 个订单）"
                        )
                    break

            page += 1
            time.sleep(0.3)  # 翻页限速，防止触发 429

        return all_orders

    def get_orders_concurrent(
        self, earliest_time=0, num_workers=3, on_progress=None
    ):
        """多线程并行获取订单数据（基于 page 参数）。

        测试证实微信订单 API 实际上支持标准的 ``page`` 参数（忽略 nextKey）。
        因此采用多线程共享自增 ``page`` 计数器的模型，动态分配页码去并行拉取，
        直到遇到无数据的页面或触发时间窗口早停。

        Args:
            earliest_time: 订单时间下限（Unix 时间戳）。当某页订单全部
                早于此值时，触发全局早停。设为 0 则不启用。
            num_workers: 并发线程数（推荐 3~5）。
            on_progress: 可选回调 ``on_progress(message)``。

        Returns:
            去重后的订单列表。
        """
        import threading
        from concurrent.futures import ThreadPoolExecutor, as_completed

        if on_progress:
            on_progress(
                f"启动 {num_workers} 个线程并行通过页码拉取订单..."
            )

        all_orders_lock = threading.Lock()
        page_lock = threading.Lock()

        # 共享状态
        shared_state = {
            "all_orders": [],
            "next_page": 1,
            "stop_event": threading.Event(),
            "errors": [],
        }

        # 预构防 429 请求头
        headers = self._build_headers("https://store.weixin.qq.com/shop/order/list")

        def _worker_loop(worker_id):
            tag = f"[订单线程{worker_id}]"
            while not shared_state["stop_event"].is_set() and not self._stopped:
                # 获取要拉取的页码
                with page_lock:
                    current_page = shared_state["next_page"]
                    shared_state["next_page"] += 1

                params = {"token": "", "lang": "zh_CN"}
                data = {
                    "pageSize": 100,
                    "page": current_page,  # 关键：使用 page 替代 nextKey
                    "nextKey": "",
                    "orderStatus": "",
                    "searchType": 0,
                }

                if on_progress and current_page % 5 == 1:
                    # 避免日志过多，每 5 页打印一次
                    on_progress(f"{tag} 正在获取第 {current_page} 页订单...")

                try:
                    response = requests.post(
                        ORDER_SEARCH_URL,
                        params=params,
                        json=data,
                        headers=headers,
                        timeout=REQUEST_TIMEOUT,
                    )
                except Exception as exc:
                    shared_state["errors"].append(f"{tag} 第 {current_page} 页异常: {exc}")
                    shared_state["stop_event"].set()
                    break

                # 429 防御
                if response.status_code == 429:
                    retry_success = False
                    for retry in range(3):
                        wait = 2 ** (retry + 1)
                        if on_progress:
                            on_progress(f"{tag} 触发429限流，等待 {wait}s 后重试第 {current_page} 页...")
                        time.sleep(wait)
                        try:
                            response = requests.post(
                                ORDER_SEARCH_URL,
                                params=params,
                                json=data,
                                headers=headers,
                                timeout=REQUEST_TIMEOUT,
                            )
                        except Exception:
                            pass
                        if response.status_code != 429:
                            retry_success = True
                            break
                    if not retry_success:
                        shared_state["errors"].append(f"{tag} 持续429限流，强制终止")
                        shared_state["stop_event"].set()
                        break

                if response.status_code not in (200, 201):
                    shared_state["errors"].append(f"{tag} 请求失败: HTTP {response.status_code}")
                    shared_state["stop_event"].set()
                    break

                result = response.json()
                # API内部可能返回 code=429
                if result.get("code") == 429:
                    retry_success = False
                    for retry in range(3):
                        wait = 2 ** (retry + 1)
                        if on_progress:
                            on_progress(f"{tag} 触发429限流(API)，等待 {wait}s 后重试...")
                        time.sleep(wait)
                        try:
                            response = requests.post(
                                ORDER_SEARCH_URL,
                                params=params,
                                json=data,
                                headers=headers,
                                timeout=REQUEST_TIMEOUT,
                            )
                            result = response.json()
                        except Exception:
                            pass
                        if result.get("code") != 429:
                            retry_success = True
                            break
                    if not retry_success:
                        shared_state["errors"].append(f"{tag} 持续429限流(API)，强制终止")
                        shared_state["stop_event"].set()
                        break

                if result.get("code") != 0:
                    shared_state["errors"].append(f"{tag} API错误: {result}")
                    shared_state["stop_event"].set()
                    break

                orders = result.get("orderList", [])

                # 如果当前页为空，说明到底了，通知所有线程停止
                if not orders:
                    shared_state["stop_event"].set()
                    if on_progress:
                        on_progress(f"{tag} 第 {current_page} 页为空，订单已全部拉取完毕。")
                    break

                with all_orders_lock:
                    shared_state["all_orders"].extend(orders)

                # 判断是否触发时间早停
                # 取该页所有订单的最大（最新）创建时间
                if earliest_time > 0:
                    latest_in_page = max(
                        o.get("commonInfo", {}).get("createTime", 0) for o in orders
                    )
                    if latest_in_page < earliest_time:
                        shared_state["stop_event"].set()
                        if on_progress:
                            on_progress(f"{tag} 第 {current_page} 页订单均早于筛选时间，触发早停。")
                        break

                # 翻页限速
                time.sleep(0.3)

        # 启动线程池
        with ThreadPoolExecutor(max_workers=num_workers, thread_name_prefix="order") as pool:
            futures = [pool.submit(_worker_loop, i + 1) for i in range(num_workers)]
            for f in as_completed(futures):
                pass

        if shared_state["errors"]:
            raise RuntimeError("; ".join(shared_state["errors"]))

        # 按 orderId 去重
        seen = set()
        merged = []
        for order in shared_state["all_orders"]:
            oid = order.get("commonInfo", {}).get("orderId")
            if oid and oid not in seen:
                seen.add(oid)
                merged.append(order)

        if on_progress:
            on_progress(
                f"并发拉取结束：共覆盖 {shared_state['next_page'] - 1} 页，"
                f"去重后 {len(merged)} 个订单。"
            )

        return merged

    # -------------------------------------------------------------------
    # 昵称标准化
    # -------------------------------------------------------------------

    # 通用昵称前缀列表（以这些开头的均视为无效昵称，得 0 分）
    _GENERIC_NICKNAME_PREFIXES = ("匿名", "微信用户", "默认昵称")

    @classmethod
    def _is_generic_nickname(cls, name: str) -> bool:
        """判断昵称是否为通用名（空昵称或以通用前缀开头的均视为通用名）。"""
        if not name:
            return True
        return any(name.startswith(prefix) for prefix in cls._GENERIC_NICKNAME_PREFIXES)

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

            # confirmReceiptTime 单位：秒（字符串格式，需转 int）
            confirm_receipt_time = order.get("acceptInfo", {}).get("confirmReceiptTime", "")
            confirm_receipt_timestamp = 0
            if confirm_receipt_time and str(confirm_receipt_time).isdigit():
                confirm_receipt_timestamp = int(confirm_receipt_time)

            # 优化1：已送达信息（快递到达但买家未手动确认收货时的兜底 reference_time）
            auto_confirm_info = order.get("orderStatus", {}).get("autoConfirmInfo", {})
            is_waybill_received = bool(auto_confirm_info.get("isWaybillReceived", False))
            waybill_received_time = int(auto_confirm_info.get("waybillReceivedTime", 0) or 0)

            order_status = order.get("commonInfo", {}).get("status", 0)
            # 优化3：isEducationOrder：教育培训类商品首评时效 60 天，普通商品 30 天
            is_education_order = bool(order.get("commonInfo", {}).get("isEducationOrder", False))
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
                        "isWaybillReceived": is_waybill_received,
                        "waybillReceivedTime": waybill_received_time,
                        "isEducationOrder": is_education_order,
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

                        # ==========================================================
                        # 权重重构：核心三大维度决定入围资格 (满分 100)
                        # 副维度（时间差等）仅用作加分项与同分排序依据 (>100分)
                        # ==========================================================

                        # 优化1：[前置拦截 1]：确定 reference_time
                        # 已确认收货：直接使用 confirmReceiptTime
                        # 已送达未签收：以快递到达时间（waybillReceivedTime）兜底
                        # 两者都没有：无法评价，直接淘汰
                        if order_data["confirmReceiptTime"] <= 0:
                            if not order_data["isWaybillReceived"] or order_data["waybillReceivedTime"] <= 0:
                                continue
                            reference_time = order_data["waybillReceivedTime"]
                        else:
                            reference_time = order_data["confirmReceiptTime"]

                        # 优化2：[前置拦截 2]：评价时间不能早于 reference_time（改用 < 允许同秒评价）
                        if eval_time <= 0 or reference_time <= 0 or eval_time < reference_time:
                            continue

                        # 优化3+9：[前置拦截 3]：时效超期拦截（提前至维度计算前，节省计算资源）
                        # 教育培训类商品首评时效 60 天，普通商品 30 天
                        max_eval_days = 60 if order_data["isEducationOrder"] else 30
                        time_diff_days = (eval_time - reference_time) / 86400
                        if time_diff_days > max_eval_days:
                            continue

                        # 优化7：=== 核心维度 1: 买家昵称匹配 (基础分 Max 60分) ===
                        # 通用名（以"匿名"、"微信用户"、"默认昵称"等前缀开头）视为无效，得 0 分
                        if self._is_generic_nickname(normalized_buyer_nickname):
                            reasons.append("买家昵称为匿名/通用(0分)")
                        else:
                            if normalized_buyer_nickname == order_data["normalizedNickname"]:
                                score += 60
                                reasons.append("买家昵称完全吻合(+60)")
                            elif (len(normalized_buyer_nickname) >= 2 and len(order_data["normalizedNickname"]) >= 2):
                                if (normalized_buyer_nickname in order_data["normalizedNickname"] or
                                        order_data["normalizedNickname"] in normalized_buyer_nickname):
                                    score += 30
                                    reasons.append("买家昵称部分吻合(+30)")

                        # 优化4+8：=== 核心维度 2: 基础商品对应 (基础分 Max 30分) ===
                        # 空值防御：sku_name 或 saleParam 为空则无法判断规格，直接一票否决
                        if not sku_name or not order_data["saleParam"]:
                            continue

                        matched_sku = False
                        if sku_name in order_data["saleParam"]:
                            matched_sku = True
                        else:
                            # 多分隔符拆分：支持 ，,、/-_|空格 等常见格式，拆分后忽略首尾空格
                            sku_parts = [
                                p for p in re.split(r"[，,、/\-_ |]+", sku_name) if p.strip()
                            ]
                            if sku_parts and all(p in order_data["saleParam"] for p in sku_parts):
                                matched_sku = True

                        if matched_sku:
                            score += 30
                            reasons.append("商品规格一致(+30)")
                        else:
                            continue  # 一票否决：规格对不上直接跳过这个订单

                        # === 核心维度 3: 订单完成状态 (基础分 Max 10分) ===
                        # orderStatus >= 100：订单已彻底完成（确认收货且交易结束）
                        # 60 <= orderStatus < 100：已发货或待评价阶段
                        # orderStatus < 60：未发货或已取消，不加分
                        if order_data["orderStatus"] >= 100:
                            score += 10
                            reasons.append("订单已彻底完成(+10)")
                        elif order_data["orderStatus"] >= 60:
                            score += 5
                            reasons.append("订单已发货或待评价(+5)")

                        # -----------------------------------------------------------------
                        # 至此，三个核心条件全部满足最高标准 (60+30+10 = 100)，
                        # 达到 100 分的基础及格线，能够自动填入 UI
                        # -----------------------------------------------------------------

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
                                    abs(eval_time - reference_time)
                                    if eval_time > 0 and reference_time > 0
                                    else float("inf")
                                ),
                            })

                    # 优化10+5：当存在多个候选订单时，在"辅助维度"实施时间差减分逻辑
                    if len(best_matches) > 1:
                        for bm in best_matches:
                            diff_days = bm["confirm_diff"] / 86400
                            # 每晚评价1天扣除 2分（round 比 int 精度更符合语义）
                            penalty = round(diff_days * 2)
                            if penalty > 0:
                                bm["score"] -= penalty
                                bm["reasons"].append(f"同源多单时效偏差(-{penalty})")

                        # 扣分后二次过滤：仅保留分数仍 >= 40 的候选
                        filtered = [bm for bm in best_matches if bm["score"] >= 40]
                        if filtered:
                            best_matches = filtered
                        # 若全部跌破 40 分，保留原列表并取分数最高者（不丢弃唯一可能的结果）

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
