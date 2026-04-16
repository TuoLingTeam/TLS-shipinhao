use api_contracts::{LicenseLease, LicenseState};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LEASE_RENEWAL_HOURS: i64 = 24;
pub const LEASE_HARD_EXPIRY_HOURS: i64 = 72;
pub const LICENSE_PROTOCOL_VERSION: u32 = 3;
pub const ISSUER: &str = "tls-license-backend";
pub const DEFAULT_TASK_POLICY: &[&str] = &[
    "review_find",
    "review_full_scan",
    "quality_refund",
    "batch_delivery",
    "cache_manage",
];

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseServiceResponse {
    pub success: bool,
    pub message: String,
    pub license_state: LicenseState,
    pub expired: bool,
    pub activated_at: Option<String>,
    pub license_expires_at: Option<String>,
    pub license_lease: Option<LicenseLease>,
}

pub trait LicenseRepository {
    fn load_generated_key(&self, license_key: &str) -> anyhow::Result<Option<GeneratedKeyRecord>>;
    fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()>;
    fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>>;
    fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()>;
    fn load_device_registration(
        &self,
        license_key: &str,
        device_id: &str,
    ) -> anyhow::Result<Option<DeviceRegistration>>;
    fn save_device_registration(&self, registration: &DeviceRegistration) -> anyhow::Result<()>;
    fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()>;
}

pub struct LicenseService<R> {
    repository: R,
}

impl<R> LicenseService<R>
where
    R: LicenseRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn activate(&self, input: ActivationInput) -> anyhow::Result<LicenseServiceResponse> {
        self.activate_at(input, Utc::now())
    }

    pub fn verify(&self, input: VerifyInput) -> anyhow::Result<LicenseServiceResponse> {
        self.verify_at(input, Utc::now())
    }

    pub fn activate_at(
        &self,
        input: ActivationInput,
        now: DateTime<Utc>,
    ) -> anyhow::Result<LicenseServiceResponse> {
        let normalized_key = normalize_key(&input.license_key);
        let Some(mut key_record) = self.repository.load_generated_key(&normalized_key)? else {
            return Ok(failure_response(
                "该卡密不存在或已被吊销",
                LicenseState::Revoked,
                false,
                None,
            ));
        };
        if key_record.status == GeneratedKeyStatus::Revoked {
            return Ok(failure_response(
                "该卡密已被吊销，无法使用",
                LicenseState::Revoked,
                false,
                None,
            ));
        }
        if key_record.plan_days == 0 {
            return Ok(failure_response(
                "卡密无效：有效期异常",
                LicenseState::Invalid,
                false,
                None,
            ));
        }

        let now_iso = iso8601(now);
        let existing = self.repository.load_license(&normalized_key)?;
        let (record, message) = if let Some(mut record) = existing {
            if record.device_id != input.device_id {
                return Ok(failure_response(
                    "该卡密已在其他设备激活，不允许更换设备。如需帮助请联系作者。",
                    LicenseState::DeviceMismatch,
                    false,
                    Some(record),
                ));
            }
            record.device_fingerprint = input.device_fingerprint.clone();
            record.updated_at = now_iso.clone();
            record.binding_version = LICENSE_PROTOCOL_VERSION;
            record.status = LicenseState::Active;
            record.last_verify_at = now_iso.clone();
            self.repository.save_license(&record)?;
            (record, "重新激活成功")
        } else {
            let expires_at = iso8601(now + Duration::days(key_record.plan_days as i64));
            let record = LicenseRecord {
                license_key: normalized_key.clone(),
                device_id: input.device_id.clone(),
                device_fingerprint: input.device_fingerprint.clone(),
                plan_days: key_record.plan_days,
                activated_at: now_iso.clone(),
                license_expires_at: expires_at,
                updated_at: now_iso.clone(),
                binding_version: LICENSE_PROTOCOL_VERSION,
                status: LicenseState::Active,
                last_verify_at: now_iso.clone(),
            };
            self.repository.save_license(&record)?;
            key_record.status = GeneratedKeyStatus::Activated;
            self.repository.save_generated_key(&key_record)?;
            (record, "激活成功")
        };

        self.upsert_device_registration(&normalized_key, &input.device_id, &input.device_fingerprint, now)?;
        self.repository.append_audit_event(&AuditEvent {
            action: "activate".into(),
            license_key: normalized_key,
            device_id: input.device_id,
            reason: if input.client_version.is_empty() {
                "client_activate".into()
            } else {
                format!("client_activate:{}", input.client_version)
            },
            created_at: now_iso,
        })?;
        Ok(success_response(record, message, now))
    }

    pub fn verify_at(
        &self,
        input: VerifyInput,
        now: DateTime<Utc>,
    ) -> anyhow::Result<LicenseServiceResponse> {
        let normalized_key = normalize_key(&input.license_key);
        let Some(key_record) = self.repository.load_generated_key(&normalized_key)? else {
            return Ok(failure_response("该卡密已被吊销", LicenseState::Revoked, true, None));
        };
        if key_record.status == GeneratedKeyStatus::Revoked {
            return Ok(failure_response("该卡密已被吊销", LicenseState::Revoked, true, None));
        }

        let Some(mut record) = self.repository.load_license(&normalized_key)? else {
            return Ok(failure_response(
                "该卡密尚未激活",
                LicenseState::Invalid,
                false,
                None,
            ));
        };

        if record.device_id != input.device_id {
            return Ok(failure_response(
                "设备不匹配：该卡密已绑定其他设备",
                LicenseState::DeviceMismatch,
                false,
                Some(record),
            ));
        }

        if record.status == LicenseState::Revoked {
            return Ok(failure_response("该卡密已被吊销", LicenseState::Revoked, true, Some(record)));
        }

        let now_iso = iso8601(now);
        let expires_at = parse_utc(&record.license_expires_at)?;
        if now >= expires_at {
            record.status = LicenseState::Expired;
            record.updated_at = now_iso.clone();
            record.last_verify_at = now_iso.clone();
            self.repository.save_license(&record)?;
            self.repository.append_audit_event(&AuditEvent {
                action: "verify".into(),
                license_key: normalized_key,
                device_id: input.device_id,
                reason: "expired".into(),
                created_at: now_iso,
            })?;
            return Ok(failure_response("授权已过期", LicenseState::Expired, true, Some(record)));
        }

        record.status = LicenseState::Active;
        record.updated_at = now_iso.clone();
        record.last_verify_at = now_iso.clone();
        self.repository.save_license(&record)?;
        self.repository.append_audit_event(&AuditEvent {
            action: "verify".into(),
            license_key: normalized_key,
            device_id: input.device_id,
            reason: if input.client_version.is_empty() {
                "client_verify".into()
            } else {
                format!("client_verify:{}", input.client_version)
            },
            created_at: now_iso,
        })?;
        Ok(success_response(record, "授权有效", now))
    }

    fn upsert_device_registration(
        &self,
        license_key: &str,
        device_id: &str,
        device_fingerprint: &str,
        now: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let now_iso = iso8601(now);
        let hash = sha256_hex(device_fingerprint);
        let mut registration = self
            .repository
            .load_device_registration(license_key, device_id)?
            .unwrap_or(DeviceRegistration {
                license_key: license_key.to_string(),
                device_id: device_id.to_string(),
                device_fingerprint_hash: hash.clone(),
                registered_at: now_iso.clone(),
                last_seen_at: now_iso.clone(),
                registration_status: "active".into(),
            });
        registration.device_fingerprint_hash = hash;
        registration.last_seen_at = now_iso;
        registration.registration_status = "active".into();
        self.repository.save_device_registration(&registration)
    }
}

pub fn issue_license_lease(
    license_key: &str,
    device_id: &str,
    state: LicenseState,
    license_expires_at: &str,
    lease_expires_at: &str,
    renew_after: &str,
    issued_at: &str,
) -> LicenseLease {
    LicenseLease {
        license_key: license_key.to_string(),
        device_id: device_id.to_string(),
        license_status: state,
        license_expires_at: license_expires_at.to_string(),
        lease_expires_at: lease_expires_at.to_string(),
        renew_after: renew_after.to_string(),
        task_policy: DEFAULT_TASK_POLICY.iter().map(|item| (*item).to_string()).collect(),
        keyset_version: 1,
        binding_version: LICENSE_PROTOCOL_VERSION,
        issued_at: issued_at.to_string(),
    }
}

fn success_response(record: LicenseRecord, message: &str, now: DateTime<Utc>) -> LicenseServiceResponse {
    LicenseServiceResponse {
        success: true,
        message: message.to_string(),
        license_state: LicenseState::Active,
        expired: false,
        activated_at: Some(record.activated_at.clone()),
        license_expires_at: Some(record.license_expires_at.clone()),
        license_lease: Some(issue_license_lease_for_record(&record, now)),
    }
}

fn failure_response(
    message: &str,
    state: LicenseState,
    expired: bool,
    record: Option<LicenseRecord>,
) -> LicenseServiceResponse {
    LicenseServiceResponse {
        success: false,
        message: message.to_string(),
        license_state: state,
        expired,
        activated_at: record.as_ref().map(|value| value.activated_at.clone()),
        license_expires_at: record.as_ref().map(|value| value.license_expires_at.clone()),
        license_lease: None,
    }
}

fn issue_license_lease_for_record(record: &LicenseRecord, now: DateTime<Utc>) -> LicenseLease {
    let lease_expires_at = iso8601(now + Duration::hours(LEASE_HARD_EXPIRY_HOURS));
    let renew_after = iso8601(now + Duration::hours(LEASE_RENEWAL_HOURS));
    issue_license_lease(
        &record.license_key,
        &record.device_id,
        record.status.clone(),
        &record.license_expires_at,
        &lease_expires_at,
        &renew_after,
        &iso8601(now),
    )
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn normalize_key(value: &str) -> String {
    value.trim().to_uppercase()
}

fn iso8601(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn parse_utc(value: &str) -> anyhow::Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryRepo {
        generated_keys: Mutex<HashMap<String, GeneratedKeyRecord>>,
        licenses: Mutex<HashMap<String, LicenseRecord>>,
        registrations: Mutex<HashMap<(String, String), DeviceRegistration>>,
        audits: Mutex<Vec<AuditEvent>>,
    }

    impl MemoryRepo {
        fn with_generated_key(key: &str, plan_days: u32) -> Self {
            let repo = Self::default();
            repo.generated_keys.lock().unwrap().insert(
                key.to_string(),
                GeneratedKeyRecord {
                    license_key: key.to_string(),
                    plan_days,
                    status: GeneratedKeyStatus::Unused,
                    created_at: "2026-01-01T00:00:00Z".into(),
                    note: String::new(),
                },
            );
            repo
        }
    }

    impl LicenseRepository for MemoryRepo {
        fn load_generated_key(&self, license_key: &str) -> anyhow::Result<Option<GeneratedKeyRecord>> {
            Ok(self.generated_keys.lock().unwrap().get(license_key).cloned())
        }

        fn save_generated_key(&self, record: &GeneratedKeyRecord) -> anyhow::Result<()> {
            self.generated_keys.lock().unwrap().insert(record.license_key.clone(), record.clone());
            Ok(())
        }

        fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>> {
            Ok(self.licenses.lock().unwrap().get(license_key).cloned())
        }

        fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()> {
            self.licenses.lock().unwrap().insert(record.license_key.clone(), record.clone());
            Ok(())
        }

        fn load_device_registration(&self, license_key: &str, device_id: &str) -> anyhow::Result<Option<DeviceRegistration>> {
            Ok(self.registrations.lock().unwrap().get(&(license_key.to_string(), device_id.to_string())).cloned())
        }

        fn save_device_registration(&self, registration: &DeviceRegistration) -> anyhow::Result<()> {
            self.registrations.lock().unwrap().insert(
                (registration.license_key.clone(), registration.device_id.clone()),
                registration.clone(),
            );
            Ok(())
        }

        fn append_audit_event(&self, event: &AuditEvent) -> anyhow::Result<()> {
            self.audits.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    #[test]
    fn activate_creates_license_and_lease() {
        let repo = MemoryRepo::with_generated_key("TLS-TEST", 30);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);
        let response = service.activate_at(ActivationInput {
            license_key: "tls-test".into(),
            device_id: "device-1".into(),
            device_fingerprint: "fingerprint-1".into(),
            client_version: "4.3.0".into(),
        }, now).unwrap();

        assert!(response.success);
        assert_eq!(response.message, "激活成功");
        let lease = response.license_lease.expect("lease");
        assert_eq!(lease.license_key, "TLS-TEST");
        assert_eq!(lease.device_id, "device-1");
        assert_eq!(lease.renew_after, "2026-04-17T00:00:00Z");
        assert_eq!(lease.lease_expires_at, "2026-04-19T00:00:00Z");
        assert_eq!(response.license_expires_at.as_deref(), Some("2026-05-16T00:00:00Z"));
    }

    #[test]
    fn activate_rejects_second_device() {
        let repo = MemoryRepo::with_generated_key("TLS-TEST", 30);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);
        service.activate_at(ActivationInput {
            license_key: "TLS-TEST".into(),
            device_id: "device-1".into(),
            device_fingerprint: "fp-1".into(),
            client_version: String::new(),
        }, now).unwrap();

        let response = service.activate_at(ActivationInput {
            license_key: "TLS-TEST".into(),
            device_id: "device-2".into(),
            device_fingerprint: "fp-2".into(),
            client_version: String::new(),
        }, now + Duration::minutes(5)).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::DeviceMismatch);
        assert!(response.license_lease.is_none());
    }

    #[test]
    fn verify_marks_expired_license() {
        let repo = MemoryRepo::with_generated_key("TLS-TEST", 30);
        repo.licenses.lock().unwrap().insert(
            "TLS-TEST".into(),
            LicenseRecord {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                device_fingerprint: "fp-1".into(),
                plan_days: 30,
                activated_at: "2026-01-01T00:00:00Z".into(),
                license_expires_at: "2026-01-31T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
                binding_version: LICENSE_PROTOCOL_VERSION,
                status: LicenseState::Active,
                last_verify_at: "2026-01-01T00:00:00Z".into(),
            },
        );
        repo.generated_keys.lock().unwrap().get_mut("TLS-TEST").unwrap().status = GeneratedKeyStatus::Activated;
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.verify_at(VerifyInput {
            license_key: "TLS-TEST".into(),
            device_id: "device-1".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::Expired);
        assert!(response.expired);
    }

    #[test]
    fn verify_returns_active_lease_for_bound_device() {
        let repo = MemoryRepo::with_generated_key("TLS-TEST", 30);
        repo.licenses.lock().unwrap().insert(
            "TLS-TEST".into(),
            LicenseRecord {
                license_key: "TLS-TEST".into(),
                device_id: "device-1".into(),
                device_fingerprint: "fp-1".into(),
                plan_days: 30,
                activated_at: "2026-04-01T00:00:00Z".into(),
                license_expires_at: "2026-05-01T00:00:00Z".into(),
                updated_at: "2026-04-01T00:00:00Z".into(),
                binding_version: LICENSE_PROTOCOL_VERSION,
                status: LicenseState::Active,
                last_verify_at: "2026-04-01T00:00:00Z".into(),
            },
        );
        repo.generated_keys.lock().unwrap().get_mut("TLS-TEST").unwrap().status = GeneratedKeyStatus::Activated;
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.verify_at(VerifyInput {
            license_key: "TLS-TEST".into(),
            device_id: "device-1".into(),
            client_version: "4.3.0".into(),
        }, now).unwrap();

        assert!(response.success);
        assert_eq!(response.license_state, LicenseState::Active);
        let lease = response.license_lease.expect("lease");
        assert_eq!(lease.task_policy, DEFAULT_TASK_POLICY.iter().map(|item| (*item).to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn activate_rejects_revoked_key() {
        let repo = MemoryRepo::with_generated_key("TLS-REV", 30);
        repo.generated_keys.lock().unwrap().get_mut("TLS-REV").unwrap().status = GeneratedKeyStatus::Revoked;
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.activate_at(ActivationInput {
            license_key: "TLS-REV".into(),
            device_id: "d".into(),
            device_fingerprint: "fp".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::Revoked);
    }

    #[test]
    fn activate_rejects_zero_plan_days() {
        let repo = MemoryRepo::with_generated_key("TLS-ZERO", 0);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.activate_at(ActivationInput {
            license_key: "TLS-ZERO".into(),
            device_id: "d".into(),
            device_fingerprint: "fp".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::Invalid);
    }

    #[test]
    fn activate_nonexistent_key_returns_revoked() {
        let repo = MemoryRepo::default();
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.activate_at(ActivationInput {
            license_key: "DOES-NOT-EXIST".into(),
            device_id: "d".into(),
            device_fingerprint: "fp".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::Revoked);
    }

    #[test]
    fn reactivate_same_device_succeeds() {
        let repo = MemoryRepo::with_generated_key("TLS-RE", 30);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        service.activate_at(ActivationInput {
            license_key: "TLS-RE".into(),
            device_id: "dev-1".into(),
            device_fingerprint: "fp-old".into(),
            client_version: String::new(),
        }, now).unwrap();

        let response = service.activate_at(ActivationInput {
            license_key: "TLS-RE".into(),
            device_id: "dev-1".into(),
            device_fingerprint: "fp-new".into(),
            client_version: "5.0.0".into(),
        }, now + Duration::hours(1)).unwrap();

        assert!(response.success);
        assert_eq!(response.message, "重新激活成功");
        assert!(response.license_lease.is_some());
    }

    #[test]
    fn verify_nonexistent_key_returns_revoked() {
        let repo = MemoryRepo::default();
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.verify_at(VerifyInput {
            license_key: "NOPE".into(),
            device_id: "d".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::Revoked);
    }

    #[test]
    fn verify_unactivated_key_returns_invalid() {
        let repo = MemoryRepo::with_generated_key("TLS-NOACT", 30);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        let response = service.verify_at(VerifyInput {
            license_key: "TLS-NOACT".into(),
            device_id: "d".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::Invalid);
    }

    #[test]
    fn verify_device_mismatch() {
        let repo = MemoryRepo::with_generated_key("TLS-DM", 30);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        service.activate_at(ActivationInput {
            license_key: "TLS-DM".into(),
            device_id: "dev-A".into(),
            device_fingerprint: "fp".into(),
            client_version: String::new(),
        }, now).unwrap();

        let response = service.verify_at(VerifyInput {
            license_key: "TLS-DM".into(),
            device_id: "dev-B".into(),
            client_version: String::new(),
        }, now).unwrap();

        assert!(!response.success);
        assert_eq!(response.license_state, LicenseState::DeviceMismatch);
    }

    #[test]
    fn activate_records_audit_event() {
        let repo = MemoryRepo::with_generated_key("TLS-AUD", 30);
        let service = LicenseService::new(repo);
        let now = DateTime::parse_from_rfc3339("2026-04-16T00:00:00Z").unwrap().with_timezone(&Utc);

        service.activate_at(ActivationInput {
            license_key: "tls-aud".into(),
            device_id: "dev-1".into(),
            device_fingerprint: "fp".into(),
            client_version: "5.0.0".into(),
        }, now).unwrap();

        let audits = service.repository.audits.lock().unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "activate");
        assert_eq!(audits[0].license_key, "TLS-AUD");
        assert!(audits[0].reason.contains("5.0.0"));
    }

    #[test]
    fn key_normalization_trims_and_uppercases() {
        assert_eq!(normalize_key("  tls-test  "), "TLS-TEST");
        assert_eq!(normalize_key("TLS-TEST"), "TLS-TEST");
    }
}
