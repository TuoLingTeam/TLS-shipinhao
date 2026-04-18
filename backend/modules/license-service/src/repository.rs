use crate::model::{AuditEvent, DeviceRegistration, GeneratedKeyRecord, LicenseRecord};

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
