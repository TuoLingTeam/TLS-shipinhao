import { defineStore } from "pinia";
import { ref } from "vue";
import type { OrderCacheEntry } from "../types/order";

export const useOrderStore = defineStore("order", () => {
  const cachedOrders = ref<OrderCacheEntry[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const lastSyncAt = ref<string | null>(null);

  return { cachedOrders, loading, error, lastSyncAt };
});
