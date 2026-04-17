use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    ReviewFind,
    ReviewFullScan,
    QualityRefund,
    BatchDelivery,
    CacheManage,
}

impl Default for TaskKind {
    fn default() -> Self {
        Self::ReviewFind
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchSource {
    ExactOrderId,
    ReceiverAndTimeWindow,
    ReceiverAndAmount,
    ManualFallback,
}

impl Default for MatchSource {
    fn default() -> Self {
        Self::ManualFallback
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchStrategy {
    ExactMatch,
    HighConfidence,
    ProbableMatch,
    Fallback,
}

impl Default for MatchStrategy {
    fn default() -> Self {
        Self::Fallback
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct TimeWindow {
    pub start_at: String,
    pub end_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct OrderCacheEntry {
    pub order_id: String,
    pub buyer_name: String,
    pub receiver_name: String,
    pub amount_cent: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct QualityRefundInfo {
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub source: String,
}

fn default_replyable() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct OrderMatchResult {
    pub evaluation_id: String,
    pub order_id: String,
    #[serde(default)]
    pub buyer_nickname: String,
    #[serde(default)]
    pub evaluation_content: String,
    #[serde(default)]
    pub product_id: String,
    #[serde(default)]
    pub sku_id: String,
    #[serde(default)]
    pub sku_name: String,
    #[serde(default)]
    pub product_name: String,
    pub matched: bool,
    pub source: MatchSource,
    #[serde(default)]
    pub strategy: MatchStrategy,
    #[serde(default = "default_replyable")]
    pub replyable: bool,
    #[serde(default)]
    pub reply_deadline: Option<String>,
    pub confidence_score: u32,
    #[serde(default)]
    pub quality_refund_info: Option<QualityRefundInfo>,
    #[serde(default)]
    pub match_reasons: Vec<String>,
    #[serde(default)]
    pub candidate_count: usize,
    #[serde(default)]
    pub top_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeliveryUpdateRequest {
    pub order_id: String,
    pub tracking_number: String,
    pub carrier_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct DeliveryUpdateResult {
    pub order_id: String,
    pub success: bool,
    pub previous_waybill: Option<String>,
    pub error_message: Option<String>,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid time window")]
    InvalidTimeWindow,
    #[error("order cache miss: {0}")]
    OrderCacheMiss(String),
    #[error("delivery update rejected: {0}")]
    DeliveryUpdateRejected(String),
    #[error("unsupported task kind")]
    UnsupportedTaskKind,
}
