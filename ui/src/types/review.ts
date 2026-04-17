import type { TimeWindow } from "./common";

export type MatchSource =
  | "exact_order_id"
  | "receiver_and_time_window"
  | "receiver_and_amount"
  | "manual_fallback";

export type MatchStrategy =
  | "exact_match"
  | "high_confidence"
  | "probable_match"
  | "fallback";

export interface QualityRefundInfo {
  reason: string;
  source: string;
}

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
  strategy: MatchStrategy;
  replyable: boolean;
  reply_deadline: string | null;
  confidence_score: number;
  quality_refund_info: QualityRefundInfo | null;
  match_reasons: string[];
  candidate_count: number;
  top_score: number;
}

export interface ReviewQuery {
  days: number;
  time_window: TimeWindow;
}

export interface ReviewMatchResponse {
  results: OrderMatchResult[];
  cache_warnings: string[];
  cache_coverage_start: string | null;
  cache_coverage_end: string | null;
  cache_sync_performed: boolean;
  cache_sync_written_count: number;
}
