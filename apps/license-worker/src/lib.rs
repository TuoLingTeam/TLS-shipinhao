use license_service::{ActivationInput, LicenseRepository, LicenseService, LicenseServiceResponse, VerifyInput};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRoute {
    Activate,
    Verify,
    NotFound,
}

pub fn handle_activate<R: LicenseRepository>(
    service: &LicenseService<R>,
    input: ActivationInput,
) -> anyhow::Result<LicenseServiceResponse> {
    service.activate(input)
}

pub fn handle_verify<R: LicenseRepository>(
    service: &LicenseService<R>,
    input: VerifyInput,
) -> anyhow::Result<LicenseServiceResponse> {
    service.verify(input)
}

pub fn parse_route(path: &str) -> WorkerRoute {
    match path {
        "/api/activate" => WorkerRoute::Activate,
        "/api/verify" => WorkerRoute::Verify,
        _ => WorkerRoute::NotFound,
    }
}

pub fn route_request(path: &str) -> &'static str {
    match parse_route(path) {
        WorkerRoute::Activate => "activate",
        WorkerRoute::Verify => "verify",
        WorkerRoute::NotFound => "not_found",
    }
}

pub fn handle_json_request<R: LicenseRepository>(
    service: &LicenseService<R>,
    path: &str,
    body: &str,
) -> anyhow::Result<String> {
    let route = parse_route(path);
    let payload: Value = serde_json::from_str(body)?;
    let response = match route {
        WorkerRoute::Activate => {
            let input: ActivationInput = serde_json::from_value(payload)?;
            handle_activate(service, input)?
        }
        WorkerRoute::Verify => {
            let input: VerifyInput = serde_json::from_value(payload)?;
            handle_verify(service, input)?
        }
        WorkerRoute::NotFound => LicenseServiceResponse {
            success: false,
            message: "not_found".into(),
            license_state: api_contracts::LicenseState::Invalid,
            expired: false,
            activated_at: None,
            license_expires_at: None,
            license_lease: None,
        },
    };
    Ok(serde_json::to_string(&response)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use license_service::{
        AuditEvent, DeviceRegistration, GeneratedKeyRecord, GeneratedKeyStatus, LicenseRecord,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Repo {
        generated_keys: Mutex<HashMap<String, GeneratedKeyRecord>>,
        licenses: Mutex<HashMap<String, LicenseRecord>>,
        registrations: Mutex<HashMap<(String, String), DeviceRegistration>>,
        audits: Mutex<Vec<AuditEvent>>,
    }

    impl Repo {
        fn seeded() -> Self {
            let repo = Self::default();
            repo.generated_keys.lock().unwrap().insert(
                "TLS-TEST".into(),
                GeneratedKeyRecord {
                    license_key: "TLS-TEST".into(),
                    plan_days: 30,
                    status: GeneratedKeyStatus::Unused,
                    created_at: "2026-01-01T00:00:00Z".into(),
                    note: String::new(),
                },
            );
            repo
        }
    }

    impl LicenseRepository for Repo {
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
    fn parses_routes() {
        assert_eq!(parse_route("/api/activate"), WorkerRoute::Activate);
        assert_eq!(parse_route("/api/verify"), WorkerRoute::Verify);
        assert_eq!(parse_route("/missing"), WorkerRoute::NotFound);
    }

    #[test]
    fn handles_activate_json() {
        let repo = Repo::seeded();
        let service = LicenseService::new(repo);
        let response = handle_json_request(
            &service,
            "/api/activate",
            r#"{"license_key":"TLS-TEST","device_id":"device-1","device_fingerprint":"fp-1","client_version":"4.3.0"}"#,
        )
        .unwrap();
        let payload: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(payload["success"], true);
        assert_eq!(payload["license_state"], "active");
        assert_eq!(payload["license_lease"]["device_id"], "device-1");
    }
}
