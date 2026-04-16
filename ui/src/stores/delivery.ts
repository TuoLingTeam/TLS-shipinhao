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

  return { loading, error, batchProgress };
});
