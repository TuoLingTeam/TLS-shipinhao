pub mod lease;
pub mod local_verify;
pub mod model;
pub mod service;
pub mod task_grant;

pub use lease::{LeaseError, LeaseVerifier};
pub use local_verify::verify_stored_lease_local;
pub use model::{
    ActivationInput, AuditEvent, DeviceRegistration, GeneratedKeyRecord, GeneratedKeyStatus,
    LicenseRecord, VerifyInput,
};
pub use service::{
    issue_license_lease, DEFAULT_TASK_POLICY, ISSUER, LEASE_HARD_EXPIRY_HOURS, LEASE_RENEWAL_HOURS,
    LICENSE_PROTOCOL_VERSION, LICENSE_PUBLIC_KEY_B64, LICENSE_RUNTIME_GRANT_MINUTES,
};
pub use task_grant::{authorize_task_local, GrantError, TaskGrantCache};
