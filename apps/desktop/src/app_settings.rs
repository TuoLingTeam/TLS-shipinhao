//! 与 `backup/legacy-src/app/settings.py` 保持一致的远程配置，避免桌面端与 Python 业务分叉。
//!
//! 授权 API 域名属敏感数据，release 二进制中不再以明文 const 形式存放。
//! 通过 obfstr 编译期加密 + 运行时 XOR 解码，`strings` 扫不到原文。

/// 卡密验证后端（按顺序依次回退）。每次调用即时解密，避免在 static 区留下明文。
pub fn license_api_base_urls() -> Vec<String> {
    vec![
        obfstr::obfstr!("https://sphapi.199908.top").to_string(),
        obfstr::obfstr!("https://sphapi.tuoling.ccwu.cc").to_string(),
        obfstr::obfstr!("https://sphapi.tuoling.us.ci").to_string(),
        obfstr::obfstr!("https://sphapi.tuoling.eu.cc").to_string(),
    ]
}

/// 与 `LICENSE_API_TIMEOUT` 对齐（秒）。
pub const LICENSE_API_TIMEOUT_SECS: u64 = 10;
