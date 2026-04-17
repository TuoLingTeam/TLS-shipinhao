use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct RollingConfig {
    pub percentage: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct VersionManifest {
    pub app: String,
    pub version: String,
    pub build: u32,
    pub mandatory: bool,
    pub platform: String,
    pub download_url: String,
    pub tutorial_url: String,
    pub notes: Vec<String>,
    pub rolling: RollingConfig,
}

pub fn run_release_command(args: &[std::ffi::OsString]) -> Result<()> {
    let version = args
        .first()
        .and_then(|value| value.to_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let output_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("backend/dist/release"));

    fs::create_dir_all(&output_dir)
        .with_context(|| format!("创建发布目录失败：{}", output_dir.display()))?;

    let notes = vec![
        "灰度发布默认 10%，如需全量请将 rolling.percentage 调整为 100。".to_string(),
        "如需紧急回滚，可回退 version 并设置 mandatory=true。".to_string(),
    ];

    let manifest = VersionManifest {
        app: "TLS-shipinhao".into(),
        version: version.into(),
        build: parse_build_number(version),
        mandatory: false,
        platform: "mac,windows".into(),
        download_url: format!("https://example.invalid/downloads/TLS-shipinhao-{version}"),
        tutorial_url: "https://example.invalid/tutorial/update".into(),
        notes,
        rolling: RollingConfig {
            percentage: validate_percentage(10)?,
        },
    };

    let version_json = output_dir.join("version.json");
    fs::write(&version_json, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("写入 version.json 失败：{}", version_json.display()))?;

    println!("release manifest written: {}", version_json.display());
    Ok(())
}

fn parse_build_number(version: &str) -> u32 {
    version
        .split('.')
        .take(3)
        .fold(0u32, |acc, part| acc * 100 + part.parse::<u32>().unwrap_or(0))
}

pub fn validate_percentage(percentage: u8) -> Result<u8> {
    if percentage > 100 {
        return Err(anyhow!("rolling percentage must be between 0 and 100"));
    }
    Ok(percentage)
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn release_command_writes_version_manifest() {
        let dir = tempdir().unwrap();
        run_release_command(&[
            std::ffi::OsString::from("5.1.0"),
            dir.path().as_os_str().to_os_string(),
        ])
        .unwrap();
        let raw = fs::read_to_string(dir.path().join("version.json")).unwrap();
        assert!(raw.contains("rolling"));
        assert!(raw.contains("5.1.0"));
    }

    #[test]
    fn validate_percentage_rejects_values_over_100() {
        assert!(validate_percentage(10).is_ok());
        assert!(validate_percentage(100).is_ok());
    }
}
