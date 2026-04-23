import { defineStore } from "pinia";
import { ref } from "vue";
import type { BatchDeliveryStep } from "./types";

export interface BatchProgress {
  totalCount: number;
  successCount: number;
  failureCount: number;
  processedCount: number;
  fatalError: string | null;
  stopped: boolean;
  running: boolean;
  cancelRequested: boolean;
  steps: BatchDeliveryStep[];
}

function emptyProgress(totalCount = 0): BatchProgress {
  return {
    totalCount,
    successCount: 0,
    failureCount: 0,
    processedCount: 0,
    fatalError: null,
    stopped: false,
    running: totalCount > 0,
    cancelRequested: false,
    steps: [],
  };
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

  // 批量预填订单号，用换行拼接后存入 draftOrderId
  // DeliveryView 的 watch 会按行拆分并合并到左侧订单号输入框
  function prefillOrders(orderIds: string[], source: string) {
    const joined = orderIds
      .map((id) => id.trim())
      .filter(Boolean)
      .join("\n");
    if (!joined) return;
    draftOrderId.value = joined;
    draftSource.value = source;
  }

  function clearPrefillOrder() {
    draftOrderId.value = "";
    draftSource.value = null;
  }

  function startBatch(totalCount: number) {
    batchProgress.value = emptyProgress(totalCount);
  }

  function applyBatchStep(step: BatchDeliveryStep, successCount: number, failureCount: number, processedCount: number) {
    if (!batchProgress.value) {
      batchProgress.value = emptyProgress(processedCount);
    }
    const current = batchProgress.value;
    current.successCount = successCount;
    current.failureCount = failureCount;
    current.processedCount = processedCount;
    current.steps = [...current.steps, step];
    current.running = true;
  }

  function finalizeBatch(payload: {
    totalCount: number;
    successCount: number;
    failureCount: number;
    processedCount: number;
    fatalError: string | null;
    stopped: boolean;
    steps?: BatchDeliveryStep[];
  }) {
    const previous = batchProgress.value ?? emptyProgress(payload.totalCount);
    batchProgress.value = {
      ...previous,
      totalCount: payload.totalCount,
      successCount: payload.successCount,
      failureCount: payload.failureCount,
      processedCount: payload.processedCount,
      fatalError: payload.fatalError,
      stopped: payload.stopped,
      running: false,
      cancelRequested: false,
      steps: payload.steps && payload.steps.length > 0 ? payload.steps : previous.steps,
    };
  }

  function markCancelRequested() {
    if (!batchProgress.value) return;
    batchProgress.value.cancelRequested = true;
  }

  function resetBatch() {
    batchProgress.value = null;
  }

  function resetForStoreSwitch() {
    loading.value = false;
    error.value = null;
    batchProgress.value = null;
    clearPrefillOrder();
  }

  return {
    loading,
    error,
    batchProgress,
    draftOrderId,
    draftSource,
    prefillOrder,
    prefillOrders,
    clearPrefillOrder,
    startBatch,
    applyBatchStep,
    finalizeBatch,
    markCancelRequested,
    resetBatch,
    resetForStoreSwitch,
  };
});
