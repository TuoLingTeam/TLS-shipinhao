import { invoke } from "@tauri-apps/api/core";
import { useOrderStore } from "../stores/order";
import { useTauriInvoke } from "./useTauriInvoke";
import type { OrderCacheEntry } from "../types/order";

interface OrderSyncResult {
  orders_saved: number;
}

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

  /** 远程拉单写入本地库，再加载当前时间窗内的缓存行。 */
  async function syncOrders(startAt: string, endAt: string) {
    store.loading = true;
    store.error = null;
    try {
      const sync = await invoke<OrderSyncResult>("sync_orders", {
        start_at: startAt,
        end_at: endAt,
      });
      void sync.orders_saved;
      await loadCache(startAt, endAt);
    } catch (e) {
      store.error = typeof e === "string" ? e : String(e);
      store.loading = false;
    }
  }

  return { loadCache, syncOrders, loading };
}
