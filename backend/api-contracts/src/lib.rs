use serde::{Deserialize, Serialize};

/// release 占位 Debug：避免二进制里残留 struct/字段名；dev build 仍走 derive(Debug)。
/// 调用点 `{:?}` 依旧能编译，但 release 下输出统一是 "_"。
/// 跨 crate 共享以避免各 `src/*.rs` 重复定义同款宏。
#[macro_export]
macro_rules! blank_debug_release {
    ($t:ty) => {
        #[cfg(not(debug_assertions))]
        impl ::core::fmt::Debug for $t {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str("_")
            }
        }
    };
}

blank_debug_release!(Rg);
blank_debug_release!(Lp);

/// 授权校验结果的原因枚举（与 Python 协议 v3 的 `LicenseReason` 对齐）。
///
/// JSON 形态始终为 snake_case。序列化用命名与 Python 版完全一致，历史枚举
/// 命名 `Active` 与 Python 的 `"ok"` 互通（通过 serde alias 支持），所以前端
/// 收到历史数据里的 `"active"` 仍能反序列化。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LicenseState {
    /// 已授权；JSON 形态为 `"active"`（兼容历史），同时接受 `"ok"`。
    #[serde(alias = "ok")]
    Active,
    /// 卡密未激活 / 未找到本地 Lease。
    NotFound,
    /// 卡密无效或签名校验失败。
    Invalid,
    /// 已过期。
    Expired,
    /// 设备指纹与签发时不一致。
    DeviceMismatch,
    /// 需要重新激活（多见于设备重置、binding 版本升级）。
    ReactivationRequired,
    /// 已被管理员吊销。
    Revoked,
    /// 需要联网续约（本地 Lease 超过 renew_after 但未过 exp）。
    OnlineRefreshRequired,
    /// 租约进入待续期窗口，仍可本地使用。
    RenewalDue,
    /// 完整性校验失败，运行时已被标记为「已篡改」。
    Compromised,
}

impl Default for LicenseState {
    fn default() -> Self {
        Self::Active
    }
}

/// 允许在离线模式下继续使用的状态集合（与 Python 版 `_ALLOWED_LOCAL_REASONS` 对齐）。
///
/// 命中这些状态时，客户端无需强制联网校验即可让用户继续使用功能。
pub const ALLOWED_LOCAL_STATES: &[LicenseState] = &[LicenseState::Active, LicenseState::RenewalDue];

impl LicenseState {
    /// 判断当前状态是否允许离线继续使用。
    pub fn is_locally_allowed(self) -> bool {
        ALLOWED_LOCAL_STATES.contains(&self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl Default for RiskLevel {
    fn default() -> Self {
        Self::Low
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct LicenseLease {
    pub license_key: String,
    pub device_id: String,
    pub license_status: LicenseState,
    pub license_expires_at: String,
    pub lease_expires_at: String,
    pub renew_after: String,
    #[serde(default)]
    pub task_policy: Vec<String>,
    pub keyset_version: u32,
    pub binding_version: u32,
    pub issued_at: String,
}

#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Rg {
    pub task_type: String,
    pub granted: bool,
    pub grant_id: String,
    pub valid_until: String,
    pub risk_level: Option<RiskLevel>,
    #[serde(default)]
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct IntegrityManifestFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct IntegrityManifest {
    pub version: u32,
    pub generated_at: String,
    #[serde(default)]
    pub files: Vec<IntegrityManifestFile>,
    pub signature: String,
}

impl IntegrityManifest {
    /// 返回签名所覆盖的 canonical payload 字节（`version` / `generated_at` / `files`，**不含 `signature`**）。
    ///
    /// 打包侧（`build-tools::sign_manifest`）与校验侧（`security_core::integrity::canonicalize_manifest`）
    /// 必须落在**完全一致**的字节串上，否则 Ed25519 签名会失效。抽到本方法后：
    ///
    /// - 字段顺序固定（按 struct 声明顺序）
    /// - `version: u32` / `generated_at: &str` / `files: &[..]` 的序列化规则固定，无尾随空白
    /// - 后续如需扩字段，同步调整 `security_core::integrity::ManifestPayload`（需加 regression fixture）
    pub fn canonical_payload_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        #[derive(Serialize)]
        struct PayloadView<'a> {
            version: u32,
            generated_at: &'a str,
            files: &'a [IntegrityManifestFile],
        }
        serde_json::to_vec(&PayloadView {
            version: self.version,
            generated_at: &self.generated_at,
            files: &self.files,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RiskReport {
    pub level: Option<RiskLevel>,
    #[serde(default)]
    pub issues: Vec<String>,
    #[serde(default)]
    pub runtime_backend: Option<String>,
}

/// Ed25519 签名载荷里 `kind` 字段的固定取值。
pub const LEASE_KIND_LICENSE: &str = "license_lease";

/// Protocol v3 的 Lease 签名载荷（与 Worker 端 base64url(payload) 一一对齐）。
///
/// 这是「**签名内容**」，与 `LicenseLease`（UI 展示层）不同——Worker 签发时
/// 只会对本结构 canonical JSON 做 Ed25519 签名，客户端必须先验签再用此结构
/// 的字段恢复 `RuntimeState`。
#[cfg_attr(debug_assertions, derive(Debug))]
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct Lp {
    /// 固定为 `LEASE_KIND_LICENSE = "license_lease"`。其他值一律视为非法。
    pub kind: String,
    pub license_key: String,
    pub device_id: String,
    /// 签发时刻的 Unix 秒。
    pub issued_at: i64,
    /// 硬过期时间（Unix 秒）；Python 版默认 issued_at + 72h。
    pub exp: i64,
    /// 软刷新时间（Unix 秒）；超过即需要续约，但未过 `exp` 仍可离线使用。
    pub renew_after: i64,
    #[serde(default)]
    pub task_policy: Vec<String>,
    /// 风险等级字符串形式（"low" / "medium" / "high"），保持协议原样以便 Worker
    /// 侧后续扩展。
    #[serde(default)]
    pub risk_level: String,
}

impl Lp {
    /// 载荷 kind 是否合法。
    pub fn has_valid_kind(&self) -> bool {
        self.kind == LEASE_KIND_LICENSE
    }

    /// 判断给定时刻是否允许本地使用（未过硬过期）。
    pub fn is_still_valid_at(&self, now_epoch: i64) -> bool {
        now_epoch < self.exp
    }

    /// 判断给定时刻是否到了软刷新窗口。
    pub fn is_renewal_due_at(&self, now_epoch: i64) -> bool {
        now_epoch >= self.renew_after
    }
}

/// 客户端运行时可见的授权状态（前端可直接消费）。
///
/// 与 Python 版 `RuntimeState` 字段完全对齐，方便「桌面端 Rust ↔ Worker ↔ 前端」
/// 三方协议一致。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeState {
    pub license_key: String,
    pub device_id: String,
    /// 核心状态（原因）。
    pub reason: LicenseState,
    /// 面向 UI 的状态提示：同值则简洁展示，不同值表达「当前可用但即将失效」等。
    pub status_hint: LicenseState,
    /// ISO8601，卡密硬过期时间。
    #[serde(default)]
    pub license_expires_at: String,
    /// ISO8601，Lease 硬过期时间（通常等于 license_expires_at，除非 Worker 提前签发）。
    #[serde(default)]
    pub lease_expires_at: String,
    /// ISO8601，Lease 到达软刷新窗口的时刻。
    #[serde(default)]
    pub renew_after: String,
    #[serde(default)]
    pub task_policy: Vec<String>,
    #[serde(default)]
    pub risk_level: String,
    /// 校验入口的来源：`"rust"` / `"python_legacy"` / `"cache"` 等。
    #[serde(default)]
    pub runtime_backend: String,
    /// 完整性校验是否失败。`true` 时前端应禁用所有业务功能。
    #[serde(default)]
    pub compromised: bool,
    /// 上一次调服务端校验的时刻（ISO8601，空表示从未联网校验）。
    #[serde(default)]
    pub last_verify_at: String,
}

impl RuntimeState {
    /// 仅填充状态原因的快捷构造（用于错误分支）。
    pub fn reason_only(reason: LicenseState) -> Self {
        Self {
            reason,
            status_hint: reason,
            runtime_backend: "rust".to_string(),
            ..Self::default()
        }
    }
}

// ---- 任务级授权常量（与 Worker /api/task/authorize 对齐） --------------------

pub const LICENSE_TASK_REVIEW_FIND: &str = "review_find";
pub const LICENSE_TASK_REVIEW_FULL_SCAN: &str = "review_full_scan";
pub const LICENSE_TASK_QUALITY_REFUND: &str = "quality_refund";
pub const LICENSE_TASK_BATCH_DELIVERY: &str = "batch_delivery";
pub const LICENSE_TASK_CACHE_MANAGE: &str = "cache_manage";

/// 当前支持的任务类型白名单。新增任务时同时扩展此数组与 Worker 端的 policy。
pub const SUPPORTED_TASKS: &[&str] = &[
    LICENSE_TASK_REVIEW_FIND,
    LICENSE_TASK_REVIEW_FULL_SCAN,
    LICENSE_TASK_QUALITY_REFUND,
    LICENSE_TASK_BATCH_DELIVERY,
    LICENSE_TASK_CACHE_MANAGE,
];

/// 判断任务类型是否受支持。业务层在调 authorize_task 前应先用此函数过滤。
pub fn is_supported_task(task_type: &str) -> bool {
    SUPPORTED_TASKS.contains(&task_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn license_state_serializes_as_snake_case() {
        for (state, expected) in [
            (LicenseState::Active, "\"active\""),
            (LicenseState::NotFound, "\"not_found\""),
            (LicenseState::Invalid, "\"invalid\""),
            (LicenseState::Expired, "\"expired\""),
            (LicenseState::DeviceMismatch, "\"device_mismatch\""),
            (
                LicenseState::ReactivationRequired,
                "\"reactivation_required\"",
            ),
            (LicenseState::Revoked, "\"revoked\""),
            (
                LicenseState::OnlineRefreshRequired,
                "\"online_refresh_required\"",
            ),
            (LicenseState::RenewalDue, "\"renewal_due\""),
            (LicenseState::Compromised, "\"compromised\""),
        ] {
            let s = serde_json::to_string(&state).unwrap();
            assert_eq!(s, expected, "{state:?} 必须序列化为 {expected}");
        }
    }

    #[test]
    fn license_state_deserializes_alias_ok_to_active() {
        // Python 协议 v3 有些接口返回 `"ok"`，桌面端历史用 `"active"`；需要兼容。
        let state: LicenseState = serde_json::from_str("\"ok\"").unwrap();
        assert_eq!(state, LicenseState::Active);
        let state: LicenseState = serde_json::from_str("\"active\"").unwrap();
        assert_eq!(state, LicenseState::Active);
    }

    #[test]
    fn allowed_local_states_matches_python_reference() {
        assert_eq!(ALLOWED_LOCAL_STATES.len(), 2);
        assert!(ALLOWED_LOCAL_STATES.contains(&LicenseState::Active));
        assert!(ALLOWED_LOCAL_STATES.contains(&LicenseState::RenewalDue));
    }

    #[test]
    fn license_state_is_locally_allowed_gates_offline_usage() {
        assert!(LicenseState::Active.is_locally_allowed());
        assert!(LicenseState::RenewalDue.is_locally_allowed());
        for denied in [
            LicenseState::NotFound,
            LicenseState::Invalid,
            LicenseState::Expired,
            LicenseState::DeviceMismatch,
            LicenseState::ReactivationRequired,
            LicenseState::Revoked,
            LicenseState::OnlineRefreshRequired,
            LicenseState::Compromised,
        ] {
            assert!(!denied.is_locally_allowed(), "{denied:?} 不应允许离线使用");
        }
    }

    #[test]
    fn lease_payload_roundtrips_through_json() {
        let payload = Lp {
            kind: LEASE_KIND_LICENSE.into(),
            license_key: "ABCD-EFGH".into(),
            device_id: "dev-123".into(),
            issued_at: 1_730_000_000,
            exp: 1_730_172_800,
            renew_after: 1_730_086_400,
            task_policy: vec!["review_find".into(), "batch_delivery".into()],
            risk_level: "low".into(),
        };
        let json_str = serde_json::to_string(&payload).unwrap();
        let restored: Lp = serde_json::from_str(&json_str).unwrap();
        assert_eq!(payload, restored);
        // 字段名 snake_case，避免 camelCase 漂移。
        assert!(json_str.contains("\"license_key\""));
        assert!(json_str.contains("\"renew_after\""));
        assert!(json_str.contains("\"task_policy\""));
    }

    #[test]
    fn lease_payload_kind_validation_rejects_wrong_kind() {
        let mut payload = Lp::default();
        payload.kind = "task_grant".into();
        assert!(!payload.has_valid_kind());
        payload.kind = LEASE_KIND_LICENSE.into();
        assert!(payload.has_valid_kind());
    }

    #[test]
    fn lease_payload_time_gates_track_exp_and_renew_after() {
        let payload = Lp {
            kind: LEASE_KIND_LICENSE.into(),
            issued_at: 1000,
            renew_after: 2000,
            exp: 3000,
            ..Lp::default()
        };
        assert!(payload.is_still_valid_at(2500));
        assert!(!payload.is_still_valid_at(3000));
        assert!(!payload.is_renewal_due_at(1999));
        assert!(payload.is_renewal_due_at(2000));
    }

    #[test]
    fn runtime_state_serializes_with_expected_fields() {
        let state = RuntimeState {
            license_key: "ABCD".into(),
            device_id: "dev-1".into(),
            reason: LicenseState::Active,
            status_hint: LicenseState::RenewalDue,
            license_expires_at: "2026-12-31T00:00:00Z".into(),
            lease_expires_at: "2026-12-31T00:00:00Z".into(),
            renew_after: "2026-11-30T00:00:00Z".into(),
            task_policy: vec!["review_find".into()],
            risk_level: "low".into(),
            runtime_backend: "rust".into(),
            compromised: false,
            last_verify_at: "".into(),
        };
        let v: serde_json::Value = serde_json::to_value(&state).unwrap();
        assert_eq!(v["reason"], "active");
        assert_eq!(v["status_hint"], "renewal_due");
        assert_eq!(v["task_policy"][0], "review_find");
        assert_eq!(v["runtime_backend"], "rust");
        assert_eq!(v["compromised"], false);
    }

    #[test]
    fn runtime_state_reason_only_builds_defaults() {
        let state = RuntimeState::reason_only(LicenseState::Expired);
        assert_eq!(state.reason, LicenseState::Expired);
        assert_eq!(state.status_hint, LicenseState::Expired);
        assert_eq!(state.runtime_backend, "rust");
        assert!(state.license_key.is_empty());
    }

    #[test]
    fn supported_tasks_covers_five_prd_items() {
        assert_eq!(SUPPORTED_TASKS.len(), 5);
        assert!(is_supported_task(LICENSE_TASK_REVIEW_FIND));
        assert!(is_supported_task(LICENSE_TASK_REVIEW_FULL_SCAN));
        assert!(is_supported_task(LICENSE_TASK_QUALITY_REFUND));
        assert!(is_supported_task(LICENSE_TASK_BATCH_DELIVERY));
        assert!(is_supported_task(LICENSE_TASK_CACHE_MANAGE));
        assert!(!is_supported_task("unknown_task"));
        assert!(!is_supported_task(""));
    }

    #[test]
    fn runtime_grant_serializes_and_supports_defaults() {
        let grant = Rg {
            task_type: LICENSE_TASK_REVIEW_FIND.into(),
            granted: true,
            grant_id: "g-1".into(),
            valid_until: "2026-05-01T00:00:00Z".into(),
            risk_level: Some(RiskLevel::Medium),
            degraded_reason: None,
        };
        let v = serde_json::to_value(&grant).unwrap();
        assert_eq!(v["task_type"], "review_find");
        assert_eq!(v["granted"], true);
        assert_eq!(v["risk_level"], "medium");
        let _ = json!(v); // 断言可进一步 reshape
    }
}
