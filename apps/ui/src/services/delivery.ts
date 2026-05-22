import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useDeliveryStore } from "@/stores/delivery";
import { useTauriInvoke } from "../shared/useTauriInvoke";
import { toErrorMessage } from "../shared/toErrorMessage";
import type {
  BatchDeliveryProgressPayload,
  BatchDeliveryStep,
  BatchDeliveryStepRaw,
  DeliveryUpdateResult,
} from "@/services/deliveryTypes";

interface BatchResult {
  total_count: number;
  success_count: number;
  failure_count: number;
  stopped: boolean;
  fatal_error: string | null;
  steps: BatchDeliveryStepRaw[];
}

const DELIVERY_ERROR_STRIP_PREFIXES = [
  "更新物流信息失败：",
  "更新物流信息失败:",
  "获取订单详情失败：",
  "获取订单详情失败:",
  "发货初始化接口返回失败：",
  "发货初始化接口返回失败:",
];

function simplifyErrorMessage(msg: string | null | undefined): string | null {
  if (!msg) return null;
  let result = msg;
  for (const prefix of DELIVERY_ERROR_STRIP_PREFIXES) {
    if (result.startsWith(prefix)) {
      result = result.slice(prefix.length);
      break;
    }
  }
  return result;
}

function normalizeStep(raw: BatchDeliveryStepRaw): BatchDeliveryStep {
  return {
    index: raw.index,
    orderId: raw.orderId,
    trackingNumber: raw.trackingNumber,
    status: raw.status,
    retryable: raw.retryable ?? (raw.status === "failed"),
    oldWaybill: raw.oldWaybill ?? null,
    errorMessage: simplifyErrorMessage(raw.errorMessage),
  };
}

export function useDelivery() {
  const store = useDeliveryStore();
  const single = useTauriInvoke<DeliveryUpdateResult>("update_delivery");
  const batch = useTauriInvoke<BatchResult>("batch_delivery");

  async function updateDelivery(orderId: string, trackingNumber: string, carrierCode: string) {
    store.loading = true;
    store.error = null;
    const result = await single.execute({
      order_id: orderId,
      tracking_number: trackingNumber,
      carrier_code: carrierCode,
    });
    if (!result) {
      store.error = single.error.value ?? "发货更新失败";
    }
    store.loading = false;
    return result;
  }

  async function batchDelivery(items: { order_id: string; tracking_number: string }[]) {
    store.loading = true;
    store.error = null;
    store.startBatch(items.length);

    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<BatchDeliveryProgressPayload>("batch-delivery-progress", ({ payload }) => {
        if (!payload) return;
        if (payload.phase === "step" && payload.step) {
          store.applyBatchStep(
            normalizeStep(payload.step),
            payload.success_count,
            payload.failure_count,
            payload.processed_count,
          );
        }
      });

      const result = await batch.execute({ items });
      if (result) {
        store.finalizeBatch({
          totalCount: result.total_count,
          successCount: result.success_count,
          failureCount: result.failure_count,
          processedCount: result.success_count + result.failure_count,
          fatalError: result.fatal_error,
          stopped: result.stopped,
          steps: result.steps?.map(normalizeStep),
        });
      } else {
        store.error = batch.error.value ?? "批量修改物流失败";
        store.finalizeBatch({
          totalCount: items.length,
          successCount: store.batchProgress?.successCount ?? 0,
          failureCount: store.batchProgress?.failureCount ?? 0,
          processedCount: store.batchProgress?.processedCount ?? 0,
          fatalError: store.error,
          stopped: true,
        });
      }
      return result;
    } finally {
      if (unlisten) {
        try {
          unlisten();
        } catch {
          /* ignore */
        }
      }
      store.loading = false;
    }
  }

  async function cancelBatchDelivery() {
    if (!store.batchProgress?.running) return;
    store.markCancelRequested();
    try {
      await invoke<boolean>("cancel_batch_delivery");
    } catch (err) {
      store.error = toErrorMessage(err);
    }
  }

  async function retryFailedItems() {
    const progress = store.batchProgress;
    if (!progress) return null;
    const failed = progress.steps
      .filter((item) => item.status === "failed" && item.retryable)
      .map((item) => ({ order_id: item.orderId, tracking_number: item.trackingNumber }));
    if (failed.length === 0) return null;
    return batchDelivery(failed);
  }

  function exportFailedCsv() {
    const progress = store.batchProgress;
    if (!progress) return;
    const failed = progress.steps.filter((item) => item.status === "failed");
    if (failed.length === 0) return;
    const header = ["序号", "订单号", "快递单号", "错误信息"];
    const rows = failed.map((item) => [
      String(item.index),
      item.orderId,
      item.trackingNumber,
      (item.errorMessage ?? "").replace(/\r?\n/g, " "),
    ]);
    const csv = [header, ...rows]
      .map((columns) =>
        columns
          .map((field) => {
            const needsQuote = /[",\n\r]/.test(field);
            const escaped = field.replace(/"/g, '""');
            return needsQuote ? `"${escaped}"` : escaped;
          })
          .join(","),
      )
      .join("\r\n");
    const blobContent = `\ufeff${csv}`;
    const blob = new Blob([blobContent], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    const stamp = new Date().toISOString().replace(/[:.]/g, "-");
    anchor.href = url;
    anchor.download = `批量修改物流失败明细-${stamp}.csv`;
    document.body.appendChild(anchor);
    anchor.click();
    document.body.removeChild(anchor);
    setTimeout(() => URL.revokeObjectURL(url), 5_000);
  }

  return { updateDelivery, batchDelivery, cancelBatchDelivery, retryFailedItems, exportFailedCsv };
}
