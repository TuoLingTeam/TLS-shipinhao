import { useReviewStore } from "../stores/review";
import { useDeliveryStore } from "../stores/delivery";
import { useOrderStore } from "../stores/order";
import { useTauriInvoke } from "./useTauriInvoke";
import type { OrderMatchResult } from "../types/review";
import { computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface OrderSyncResult {
  orders_saved: number;
}

interface OrderCacheEntry {
  order_id: string;
  buyer_name: string;
  receiver_name: string;
  amount_cent: number;
  created_at: string;
  updated_at: string;
}

export function useReview() {
  const store = useReviewStore();
  const deliveryStore = useDeliveryStore();
  const orderStore = useOrderStore();
  const reviewInvoke = useTauriInvoke<OrderMatchResult[]>("find_reviews");
  const qualityRefundInvoke = useTauriInvoke<OrderMatchResult[]>("find_quality_refund_orders");
  const loading = computed(() => reviewInvoke.loading.value || qualityRefundInvoke.loading.value);

  function applyResults(result: OrderMatchResult[], source: "评价匹配" | "品退匹配") {
    store.setResults(result);
    const firstMatched = result.find((item) => item.matched && item.order_id?.trim());
    if (firstMatched) {
      deliveryStore.prefillOrder(firstMatched.order_id, source);
    }
  }

  async function findReviews(days: number, startAt: string, endAt: string) {
    store.setLoading(true);
    store.setError(null);
    store.setLastMode("bad_review");
    store.setLastQuery({ days, time_window: { start_at: startAt, end_at: endAt } });
    const result = await reviewInvoke.execute({ days, start_at: startAt, end_at: endAt });
    if (result) {
      const needsSync = result.some((item) => !item.matched && item.order_id?.trim());
      if (needsSync) {
        try {
          orderStore.loading = true;
          orderStore.error = null;
          orderStore.syncSource = "review_auto_sync";
          orderStore.syncPhase = "request_remote";
          orderStore.syncProgress = 20;
          orderStore.syncMessage = "检测到订单缓存缺口，正在自动同步订单缓存…";
          await invoke<OrderSyncResult>("sync_orders", {
            start_at: startAt,
            end_at: endAt,
          });
          orderStore.syncPhase = "load_cache";
          orderStore.syncProgress = 72;
          orderStore.syncMessage = "订单已拉取完成，正在加载本地缓存并重新匹配差评…";
          const cachedOrders = await invoke<OrderCacheEntry[]>("load_order_cache", {
            start_at: startAt,
            end_at: endAt,
          });
          orderStore.cachedOrders = cachedOrders;
          orderStore.lastSyncAt = new Date().toISOString();
          orderStore.syncPhase = "re_match";
          orderStore.syncProgress = 90;
          orderStore.syncMessage = "缓存已刷新，正在重新匹配差评订单…";
          const retried = await reviewInvoke.execute({ days, start_at: startAt, end_at: endAt });
          if (retried) {
            orderStore.syncPhase = "completed";
            orderStore.syncProgress = 100;
            orderStore.syncMessage = `自动同步完成，已重新匹配 ${retried.length} 条差评记录。`;
            applyResults(retried, "评价匹配");
          } else {
            store.setError(reviewInvoke.error.value ?? "订单同步后重新匹配失败");
            store.setResults(result);
          }
        } catch (error) {
          orderStore.error = typeof error === "string" ? error : String(error);
          orderStore.syncPhase = "failed";
          orderStore.syncProgress = 100;
          orderStore.syncMessage = `自动同步失败：${orderStore.error}`;
          store.setError(`自动同步订单失败：${orderStore.error}`);
          store.setResults(result);
        } finally {
          orderStore.loading = false;
          if (orderStore.syncSource === "review_auto_sync") {
            window.setTimeout(() => {
              if (!orderStore.loading) {
                orderStore.syncSource = null;
                orderStore.syncPhase = null;
                orderStore.syncMessage = null;
                orderStore.syncProgress = 0;
              }
            }, 1200);
          }
        }
      } else {
        applyResults(result, "评价匹配");
      }
    } else {
      store.setError(reviewInvoke.error.value ?? "查找失败");
    }
    store.setLoading(false);
  }

  async function findQualityRefundOrders(days: number, startAt: string, endAt: string) {
    store.setLoading(true);
    store.setError(null);
    store.setLastMode("quality_refund");
    store.setLastQuery({ days, time_window: { start_at: startAt, end_at: endAt } });
    const result = await qualityRefundInvoke.execute({ days, start_at: startAt, end_at: endAt });
    if (result) {
      applyResults(result, "品退匹配");
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
