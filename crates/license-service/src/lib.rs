use api_contracts::{LicenseLease, LicenseState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicenseRecord {
    pub license_key: String,
    pub device_id: String,
    pub status: LicenseState,
    pub license_expires_at: String,
    pub activated_at: String,
    pub updated_at: String,
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

pub trait LicenseRepository {
    fn load_license(&self, license_key: &str) -> anyhow::Result<Option<LicenseRecord>>;
    fn save_license(&self, record: &LicenseRecord) -> anyhow::Result<()>;
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

    pub fn activate(&self, input: ActivationInput) -> anyhow::Result<LicenseLease> {
        let lease = issue_license_lease(
            &input.license_key,
            &input.device_id,
            LicenseState::Active,
            "license_expires_at",
            "lease_expires_at",
            "renew_after",
            "issued_at",
        );
        let record = LicenseRecord {
            license_key: input.license_key.clone(),
            device_id: input.device_id.clone(),
            status: LicenseState::Active,
            license_expires_at: lease.license_expires_at.clone(),
            activated_at: lease.issued_at.clone(),
            updated_at: lease.issued_at.clone(),
        };
        self.repository.save_license(&record)?;
        self.repository.append_audit_event(&AuditEvent {
            action: "activate".into(),
            license_key: input.license_key,
            device_id: input.device_id,
            reason: "client_activate".into(),
            created_at: lease.issued_at.clone(),
        })?;
        Ok(lease)
    }

    pub fn verify(&self, input: VerifyInput) -> anyhow::Result<Option<LicenseLease>> {
        let Some(record) = self.repository.load_license(&input.license_key)? else {
            return Ok(None);
        };
        if record.device_id != input.device_id {
            return Ok(Some(issue_license_lease(
                &record.license_key,
                &record.device_id,
                LicenseState::DeviceMismatch,
                &record.license_expires_at,
                "lease_expires_at",
                "renew_after",
                "issued_at",
            )));
        }
        let lease = issue_license_lease(
            &record.license_key,
            &record.device_id,
            record.status,
            &record.license_expires_at,
            "lease_expires_at",
            "renew_after",
            "issued_at",
        );
        self.repository.append_audit_event(&AuditEvent {
            action: "verify".into(),
            license_key: input.license_key,
            device_id: input.device_id,
            reason: "client_verify".into(),
            created_at: lease.issued_at.clone(),
        })?;
        Ok(Some(lease))
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
        task_policy: vec![
            "review_find".into(),
            "review_full_scan".into(),
            "quality_refund".into(),
            "batch_delivery".into(),
            "cache_manage".into(),
        ],
        keyset_version: 1,
        binding_version: 3,
        issued_at: issued_at.to_string(),
    }
}
