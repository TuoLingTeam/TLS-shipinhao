export interface OrderProduct {
  product_id: string;
  sku_id: string;
  sale_param: string;
  title: string;
  thumb_img: string;
}

export interface QualityRefundInfo {
  reason: string;
  source: string;
}

export interface OrderCacheEntry {
  order_id: string;
  buyer_name: string;
  amount_cent: number;
  created_at: string;
  updated_at: string;
  is_education_order?: boolean;
  openid?: string;
  products?: OrderProduct[];
  quality_refund_info?: QualityRefundInfo | null;
}

export interface OrderCacheStatus {
  cached_order_count: number;
  today_count: number;
  yesterday_count: number;
  last_7_days_count: number;
  last_30_days_count: number;
  today_latest_order_at: string | null;
  last_sync_at: string | null;
  coverage_start: string | null;
  coverage_end: string | null;
  coverage_complete: boolean;
  missing_segment_count: number;
  last_mode?: string | null;
  last_error?: string | null;
}

export interface OrderCacheCounts {
  today_count: number;
  yesterday_count: number;
  last_7_days_count: number;
  last_30_days_count: number;
  today_latest_order_at: string | null;
}

export interface OrderSyncResult {
  orders_saved: number;
  cache_sync_performed: boolean;
  cache_coverage_start: string | null;
  cache_coverage_end: string | null;
  cache_warnings: string[];
}

export interface OrderSyncProgressEvent {
  source: string;
  phase: string;
  progress: number;
  message: string;
}
