use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;

const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 3600);
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_REMOTE_BODY: usize = 64 * 1024;
const CACHE_FILE_NAME: &str = "endpoints.enc";

static GLOBAL: OnceLock<ManagedEndpoints> = OnceLock::new();

/// 初始化全局托管端点管理器（仅在 main 中调用一次）。
pub fn init_global(fetch_url: String, cache_dir: &std::path::Path, secret_hex: &str) {
    let _ = GLOBAL.set(ManagedEndpoints::new(fetch_url, cache_dir, secret_hex));
}

/// 返回托管端点的 URL 列表；未初始化或无数据时返回空 Vec。
pub fn global_urls() -> Vec<String> {
    GLOBAL.get().map(|m| m.urls()).unwrap_or_default()
}

/// 全局 bootstrap（启动时调用）。
pub async fn global_bootstrap() {
    if let Some(m) = GLOBAL.get() {
        m.bootstrap().await;
    }
}

/// 全局 24h 定时刷新循环。
pub async fn global_run(cancel: tokio::sync::watch::Receiver<()>) {
    if let Some(m) = GLOBAL.get() {
        m.run(cancel).await;
    }
}

#[derive(Deserialize, Clone)]
pub struct Endpoint {
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    pub url: String,
}

/// 托管 License API 端点管理器。
///
/// 启动时优先读本地 .enc 缓存，后台拉远端刷新；
/// 托管不可用时 fallback 到 obfstr 硬编码默认值。
pub struct ManagedEndpoints {
    fetch_url: String,
    cache_path: PathBuf,
    key: [u8; 32],
    endpoints: RwLock<Vec<Endpoint>>,
}

impl ManagedEndpoints {
    pub fn new(fetch_url: String, cache_dir: &std::path::Path, secret_hex: &str) -> Self {
        let key = decode_secret_hex(secret_hex);
        Self {
            fetch_url,
            cache_path: cache_dir.join(CACHE_FILE_NAME),
            key,
            endpoints: RwLock::new(Vec::new()),
        }
    }

    /// 返回当前端点的 URL 列表，按地域排序。
    pub fn urls(&self) -> Vec<String> {
        let guard = self.endpoints.read().unwrap();
        let is_cn = is_likely_china();
        let mut urls: Vec<String> = guard.iter().map(|ep| ep.url.clone()).collect();
        if !is_cn && urls.len() > 1 {
            if let Some(pos) = guard.iter().position(|ep| ep.id == "global") {
                let url = urls.remove(pos);
                urls.insert(0, url);
            }
        }
        urls
    }

    pub fn has_endpoints(&self) -> bool {
        !self.endpoints.read().unwrap().is_empty()
    }

    /// 启动引导：优先加载本地缓存，后台拉取远端刷新。
    pub async fn bootstrap(&self) {
        if self.load_cache().is_ok() && self.has_endpoints() {
            let url = self.fetch_url.clone();
            let cache = self.cache_path.clone();
            let key = self.key;
            tokio::spawn(async move {
                if let Ok(data) = fetch_remote_with_timeout(&url, BOOTSTRAP_TIMEOUT).await {
                    if let Ok(endpoints) = decrypt_endpoints(&key, &data) {
                        persist_cache(&cache, &data);
                        tracing::info!(
                            target: "managed_endpoints",
                            count = endpoints.len(),
                            "后台刷新托管端点成功"
                        );
                    }
                }
            });
            return;
        }
        if let Ok(data) = fetch_remote_with_timeout(&self.fetch_url, BOOTSTRAP_TIMEOUT).await {
            if let Ok(endpoints) = decrypt_endpoints(&self.key, &data) {
                self.apply(endpoints);
                persist_cache(&self.cache_path, &data);
            }
        }
    }

    /// 后台定时刷新（24h 间隔），阻塞直到 ctx 取消。
    pub async fn run(&self, mut cancel: tokio::sync::watch::Receiver<()>) {
        let mut interval = tokio::time::interval(REFRESH_INTERVAL);
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.refresh_once().await;
                }
                _ = cancel.changed() => {
                    return;
                }
            }
        }
    }

    async fn refresh_once(&self) {
        match fetch_remote_with_timeout(&self.fetch_url, Duration::from_secs(30)).await {
            Ok(data) => match decrypt_endpoints(&self.key, &data) {
                Ok(endpoints) => {
                    self.apply(endpoints);
                    persist_cache(&self.cache_path, &data);
                }
                Err(e) => tracing::warn!(target: "managed_endpoints", "解密失败：{e}"),
            },
            Err(e) => tracing::warn!(target: "managed_endpoints", "拉取失败：{e}"),
        }
    }

    fn load_cache(&self) -> Result<(), String> {
        let raw = std::fs::read_to_string(&self.cache_path).map_err(|e| e.to_string())?;
        let cipher_b64 = raw.trim();
        if cipher_b64.is_empty() {
            return Err("缓存为空".into());
        }
        let endpoints =
            decrypt_endpoints(&self.key, cipher_b64).map_err(|e| format!("解密缓存失败：{e}"))?;
        self.apply(endpoints);
        Ok(())
    }

    fn apply(&self, endpoints: Vec<Endpoint>) {
        let mut guard = self.endpoints.write().unwrap();
        *guard = endpoints;
    }
}

fn decode_secret_hex(hex_str: &str) -> [u8; 32] {
    let hex_str = hex_str.trim();
    let bytes = hex::decode(hex_str).expect("密钥 hex 解码失败");
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    key
}

fn decrypt_endpoints(key: &[u8; 32], cipher_b64: &str) -> Result<Vec<Endpoint>, String> {
    let raw = STANDARD
        .decode(cipher_b64.trim())
        .map_err(|e| format!("base64 解码失败：{e}"))?;
    if raw.len() < 12 {
        return Err("密文太短".into());
    }
    let (nonce_bytes, ciphertext) = raw.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("AES-GCM 解密失败：{e}"))?;
    let endpoints: Vec<Endpoint> =
        serde_json::from_slice(&plaintext).map_err(|e| format!("JSON 解析失败：{e}"))?;
    if endpoints.is_empty() {
        return Err("端点列表为空".into());
    }
    Ok(endpoints)
}

async fn fetch_remote_with_timeout(url: &str, timeout: Duration) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .header("User-Agent", "tls-shipinhao/managed-endpoints")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if body.len() > MAX_REMOTE_BODY {
        return Err("响应体过大".into());
    }
    let payload = body
        .lines()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .unwrap_or("")
        .trim()
        .to_string();
    if payload.is_empty() {
        return Err("响应无有效载荷".into());
    }
    Ok(payload)
}

fn persist_cache(path: &std::path::Path, cipher_b64: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(path, format!("{cipher_b64}\n")) {
        tracing::warn!(target: "managed_endpoints", "写入缓存失败：{e}");
    }
}

fn is_likely_china() -> bool {
    chrono::Local::now().offset().local_minus_utc() == 8 * 3600
}
