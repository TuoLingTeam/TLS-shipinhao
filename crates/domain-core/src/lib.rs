pub mod brand;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    ReviewFind,
    ReviewFullScan,
    QualityRefund,
    BatchDelivery,
    CacheManage,
}

impl TaskKind {
    /// 枚举对应的 canonical 字符串。与 `api_contracts::LICENSE_TASK_*` 常量字面量一致，
    /// `#[serde(rename_all = "snake_case")]` 的序列化结果也相同。改任一侧都会破坏
    /// 已发放的 Lease / task_policy 字符串匹配，因此 `domain-core` 单测与 api-contracts
    /// 常量通过 dev-dep 锁定等价关系。
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ReviewFind => "review_find",
            Self::ReviewFullScan => "review_full_scan",
            Self::QualityRefund => "quality_refund",
            Self::BatchDelivery => "batch_delivery",
            Self::CacheManage => "cache_manage",
        }
    }

    /// 所有已知任务类型，迭代顺序与 `api_contracts::SUPPORTED_TASKS` 对齐。
    pub const ALL: &'static [TaskKind] = &[
        Self::ReviewFind,
        Self::ReviewFullScan,
        Self::QualityRefund,
        Self::BatchDelivery,
        Self::CacheManage,
    ];
}

impl AsRef<str> for TaskKind {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
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

#[cfg(test)]
mod task_kind_tests {
    use super::TaskKind;
    use api_contracts::{
        is_supported_task, LICENSE_TASK_BATCH_DELIVERY, LICENSE_TASK_CACHE_MANAGE,
        LICENSE_TASK_QUALITY_REFUND, LICENSE_TASK_REVIEW_FIND, LICENSE_TASK_REVIEW_FULL_SCAN,
        SUPPORTED_TASKS,
    };

    #[test]
    fn as_str_matches_api_contracts_string_constants() {
        // 若某一项字面量被人改写（比如把 "review_find" 改成 "reviewFind"），这一测试
        // 会立即报警——已发布的 Lease / task_policy JSON 字符串都依赖这些字面量。
        assert_eq!(TaskKind::ReviewFind.as_str(), LICENSE_TASK_REVIEW_FIND);
        assert_eq!(
            TaskKind::ReviewFullScan.as_str(),
            LICENSE_TASK_REVIEW_FULL_SCAN
        );
        assert_eq!(
            TaskKind::QualityRefund.as_str(),
            LICENSE_TASK_QUALITY_REFUND
        );
        assert_eq!(
            TaskKind::BatchDelivery.as_str(),
            LICENSE_TASK_BATCH_DELIVERY
        );
        assert_eq!(TaskKind::CacheManage.as_str(), LICENSE_TASK_CACHE_MANAGE);
    }

    #[test]
    fn all_variants_are_in_supported_tasks_and_count_matches() {
        assert_eq!(
            TaskKind::ALL.len(),
            SUPPORTED_TASKS.len(),
            "TaskKind::ALL 必须与 api_contracts::SUPPORTED_TASKS 长度一致",
        );
        for kind in TaskKind::ALL.iter() {
            assert!(
                is_supported_task(kind.as_str()),
                "{kind:?} 必须被 is_supported_task 认可",
            );
        }
    }

    #[test]
    fn serde_roundtrip_matches_as_str() {
        // `#[serde(rename_all = "snake_case")]` 的序列化结果必须与 `as_str` 一致，
        // 防止将来有人加新 variant 但忘记同步 `as_str` 的 match 分支。
        for kind in TaskKind::ALL.iter() {
            let json = serde_json::to_string(kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
            let restored: TaskKind = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, *kind);
        }
    }

    #[test]
    fn as_ref_str_delegates_to_as_str() {
        let kind = TaskKind::BatchDelivery;
        let s: &str = kind.as_ref();
        assert_eq!(s, kind.as_str());
        assert_eq!(s, "batch_delivery");
    }
}
