# -*- coding: utf-8 -*-
"""TLS-shipinhao  订单查找器（核心逻辑）。"""

import re
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timedelta
from typing import Any, Callable

import requests

from .constants import (
    AUTO_FILL_SCORE_THRESHOLD,
    EDUCATION_ORDER_MAX_DAYS,
    EVALUATION_MAX_DAYS,
    EVALUATION_MAX_PAGES,
    EVALUATION_SEARCH_URL,
    FETCH_PAGE_INTERVAL_SECONDS,
    MATCH_MIN_SCORE,
    MULTI_ORDER_PENALTY_FACTOR,
    MULTI_ORDER_PENALTY_MAX,
    ORDER_PAGE_SIZE,
    ORDER_SEARCH_URL,
    QUALITY_REFUND_ORDER_URL,
    REQUEST_TIMEOUT,
    RATE_LIMIT_RETRY_COUNT,
    SCORE_WEIGHTS,
)

ProgressCallback = Callable[[str], None]
JsonDict = dict[str, Any]
JsonList = list[JsonDict]

DEFAULT_REQUEST_PARAMS = {"token": "", "lang": "zh_CN"}
EVALUATION_REFERER = "https://store.weixin.qq.com/shop/evaluate/home"
ORDER_LIST_REFERER = "https://store.weixin.qq.com/shop/order/list"
QUALITY_REFUND_REFERER = (
    "https://store.weixin.qq.com/shop/setting/"
    "ratedetail?type=product&key=productQualityRatio_30d&detail=order"
)
ORDER_PROGRESS_PAGE_INTERVAL = 5
QUALITY_REFUND_REQUEST_METHODS = ("GET", "POST")

# 达到该分数才认为“可匹配”
PADDING_MAX = 12
# 达到该分数才自动填入订单号，低于该分数需要人工核对
MULTI_ORDER_PENALTY_MAX = 12


class BadReviewOrderFinder:
    """中差评订单查找器。

    通过微信小商店 API 获取差评数据和订单数据，
    使用多属性评分算法将差评匹配到对应订单。
    """

    def __init__(self, cookie: str, magic: str):
        self.cookie: str = cookie
        self.magic: str = magic
        self._stopped: bool = False

    def stop(self):
        """请求终止（安全退出）。"""
        self._stopped = True

    # -------------------------------------------------------------------
    # HTTP 请求
    # -------------------------------------------------------------------

    def _build_headers(
        self,
        referer: str = EVALUATION_REFERER,
    ) -> dict[str, str]:
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

    @staticmethod
    def _build_request_params() -> dict[str, str]:
        """构建通用请求参数。"""
        return dict(DEFAULT_REQUEST_PARAMS)

    @staticmethod
    def _build_evaluation_search_payload(start_time: int, end_time: int, page: int) -> JsonDict:
        """构建差评搜索请求体。"""
        return {
            "orderId": "",
            "productId": "",
            "productEvaluationId": "",
            "buyerEvaluationTimeStart": start_time,
            "buyerEvaluationTimeEnd": end_time,
            "page": page,
            "status": 2,
            "visibleType": 0,
        }

    @staticmethod
    def _build_order_search_payload(
        *,
        next_key: str = "",
        page: int | None = None,
        create_time_start: int = 0,
        create_time_end: int = 0,
    ) -> JsonDict:
        """构建订单搜索请求体。"""
        data = {
            "pageSize": ORDER_PAGE_SIZE,
            "nextKey": next_key,
            "orderStatus": "",
            "searchType": 0,
        }
        if page is not None:
            data["page"] = page
        if create_time_start > 0:
            data["createTimeStart"] = create_time_start
        if create_time_end > 0:
            data["createTimeEnd"] = create_time_end
        return data

    def _post_json(self, url: str, data: JsonDict, headers: JsonDict):
        """发送通用 POST JSON 请求。"""
        return requests.post(
            url,
            params=self._build_request_params(),
            json=data,
            headers=headers,
            timeout=REQUEST_TIMEOUT,
        )

    def _request_quality_refund_result(
        self,
        on_progress: ProgressCallback | None = None,
    ) -> JsonDict:
        """请求品质退款订单接口，优先 GET，必要时回退 POST。"""
        headers = self._build_headers(QUALITY_REFUND_REFERER)
        errors = []

        for method in QUALITY_REFUND_REQUEST_METHODS:
            try:
                response = requests.request(
                    method=method,
                    url=QUALITY_REFUND_ORDER_URL,
                    params=self._build_request_params(),
                    json={} if method == "POST" else None,
                    headers=headers,
                    timeout=REQUEST_TIMEOUT,
                )
            except Exception as exc:  # noqa: BLE001
                errors.append(f"{method} 请求异常: {exc}")
                continue

            if response.status_code not in (200, 201):
                errors.append(f"{method} 请求失败: HTTP {response.status_code}")
                continue

            try:
                result = response.json()
            except Exception as exc:  # noqa: BLE001
                errors.append(f"{method} 响应解析失败: {exc}")
                continue

            if result.get("code") == 0:
                if on_progress:
                    on_progress(f"[品退] 使用 {method} 请求成功。")
                return result

            errors.append(f"{method} API错误: {result}")

        raise RuntimeError("；".join(errors) if errors else "未知错误")

    @staticmethod
    def _latest_create_time(orders: JsonList) -> int:
        """返回当前订单列表中的最新下单时间。"""
        if not orders:
            return 0
        return max(o.get("commonInfo", {}).get("createTime", 0) for o in orders)

    def _is_page_outside_earliest_time(self, orders: JsonList, earliest_time: int) -> bool:
        """判断当前页订单是否全部早于筛选下限。"""
        if earliest_time <= 0 or not orders:
            return False
        return self._latest_create_time(orders) < earliest_time

    @staticmethod
    def _filter_orders_by_earliest_time(orders: JsonList, earliest_time: int) -> JsonList:
        """按下单时间下限过滤订单。"""
        if earliest_time <= 0:
            return list(orders)

        filtered = []
        for order in orders:
            create_time = int(order.get("commonInfo", {}).get("createTime", 0) or 0)
            if create_time >= earliest_time:
                filtered.append(order)
        return filtered

    def _merge_quality_refund_orders(
        self,
        orders: JsonList,
        earliest_time: int = 0,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
        """将品质退款订单并入订单列表。"""
        base_orders = self._deduplicate_orders_by_id(orders)
        quality_refund_orders = self.get_quality_refund_orders(
            earliest_time=earliest_time,
            on_progress=on_progress,
        )
        if not quality_refund_orders:
            return base_orders

        merged_orders = self._deduplicate_orders_by_id(base_orders + quality_refund_orders)
        added_count = len(merged_orders) - len(base_orders)
        if on_progress:
            on_progress(
                f"[品退] 已并入 {added_count} 个订单"
                f"（接口返回 {len(quality_refund_orders)} 个）。"
            )
        return merged_orders

    # -------------------------------------------------------------------
    # 获取差评
    # -------------------------------------------------------------------

    def get_bad_evaluations(
        self,
        days: int = 30,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
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

        while page <= EVALUATION_MAX_PAGES:
            if self._stopped:
                break

            if on_progress:
                on_progress(f"正在获取第 {page} 页评价...")

            data = self._build_evaluation_search_payload(start_time, end_time, page)

            try:
                response = self._post_json(
                    EVALUATION_SEARCH_URL,
                    data,
                    self._build_headers(),
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
            time.sleep(FETCH_PAGE_INTERVAL_SECONDS)

        return all_bad_reviews

    # -------------------------------------------------------------------
    # 获取订单
    # -------------------------------------------------------------------

    def get_orders(
        self,
        max_pages: int | None = None,
        earliest_time: int = 0,
        create_time_start: int = 0,
        create_time_end: int = 0,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
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

        headers = self._build_headers(ORDER_LIST_REFERER)

        while True:
            if self._stopped:
                break

            if on_progress:
                on_progress(f"正在获取第 {page} 页订单...")

            data = self._build_order_search_payload(
                next_key=next_key,
                create_time_start=create_time_start,
                create_time_end=create_time_end,
            )

            try:
                response = self._post_json(
                    ORDER_SEARCH_URL,
                    data,
                    headers,
                )
            except Exception as exc:
                raise RuntimeError(f"订单请求异常: {exc}") from exc

            # 429 频率限制自动重试（指数退避）
            if response.status_code == 429:
                for retry in range(RATE_LIMIT_RETRY_COUNT):
                    wait = 2 ** (retry + 1)  # 2, 4, 8 秒
                    if on_progress:
                        on_progress(f"触发频率限制，等待 {wait} 秒后重试...")
                    time.sleep(wait)
                    try:
                        response = self._post_json(
                            ORDER_SEARCH_URL,
                            data,
                            headers,
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
                for retry in range(RATE_LIMIT_RETRY_COUNT):
                    wait = 2 ** (retry + 1)
                    if on_progress:
                        on_progress(f"触发频率限制，等待 {wait} 秒后重试...")
                    time.sleep(wait)
                    try:
                        response = self._post_json(
                            ORDER_SEARCH_URL,
                            data,
                            headers,
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
            if self._is_page_outside_earliest_time(orders, earliest_time):
                if on_progress:
                    on_progress(
                        f"后续订单已超出时间窗口，提前结束（已获取 {len(all_orders)} 个订单）"
                    )
                break

            page += 1
            time.sleep(FETCH_PAGE_INTERVAL_SECONDS)  # 翻页限速，防止触发 429

        return self._merge_quality_refund_orders(
            all_orders,
            earliest_time=earliest_time,
            on_progress=on_progress,
        )

    def get_orders_concurrent(
        self,
        earliest_time: int = 0,
        num_workers: int = 3,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
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
        if on_progress:
            on_progress(
                f"启动 {num_workers} 个线程并行通过页码拉取订单..."
            )

        all_orders_lock = threading.Lock()
        page_lock = threading.Lock()

        # 共享状态
        shared_state: dict[str, Any] = {
            "all_orders": [],
            "next_page": 1,
            "stop_event": threading.Event(),
            "errors": [],
        }

        # 预构防 429 请求头
        headers = self._build_headers(ORDER_LIST_REFERER)

        def _worker_loop(worker_id: int) -> None:
            tag = f"[订单线程{worker_id}]"
            while not shared_state["stop_event"].is_set() and not self._stopped:
                # 获取要拉取的页码
                with page_lock:
                    current_page = shared_state["next_page"]
                    shared_state["next_page"] += 1

                data = self._build_order_search_payload(
                    page=current_page,  # 关键：使用 page 替代 nextKey
                )

                if on_progress and current_page % ORDER_PROGRESS_PAGE_INTERVAL == 1:
                    # 避免日志过多，每 5 页打印一次
                    on_progress(f"{tag} 正在获取第 {current_page} 页订单...")

                try:
                    response = self._post_json(
                        ORDER_SEARCH_URL,
                        data,
                        headers,
                    )
                except Exception as exc:
                    shared_state["errors"].append(f"{tag} 第 {current_page} 页异常: {exc}")
                    shared_state["stop_event"].set()
                    break

                # 429 防御
                if response.status_code == 429:
                    retry_success = False
                    for retry in range(RATE_LIMIT_RETRY_COUNT):
                        wait = 2 ** (retry + 1)
                        if on_progress:
                            on_progress(f"{tag} 触发429限流，等待 {wait}s 后重试第 {current_page} 页...")
                        time.sleep(wait)
                        try:
                            response = self._post_json(
                                ORDER_SEARCH_URL,
                                data,
                                headers,
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
                    for retry in range(RATE_LIMIT_RETRY_COUNT):
                        wait = 2 ** (retry + 1)
                        if on_progress:
                            on_progress(f"{tag} 触发429限流(API)，等待 {wait}s 后重试...")
                        time.sleep(wait)
                        try:
                            response = self._post_json(
                                ORDER_SEARCH_URL,
                                data,
                                headers,
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
                if self._is_page_outside_earliest_time(orders, earliest_time):
                    shared_state["stop_event"].set()
                    if on_progress:
                        on_progress(f"{tag} 第 {current_page} 页订单均早于筛选时间，触发早停。")
                    break

                # 翻页限速
                time.sleep(FETCH_PAGE_INTERVAL_SECONDS)

        # 启动线程池
        with ThreadPoolExecutor(max_workers=num_workers, thread_name_prefix="order") as pool:
            futures = [pool.submit(_worker_loop, i + 1) for i in range(num_workers)]
            for _ in as_completed(futures):
                pass

        if shared_state["errors"]:
            raise RuntimeError("; ".join(shared_state["errors"]))

        # 按 orderId 去重，并补充品质退款订单来源
        merged = self._merge_quality_refund_orders(
            shared_state["all_orders"],
            earliest_time=earliest_time,
            on_progress=on_progress,
        )

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
    _GENERIC_NICKNAME_PREFIXES: tuple[str, ...] = ("匿名", "微信用户", "默认昵称")

    @classmethod
    def _is_generic_nickname(cls, name: str) -> bool:
        """判断昵称是否为通用名（空昵称或以通用前缀开头的均视为通用名）。"""
        if not name:
            return True
        return any(name.startswith(prefix) for prefix in cls._GENERIC_NICKNAME_PREFIXES)

    @staticmethod
    def normalize_nickname(nickname: str | None) -> str:
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

    @staticmethod
    def _parse_confirm_receipt_timestamp(confirm_receipt_time: Any) -> int:
        """解析 confirmReceiptTime（字符串秒级时间戳）为 int。"""
        if confirm_receipt_time and str(confirm_receipt_time).isdigit():
            return int(confirm_receipt_time)
        return 0

    @staticmethod
    def _resolve_reference_time(order_data: JsonDict) -> int:
        """根据收货信息计算评价参考时间。"""
        if order_data["confirmReceiptTime"] > 0:
            return order_data["confirmReceiptTime"]

        if order_data["isWaybillReceived"] and order_data["waybillReceivedTime"] > 0:
            return order_data["waybillReceivedTime"]

        return 0

    @staticmethod
    def _match_strategy_by_score(score: int) -> str:
        """根据匹配分数映射策略名称。"""
        if score >= AUTO_FILL_SCORE_THRESHOLD:
            return "exact_match"
        if score >= 65:
            return "high_confidence"
        if score >= MATCH_MIN_SCORE:
            return "probable_match"
        return "fallback"

    @staticmethod
    def _deduplicate_orders_by_id(orders: JsonList) -> JsonList:
        """按 orderId 去重并保持原顺序。"""
        seen = set()
        merged = []
        for order in orders:
            oid = order.get("commonInfo", {}).get("orderId")
            if oid and oid not in seen:
                seen.add(oid)
                merged.append(order)
        return merged

    @staticmethod
    def _first_non_empty(data: JsonDict, keys: tuple[str, ...]) -> Any:
        """从多个候选字段中取第一个非空值。"""
        for key in keys:
            if key not in data:
                continue
            value = data.get(key)
            if isinstance(value, str):
                if value.strip():
                    return value.strip()
                continue
            if value not in (None, [], {}):
                return value
        return None

    @staticmethod
    def _normalize_product_text(text: str | None) -> str:
        """标准化商品字段值，便于跨字段名比较。"""
        if not text:
            return ""
        return re.sub(r"[\s，,、/\-_|（）()]+", "", str(text)).lower()

    @classmethod
    def _normalize_sale_param_value(cls, raw_value: Any) -> str:
        """将 saleParam / skuName 等规格字段统一为字符串。"""
        if isinstance(raw_value, list):
            tokens = [str(v).strip() for v in raw_value if str(v).strip()]
            return "|".join(tokens)
        if raw_value is None:
            return ""
        return str(raw_value).strip()

    @staticmethod
    def _build_product_id_key(product_id: str, sku_id: str) -> str | None:
        """构建 ID 维度索引键。"""
        if not product_id or not sku_id:
            return None
        return f"id::{product_id}::{sku_id}"

    @classmethod
    def _build_product_value_key(cls, product_name: str, sku_text: str) -> str | None:
        """构建值维度索引键（商品名 + 规格）。"""
        name_norm = cls._normalize_product_text(product_name)
        sku_norm = cls._normalize_product_text(sku_text)
        if not name_norm or not sku_norm:
            return None
        return f"value::{name_norm}::{sku_norm}"

    def _extract_product_fields(
        self,
        data: JsonDict,
        *,
        product_id_keys: tuple[str, ...],
        sku_id_keys: tuple[str, ...],
        sku_text_keys: tuple[str, ...],
        product_name_keys: tuple[str, ...],
        image_keys: tuple[str, ...] = (),
    ) -> JsonDict:
        """按字段别名提取商品匹配字段。"""
        raw_product_id = self._first_non_empty(data, product_id_keys)
        raw_sku_id = self._first_non_empty(data, sku_id_keys)
        raw_sku_text = self._first_non_empty(data, sku_text_keys)
        raw_product_name = self._first_non_empty(data, product_name_keys)
        raw_image = self._first_non_empty(data, image_keys) if image_keys else ""

        return {
            "productId": str(raw_product_id).strip() if raw_product_id is not None else "",
            "skuId": str(raw_sku_id).strip() if raw_sku_id is not None else "",
            "skuText": self._normalize_sale_param_value(raw_sku_text),
            "productName": str(raw_product_name).strip() if raw_product_name else "",
            "imageUrl": str(raw_image).strip() if raw_image else "",
        }

    def _build_order_context(self, order: JsonDict) -> JsonDict:
        """提取订单级公共字段。"""
        buyer_nickname = order.get("buyerInfo", {}).get("nickName", "")
        confirm_receipt_time = order.get("acceptInfo", {}).get("confirmReceiptTime", "")
        confirm_receipt_timestamp = self._parse_confirm_receipt_timestamp(
            confirm_receipt_time
        )
        auto_confirm_info = order.get("orderStatus", {}).get("autoConfirmInfo", {})

        return {
            "orderId": order.get("commonInfo", {}).get("orderId"),
            "buyerNickname": buyer_nickname,
            "normalizedNickname": self.normalize_nickname(buyer_nickname),
            "createTime": order.get("commonInfo", {}).get("createTime", 0),
            "confirmReceiptTime": confirm_receipt_timestamp,
            "isWaybillReceived": bool(auto_confirm_info.get("isWaybillReceived", False)),
            "waybillReceivedTime": int(auto_confirm_info.get("waybillReceivedTime", 0) or 0),
            "isEducationOrder": bool(
                order.get("commonInfo", {}).get("isEducationOrder", False)
            ),
            "orderStatus": order.get("commonInfo", {}).get("status", 0),
            "openid": order.get("commonInfo", {}).get("openid", ""),
            "orderData": order,
        }

    def _build_order_product_data(self, order: JsonDict, product: JsonDict) -> JsonDict | None:
        """组装单个订单商品的匹配数据。"""
        product_fields = self._extract_product_fields(
            product,
            product_id_keys=("productId", "product_id", "spuId", "spu_id"),
            sku_id_keys=("skuId", "sku_id"),
            sku_text_keys=("saleParam", "sale_param", "skuName", "specName", "spec"),
            product_name_keys=("title", "spuName", "productName", "name"),
            image_keys=("thumbImg", "imgUrl", "image", "imageUrl"),
        )
        id_key = self._build_product_id_key(
            product_fields["productId"],
            product_fields["skuId"],
        )
        value_key = self._build_product_value_key(
            product_fields["productName"],
            product_fields["skuText"],
        )
        if not id_key and not value_key:
            return None

        order_context = self._build_order_context(order)
        return {
            **order_context,
            "productId": product_fields["productId"],
            "skuId": product_fields["skuId"],
            "saleParam": product_fields["skuText"],
            "productName": product_fields["productName"],
            "thumbImg": product_fields["imageUrl"],
        }

    def _build_quality_refund_order_stub(self, item: JsonDict) -> JsonDict | None:
        """将品退接口返回项转换为统一订单结构。"""
        order_info = item.get("orderInfo", {}) or {}
        raw_order_id = self._first_non_empty(order_info, ("orderId", "order_id"))
        if raw_order_id is None:
            return None

        raw_create_time = self._first_non_empty(order_info, ("createTime", "create_time"))
        create_time = 0
        if raw_create_time is not None:
            raw_text = str(raw_create_time).strip()
            if raw_text.isdigit():
                create_time = int(raw_text)

        product_fields = self._extract_product_fields(
            order_info,
            product_id_keys=("spuId", "spu_id", "productId", "product_id"),
            sku_id_keys=("skuCode", "skuId", "sku_id"),
            sku_text_keys=("skuName", "saleParam", "sale_param", "specName", "spec"),
            product_name_keys=("name", "title", "spuName", "productName"),
            image_keys=("imgUrl", "skuThumbUrl", "thumbImg", "imageUrl"),
        )

        return {
            "commonInfo": {
                "orderId": str(raw_order_id).strip(),
                "createTime": create_time,
                "status": 0,
                "openid": "",
                "isEducationOrder": False,
            },
            "buyerInfo": {"nickName": ""},
            "acceptInfo": {"confirmReceiptTime": ""},
            "orderStatus": {
                "autoConfirmInfo": {
                    "isWaybillReceived": False,
                    "waybillReceivedTime": 0,
                }
            },
            "orderProductInfo": [
                {
                    "productId": product_fields["productId"],
                    "skuId": product_fields["skuId"],
                    "saleParam": product_fields["skuText"],
                    "title": product_fields["productName"],
                    "thumbImg": product_fields["imageUrl"],
                }
            ],
            "qualityRefundInfo": {
                "reason": str(item.get("reason", "") or "").strip(),
                "source": "quality_refund_api",
            },
        }

    def get_quality_refund_orders(
        self,
        earliest_time: int = 0,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
        """获取品质退款订单。"""
        if self._stopped:
            return []

        if on_progress:
            on_progress("[品退] 正在获取品质退款订单...")

        try:
            result = self._request_quality_refund_result(on_progress=on_progress)
        except Exception as exc:
            raise RuntimeError(f"品质退款订单请求失败: {exc}") from exc

        quality_refund_orders = []
        for item in result.get("data", []) or []:
            order = self._build_quality_refund_order_stub(item)
            if order is not None:
                quality_refund_orders.append(order)

        filtered_orders = self._filter_orders_by_earliest_time(
            quality_refund_orders,
            earliest_time,
        )

        if on_progress:
            total_count = len(quality_refund_orders)
            filtered_count = len(filtered_orders)
            if earliest_time > 0 and filtered_count != total_count:
                on_progress(
                    f"[品退] 获取到 {total_count} 个订单，"
                    f"按时间窗口保留 {filtered_count} 个。"
                )
            else:
                on_progress(f"[品退] 获取到 {filtered_count} 个订单。")

        return filtered_orders

    def _build_candidate_index_keys(self, evaluation_context: JsonDict) -> tuple[str, ...]:
        """根据评价上下文生成候选索引键。"""
        keys = []
        for index_key in (
            self._build_product_id_key(
                evaluation_context["product_id"],
                evaluation_context["sku_id"],
            ),
            self._build_product_value_key(
                evaluation_context["product_name"],
                evaluation_context["sku_name"],
            ),
        ):
            if index_key and index_key not in keys:
                keys.append(index_key)
        return tuple(keys)

    def _collect_candidate_orders(
        self,
        product_index: dict[str, JsonList],
        evaluation_context: JsonDict,
    ) -> JsonList:
        """根据评价上下文收集并去重候选订单。"""
        candidate_orders = []
        seen_candidates = set()

        for index_key in self._build_candidate_index_keys(evaluation_context):
            for order_data in product_index.get(index_key, []):
                candidate_key = (
                    order_data.get("orderId"),
                    order_data.get("productId"),
                    order_data.get("skuId"),
                )
                if candidate_key in seen_candidates:
                    continue
                seen_candidates.add(candidate_key)
                candidate_orders.append(order_data)

        return candidate_orders

    # -------------------------------------------------------------------
    # 匹配算法
    # -------------------------------------------------------------------

    def _build_product_sku_index(self, orders: JsonList) -> dict[str, JsonList]:
        """构建订单商品索引（ID键 + 值键）。"""
        product_sku_index = {}

        for order in orders:
            order_products = (
                order.get("orderProductInfo", [])
                or order.get("productInfos", [])
                or []
            )
            for product in order_products:
                order_data = self._build_order_product_data(order, product)
                if order_data is None:
                    continue

                for index_key in (
                    self._build_product_id_key(order_data["productId"], order_data["skuId"]),
                    self._build_product_value_key(order_data["productName"], order_data["saleParam"]),
                ):
                    if not index_key:
                        continue
                    if index_key not in product_sku_index:
                        product_sku_index[index_key] = []
                    product_sku_index[index_key].append(order_data)

        return product_sku_index

    def _extract_evaluation_context(self, evaluation: JsonDict) -> JsonDict:
        """提取单条评价匹配所需字段。"""
        eval_info = evaluation.get("evaluationInfo", {})
        product_info = evaluation.get("productInfo", {})
        operation_info = evaluation.get("operationInfo", {})

        buyer_nickname = eval_info.get("buyer", {}).get("identity", {}).get("nickname", "")
        eval_time = (
            eval_info.get("firstEvaluationInfo", {})
            .get("buyerEvaluationInfo", {})
            .get("createTime", 0)
        )
        product_fields = self._extract_product_fields(
            product_info,
            product_id_keys=("productId", "product_id", "spuId", "spu_id"),
            sku_id_keys=("skuId", "sku_id"),
            sku_text_keys=("skuName", "saleParam", "sale_param", "specName", "spec"),
            product_name_keys=("spuName", "title", "productName", "name"),
        )

        return {
            "eval_info": eval_info,
            "product_info": product_info,
            "operation_info": operation_info,
            "evaluation_id": evaluation.get("productEvaluationId"),
            "product_id": product_fields["productId"],
            "sku_id": product_fields["skuId"],
            "sku_name": product_fields["skuText"],
            "product_name": product_fields["productName"],
            "buyer_nickname": buyer_nickname,
            "normalized_buyer_nickname": self.normalize_nickname(buyer_nickname),
            "eval_time": eval_time,
        }

    @staticmethod
    def _split_sku_tokens(raw_text: str) -> list[str]:
        """按常见分隔符拆分规格文本。"""
        if not raw_text:
            return []
        return [t.strip() for t in re.split(r"[，,、/\-_ |]+", raw_text) if t.strip()]

    @classmethod
    def _is_sku_exact_matched(cls, sku_name: str, sale_param: str) -> bool:
        """规格严格一致校验（用于一票否决）。"""
        if not sku_name or not sale_param:
            return False

        left = sku_name.strip()
        right = sale_param.strip()
        if left == right:
            return True

        # 同义格式兼容：忽略常见分隔符与空白后比较
        normalize_pattern = r"[，,、/\-_ |（）()]+"
        normalized_left = re.sub(normalize_pattern, "", left)
        normalized_right = re.sub(normalize_pattern, "", right)
        if normalized_left and normalized_left == normalized_right:
            return True

        # 兜底：分词后需双向完全覆盖，避免“部分重叠”误判为一致
        left_tokens = cls._split_sku_tokens(left)
        right_tokens = cls._split_sku_tokens(right)
        if not left_tokens or not right_tokens:
            return False

        return (
            len(left_tokens) == len(right_tokens)
            and set(left_tokens) == set(right_tokens)
        )

    def _score_nickname_dimension(
        self,
        normalized_eval_nickname: str,
        normalized_order_nickname: str,
    ) -> tuple[int, str]:
        """昵称维度（辅助项）。"""
        weight = SCORE_WEIGHTS["nickname"]

        if self._is_generic_nickname(normalized_eval_nickname):
            return 0, f"昵称为通用名(+0/{weight})"

        if not normalized_order_nickname:
            return 0, f"订单昵称缺失(+0/{weight})"

        if normalized_eval_nickname == normalized_order_nickname:
            return weight, f"昵称完全匹配(+{weight}/{weight})"

        if (
            len(normalized_eval_nickname) >= 2
            and len(normalized_order_nickname) >= 2
            and (
                normalized_eval_nickname in normalized_order_nickname
                or normalized_order_nickname in normalized_eval_nickname
            )
        ):
            partial_score = round(weight * 0.73)
            return partial_score, f"昵称强相关(+{partial_score}/{weight})"

        shared_chars = len(
            set(normalized_eval_nickname) & set(normalized_order_nickname)
        )
        if shared_chars >= 2:
            weak_score = round(weight * 0.47)
            return weak_score, f"昵称弱相关(+{weak_score}/{weight})"

        return 0, f"昵称不一致(可能改名,+0/{weight})"

    def _score_sku_dimension(self, sku_name: str, sale_param: str) -> tuple[int, str]:
        """规格维度（主项，严格一致）。"""
        weight = SCORE_WEIGHTS["sku"]

        if self._is_sku_exact_matched(sku_name, sale_param):
            return weight, f"规格完全一致(+{weight}/{weight})"

        return 0, f"规格不一致(淘汰,+0/{weight})"

    @staticmethod
    def _score_reference_time_dimension(confirm_diff_seconds: int) -> tuple[int, str]:
        """评价时间与收货时间贴合度（主项）。"""
        weight = SCORE_WEIGHTS["reference_time"]
        diff_hours = confirm_diff_seconds / 3600

        if diff_hours <= 6:
            return weight, f"评价紧邻收货(+{weight}/{weight})"
        if diff_hours <= 24:
            score = round(weight * 0.88)
            return score, f"评价与收货同日(+{score}/{weight})"
        if diff_hours <= 72:
            score = round(weight * 0.72)
            return score, f"评价与收货间隔较短(+{score}/{weight})"
        if diff_hours <= 24 * 7:
            score = round(weight * 0.52)
            return score, f"评价与收货间隔一周内(+{score}/{weight})"
        if diff_hours <= 24 * 15:
            score = round(weight * 0.32)
            return score, f"评价与收货间隔偏长(+{score}/{weight})"
        if diff_hours <= 24 * 30:
            score = round(weight * 0.16)
            return score, f"评价与收货间隔较远(+{score}/{weight})"
        return 0, f"评价与收货间隔过远(+0/{weight})"

    @staticmethod
    def _score_create_time_dimension(create_diff_seconds: int) -> tuple[int, str]:
        """评价时间与下单时间合理性（主项）。"""
        weight = SCORE_WEIGHTS["create_time"]
        diff_days = create_diff_seconds / 86400

        if diff_days <= 1:
            return weight, f"下单后很快评价(+{weight}/{weight})"
        if diff_days <= 3:
            score = round(weight * 0.8)
            return score, f"下单后短期评价(+{score}/{weight})"
        if diff_days <= 7:
            score = round(weight * 0.6)
            return score, f"下单后一周内评价(+{score}/{weight})"
        if diff_days <= 15:
            score = round(weight * 0.4)
            return score, f"下单后两周内评价(+{score}/{weight})"
        if diff_days <= 30:
            score = round(weight * 0.2)
            return score, f"下单后一个月内评价(+{score}/{weight})"
        return 0, f"下单后评价间隔偏长(+0/{weight})"

    @staticmethod
    def _score_order_status_dimension(order_status: int) -> tuple[int, str]:
        """订单状态可靠性（5分）。"""
        weight = SCORE_WEIGHTS["order_status"]
        if order_status >= 100:
            return weight, f"订单已完成(+{weight}/{weight})"
        if order_status >= 60:
            score = round(weight * 0.6)
            return score, f"订单已发货/待评价(+{score}/{weight})"
        if order_status >= 40:
            score = 1
            return score, f"订单处理中(+{score}/{weight})"
        return 0, f"订单状态弱相关(+0/{weight})"

    def _score_candidate_order(
        self,
        order_data: JsonDict,
        normalized_buyer_nickname: str,
        sku_name: str,
        eval_time: int,
    ) -> JsonDict | None:
        """对单个候选订单评分，返回候选匹配结果或 None。"""
        score = 0
        reasons: list[str] = []

        reference_time = self._resolve_reference_time(order_data)
        if reference_time <= 0:
            return None

        if eval_time <= 0 or eval_time < reference_time:
            return None

        max_eval_days = 60 if order_data["isEducationOrder"] else 30
        time_diff_days = (eval_time - reference_time) / 86400
        if time_diff_days > max_eval_days:
            return None

        create_time = int(order_data.get("createTime", 0) or 0)
        if create_time > 0 and eval_time < create_time:
            return None

        nickname_score, nickname_reason = self._score_nickname_dimension(
            normalized_buyer_nickname,
            order_data["normalizedNickname"],
        )
        score += nickname_score
        reasons.append(nickname_reason)

        sku_score, sku_reason = self._score_sku_dimension(sku_name, order_data["saleParam"])
        score += sku_score
        reasons.append(sku_reason)
        if sku_score <= 0:
            return None

        confirm_diff = eval_time - reference_time
        reference_score, reference_reason = self._score_reference_time_dimension(confirm_diff)
        score += reference_score
        reasons.append(reference_reason)

        if create_time > 0:
            create_diff = eval_time - create_time
            create_score, create_reason = self._score_create_time_dimension(create_diff)
        else:
            create_score = 0
            create_reason = f"订单下单时间缺失(+0/{SCORE_WEIGHTS['create_time']})"
        score += create_score
        reasons.append(create_reason)

        status_score, status_reason = self._score_order_status_dimension(
            int(order_data.get("orderStatus", 0) or 0)
        )
        score += status_score
        reasons.append(status_reason)

        if score < MATCH_MIN_SCORE:
            return None

        return {
            "order_data": order_data,
            "score": score,
            "reasons": reasons,
            "time_diff": (
                eval_time - create_time
                if eval_time > 0 and create_time > 0
                else float("inf")
            ),
            "confirm_diff": (
                confirm_diff
                if eval_time > 0 and reference_time > 0
                else float("inf")
            ),
        }

    @staticmethod
    def _apply_multi_order_penalty(best_matches: JsonList) -> JsonList:
        """多候选时按时效偏差扣分并做二次过滤。"""
        if len(best_matches) <= 1:
            return best_matches

        best_confirm_diff = min(bm["confirm_diff"] for bm in best_matches)

        for bm in best_matches:
            extra_diff_days = max(0.0, (bm["confirm_diff"] - best_confirm_diff) / 86400)
            # 候选订单存在竞争时，仅对比“相对最优候选”的时效差做惩罚
            penalty = min(12, round(extra_diff_days * 1.5))
            if penalty > 0:
                bm["score"] -= penalty
                bm["reasons"].append(f"多单竞争时效劣势(-{penalty})")

        # 扣分后二次过滤：仅保留分数仍满足最低匹配阈值的候选
        filtered = [bm for bm in best_matches if bm["score"] >= MATCH_MIN_SCORE]
        if filtered:
            return filtered

        # 若全部跌破阈值，保留原列表并取分数最高者（避免误删最后候选）
        return best_matches

    @staticmethod
    def _pick_best_match(best_matches: JsonList) -> JsonDict | None:
        """按既定优先级选择最佳候选。"""
        if not best_matches:
            return None

        best_matches.sort(
            key=lambda x: (-x["score"], x["confirm_diff"], x["time_diff"])
        )
        return best_matches[0]

    def _match_single_evaluation(
        self,
        evaluation_context: JsonDict,
        product_index: dict[str, JsonList],
    ) -> tuple[JsonDict | None, str | None, int]:
        """执行单条评价的候选收集、评分和最佳匹配选择。"""
        candidate_orders = self._collect_candidate_orders(product_index, evaluation_context)
        if not candidate_orders:
            return None, None, 0

        best_matches = []
        for order_data in candidate_orders:
            candidate = self._score_candidate_order(
                order_data,
                evaluation_context["normalized_buyer_nickname"],
                evaluation_context["sku_name"],
                evaluation_context["eval_time"],
            )
            if candidate:
                best_matches.append(candidate)

        best_matches = self._apply_multi_order_penalty(best_matches)
        best_match = self._pick_best_match(best_matches)
        if best_match is None:
            return None, None, 0

        matched_order = best_match["order_data"]
        match_score = best_match["score"]
        match_strategy = self._match_strategy_by_score(match_score)
        return matched_order, match_strategy, match_score

    @staticmethod
    def _build_match_result(
        evaluation_context: JsonDict,
        matched_order: JsonDict | None,
        match_strategy: str | None,
        match_score: int,
    ) -> JsonDict:
        """组装最终匹配结果结构。"""
        eval_info = evaluation_context["eval_info"]
        product_info = evaluation_context["product_info"]
        operation_info = evaluation_context["operation_info"]
        eval_time = evaluation_context["eval_time"]
        reference_time = (
            BadReviewOrderFinder._resolve_reference_time(matched_order)
            if matched_order
            else 0
        )

        return {
            "evaluationId": evaluation_context["evaluation_id"],
            "orderId": matched_order["orderId"] if matched_order else None,
            "productId": evaluation_context["product_id"],
            "skuId": evaluation_context["sku_id"],
            "skuName": evaluation_context["sku_name"],
            "saleParam": matched_order["saleParam"] if matched_order else "",
            "buyerNickname": evaluation_context["buyer_nickname"],
            "orderBuyerNickname": matched_order["buyerNickname"] if matched_order else "",
            "matchStrategy": match_strategy if matched_order else None,
            "matchScore": match_score if matched_order else 0,
            "timeDiffHours": (
                (eval_time - matched_order["createTime"]) / 3600
                if matched_order and eval_time > 0 and matched_order["createTime"] > 0
                else None
            ),
            "confirmDiffHours": (
                (eval_time - reference_time) / 3600
                if matched_order and eval_time > 0 and reference_time > 0
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
            "productName": (
                evaluation_context.get("product_name")
                or product_info.get("spuName", "")
                or product_info.get("title", "")
            ),
            "canReplyExpireTime": operation_info.get("canReplyExpireTime", 0),
            "matched": matched_order is not None,
        }

    def match_orders_with_evaluations(
        self,
        bad_evaluations: JsonList,
        orders: JsonList,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
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

        product_index = self._build_product_sku_index(orders)

        if on_progress:
            on_progress(f"索引构建完成: {len(product_index)} 个商品+SKU 组合")

        results = []
        total = len(bad_evaluations)

        for i, evaluation in enumerate(bad_evaluations, 1):
            if self._stopped:
                break

            evaluation_context = self._extract_evaluation_context(evaluation)

            if on_progress:
                on_progress(
                    f"[{i}/{total}] 匹配评价: {evaluation_context['buyer_nickname']}"
                )

            matched_order = None
            match_strategy = None
            match_score = 0

            matched_order, match_strategy, match_score = self._match_single_evaluation(
                evaluation_context,
                product_index,
            )
            if matched_order and on_progress:
                on_progress(
                    f"  ✅ 匹配成功 (得分: {match_score}, "
                    f"策略: {match_strategy}) → 订单 {matched_order['orderId']}"
                )

            if not matched_order and on_progress:
                on_progress("  ❌ 未找到匹配订单")

            results.append(
                self._build_match_result(
                    evaluation_context,
                    matched_order,
                    match_strategy,
                    match_score,
                )
            )

        return results
