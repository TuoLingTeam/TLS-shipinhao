import { defineStore } from "pinia";
import { ref } from "vue";

export interface BatchProgress {
  totalCount: number;
  successCount: number;
  failureCount: number;
  fatalError: string | null;
}

export const useDeliveryStore = defineStore("delivery", () => {
  const loading = ref(false);
  const error = ref<string | null>(null);
  const batchProgress = ref<BatchProgress | null>(null);
  const draftOrderId = ref("");
  const draftSource = ref<string | null>(null);

  function prefillOrder(orderId: string, source: string) {
    if (!orderId.trim()) return;
    draftOrderId.value = orderId.trim();
    draftSource.value = source;
  }

  function clearPrefillOrder() {
    draftOrderId.value = "";
    draftSource.value = null;
  }

  return {
    loading,
    error,
    batchProgress,
    draftOrderId,
    draftSource,
    prefillOrder,
    clearPrefillOrder,
  };
});
