use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

static BACKEND_NAME: &[u8] = b"rust-security-core\0";

#[derive(Serialize)]
struct CanonicalManifestFile<'a> {
    path: &'a str,
    sha256: &'a str,
}

#[derive(Serialize)]
struct CanonicalManifest<'a> {
    files: Vec<CanonicalManifestFile<'a>>,
    generated_at: &'a str,
    version: u64,
}

fn into_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .unwrap_or_else(|_| CString::new("{}").unwrap())
        .into_raw()
}

fn opt_str_from_ptr(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let raw = unsafe { CStr::from_ptr(ptr) };
    Some(raw.to_string_lossy().into_owned())
}

fn response_json(value: Value) -> *mut c_char {
    into_c_string(value.to_string())
}

fn decode_b64url(value: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .map_err(|err| format!("base64url decode failed: {err}"))
}

fn verifying_key_from_b64url(public_key_b64url: &str) -> Result<VerifyingKey, String> {
    let bytes = decode_b64url(public_key_b64url)?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "invalid public key length".to_string())?;
    VerifyingKey::from_bytes(&key_bytes).map_err(|err| format!("invalid public key: {err}"))
}

fn current_device_fingerprint() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("IOPlatformSerialNumber") {
                    let parts: Vec<&str> = line.split('=').collect();
                    if let Some(last) = parts.last() {
                        return Some(last.trim().trim_matches('"').to_string());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let commands = [
            ("wmic", vec!["csproduct", "get", "UUID"]),
            (
                "powershell",
                vec![
                    "-Command",
                    "(Get-CimInstance Win32_ComputerSystemProduct).UUID",
                ],
            ),
        ];
        for (program, args) in commands {
            if let Ok(output) = Command::new(program).args(args).output() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && trimmed.to_uppercase() != "UUID" {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
            if let Ok(raw) = fs::read_to_string(path) {
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    Some(format!(
        "{}-{}-{}",
        std::env::var("HOSTNAME").unwrap_or_default(),
        std::env::consts::ARCH,
        std::env::consts::OS
    ))
}

fn derive_device_id(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn verify_lease_impl(
    token: &str,
    public_key_b64url: &str,
    expected_device_id: Option<&str>,
    current_epoch_seconds: i64,
    allow_expired: bool,
) -> Value {
    let mut parts = token.splitn(2, '.');
    let encoded_payload = match parts.next() {
        Some(value) if !value.is_empty() => value,
        _ => return json!({"ok": false, "reason": "invalid", "payload": null}),
    };
    let encoded_signature = match parts.next() {
        Some(value) if !value.is_empty() => value,
        _ => return json!({"ok": false, "reason": "invalid", "payload": null}),
    };

    let public_key = match verifying_key_from_b64url(public_key_b64url) {
        Ok(value) => value,
        Err(err) => return json!({"ok": false, "reason": "invalid", "error": err, "payload": null}),
    };
    let signature_bytes = match decode_b64url(encoded_signature) {
        Ok(value) => value,
        Err(err) => return json!({"ok": false, "reason": "invalid", "error": err, "payload": null}),
    };
    let signature = match Signature::from_slice(&signature_bytes) {
        Ok(value) => value,
        Err(err) => {
            return json!({"ok": false, "reason": "invalid", "error": err.to_string(), "payload": null})
        }
    };

    if let Err(err) = public_key.verify(encoded_payload.as_bytes(), &signature) {
        return json!({"ok": false, "reason": "invalid", "error": err.to_string(), "payload": null});
    }

    let payload_bytes = match decode_b64url(encoded_payload) {
        Ok(value) => value,
        Err(err) => return json!({"ok": false, "reason": "invalid", "error": err, "payload": null}),
    };
    let payload: Value = match serde_json::from_slice(&payload_bytes) {
        Ok(value) => value,
        Err(err) => {
            return json!({"ok": false, "reason": "invalid", "error": err.to_string(), "payload": null})
        }
    };

    if payload.get("kind").and_then(Value::as_str) != Some("license_lease") {
        return json!({"ok": false, "reason": "invalid", "payload": null});
    }

    if let Some(expected) = expected_device_id.filter(|value| !value.is_empty()) {
        if payload.get("device_id").and_then(Value::as_str) != Some(expected) {
            return json!({"ok": false, "reason": "device_mismatch", "payload": payload});
        }
    }

    if !allow_expired {
        let exp = payload
            .get("exp")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if exp <= 0 || current_epoch_seconds >= exp {
            return json!({"ok": false, "reason": "expired", "payload": payload});
        }
    }

    json!({"ok": true, "reason": "ok", "payload": payload})
}

fn canonical_manifest_bytes(payload: &Value) -> Result<Vec<u8>, String> {
    let files = payload
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest files missing".to_string())?;
    let mut normalized_files = Vec::with_capacity(files.len());
    for file in files {
        let path = file
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "manifest path missing".to_string())?;
        let sha256 = file
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| "manifest sha256 missing".to_string())?;
        normalized_files.push(CanonicalManifestFile { path, sha256 });
    }
    serde_json::to_vec(&CanonicalManifest {
        files: normalized_files,
        generated_at: payload
            .get("generated_at")
            .and_then(Value::as_str)
            .unwrap_or(""),
        version: payload.get("version").and_then(Value::as_u64).unwrap_or(1),
    })
    .map_err(|err| err.to_string())
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn verify_integrity_manifest_impl(manifest_path: &Path, public_key_b64url: &str) -> Value {
    let raw = match fs::read_to_string(manifest_path) {
        Ok(value) => value,
        Err(err) => {
            return json!({"status": "compromised", "message": format!("manifest read error: {err}")})
        }
    };
    let payload: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(err) => {
            return json!({"status": "compromised", "message": format!("manifest parse error: {err}")})
        }
    };
    let signature = match payload.get("signature").and_then(Value::as_str) {
        Some(value) if !value.is_empty() => value,
        _ => return json!({"status": "compromised", "message": "manifest signature missing"}),
    };
    let public_key = match verifying_key_from_b64url(public_key_b64url) {
        Ok(value) => value,
        Err(err) => return json!({"status": "compromised", "message": err}),
    };
    let signature_bytes = match decode_b64url(signature) {
        Ok(value) => value,
        Err(err) => return json!({"status": "compromised", "message": err}),
    };
    let signature = match Signature::from_slice(&signature_bytes) {
        Ok(value) => value,
        Err(err) => return json!({"status": "compromised", "message": err.to_string()}),
    };
    let canonical_bytes = match canonical_manifest_bytes(&payload) {
        Ok(value) => value,
        Err(err) => return json!({"status": "compromised", "message": err}),
    };
    if let Err(err) = public_key.verify(&canonical_bytes, &signature) {
        return json!({"status": "compromised", "message": format!("manifest signature invalid: {err}")});
    }

    let base_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    for file in payload
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let rel = match file.get("path").and_then(Value::as_str) {
            Some(value) if !value.is_empty() => value,
            _ => return json!({"status": "compromised", "message": "manifest entry invalid"}),
        };
        let expected = match file.get("sha256").and_then(Value::as_str) {
            Some(value) if !value.is_empty() => value,
            _ => return json!({"status": "compromised", "message": "manifest entry invalid"}),
        };
        let target = base_dir.join(rel);
        let actual = match sha256_hex(&target) {
            Ok(value) => value,
            Err(err) => {
                return json!({"status": "compromised", "message": format!("integrity error: {rel}: {err}")})
            }
        };
        if actual != expected {
            return json!({"status": "compromised", "message": format!("integrity mismatch: {rel}")});
        }
    }
    json!({"status": "ok", "message": "integrity ok"})
}

#[no_mangle]
pub extern "C" fn security_core_backend_name() -> *const c_char {
    BACKEND_NAME.as_ptr() as *const c_char
}

#[no_mangle]
pub extern "C" fn security_core_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[no_mangle]
pub extern "C" fn security_core_collect_device_id() -> *mut c_char {
    let value = current_device_fingerprint()
        .map(|raw| derive_device_id(&raw))
        .unwrap_or_default();
    into_c_string(value)
}

#[no_mangle]
pub extern "C" fn security_core_verify_lease(
    token: *const c_char,
    public_key_b64url: *const c_char,
    expected_device_id: *const c_char,
    current_epoch_seconds: i64,
    allow_expired: i32,
) -> *mut c_char {
    let token = opt_str_from_ptr(token).unwrap_or_default();
    let public_key_b64url = opt_str_from_ptr(public_key_b64url).unwrap_or_default();
    let expected_device_id = opt_str_from_ptr(expected_device_id);
    response_json(verify_lease_impl(
        &token,
        &public_key_b64url,
        expected_device_id.as_deref(),
        current_epoch_seconds,
        allow_expired != 0,
    ))
}

#[no_mangle]
pub extern "C" fn security_core_verify_integrity_manifest(
    manifest_path: *const c_char,
    public_key_b64url: *const c_char,
) -> *mut c_char {
    let manifest_path = opt_str_from_ptr(manifest_path).unwrap_or_default();
    let public_key_b64url = opt_str_from_ptr(public_key_b64url).unwrap_or_default();
    let path = PathBuf::from(manifest_path);
    response_json(verify_integrity_manifest_impl(&path, &public_key_b64url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use serde_json::json;
    use std::io::Write;

    fn make_keypair() -> (SigningKey, VerifyingKey) {
        let sk = SigningKey::generate(&mut OsRng);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    fn b64url(bytes: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn sign_payload(sk: &SigningKey, payload: &Value) -> String {
        use ed25519_dalek::Signer;
        let payload_bytes = serde_json::to_vec(payload).unwrap();
        let encoded_payload = b64url(&payload_bytes);
        let sig = sk.sign(encoded_payload.as_bytes());
        let encoded_sig = b64url(&sig.to_bytes());
        format!("{encoded_payload}.{encoded_sig}")
    }

    fn lease_payload(device_id: &str, exp: i64) -> Value {
        json!({
            "kind": "license_lease",
            "device_id": device_id,
            "exp": exp
        })
    }

    // --- derive_device_id ---

    #[test]
    fn derive_device_id_is_deterministic_and_16_hex_chars() {
        let id = derive_device_id("test-serial-12345");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id, derive_device_id("test-serial-12345"));
    }

    #[test]
    fn derive_device_id_differs_for_different_input() {
        assert_ne!(derive_device_id("serial-A"), derive_device_id("serial-B"));
    }

    // --- verify_lease_impl ---

    #[test]
    fn verify_lease_valid_token() {
        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());
        let payload = lease_payload("dev-1", i64::MAX);
        let token = sign_payload(&sk, &payload);

        let result = verify_lease_impl(&token, &pk_b64, Some("dev-1"), 1000, false);
        assert_eq!(result["ok"], true);
        assert_eq!(result["reason"], "ok");
        assert_eq!(result["payload"]["device_id"], "dev-1");
    }

    #[test]
    fn verify_lease_rejects_expired_token() {
        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());
        let payload = lease_payload("dev-1", 500);
        let token = sign_payload(&sk, &payload);

        let result = verify_lease_impl(&token, &pk_b64, Some("dev-1"), 1000, false);
        assert_eq!(result["ok"], false);
        assert_eq!(result["reason"], "expired");
    }

    #[test]
    fn verify_lease_allows_expired_when_flag_set() {
        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());
        let payload = lease_payload("dev-1", 500);
        let token = sign_payload(&sk, &payload);

        let result = verify_lease_impl(&token, &pk_b64, Some("dev-1"), 1000, true);
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn verify_lease_rejects_device_mismatch() {
        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());
        let payload = lease_payload("dev-1", i64::MAX);
        let token = sign_payload(&sk, &payload);

        let result = verify_lease_impl(&token, &pk_b64, Some("dev-OTHER"), 1000, false);
        assert_eq!(result["ok"], false);
        assert_eq!(result["reason"], "device_mismatch");
    }

    #[test]
    fn verify_lease_rejects_tampered_signature() {
        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());
        let payload = lease_payload("dev-1", i64::MAX);
        let token = sign_payload(&sk, &payload);
        let tampered = format!("{}X", &token[..token.len() - 1]);

        let result = verify_lease_impl(&tampered, &pk_b64, None, 1000, false);
        assert_eq!(result["ok"], false);
        assert_eq!(result["reason"], "invalid");
    }

    #[test]
    fn verify_lease_rejects_wrong_kind() {
        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());
        let payload = json!({"kind": "other", "device_id": "d", "exp": i64::MAX});
        let token = sign_payload(&sk, &payload);

        let result = verify_lease_impl(&token, &pk_b64, None, 1000, false);
        assert_eq!(result["ok"], false);
        assert_eq!(result["reason"], "invalid");
    }

    #[test]
    fn verify_lease_rejects_empty_token() {
        let result = verify_lease_impl("", "abc", None, 0, false);
        assert_eq!(result["ok"], false);
    }

    #[test]
    fn verify_lease_rejects_token_without_dot() {
        let result = verify_lease_impl("nodot", "abc", None, 0, false);
        assert_eq!(result["ok"], false);
    }

    // --- canonical_manifest_bytes ---

    #[test]
    fn canonical_manifest_bytes_preserves_sorted_fields() {
        let manifest = json!({
            "version": 1,
            "generated_at": "2026-04-16T00:00:00Z",
            "files": [
                {"path": "a.txt", "sha256": "aaa"},
                {"path": "b.txt", "sha256": "bbb"}
            ],
            "signature": "ignored"
        });
        let bytes = canonical_manifest_bytes(&manifest).unwrap();
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["files"][0]["path"], "a.txt");
        assert_eq!(parsed["files"][1]["path"], "b.txt");
    }

    #[test]
    fn canonical_manifest_bytes_rejects_missing_files() {
        let manifest = json!({"version": 1, "generated_at": "x"});
        assert!(canonical_manifest_bytes(&manifest).is_err());
    }

    // --- verify_integrity_manifest_impl ---

    #[test]
    fn verify_integrity_manifest_with_valid_signed_manifest() {
        use ed25519_dalek::Signer;

        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("app.bin");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello world").unwrap();
        drop(f);

        let file_hash = sha256_hex(&file_path).unwrap();

        let manifest_no_sig = json!({
            "version": 1,
            "generated_at": "2026-04-16T00:00:00Z",
            "files": [{"path": "app.bin", "sha256": file_hash}]
        });
        let canonical = canonical_manifest_bytes(&manifest_no_sig).unwrap();
        let sig = sk.sign(&canonical);
        let sig_b64 = b64url(&sig.to_bytes());

        let manifest_with_sig = json!({
            "version": 1,
            "generated_at": "2026-04-16T00:00:00Z",
            "files": [{"path": "app.bin", "sha256": file_hash}],
            "signature": sig_b64
        });

        let manifest_path = dir.path().join("manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest_with_sig).unwrap(),
        )
        .unwrap();

        let result = verify_integrity_manifest_impl(&manifest_path, &pk_b64);
        assert_eq!(result["status"], "ok");
    }

    #[test]
    fn verify_integrity_manifest_detects_tampered_file() {
        use ed25519_dalek::Signer;

        let (sk, vk) = make_keypair();
        let pk_b64 = b64url(vk.as_bytes());

        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("app.bin");
        std::fs::write(&file_path, b"hello world").unwrap();
        let file_hash = sha256_hex(&file_path).unwrap();

        let manifest_no_sig = json!({
            "version": 1,
            "generated_at": "2026-04-16T00:00:00Z",
            "files": [{"path": "app.bin", "sha256": file_hash}]
        });
        let canonical = canonical_manifest_bytes(&manifest_no_sig).unwrap();
        let sig = sk.sign(&canonical);
        let sig_b64 = b64url(&sig.to_bytes());

        let manifest_with_sig = json!({
            "version": 1,
            "generated_at": "2026-04-16T00:00:00Z",
            "files": [{"path": "app.bin", "sha256": file_hash}],
            "signature": sig_b64
        });
        let manifest_path = dir.path().join("manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest_with_sig).unwrap(),
        )
        .unwrap();

        // 篡改文件内容
        std::fs::write(&file_path, b"tampered content").unwrap();

        let result = verify_integrity_manifest_impl(&manifest_path, &pk_b64);
        assert_eq!(result["status"], "compromised");
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("integrity mismatch"));
    }

    #[test]
    fn verify_integrity_manifest_rejects_missing_manifest() {
        let result = verify_integrity_manifest_impl(Path::new("/nonexistent/manifest.json"), "abc");
        assert_eq!(result["status"], "compromised");
    }
}
