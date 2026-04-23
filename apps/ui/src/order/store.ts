import { defineStore } from "pinia";
import { ref } from "vue";
import type { OrderCacheEntry, OrderCacheStatus } from "./types";

export const useOrderStore = defineStore("order", () => {
  const cachedOrders = ref<OrderCacheEntry[]>([]);
  const cacheStatus = ref<OrderCacheStatus | null>(null);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const lastSyncAt = ref<string | null>(null);
  const syncPhase = ref<string | null>(null);
  const syncMessage = ref<string | null>(null);
  const syncProgress = ref(0);
  const syncSource = ref<"manual" | "review_query" | null>(null);

  function setCacheStatus(status: OrderCacheStatus | null) {
    cacheStatus.value = status;
    lastSyncAt.value = status?.last_sync_at ?? null;
  }

  function reset() {
    cachedOrders.value = [];
    cacheStatus.value = null;
    loading.value = false;
    error.value = null;
    lastSyncAt.value = null;
    syncPhase.value = null;
    syncMessage.value = null;
    syncProgress.value = 0;
    syncSource.value = null;
  }

  return {
    cachedOrders,
    cacheStatus,
    loading,
    error,
    lastSyncAt,
    syncPhase,
    syncMessage,
    syncProgress,
    syncSource,
    setCacheStatus,
    reset,
  };
});
