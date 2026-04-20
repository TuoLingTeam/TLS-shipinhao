use api_contracts::LicenseState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedKeyStatus {
    Unused,
    Activated,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeneratedKeyRecord {
    pub license_key: String,
    pub plan_days: u32,
    pub status: GeneratedKeyStatus,
    pub created_at: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseRecord {
    pub license_key: String,
    pub device_id: String,
    pub device_fingerprint: String,
    pub plan_days: u32,
    pub activated_at: String,
    pub license_expires_at: String,
    pub updated_at: String,
    pub binding_version: u32,
    pub status: LicenseState,
    pub last_verify_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceRegistration {
    pub license_key: String,
    pub device_id: String,
    pub device_fingerprint_hash: String,
    pub registered_at: String,
    pub last_seen_at: String,
    pub registration_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationInput {
    pub license_key: String,
    pub device_id: String,
    pub device_fingerprint: String,
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyInput {
    pub license_key: String,
    pub device_id: String,
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub action: String,
    pub license_key: String,
    pub device_id: String,
    pub reason: String,
    pub created_at: String,
}
