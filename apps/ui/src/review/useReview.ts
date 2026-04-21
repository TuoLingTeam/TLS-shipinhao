import { computed } from "vue";
import { useReviewStore } from "./review.store";
import { useDeliveryStore } from "../delivery/delivery.store";
import { useOrderStore } from "../order/order.store";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import { useOrder } from "../order/useOrder";
import type { OrderMatchResult, ReviewMatchResponse } from "./review.types";
import { toErrorMessage } from "../shared/toErrorMessage";

export function useReview() {
  const store = useReviewStore();
  const deliveryStore = useDeliveryStore();
  const orderStore = useOrderStore();
  const { withSyncEvents, loadCacheStatus, loadRecentCache } = useOrder();
  const reviewInvoke = useTauriInvoke<ReviewMatchResponse>("find_reviews");
  const qualityRefundInvoke = useTauriInvoke<ReviewMatchResponse>("find_quality_refund_orders");
  const loading = computed(() => reviewInvoke.loading.value || qualityRefundInvoke.loading.value);

  /**
   * 从匹配结果中挑出可自动带入发货的订单号集合（已去重）。
   *
   * 差评模式（strictExactMatch）：仅选 strategy === "exact_match"（满分 100）的条目，
   * 避免低置信度误发；品退模式：全量带入后端判定为 matched 的订单号。
   */
  function pickAutofillOrderIds(
    results: OrderMatchResult[],
    options: { strictExactMatch: boolean },
  ): string[] {
    const ids = results
      .filter(
        (item) =>
          item.matched &&
          item.order_id?.trim() &&
          (!options.strictExactMatch || item.strategy === "exact_match"),
      )
      .map((item) => item.order_id.trim());
    return Array.from(new Set(ids));
  }

  function applyResults(
    payload: ReviewMatchResponse,
    source: "评价匹配" | "品退匹配",
  ) {
    store.setResults(payload.results);
    store.cacheWarnings = payload.cache_warnings;
    store.cacheCoverageStart = payload.cache_coverage_start;
    store.cacheCoverageEnd = payload.cache_coverage_end;
    store.cacheSyncPerformed = payload.cache_sync_performed;
    store.cacheSyncWrittenCount = payload.cache_sync_written_count;

    const uniqueOrderIds = pickAutofillOrderIds(payload.results, {
      strictExactMatch: source === "评价匹配",
    });

    if (uniqueOrderIds.length > 0) {
      deliveryStore.prefillOrders(uniqueOrderIds, source);
    } else if (source === "评价匹配") {
      deliveryStore.clearPrefillOrder();
    }
  }

  async function findReviews(days: number, startAt: string, endAt: string) {
    store.setLoading(true);
    store.setError(null);
    store.setLastMode("bad_review");
    store.setLastQuery({ days, time_window: { start_at: startAt, end_at: endAt } });
    try {
      const payload = await withSyncEvents("review_query", () =>
        reviewInvoke.execute({ days, start_at: startAt, end_at: endAt }),
      );
      if (payload) {
        applyResults(payload, "评价匹配");
        void Promise.all([loadRecentCache(), loadCacheStatus()]).catch((error) => {
          orderStore.error = toErrorMessage(error);
        });
      } else {
        store.setError(reviewInvoke.error.value ?? "查找失败");
      }
    } catch (error) {
      orderStore.error = toErrorMessage(error);
      store.setError(`差评匹配失败：${orderStore.error}`);
    } finally {
      store.setLoading(false);
    }
  }

  async function findQualityRefundOrders(days: number, startAt: string, endAt: string) {
    // 与 `findReviews` 链路对齐：统一加 try/catch/finally 防 loading 卡死，
    // 失败文案带「品退匹配失败：」前缀，便于用户识别哪条链路。
    // 注：后端 `find_quality_refund_orders` 不发 `order-sync-progress` 事件，
    // 因此这里不像差评链路那样用 `withSyncEvents` 包裹（否则就是死监听）。
    store.setLoading(true);
    store.setError(null);
    store.setLastMode("quality_refund");
    store.setLastQuery({ days, time_window: { start_at: startAt, end_at: endAt } });
    try {
      const payload = await qualityRefundInvoke.execute({
        days,
        start_at: startAt,
        end_at: endAt,
      });
      if (payload) {
        applyResults(payload, "品退匹配");
        // 品退链路刷新轻量的缓存状态（订单计数/覆盖缺口）即可；
        // 完整订单列表由「订单管理」按需拉取，不在此链路触发，避免多余开销。
        void loadCacheStatus().catch((error) => {
          orderStore.error = toErrorMessage(error);
        });
      } else {
        store.setError(qualityRefundInvoke.error.value ?? "获取品退订单失败");
      }
    } catch (error) {
      orderStore.error = toErrorMessage(error);
      store.setError(`品退匹配失败：${orderStore.error}`);
    } finally {
      store.setLoading(false);
    }
  }

  function prefillMatchedOrder(orderId: string, source: "评价匹配" | "品退匹配" = "评价匹配") {
    deliveryStore.prefillOrder(orderId, source);
  }

  return {
    findReviews,
    findQualityRefundOrders,
    prefillMatchedOrder,
    loading,
  };
}
