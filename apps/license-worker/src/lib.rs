use api_contracts::LicenseLease;
use license_service::{ActivationInput, LicenseRepository, LicenseService, VerifyInput};

pub fn handle_activate<R: LicenseRepository>(service: &LicenseService<R>, input: ActivationInput) -> anyhow::Result<LicenseLease> {
    service.activate(input)
}

pub fn handle_verify<R: LicenseRepository>(service: &LicenseService<R>, input: VerifyInput) -> anyhow::Result<Option<LicenseLease>> {
    service.verify(input)
}

pub fn route_request(path: &str) -> &'static str {
    match path {
        "/api/activate" => "activate",
        "/api/verify" => "verify",
        _ => "not_found",
    }
}
