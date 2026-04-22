//! 任务级授权（PRD §5.6 / M2-08）。
//!
//! 功能：每次执行「差评查询 / 全量扫描 / 品退查询 / 批量发货 / 缓存管理」
//! 等危险操作之前，向 LicenseService 申请一个 30 分钟有效的 `Rg`；
//! Grant 结构体里带 `grant_id`，业务层把它作为 `X-Grant-Id` 头随请求发往
//! 平台 API，方便运营端追溯每次高风险操作。
//!
//! 本文件只做「**本地快速通道**」和「**缓存**」：
//! - 本地快通道：Lease 有效 + task_policy 命中 → 直接签发 grant，不走网络
//! - 缓存：`TaskGrantCache` 按 task_type 缓存已签发的 grant，30 分钟内复用
//! - 联网升级：当 `risk_level=high` 或本地通道拒绝时，调用方可以选择调
//!   Worker `/api/task/authorize` 升级授权；该路径在 M2-11 真正接通 Worker
//!   后由上层组合 `authorize_task_local` + Worker 调用实现，本层不重复造
//!   HTTP 客户端。

use std::collections::HashMap;
use std::sync::Mutex;

use api_contracts::{is_supported_task, Lp, Rg, RiskLevel};
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::LICENSE_RUNTIME_GRANT_MINUTES;

/// 任务授权过程中的错误分类。
#[derive(Debug, Error)]
pub enum GrantError {
    /// task_type 不在 `SUPPORTED_TASKS` 白名单里。
    #[error("任务类型不受支持：{0}")]
    UnsupportedTask(String),
    /// 本地 Lease 的 task_policy 不含该任务。
    #[error("本地授权策略不允许执行 {0}")]
    PolicyDenied(String),
    /// 本地 Lease 已硬过期，必须先续约 / 重激。
    #[error("Lease 已失效：{0}")]
    InvalidLease(String),
}

/// 本地快速通道：基于当前 Lease 直接签发 30 分钟有效的 `Rg`。
///
/// 入参：
/// - `payload`：已验签的 Lease（上层保证已通过 `LeaseVerifier`）
/// - `task_type`：需授权的任务（5 个白名单）
/// - `now_epoch`：Unix 秒
/// - `grant_id_gen`：grant_id 生成器（生产用 UUID v4；测试可注入固定值）
///
/// 语义：
/// - `task_type` 不在白名单 → `UnsupportedTask`
/// - Lease 的 task_policy 不包含 → `PolicyDenied`
/// - Lease 硬过期 → `InvalidLease`
/// - 否则返回 `Rg`，`valid_until = now + 30min` ISO8601
pub fn authorize_task_local<F>(
    payload: &Lp,
    task_type: &str,
    now_epoch: i64,
    grant_id_gen: F,
) -> Result<Rg, GrantError>
where
    F: FnOnce() -> String,
{
    if !is_supported_task(task_type) {
        return Err(GrantError::UnsupportedTask(task_type.to_string()));
    }
    if !payload.task_policy.iter().any(|t| t == task_type) {
        return Err(GrantError::PolicyDenied(task_type.to_string()));
    }
    if !payload.is_still_valid_at(now_epoch) {
        return Err(GrantError::InvalidLease("Lease 硬过期".into()));
    }

    let valid_until_epoch = now_epoch.saturating_add(LICENSE_RUNTIME_GRANT_MINUTES * 60);
    let valid_until = epoch_to_iso(valid_until_epoch);
    let risk_level = parse_risk_level(&payload.risk_level);

    Ok(Rg {
        task_type: task_type.to_string(),
        granted: true,
        grant_id: grant_id_gen(),
        valid_until,
        risk_level,
        degraded_reason: None,
    })
}

fn epoch_to_iso(epoch: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

fn iso_to_epoch(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp())
}

fn parse_risk_level(raw: &str) -> Option<RiskLevel> {
    match raw.to_ascii_lowercase().as_str() {
        "high" => Some(RiskLevel::High),
        "medium" => Some(RiskLevel::Medium),
        "low" | "" => Some(RiskLevel::Low),
        _ => None,
    }
}

/// 基于 task_type 的 Grant 缓存。
///
/// 每个 task_type 对应一条记录；valid_until 过期自动视为缓存失效。
/// 内部 `Mutex` 保证多线程下 set/get/invalidate 原子。
#[derive(Debug, Default)]
pub struct TaskGrantCache {
    entries: Mutex<HashMap<String, Rg>>,
}

impl TaskGrantCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 命中未过期的缓存才返回 Some；过期项会被动失效。
    pub fn get_valid(&self, task_type: &str, now_epoch: i64) -> Option<Rg> {
        let guard = self.entries.lock().ok()?;
        let grant = guard.get(task_type)?.clone();
        let valid_until = iso_to_epoch(&grant.valid_until)?;
        if valid_until > now_epoch {
            Some(grant)
        } else {
            None
        }
    }

    /// 写入/覆盖 grant。
    pub fn put(&self, grant: Rg) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(grant.task_type.clone(), grant);
        }
    }

    /// 使指定任务的缓存立刻失效（如服务端主动吊销场景）。
    pub fn invalidate(&self, task_type: &str) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.remove(task_type);
        }
    }

    /// 清空所有缓存（Lease 吊销 / 重新激活时使用）。
    pub fn clear(&self) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_contracts::{
        LEASE_KIND_LICENSE, LICENSE_TASK_BATCH_DELIVERY, LICENSE_TASK_CACHE_MANAGE,
        LICENSE_TASK_QUALITY_REFUND, LICENSE_TASK_REVIEW_FIND, LICENSE_TASK_REVIEW_FULL_SCAN,
    };

    fn sample_payload(task_policy: Vec<String>, exp: i64, risk_level: &str) -> Lp {
        Lp {
            kind: LEASE_KIND_LICENSE.into(),
            license_key: "ABCD".into(),
            device_id: "dev-1".into(),
            issued_at: 1_000,
            exp,
            renew_after: 2_000,
            task_policy,
            risk_level: risk_level.into(),
        }
    }

    #[test]
    fn rejects_unsupported_task_type() {
        let payload = sample_payload(vec!["some".into()], i64::MAX, "low");
        let err = authorize_task_local(&payload, "wrong_task", 100, || "g".into()).unwrap_err();
        assert!(matches!(err, GrantError::UnsupportedTask(_)));
    }

    #[test]
    fn rejects_when_policy_missing_task() {
        let payload = sample_payload(vec![LICENSE_TASK_REVIEW_FIND.into()], i64::MAX, "low");
        let err = authorize_task_local(&payload, LICENSE_TASK_BATCH_DELIVERY, 100, || "g".into())
            .unwrap_err();
        assert!(matches!(err, GrantError::PolicyDenied(_)));
    }

    #[test]
    fn rejects_expired_lease() {
        let payload = sample_payload(vec![LICENSE_TASK_REVIEW_FIND.into()], 500, "low");
        let err = authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_000, || "g".into())
            .unwrap_err();
        assert!(matches!(err, GrantError::InvalidLease(_)));
    }

    #[test]
    fn grants_local_with_expected_validity_and_fields() {
        let payload = sample_payload(vec![LICENSE_TASK_REVIEW_FIND.into()], i64::MAX, "low");
        let grant = authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_700_000_000, || {
            "grant-xyz".into()
        })
        .unwrap();
        assert_eq!(grant.task_type, LICENSE_TASK_REVIEW_FIND);
        assert!(grant.granted);
        assert_eq!(grant.grant_id, "grant-xyz");
        assert_eq!(grant.risk_level, Some(RiskLevel::Low));
        let valid_until_epoch = iso_to_epoch(&grant.valid_until).unwrap();
        assert_eq!(
            valid_until_epoch,
            1_700_000_000 + LICENSE_RUNTIME_GRANT_MINUTES * 60
        );
    }

    #[test]
    fn risk_level_parses_all_prd_strings() {
        assert_eq!(parse_risk_level("low"), Some(RiskLevel::Low));
        assert_eq!(parse_risk_level("medium"), Some(RiskLevel::Medium));
        assert_eq!(parse_risk_level("high"), Some(RiskLevel::High));
        assert_eq!(parse_risk_level(""), Some(RiskLevel::Low));
        assert_eq!(parse_risk_level("HIGH"), Some(RiskLevel::High)); // 大小写不敏感
        assert_eq!(parse_risk_level("unknown"), None);
    }

    #[test]
    fn cache_get_valid_returns_none_when_empty() {
        let cache = TaskGrantCache::new();
        assert!(cache.get_valid(LICENSE_TASK_REVIEW_FIND, 1_000).is_none());
    }

    #[test]
    fn cache_roundtrip_hits_within_validity_window() {
        let cache = TaskGrantCache::new();
        let payload = sample_payload(vec![LICENSE_TASK_REVIEW_FIND.into()], i64::MAX, "low");
        let grant =
            authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_000, || "g".into()).unwrap();
        cache.put(grant.clone());

        // 29 分钟 59 秒内命中缓存
        let hit = cache
            .get_valid(LICENSE_TASK_REVIEW_FIND, 1_000 + 29 * 60)
            .unwrap();
        assert_eq!(hit.grant_id, "g");
    }

    #[test]
    fn cache_miss_when_beyond_validity_window() {
        let cache = TaskGrantCache::new();
        let payload = sample_payload(vec![LICENSE_TASK_REVIEW_FIND.into()], i64::MAX, "low");
        let grant =
            authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_000, || "g".into()).unwrap();
        cache.put(grant);

        // 30 分钟后过期
        let missed = cache.get_valid(
            LICENSE_TASK_REVIEW_FIND,
            1_000 + LICENSE_RUNTIME_GRANT_MINUTES * 60 + 1,
        );
        assert!(missed.is_none());
    }

    #[test]
    fn cache_put_overwrites_previous_grant_for_same_task() {
        let cache = TaskGrantCache::new();
        let payload = sample_payload(vec![LICENSE_TASK_REVIEW_FIND.into()], i64::MAX, "low");
        let g1 = authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_000, || "old".into())
            .unwrap();
        let g2 = authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_500, || "new".into())
            .unwrap();
        cache.put(g1);
        cache.put(g2);

        let hit = cache.get_valid(LICENSE_TASK_REVIEW_FIND, 1_600).unwrap();
        assert_eq!(hit.grant_id, "new");
    }

    #[test]
    fn cache_invalidate_removes_only_specified_task() {
        let cache = TaskGrantCache::new();
        let payload_full = sample_payload(
            vec![
                LICENSE_TASK_REVIEW_FIND.into(),
                LICENSE_TASK_BATCH_DELIVERY.into(),
            ],
            i64::MAX,
            "low",
        );
        cache.put(
            authorize_task_local(&payload_full, LICENSE_TASK_REVIEW_FIND, 1_000, || {
                "rf".into()
            })
            .unwrap(),
        );
        cache.put(
            authorize_task_local(&payload_full, LICENSE_TASK_BATCH_DELIVERY, 1_000, || {
                "bd".into()
            })
            .unwrap(),
        );

        cache.invalidate(LICENSE_TASK_REVIEW_FIND);
        assert!(cache.get_valid(LICENSE_TASK_REVIEW_FIND, 1_500).is_none());
        assert!(cache
            .get_valid(LICENSE_TASK_BATCH_DELIVERY, 1_500)
            .is_some());
    }

    #[test]
    fn cache_clear_removes_all_tasks() {
        let cache = TaskGrantCache::new();
        let payload = sample_payload(
            vec![
                LICENSE_TASK_REVIEW_FIND.into(),
                LICENSE_TASK_BATCH_DELIVERY.into(),
            ],
            i64::MAX,
            "low",
        );
        cache.put(
            authorize_task_local(&payload, LICENSE_TASK_REVIEW_FIND, 1_000, || "rf".into())
                .unwrap(),
        );
        cache.put(
            authorize_task_local(&payload, LICENSE_TASK_BATCH_DELIVERY, 1_000, || "bd".into())
                .unwrap(),
        );
        cache.clear();
        assert!(cache.get_valid(LICENSE_TASK_REVIEW_FIND, 1_500).is_none());
        assert!(cache
            .get_valid(LICENSE_TASK_BATCH_DELIVERY, 1_500)
            .is_none());
    }

    #[test]
    fn supported_task_white_list_matches_prd() {
        let payload = sample_payload(
            vec![
                LICENSE_TASK_REVIEW_FIND.into(),
                LICENSE_TASK_REVIEW_FULL_SCAN.into(),
                LICENSE_TASK_QUALITY_REFUND.into(),
                LICENSE_TASK_BATCH_DELIVERY.into(),
                LICENSE_TASK_CACHE_MANAGE.into(),
            ],
            i64::MAX,
            "low",
        );
        for task in [
            LICENSE_TASK_REVIEW_FIND,
            LICENSE_TASK_REVIEW_FULL_SCAN,
            LICENSE_TASK_QUALITY_REFUND,
            LICENSE_TASK_BATCH_DELIVERY,
            LICENSE_TASK_CACHE_MANAGE,
        ] {
            let grant = authorize_task_local(&payload, task, 1_000, || "g".into()).unwrap();
            assert_eq!(grant.task_type, task);
        }
    }
}
