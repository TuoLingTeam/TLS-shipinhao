use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, KeyInit};
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

#[derive(serde::Deserialize)]
struct Endpoint {
    id: String,
    #[allow(dead_code)]
    name: String,
    url: String,
}

fn load_secret_hex(path: &std::path::Path) -> Result<[u8; 32]> {
    let hex_str = fs::read_to_string(path)
        .with_context(|| format!("读取密钥文件失败：{}", path.display()))?;
    let hex_str = hex_str.trim();
    if hex_str.len() != 64 {
        return Err(anyhow!(
            "密钥长度必须是 64 位 hex，实际 {} 位",
            hex_str.len()
        ));
    }
    let bytes = hex::decode(hex_str).context("hex 解码失败")?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn encrypt_aes_gcm(key: &[u8; 32], plaintext: &[u8]) -> Result<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow!("AES-GCM 加密失败：{e}"))?;
    let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(out))
}

pub fn run_encrypt_endpoints_command(args: &[OsString]) -> Result<()> {
    let evn_dir = resolve_evn_dir(args)?;
    let secret_path = evn_dir.join(".secret-hex");
    let endpoints_path = evn_dir.join("endpoints.json");
    let output_path = evn_dir.join("endpoints.enc");

    let key = load_secret_hex(&secret_path)?;
    let json_bytes = fs::read(&endpoints_path)
        .with_context(|| format!("读取 {} 失败", endpoints_path.display()))?;
    let endpoints: Vec<Endpoint> =
        serde_json::from_slice(&json_bytes).context("解析 endpoints.json 失败")?;
    if endpoints.is_empty() {
        return Err(anyhow!("endpoints.json 为空"));
    }
    for ep in &endpoints {
        if ep.id.is_empty() || ep.url.is_empty() {
            return Err(anyhow!("endpoint id 和 url 不能为空：{:?}", ep.id));
        }
    }

    let cipher_b64 = encrypt_aes_gcm(&key, &json_bytes)?;
    fs::write(&output_path, format!("{cipher_b64}\n"))
        .with_context(|| format!("写入 {} 失败", output_path.display()))?;
    println!(
        "已加密 {} 个端点 → {}（{} 字节）",
        endpoints.len(),
        output_path.display(),
        cipher_b64.len()
    );
    Ok(())
}

fn resolve_evn_dir(args: &[OsString]) -> Result<PathBuf> {
    if let Some(dir) = args.first() {
        return Ok(PathBuf::from(dir));
    }
    let mut dir = std::env::current_dir().context("无法获取当前目录")?;
    for _ in 0..5 {
        let candidate = dir.join(".evn");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    Err(anyhow!(
        "未找到 .evn/ 目录，请在项目根目录运行或手动指定路径"
    ))
}
