# -*- coding: utf-8 -*-
"""TLS-shipinhao 订单查找器（核心逻辑）。"""

import re
import threading
import time
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime
from typing import Any, Callable

import requests

from settings import (
    AUTO_FILL_SCORE_THRESHOLD,
    EDUCATION_ORDER_MAX_DAYS,
    EVALUATION_MAX_DAYS,
    EVALUATION_MAX_PAGES,
    EVALUATION_PAGE_SIZE,
    EVALUATION_SEARCH_URL,
    FETCH_PAGE_INTERVAL_SECONDS,
    MATCH_MIN_SCORE,
    ORDER_PAGE_SIZE,
    ORDER_RISK_PAGE_INTERVAL_SECONDS,
    ORDER_RISK_WINDOW_WORKERS,
    ORDER_SEARCH_URL,
    ORDER_WINDOW_WORKERS,
    QUALITY_REFUND_ORDER_URL,
    RATE_LIMIT_RETRY_COUNT,
    REQUEST_TIMEOUT,
)
from core.day_window import recent_day_range_timestamps
from core.http_utils import build_request_params
from services.order_match_scoring import compute_match_score

ProgressCallback = Callable[[str], None]
JsonDict = dict[str, Any]
JsonList = list[JsonDict]

EVALUATION_REFERER = "https://store.weixin.qq.com/shop/evaluate/home"
ORDER_LIST_REFERER = "https://store.weixin.qq.com/shop/order/list"
QUALITY_REFUND_REFERER = (
    "https://store.weixin.qq.com/shop/setting/"
    "ratedetail?type=product&key=productQualityRatio_30d&detail=order"
)
ORDER_PROGRESS_PAGE_INTERVAL = 5
QUALITY_REFUND_REQUEST_METHODS = ("GET", "POST")
_CACHE_FETCH_BATCH_ORDERS = 1000  # 每积累多少订单触发一次持久化回调


@dataclass(frozen=True)
class OrderWindow:
    """订单抓取时间窗口。"""

    start_ts: int
    end_ts: int
    depth: int
    window_id: str



class OrderRiskControlError(RuntimeError):
    """平台风控触发，需要切换抓取模式。"""

    def __init__(self, message: str, partial_orders=None, warnings=None, stats=None, remaining_windows=None):
        super().__init__(message)
        self.partial_orders = partial_orders or []
        self.warnings = warnings or []
        self.stats = stats or {}
        self.remaining_windows = remaining_windows or []

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
            params=build_request_params(),
            json=data,
            headers=headers,
            timeout=REQUEST_TIMEOUT,
        )

    @staticmethod
    def _rate_limit_wait_seconds(retry_index: int) -> int:
        """根据重试轮次返回指数退避等待时间。"""
        return 2 ** (retry_index + 1)

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
                    params=build_request_params(),
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

    def _request_order_search_result(
        self,
        data: JsonDict,
        headers: JsonDict,
        page_index: int,
        on_progress: ProgressCallback | None = None,
    ) -> JsonDict:
        """请求订单接口，并统一处理 429 限流。"""
        try:
            response = self._post_json(
                ORDER_SEARCH_URL,
                data,
                headers,
            )
        except Exception as exc:
            raise RuntimeError(f"订单请求异常: {exc}") from exc

        if response.status_code == 429:
            for retry in range(RATE_LIMIT_RETRY_COUNT):
                wait = self._rate_limit_wait_seconds(retry)
                if on_progress:
                    on_progress(f"第 {page_index} 页触发频率限制，等待 {wait} 秒后重试...")
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
                wait = self._rate_limit_wait_seconds(retry)
                if on_progress:
                    on_progress(f"第 {page_index} 页触发频率限制(API)，等待 {wait} 秒后重试...")
                time.sleep(wait)
                try:
                    response = self._post_json(
                        ORDER_SEARCH_URL,
                        data,
                        headers,
                    )
                except Exception as exc:
                    raise RuntimeError(f"订单请求异常: {exc}") from exc
                if response.status_code not in (200, 201):
                    raise RuntimeError(f"订单请求失败: HTTP {response.status_code}")
                result = response.json()
                if result.get("code") != 429:
                    break
            else:
                raise RuntimeError("订单API持续频率限制，请稍后再试")

        return result

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

    def merge_quality_refund_orders(
        self,
        orders: JsonList,
        earliest_time: int = 0,
        on_progress: ProgressCallback | None = None,
    ) -> JsonList:
        """将品质退款订单并入订单列表。"""
        base_orders = self.deduplicate_orders_by_id(orders)
        quality_refund_orders = self.get_quality_refund_orders(
            earliest_time=earliest_time,
            on_progress=on_progress,
        )
        if not quality_refund_orders:
            return base_orders

        merged_orders = self.deduplicate_orders_by_id(base_orders + quality_refund_orders)
        added_count = len(merged_orders) - len(base_orders)
        if on_progress:
            on_progress(
                f"[品退] 已并入 {added_count} 个订单"
                f"（接口返回 {len(quality_refund_orders)} 个）。"
            )
        return merged_orders

    # -------------------------------------------------------------------
    # 订单抓取（页码并行方案）
    # -------------------------------------------------------------------

    @staticmethod
    def _is_risk_control_result(result: JsonDict) -> bool:
        """判断是否命中了平台风控。"""
        code = int(result.get("code", 0) or 0)
        resp_status = int(result.get("respStatusCode", 0) or 0)
        message = str(result.get("msg", "") or "")
        return code == 430 or resp_status == 430 or "异常行为" in message or "拒绝访问" in message

    def _fetch_orders_by_page(
        self,
        *,
        earliest_time: int = 0,
        num_workers: int = ORDER_WINDOW_WORKERS,
        page_interval_seconds: float = FETCH_PAGE_INTERVAL_SECONDS,
        on_progress: ProgressCallback | None = None,
        on_batch_completed: Callable[[OrderWindow, JsonList], None] | None = None,
    ) -> tuple[JsonList, list[str]]:
        """页码并行抓取：num_workers 个 worker 共享自增页码，各取不同页，无重复。

        替代时间窗口分片方案——微信订单 API 忽略 createTimeStart/End，
        时间窗口无法实现服务端过滤，只能用页码偏移实现真正的并行分工。
        """
        headers = self._build_headers(ORDER_LIST_REFERER)
        shared_page: dict[str, int] = {"next": 1}
        page_lock = threading.Lock()

        collected: JsonList = []
        pending: JsonList = []
        collect_lock = threading.Lock()
        batch_counter: dict[str, int] = {"n": 0}

        stop_event = threading.Event()
        risk_message: list[str] = []
        fatal_errors: list[str] = []

        def _extract_batch(force: bool) -> tuple[JsonList, int] | None:
            """collect_lock 内调用：取出一批待提交数据，返回 (batch, batch_n) 或 None。"""
            if not pending:
                return None
            if not force and len(pending) < _CACHE_FETCH_BATCH_ORDERS:
                return None
            batch = list(pending)
            pending.clear()
            collected.extend(batch)
            batch_counter["n"] += 1
            return batch, batch_counter["n"]

        def _commit_batch(batch: JsonList, batch_n: int) -> None:
            """collect_lock 外调用：触发持久化回调，避免锁内做 IO。"""
            if not on_batch_completed or not batch:
                return
            times = [
                int(o.get("commonInfo", {}).get("createTime", 0) or 0)
                for o in batch
                if int(o.get("commonInfo", {}).get("createTime", 0) or 0) > 0
            ]
            if not times:
                return
            window = OrderWindow(
                start_ts=min(times),
                end_ts=max(times),
                depth=0,
                window_id=f"B{batch_n}",
            )
            on_batch_completed(window, batch)

        def _worker(worker_id: int) -> None:
            while not stop_event.is_set() and not self._stopped:
                with page_lock:
                    pg = shared_page["next"]
                    shared_page["next"] += 1

                if stop_event.is_set() or self._stopped:
                    break

                if on_progress and pg % ORDER_PROGRESS_PAGE_INTERVAL == 1:
                    on_progress(f"[订单] 正在获取第 {pg} 页订单...")

                data = self._build_order_search_payload(page=pg)
                try:
                    result = self._request_order_search_result(data, headers, pg, on_progress)
                except Exception as exc:  # noqa: BLE001
                    fatal_errors.append(str(exc))
                    stop_event.set()
                    break

                if self._is_risk_control_result(result):
                    risk_message.append(f"[第 {pg} 页] {result.get('msg', str(result))}")
                    stop_event.set()
                    break

                if result.get("code") == 10003:
                    if on_progress:
                        on_progress(f"[订单] 第 {pg} 页返回 10003，已到达数据末尾。")
                    stop_event.set()
                    break

                if result.get("code") != 0:
                    fatal_errors.append(f"[第 {pg} 页] API错误: {result}")
                    stop_event.set()
                    break

                page_orders: JsonList = result.get("orderList", []) or []
                if not page_orders:
                    if on_progress:
                        on_progress(f"[订单] 第 {pg} 页为空，订单全部拉取完毕。")
                    stop_event.set()
                    break

                filtered = (
                    [
                        o for o in page_orders
                        if int(o.get("commonInfo", {}).get("createTime", 0) or 0) >= earliest_time
                    ]
                    if earliest_time > 0
                    else list(page_orders)
                )

                batch_info = None
                with collect_lock:
                    pending.extend(filtered)
                    batch_info = _extract_batch(force=False)

                if batch_info:
                    _commit_batch(*batch_info)

                if on_progress:
                    on_progress(
                        f"[订单] 第 {pg} 页获取到 {len(filtered)} 个订单"
                        f"（累计约 {len(collected) + len(pending)}）"
                    )

                if self._is_page_outside_earliest_time(page_orders, earliest_time):
                    if on_progress:
                        on_progress(f"[订单] 第 {pg} 页订单均早于筛选时间，触发早停。")
                    stop_event.set()
                    break

                time.sleep(page_interval_seconds)

        with ThreadPoolExecutor(max_workers=num_workers, thread_name_prefix="order-page") as pool:
            futures = [pool.submit(_worker, i + 1) for i in range(num_workers)]
            for future in as_completed(futures):
                try:
                    future.result()
                except Exception as exc:  # noqa: BLE001
                    fatal_errors.append(str(exc))

        last_batch = None
        with collect_lock:
            last_batch = _extract_batch(force=True)
        if last_batch:
            _commit_batch(*last_batch)

        if fatal_errors:
            raise RuntimeError("; ".join(fatal_errors))
        if risk_message:
            raise OrderRiskControlError(
                risk_message[0],
                partial_orders=self.deduplicate_orders_by_id(collected),
            )

        deduped = self.deduplicate_orders_by_id(collected)
        if on_progress:
            on_progress(
                f"[订单] 页码并行抓取完成：共 {shared_page['next'] - 1} 页，"
                f"去重后 {len(deduped)} 个订单。"
            )
        return deduped, []

    def get_orders_for_cache(
        self,
        *,
        earliest_time: int = 0,
        create_time_start: int = 0,
        create_time_end: int = 0,
        on_progress: ProgressCallback | None = None,
        on_window_completed: Callable[[OrderWindow, JsonList], None] | None = None,
    ) -> tuple[JsonList, list[str]]:
        """为本地缓存获取纯订单数据（页码并行，客户端时间边界早停）。"""
        # API 忽略 createTimeStart/createTimeEnd，使用页码并行 + 客户端早停
        effective_earliest = max(int(earliest_time or 0), int(create_time_start or 0))

        try:
            return self._fetch_orders_by_page(
                earliest_time=effective_earliest,
                num_workers=ORDER_WINDOW_WORKERS,
                page_interval_seconds=FETCH_PAGE_INTERVAL_SECONDS,
                on_progress=on_progress,
                on_batch_completed=on_window_completed,
            )
        except OrderRiskControlError as exc:
            cooldown = 60
            if on_progress:
                on_progress(
                    f"⚠️ 检测到平台风控，等待 {cooldown} 秒冷却后切换到极速模式（单线程 + 更慢间隔）。"
                )
            for remaining in range(cooldown, 0, -10):
                if self._stopped:
                    break
                if on_progress:
                    on_progress(f"[风控冷却] 还剩 {remaining} 秒...")
                time.sleep(min(10, remaining))

            risk_warning = "本次抓取触发平台风控，已自动降级到极速模式"
            try:
                risk_orders, risk_warnings = self._fetch_orders_by_page(
                    earliest_time=effective_earliest,
                    num_workers=ORDER_RISK_WINDOW_WORKERS,
                    page_interval_seconds=ORDER_RISK_PAGE_INTERVAL_SECONDS,
                    on_progress=on_progress,
                    on_batch_completed=on_window_completed,
                )
                merged_warnings = list(exc.warnings)
                merged_warnings.append(risk_warning)
                merged_warnings.extend(risk_warnings)
                merged_orders = self.deduplicate_orders_by_id(exc.partial_orders + risk_orders)
                return merged_orders, merged_warnings
            except OrderRiskControlError as risk_exc:
                merged_orders = self.deduplicate_orders_by_id(exc.partial_orders + risk_exc.partial_orders)
                merged_warnings = list(exc.warnings)
                merged_warnings.append(risk_warning)
                merged_warnings.extend(risk_exc.warnings)
                merged_warnings.append("仍有部分窗口未完成，结果可能不完整")
                if merged_orders:
                    return merged_orders, merged_warnings
                merged_warnings.append("平台风控持续触发，本次未能获取订单数据，请稍后重试")
                return [], merged_warnings

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
        start_time, end_time = recent_day_range_timestamps(days)

        all_bad_reviews = []
        page = 1
        effective_max_pages = EVALUATION_MAX_PAGES

        while page <= effective_max_pages:
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
            total_count = result.get("totalCnt")
            if total_count is not None:
                try:
                    total_count = max(0, int(total_count))
                except (TypeError, ValueError):
                    total_count = None
                if total_count is not None:
                    total_pages = max(
                        1,
                        (total_count + EVALUATION_PAGE_SIZE - 1) // EVALUATION_PAGE_SIZE,
                    )
                    effective_max_pages = min(EVALUATION_MAX_PAGES, total_pages)

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
        if score >= 100:
            return "exact_match"
        if score >= AUTO_FILL_SCORE_THRESHOLD:
            return "high_confidence"
        if score >= MATCH_MIN_SCORE:
            return "probable_match"
        return "fallback"

    @staticmethod
    def deduplicate_orders_by_id(orders: JsonList) -> JsonList:
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
    def _parse_timestamp(raw_value: Any) -> int:
        """将原始时间值转为秒级时间戳，自动处理毫秒。"""
        if raw_value is None:
            return 0
        raw_text = str(raw_value).strip()
        if not raw_text.isdigit():
            return 0
        ts = int(raw_text)
        if ts > 9_999_999_999:
            ts //= 1000
        return ts

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

        time_keys = ("createTime", "create_time", "createTs", "orderCreateTime", "refundTime")
        raw_create_time = self._first_non_empty(order_info, time_keys)
        if raw_create_time is None:
            raw_create_time = self._first_non_empty(item, time_keys)
        create_time = self._parse_timestamp(raw_create_time)

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
            on_progress(
                f"[品退] 接口返回 {total_count} 个订单，"
                f"近期匹配 {filtered_count} 个。"
            )

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

    @staticmethod
    def _build_product_reason(match_result: JsonDict) -> str:
        """根据商品匹配结果构建文案。"""
        if bool(match_result["productExact"]):
            return "商品标题/商品ID/SKU 完全匹配"
        return (
            "商品信息相似度 "
            f"{match_result['productSimilarity']}%(标题 {match_result['titleSimilarity']}%，"
            f"ID {'命中' if match_result['productIdExact'] else '未命中'}，"
            f"SKU {'命中' if match_result['skuIdExact'] else '未命中'})"
            f"(扣 {match_result['productPenalty']} 分)"
        )

    def _score_candidate_order(
        self,
        order_data: JsonDict,
        evaluation_context: JsonDict,
        eval_time: int,
    ) -> JsonDict | None:
        """对单个候选订单评分，返回候选匹配结果或 None。"""
        reasons: list[str] = []

        reference_time = self._resolve_reference_time(order_data)

        create_time = int(order_data.get("createTime", 0) or 0)
        if eval_time > 0 and create_time > 0 and eval_time < create_time:
            return None

        match_result = compute_match_score(
            evaluation_buyer_nickname=evaluation_context["buyer_nickname"],
            evaluation_product_id=evaluation_context["product_id"],
            evaluation_sku_id=evaluation_context["sku_id"],
            evaluation_title=evaluation_context["product_name"],
            order_buyer_nickname=order_data.get("buyerNickname", ""),
            order_product_id=order_data.get("productId", ""),
            order_sku_id=order_data.get("skuId", ""),
            order_title=order_data.get("productName", ""),
        )
        score = int(match_result["score"])

        if bool(match_result["buyerNicknameExact"]):
            reasons.append("买家昵称完全匹配")
        else:
            reasons.append(
                self._build_nickname_reason(
                    evaluation_context["buyer_nickname"],
                    order_data.get("buyerNickname", ""),
                    int(match_result["buyerNicknameSimilarity"]),
                    int(match_result["buyerNicknamePenalty"]),
                )
            )

        reasons.append(self._build_product_reason(match_result))

        if score < MATCH_MIN_SCORE:
            return None

        confirm_diff = (
            eval_time - reference_time
            if eval_time > 0 and reference_time > 0
            else float("inf")
        )
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
    def _build_nickname_reason(
        evaluation_buyer_nickname: str,
        order_buyer_nickname: str,
        similarity: int,
        penalty: int,
    ) -> str:
        """根据昵称关系生成更贴近业务的原因文案。"""
        eval_name = str(evaluation_buyer_nickname or "").strip()
        order_name = str(order_buyer_nickname or "").strip()
        shorter, longer = (eval_name, order_name) if len(eval_name) <= len(order_name) else (order_name, eval_name)

        if len(shorter) == 1 and shorter and shorter in longer:
            return (
                "昵称仅单字重合，歧义较高！"
                f"(相似度 {similarity}%，扣 {penalty} 分)"
            )

        return (
            "昵称相似度较高，疑似改名！"
            f"(相似度 {similarity}%，扣 {penalty} 分)"
        )

    @staticmethod
    def _apply_multi_order_penalty(best_matches: JsonList) -> JsonList:
        """多候选不再额外扣分，仅保留原列表，后续靠排序择优。"""
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
    ) -> tuple[JsonDict | None, str | None, int, list[str]]:
        """执行单条评价的候选收集、评分和最佳匹配选择。"""
        candidate_orders = self._collect_candidate_orders(product_index, evaluation_context)
        if not candidate_orders:
            return None, None, 0, []

        best_matches = []
        for order_data in candidate_orders:
            candidate = self._score_candidate_order(
                order_data,
                evaluation_context,
                evaluation_context["eval_time"],
            )
            if candidate:
                best_matches.append(candidate)

        best_matches = self._apply_multi_order_penalty(best_matches)
        best_match = self._pick_best_match(best_matches)
        if best_match is None:
            return None, None, 0, []

        matched_order = best_match["order_data"]
        match_score = best_match["score"]
        match_strategy = self._match_strategy_by_score(match_score)
        return matched_order, match_strategy, match_score, list(best_match.get("reasons", []))

    @staticmethod
    def _build_match_result(
        evaluation_context: JsonDict,
        matched_order: JsonDict | None,
        match_strategy: str | None,
        match_score: int,
        match_reasons: list[str],
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
            "matchReasons": match_reasons if matched_order else [],
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
            match_reasons: list[str] = []

            matched_order, match_strategy, match_score, match_reasons = self._match_single_evaluation(
                evaluation_context,
                product_index,
            )
            if matched_order and on_progress:
                on_progress(
                    f"  ✅ 匹配成功 (得分: {match_score}) → 订单 {matched_order['orderId']}"
                )

            if not matched_order and on_progress:
                on_progress("  ❌ 未找到匹配订单")

            results.append(
                self._build_match_result(
                    evaluation_context,
                    matched_order,
                    match_strategy,
                    match_score,
                    match_reasons,
                )
            )

        return results
