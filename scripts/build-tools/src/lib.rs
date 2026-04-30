use api_contracts::{IntegrityManifest, IntegrityManifestFile};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use chrono::Utc;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

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

pub fn generate_integrity_manifest(
    base_dir: &Path,
    files: &[PathBuf],
) -> anyhow::Result<IntegrityManifest> {
    let mut manifest_files = Vec::with_capacity(files.len());
    for file in files {
        let absolute = if file.is_absolute() {
            file.clone()
        } else {
            base_dir.join(file)
        };
        let relative_path = absolute
            .strip_prefix(base_dir)
            .unwrap_or(&absolute)
            .to_string_lossy()
            .replace('\\', "/");
        manifest_files.push(IntegrityManifestFile {
            path: relative_path,
            sha256: sha256_hex(&absolute)?,
        });
    }
    Ok(IntegrityManifest {
        version: 1,
        generated_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        files: manifest_files,
        signature: String::new(),
    })
}

pub fn sign_manifest(
    manifest: &IntegrityManifest,
    signing_private_key_b64: &str,
    key_id: &str,
) -> anyhow::Result<ManifestSignature> {
    let signing_key = load_manifest_signing_private_key(signing_private_key_b64)?;
    let signature = signing_key.sign(&canonical_manifest_payload(manifest)?);
    Ok(ManifestSignature {
        public_key_id: key_id.to_string(),
        signature: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
    })
}

pub fn attach_signature(
    manifest: &IntegrityManifest,
    signature: &ManifestSignature,
) -> IntegrityManifest {
    let mut next = manifest.clone();
    next.signature = signature.signature.clone();
    next
}

pub fn inject_version(raw: &str, version: &str) -> anyhow::Result<String> {
    Ok(raw.replace("__APP_VERSION__", version))
}

pub fn verify_manifest_signature(
    manifest: &IntegrityManifest,
    verify_key_b64url: &str,
) -> anyhow::Result<()> {
    let key_bytes: [u8; 32] = URL_SAFE_NO_PAD
        .decode(verify_key_b64url.as_bytes())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid verifying key length"))?;
    let verify_key = VerifyingKey::from_bytes(&key_bytes)?;
    let signature_bytes: [u8; 64] = URL_SAFE_NO_PAD
        .decode(manifest.signature.as_bytes())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid manifest signature length"))?;
    let signature = Signature::from_bytes(&signature_bytes);
    verify_key.verify(&canonical_manifest_payload(manifest)?, &signature)?;
    Ok(())
}

fn canonical_manifest_payload(manifest: &IntegrityManifest) -> anyhow::Result<Vec<u8>> {
    // 统一委托到 api-contracts，避免 build-tools 与 security_core 的投影字节漂移。
    Ok(manifest.canonical_payload_bytes()?)
}

fn load_manifest_signing_private_key(signing_private_key_b64: &str) -> anyhow::Result<SigningKey> {
    let raw = STANDARD.decode(signing_private_key_b64.trim())?;
    if let Ok(text) = String::from_utf8(raw.clone()) {
        if text.contains("BEGIN PRIVATE KEY") {
            let body = text
                .lines()
                .filter(|line| !line.starts_with("-----"))
                .collect::<String>();
            let der = STANDARD.decode(body.as_bytes())?;
            return Ok(SigningKey::from_pkcs8_der(&der)?);
        }
    }
    Ok(SigningKey::from_pkcs8_der(&raw)?)
}

fn sha256_hex(path: &Path) -> anyhow::Result<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use tempfile::tempdir;

    #[test]
    fn generates_manifest_with_relative_paths_and_hashes() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let file_path = base.join("nested/example.txt");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, b"hello tls").unwrap();

        let manifest =
            generate_integrity_manifest(base, &[PathBuf::from("nested/example.txt")]).unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].path, "nested/example.txt");
        assert_eq!(
            manifest.files[0].sha256,
            format!("{:x}", Sha256::digest(b"hello tls"))
        );
    }

    #[test]
    fn signs_and_verifies_manifest() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let file_path = base.join("payload.bin");
        fs::write(&file_path, b"manifest payload").unwrap();

        let manifest = generate_integrity_manifest(base, &[PathBuf::from("payload.bin")]).unwrap();
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let der = signing_key.to_pkcs8_der().unwrap();
        let signature =
            sign_manifest(&manifest, &STANDARD.encode(der.as_bytes()), "manifest-v1").unwrap();
        let signed = attach_signature(&manifest, &signature);
        let verify_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());

        verify_manifest_signature(&signed, &verify_key).unwrap();
        assert_eq!(signature.public_key_id, "manifest-v1");
        assert!(!signed.signature.is_empty());
    }
}
