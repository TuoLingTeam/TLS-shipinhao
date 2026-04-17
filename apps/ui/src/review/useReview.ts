import { computed } from "vue";
import { useReviewStore } from "./review.store";
import { useDeliveryStore } from "../delivery/delivery.store";
import { useOrderStore } from "../order/order.store";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import { useOrder } from "../order/useOrder";
import type { OrderMatchResult, ReviewMatchResponse } from "./review.types";

export function useReview() {
  const store = useReviewStore();
  const deliveryStore = useDeliveryStore();
  const orderStore = useOrderStore();
  const { withSyncEvents, loadCacheStatus, loadRecentCache } = useOrder();
  const reviewInvoke = useTauriInvoke<ReviewMatchResponse>("find_reviews");
  const qualityRefundInvoke = useTauriInvoke<ReviewMatchResponse>("find_quality_refund_orders");
  const loading = computed(() => reviewInvoke.loading.value || qualityRefundInvoke.loading.value);

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

    const autofillCandidate =
      source === "评价匹配"
        ? payload.results.find(
            (item) =>
              item.matched &&
              item.order_id?.trim() &&
              ["exact_match", "high_confidence"].includes(item.strategy),
          )
        : payload.results.find((item) => item.matched && item.order_id?.trim());

    if (autofillCandidate) {
      deliveryStore.prefillOrder(autofillCandidate.order_id, source);
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
          orderStore.error = typeof error === "string" ? error : String(error);
        });
      } else {
        store.setError(reviewInvoke.error.value ?? "查找失败");
      }
    } catch (error) {
      orderStore.error = typeof error === "string" ? error : String(error);
      store.setError(`差评匹配失败：${orderStore.error}`);
    } finally {
      store.setLoading(false);
    }
  }

  async function findQualityRefundOrders(days: number, startAt: string, endAt: string) {
    store.setLoading(true);
    store.setError(null);
    store.setLastMode("quality_refund");
    store.setLastQuery({ days, time_window: { start_at: startAt, end_at: endAt } });
    const payload = await qualityRefundInvoke.execute({ days, start_at: startAt, end_at: endAt });
    if (payload) {
      applyResults(payload, "品退匹配");
    } else {
      store.setError(qualityRefundInvoke.error.value ?? "获取品退订单失败");
    }
    store.setLoading(false);
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
