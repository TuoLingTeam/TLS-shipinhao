use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ReleaseArtifact {
    pub platform: String,
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ManifestSignature {
    pub public_key_id: String,
    pub signature: String,
}

pub fn generate_integrity_manifest(paths: &[String]) -> anyhow::Result<String> {
    Ok(serde_json::json!({ "files": paths }).to_string())
}

pub fn sign_manifest(manifest_json: &str, key_id: &str) -> anyhow::Result<ManifestSignature> {
    Ok(ManifestSignature {
        public_key_id: key_id.to_string(),
        signature: format!("signed:{}", manifest_json.len()),
    })
}

pub fn inject_version(raw: &str, version: &str) -> anyhow::Result<String> {
    Ok(raw.replace("__APP_VERSION__", version))
}
