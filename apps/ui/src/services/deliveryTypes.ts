export interface DeliveryUpdateRequest {
  order_id: string;
  tracking_number: string;
  carrier_code: string;
}

export interface DeliveryUpdateResult {
  order_id: string;
  success: boolean;
  previous_waybill: string | null;
  error_message: string | null;
}

export type BatchDeliveryStepStatus = "success" | "failed";

export interface BatchDeliveryStep {
  index: number;
  orderId: string;
  trackingNumber: string;
  status: BatchDeliveryStepStatus;
  retryable: boolean;
  oldWaybill: string | null;
  errorMessage: string | null;
}

export interface BatchDeliveryProgressPayload {
  phase: "started" | "step" | "completed";
  total_count: number;
  success_count: number;
  failure_count: number;
  processed_count: number;
  step: BatchDeliveryStepRaw | null;
  fatal_error: string | null;
  stopped: boolean;
}

export interface BatchDeliveryStepRaw {
  index: number;
  orderId: string;
  trackingNumber: string;
  status: BatchDeliveryStepStatus;
  retryable?: boolean;
  oldWaybill: string | null;
  errorMessage: string | null;
}
