import { useOrderStore } from "../stores/order";
import { useTauriInvoke } from "./useTauriInvoke";
import type { OrderCacheEntry } from "../types/order";

export function useOrder() {
  const store = useOrderStore();
  const { execute, loading } = useTauriInvoke<OrderCacheEntry[]>("load_order_cache");

  async function loadCache(startAt: string, endAt: string) {
    store.loading = true;
    store.error = null;
    const result = await execute({ start_at: startAt, end_at: endAt });
    if (result) {
      store.cachedOrders = result;
      store.lastSyncAt = new Date().toISOString();
    } else {
      store.error = "加载缓存失败";
    }
    store.loading = false;
  }

  return { loadCache, loading };
}
