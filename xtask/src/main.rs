use anyhow::{anyhow, Context, Result};
use build_tools::{attach_signature, generate_integrity_manifest, inject_version, sign_manifest};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    run(args)
}

fn run(args: Vec<OsString>) -> Result<()> {
    let command = args
        .first()
        .and_then(|value| value.to_str())
        .unwrap_or("release");
    match command {
        "release" => run_release_command(&args[1..]),
        "manifest" => run_manifest_command(&args[1..]),
        "desktop-build" => run_desktop_build_command(&args[1..]),
        other => Err(anyhow!("unknown command: {other}")),
    }
}

fn release_note_for_version(version: &str) -> Result<String> {
    inject_version("release-__APP_VERSION__", version)
}

fn run_release_command(args: &[OsString]) -> Result<()> {
    let version = args
        .first()
        .and_then(|value| value.to_str())
        .unwrap_or("dev");
    let release_note = release_note_for_version(version)?;
    println!("preparing {release_note}");
    run_desktop_build_command(&[])?;
    Ok(())
}

fn run_manifest_command(args: &[OsString]) -> Result<()> {
    if args.len() < 3 {
        return Err(anyhow!(
            "usage: cargo run -p xtask -- manifest <base_dir> <output_json> <file> [file...]"
        ));
    }
    let base_dir = PathBuf::from(&args[0]);
    let output_path = PathBuf::from(&args[1]);
    let files = args[2..].iter().map(PathBuf::from).collect::<Vec<_>>();
    let manifest = generate_integrity_manifest(&base_dir, &files)?;
    let signed_manifest = if let Ok(signing_key) = std::env::var("INTEGRITY_MANIFEST_PRIVATE_KEY_B64") {
        let key_id = std::env::var("INTEGRITY_MANIFEST_KEY_ID").unwrap_or_else(|_| "manifest-dev".into());
        let signature = sign_manifest(&manifest, &signing_key, &key_id)?;
        attach_signature(&manifest, &signature)
    } else {
        manifest
    };
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, serde_json::to_vec_pretty(&signed_manifest)?)?;
    println!("manifest written: {}", output_path.display());
    Ok(())
}

fn run_desktop_build_command(_args: &[OsString]) -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "-p", "desktop-app"])
        .status()
        .context("spawn cargo build -p desktop-app")?;
    if !status.success() {
        return Err(anyhow!("desktop-build failed with status {status}"));
    }
    println!("desktop-build ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use ed25519_dalek::SigningKey;
    use tempfile::tempdir;

    #[test]
    fn injects_version_in_release_command() {
        let result = release_note_for_version("4.3.0").unwrap();
        assert_eq!(result, "release-4.3.0");
    }

    #[test]
    fn manifest_command_writes_json_file() {
        let dir = tempdir().unwrap();
        let base_dir = dir.path().join("bundle");
        fs::create_dir_all(&base_dir).unwrap();
        fs::write(base_dir.join("app.bin"), b"payload").unwrap();
        let output = dir.path().join("out/manifest.json");
        let result = run_manifest_command(&[
            base_dir.as_os_str().to_os_string(),
            output.as_os_str().to_os_string(),
            OsString::from("app.bin"),
        ]);
        assert!(result.is_ok());
        let raw = fs::read_to_string(output).unwrap();
        assert!(raw.contains("app.bin"));
        assert!(raw.contains("generated_at"));
    }

    #[test]
    fn manifest_command_signs_when_key_present() {
        let dir = tempdir().unwrap();
        let base_dir = dir.path().join("bundle");
        fs::create_dir_all(&base_dir).unwrap();
        fs::write(base_dir.join("app.bin"), b"payload").unwrap();
        let output = dir.path().join("out/manifest.json");
        let signing_key = SigningKey::from_bytes(&[9u8; 32]);
        let der = signing_key.to_pkcs8_der().unwrap();
        unsafe {
            std::env::set_var("INTEGRITY_MANIFEST_PRIVATE_KEY_B64", STANDARD.encode(der.as_bytes()));
            std::env::set_var("INTEGRITY_MANIFEST_KEY_ID", "manifest-v1");
        }
        let result = run_manifest_command(&[
            base_dir.as_os_str().to_os_string(),
            output.as_os_str().to_os_string(),
            OsString::from("app.bin"),
        ]);
        unsafe {
            std::env::remove_var("INTEGRITY_MANIFEST_PRIVATE_KEY_B64");
            std::env::remove_var("INTEGRITY_MANIFEST_KEY_ID");
        }
        assert!(result.is_ok());
        let raw = fs::read_to_string(output).unwrap();
        assert!(raw.contains("signature"));
    }

    #[test]
    fn desktop_build_command_rejects_unknown_subcommand() {
        let result = run(vec![OsString::from("unknown")]);
        assert!(result.is_err());
    }
}
