import { useDeliveryStore } from "../stores/delivery";
import { useTauriInvoke } from "./useTauriInvoke";
import type { DeliveryUpdateResult } from "../types/delivery";

interface BatchResult {
  total_count: number;
  success_count: number;
  failure_count: number;
  fatal_error: string | null;
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
    store.loading = false;
    return result;
  }

  async function batchDelivery(items: { order_id: string; tracking_number: string }[]) {
    store.loading = true;
    store.error = null;
    store.batchProgress = null;
    const result = await batch.execute({ items });
    if (result) {
      store.batchProgress = {
        totalCount: result.total_count,
        successCount: result.success_count,
        failureCount: result.failure_count,
        fatalError: result.fatal_error,
      };
    }
    store.loading = false;
    return result;
  }

  return { updateDelivery, batchDelivery };
}
