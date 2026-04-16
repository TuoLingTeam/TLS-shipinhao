import type { TimeWindow } from "./common";

export type MatchSource =
  | "exact_order_id"
  | "receiver_and_time_window"
  | "receiver_and_amount"
  | "manual_fallback";

export interface OrderMatchResult {
  evaluation_id: string;
  order_id: string;
  buyer_nickname: string;
  evaluation_content: string;
  product_id: string;
  sku_id: string;
  sku_name: string;
  product_name: string;
  matched: boolean;
  source: MatchSource;
  confidence_score: number;
}

export interface ReviewQuery {
  days: number;
  time_window: TimeWindow;
}
