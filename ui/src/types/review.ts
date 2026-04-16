import type { TimeWindow } from "./common";

export type MatchSource =
  | "exact_order_id"
  | "receiver_and_time_window"
  | "receiver_and_amount"
  | "manual_fallback";

export interface OrderMatchResult {
  evaluation_id: string;
  order_id: string;
  matched: boolean;
  source: MatchSource;
  confidence_score: number;
}

export interface ReviewQuery {
  days: number;
  time_window: TimeWindow;
}
