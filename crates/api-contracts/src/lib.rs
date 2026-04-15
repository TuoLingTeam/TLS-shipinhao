use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseState {
    Active,
    RenewalDue,
    Expired,
    Revoked,
    DeviceMismatch,
    Invalid,
    Compromised,
}

impl Default for LicenseState {
    fn default() -> Self {
        Self::Active
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RuntimeGrant {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RiskReport {
    pub level: Option<RiskLevel>,
    #[serde(default)]
    pub issues: Vec<String>,
    #[serde(default)]
    pub runtime_backend: Option<String>,
}
