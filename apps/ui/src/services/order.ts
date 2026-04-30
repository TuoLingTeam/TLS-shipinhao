import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useOrderStore } from "@/stores/order";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import { localDaysAgoStartIso, localYesterdayEndIso } from "../shared/format";
import type {
  OrderCacheCounts,
  OrderCacheEntry,
  OrderCacheStatus,
  OrderSyncProgressEvent,
  OrderSyncResult,
} from "@/services/orderTypes";
import { toErrorMessage } from "../shared/toErrorMessage";

export function useOrder() {
  const store = useOrderStore();
  const { execute, error, loading } = useTauriInvoke<OrderCacheEntry[]>("load_order_cache");

  async function loadCache(startAt: string, endAt: string, options?: { preserveLoading?: boolean }) {
    if (!options?.preserveLoading) {
      store.loading = true;
    }
    store.error = null;
    const result = await execute({ start_at: startAt, end_at: endAt });
    if (result) {
      store.cachedOrders = result;
      store.lastSyncAt = new Date().toISOString();
    } else {
      store.error = error.value ?? "加载缓存失败";
    }
    if (!options?.preserveLoading) {
      store.loading = false;
    }
  }

  async function loadRecentCache() {
    await loadCache(localDaysAgoStartIso(30), localYesterdayEndIso());
  }

  async function loadCacheStatus() {
    const status = await invoke<OrderCacheStatus>("get_order_cache_status");
    store.setCacheStatus(status);
    store.setCacheCounts({
      today_count: status.today_count,
      yesterday_count: status.yesterday_count,
      last_7_days_count: status.last_7_days_count,
      last_30_days_count: status.last_30_days_count,
      today_latest_order_at: status.today_latest_order_at,
    });
    return status;
  }

  async function loadCacheCounts() {
    try {
      const counts = await invoke<OrderCacheCounts>("get_order_cache_counts");
      store.setCacheCounts(counts);
      return counts;
    } catch (e) {
      const status = await loadCacheStatus();
      return {
        today_count: status.today_count,
        yesterday_count: status.yesterday_count,
        last_7_days_count: status.last_7_days_count,
        last_30_days_count: status.last_30_days_count,
        today_latest_order_at: status.today_latest_order_at,
      };
    }
  }

  async function withSyncEvents<T>(
    source: "manual" | "review_query",
    runner: () => Promise<T>,
  ): Promise<T> {
    const unlisten = await listen<OrderSyncProgressEvent>("order-sync-progress", ({ payload }) => {
      if (!payload || payload.source !== source) return;
      store.syncSource = source;
      store.syncPhase = payload.phase;
      store.syncProgress = payload.progress;
      store.syncMessage = payload.message;
    });

    try {
      return await runner();
    } finally {
      unlisten();
      window.setTimeout(() => {
        if (!store.loading) {
          store.syncSource = null;
          store.syncPhase = null;
          store.syncMessage = null;
          store.syncProgress = 0;
        }
      }, 1200);
    }
  }

  async function syncRecentCache() {
    store.loading = true;
    store.error = null;
    try {
      const sync = await withSyncEvents("manual", () =>
        invoke<OrderSyncResult>("sync_recent_order_cache"),
      );
      await loadRecentCache();
      await loadCacheStatus();
      await loadCacheCounts();
      return sync;
    } catch (e) {
      store.error = toErrorMessage(e);
      return null;
    } finally {
      store.loading = false;
    }
  }

  return {
    loadCache,
    loadRecentCache,
    loadCacheStatus,
    loadCacheCounts,
    syncRecentCache,
    withSyncEvents,
    loading,
  };
}
