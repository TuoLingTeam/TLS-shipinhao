pub struct HttpLicenseClient {
    pub worker_url: String,
}

impl HttpLicenseClient {
    pub fn new(worker_url: String) -> Self {
        Self { worker_url }
    }

    pub async fn activate(
        &self,
        license_key: &str,
        device_id: &str,
        device_fingerprint: &str,
        client_version: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/activate", self.worker_url))
            .json(&serde_json::json!({
                "license_key": license_key,
                "device_id": device_id,
                "device_fingerprint": device_fingerprint,
                "client_version": client_version,
            }))
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn verify(
        &self,
        license_key: &str,
        device_id: &str,
        client_version: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/verify", self.worker_url))
            .json(&serde_json::json!({
                "license_key": license_key,
                "device_id": device_id,
                "client_version": client_version,
            }))
            .send()
            .await?
            .json()
            .await?;
        Ok(resp)
    }
}
