export interface TimeWindow {
  start_at: string;
  end_at: string;
}

export type TaskKind =
  | "review_find"
  | "review_full_scan"
  | "quality_refund"
  | "batch_delivery"
  | "cache_manage";
