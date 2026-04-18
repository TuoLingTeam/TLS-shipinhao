//! 平台化 HTTP 请求头常量。
//!
//! 与 Python 4.3.0 原版保持一致：依据运行时平台自动选择 `User-Agent` 与
//! `sec-ch-ua-platform`，避免与宿主系统不一致而被平台风控识别为异常客户端。
//!
//! Chrome 版本跟随原版锁定在 144.0.0.0；如需跟随 Chrome 大版本升级，
//! 只需在本文件集中更新一次即可传导到所有 HTTP adapter。

/// Windows 桌面端使用的 Chrome 144 User-Agent。
pub const WINDOWS_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

/// macOS 桌面端使用的 Chrome 144 User-Agent。
pub const MACOS_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

/// Linux 无桌面端发布，回退使用 Windows UA 以规避平台端对「未知 UA」的额外拦截。
pub const LINUX_FALLBACK_UA: &str = WINDOWS_UA;

/// `sec-ch-ua-platform` header 要求带双引号包裹的平台名。
pub const PLATFORM_WINDOWS: &str = "\"Windows\"";
pub const PLATFORM_MACOS: &str = "\"macOS\"";
pub const PLATFORM_LINUX: &str = "\"Linux\"";

/// 返回当前平台对应的 `User-Agent`。
///
/// 使用 `#[cfg]` 编译期分支，保证单一二进制不会误携带非本平台 UA。
pub fn get_user_agent() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        MACOS_UA
    }
    #[cfg(target_os = "windows")]
    {
        WINDOWS_UA
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        LINUX_FALLBACK_UA
    }
}

/// 返回当前平台对应的 `sec-ch-ua-platform`（含双引号）。
pub fn get_sec_ch_ua_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        PLATFORM_MACOS
    }
    #[cfg(target_os = "windows")]
    {
        PLATFORM_WINDOWS
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PLATFORM_LINUX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_is_chrome_144_with_current_platform_marker() {
        let ua = get_user_agent();
        assert!(
            ua.contains("Chrome/144.0.0.0"),
            "UA 未锁定 Chrome 版本: {ua}"
        );
        assert!(
            ua.starts_with("Mozilla/5.0 "),
            "UA 必须以 Mozilla 起头: {ua}"
        );

        #[cfg(target_os = "macos")]
        assert!(
            ua.contains("Macintosh"),
            "macOS 平台 UA 必须含 Macintosh: {ua}"
        );
        #[cfg(target_os = "windows")]
        assert!(
            ua.contains("Windows NT 10.0"),
            "Windows 平台 UA 必须含 Windows NT 10.0: {ua}"
        );
    }

    #[test]
    fn sec_ch_ua_platform_is_quoted_platform_name() {
        let value = get_sec_ch_ua_platform();
        assert!(value.starts_with('"') && value.ends_with('"'));

        #[cfg(target_os = "macos")]
        assert_eq!(value, "\"macOS\"");
        #[cfg(target_os = "windows")]
        assert_eq!(value, "\"Windows\"");
    }

    #[test]
    fn constants_match_python_source_of_truth() {
        assert_eq!(
            MACOS_UA,
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
        );
        assert_eq!(
            WINDOWS_UA,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
        );
        assert_eq!(PLATFORM_MACOS, "\"macOS\"");
        assert_eq!(PLATFORM_WINDOWS, "\"Windows\"");
    }
}
