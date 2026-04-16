import { invoke } from "@tauri-apps/api/core";
import { useOrderStore } from "../stores/order";
import { useTauriInvoke } from "./useTauriInvoke";
import type { OrderCacheEntry } from "../types/order";

interface OrderSyncResult {
  orders_saved: number;
}

export function useOrder() {
  const store = useOrderStore();
  const { execute, error, loading } = useTauriInvoke<OrderCacheEntry[]>("load_order_cache");

  async function loadCache(startAt: string, endAt: string, options?: { preserveLoading?: boolean }) {
    if (!options?.preserveLoading) {
      store.loading = true;
    }
    store.error = null;
    if (store.syncSource) {
      store.syncPhase = "load_cache";
      store.syncProgress = Math.max(store.syncProgress, 76);
      store.syncMessage = "正在加载本地订单缓存…";
    }
    const result = await execute({ start_at: startAt, end_at: endAt });
    if (result) {
      store.cachedOrders = result;
      store.lastSyncAt = new Date().toISOString();
      if (store.syncSource) {
        store.syncPhase = "completed";
        store.syncProgress = 100;
        store.syncMessage = `订单缓存已刷新，共 ${result.length} 条。`;
      }
    } else {
      store.error = error.value ?? "加载缓存失败";
    }
    if (!options?.preserveLoading) {
      store.loading = false;
    }
  }

  /** 远程拉单写入本地库，再加载当前时间窗内的缓存行。 */
  async function syncOrders(startAt: string, endAt: string) {
    store.loading = true;
    store.error = null;
    store.syncSource = "manual";
    store.syncPhase = "request_remote";
    store.syncProgress = 24;
    store.syncMessage = "正在调用远端拉单接口…";
    try {
      const sync = await invoke<OrderSyncResult>("sync_orders", {
        start_at: startAt,
        end_at: endAt,
      });
      store.syncPhase = "write_cache";
      store.syncProgress = 58;
      store.syncMessage = `远端返回 ${sync.orders_saved} 条订单，正在刷新本地缓存…`;
      await loadCache(startAt, endAt, { preserveLoading: true });
    } catch (e) {
      store.error = typeof e === "string" ? e : String(e);
      store.syncPhase = "failed";
      store.syncMessage = `订单同步失败：${store.error}`;
    } finally {
      store.loading = false;
    }
  }

  return { loadCache, syncOrders, loading };
}
