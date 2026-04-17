//! 与 `backup/legacy-src/app/settings.py` 保持一致的远程配置，避免桌面端与 Python 业务分叉。

/// 卡密验证后端（与 `LICENSE_API_BASE_URLS` 顺序一致，依次回退）。
pub const LICENSE_API_BASE_URLS: &[&str] = &[
    "https://sphapi.199908.top",
    "https://sphapi.tuoling.ccwu.cc",
    "https://sphapi.tuoling.us.ci",
    "https://sphapi.tuoling.eu.cc",
];

/// 与 `LICENSE_API_TIMEOUT` 对齐（秒）。
pub const LICENSE_API_TIMEOUT_SECS: u64 = 10;
