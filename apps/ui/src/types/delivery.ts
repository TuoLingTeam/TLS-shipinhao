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
