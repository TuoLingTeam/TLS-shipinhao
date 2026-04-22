//! 设备指纹采集 + 归一（PRD §5.7 / M2-05）。
//!
//! 设计要点：
//! - 三平台各自调用系统命令或读取系统文件拿到"原始指纹"（通常是主板 UUID、
//!   系统序列号或 machine-id）。
//! - 采集失败时降级到 hostname + arch + os 的组合（熵低但至少稳定，
//!   避免返回空串让上游 mismatch）。
//! - 归一：`SHA256(raw)[..8]` → 16 位 hex。输出长度恒定，Worker 侧 binding
//!   不受原始指纹长度/格式波动影响。
//!
//! 注意：本模块的三平台采集都是**阻塞系统调用**，调用方若在 async 上下文中
//! 使用，应包在 `tokio::task::spawn_blocking` 里避免阻塞 reactor。

use sha2::{Digest, Sha256};
use std::process::Command;
use std::sync::OnceLock;

/// 归一后设备 ID 的固定长度（hex 字符）。
pub const DEVICE_ID_HEX_LEN: usize = 16;

/// 进程生命周期内缓存原始指纹，避免每次调用都重新执行平台命令
/// （Windows 下 wmic / powershell，macOS 下 ioreg，Linux 下 machine-id 读文件）。
/// 修复 Windows 启动后"黑窗闪现"——每次 `get_device_id()` 都 spawn wmic，
/// console subsystem 子进程会被系统分配 conhost 宿主，视觉上就是一次闪黑。
static CACHED_RAW_FINGERPRINT: OnceLock<String> = OnceLock::new();

/// 采集当前机器的原始指纹字符串。
///
/// 成功时返回硬件序列号 / UUID / machine-id；任何采集失败都会回退到
/// `fallback_fingerprint()`，保证返回非空。
///
/// 进程生命周期内只采集一次，后续调用直接命中 `OnceLock` 缓存。
pub fn collect_raw_fingerprint() -> String {
    CACHED_RAW_FINGERPRINT
        .get_or_init(|| {
            if let Some(value) = platform_specific_fingerprint() {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
            fallback_fingerprint()
        })
        .clone()
}

/// 当原生采集失败时使用的降级方案。
///
/// 组合 hostname + 架构 + OS 作为"稳定但熵低"的指纹，确保同一设备多次调用
/// 仍返回相同值。
pub fn fallback_fingerprint() -> String {
    format!(
        "{}-{}-{}",
        host_name(),
        std::env::consts::ARCH,
        std::env::consts::OS
    )
}

/// 对原始指纹做 SHA256 后取前 8 字节，转成 16 位 hex 的设备 ID。
///
/// 纯函数，相同输入恒得相同输出。
pub fn derive_device_id(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

/// 便捷函数：采集 + 归一一次完成。
pub fn get_device_id() -> String {
    derive_device_id(&collect_raw_fingerprint())
}

// ---- 平台实现 ---------------------------------------------------------------

fn platform_specific_fingerprint() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        fingerprint_macos()
    }
    #[cfg(target_os = "windows")]
    {
        fingerprint_windows()
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        fingerprint_linux()
    }
}

#[cfg(target_os = "macos")]
fn fingerprint_macos() -> Option<String> {
    // ioreg 在 App Sandbox 下可能返回 Operation not permitted；命令失败视为采集失败，走兜底。
    let output = Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains("IOPlatformSerialNumber") {
            if let Some(value) = line.split('=').nth(1) {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn fingerprint_windows() -> Option<String> {
    // 优先 wmic（快），失败或被禁用回退 PowerShell。
    // CREATE_NO_WINDOW（0x0800_0000）：wmic / powershell.exe 都是 console subsystem，
    // Tauri GUI 进程没有宿主 console，默认 spawn 时 Windows 会为子进程分配新的
    // conhost 窗口并一闪即逝；加此 flag 让子进程无窗运行，彻底消除"黑窗闪现"。
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    const CANDIDATES: &[(&str, &[&str])] = &[
        ("wmic", &["csproduct", "get", "UUID"]),
        (
            "powershell",
            &[
                "-Command",
                "(Get-CimInstance Win32_ComputerSystemProduct).UUID",
            ],
        ),
    ];
    for (program, args) in CANDIDATES {
        if let Ok(output) = Command::new(program)
            .args(args.iter().copied())
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(uuid) = first_non_header_line(&text, "UUID") {
                return Some(uuid);
            }
        }
    }
    None
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn fingerprint_linux() -> Option<String> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(raw) = std::fs::read_to_string(path) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// 取文本中第一条「非空且非表头」的行。
///
/// Windows 命令（wmic / PowerShell）通常把字段名作为第一行，我们只要值。
#[allow(dead_code)]
fn first_non_header_line(text: &str, header: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case(header) {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

fn host_name() -> String {
    // HOSTNAME（Unix）/ COMPUTERNAME（Windows）是便宜的跨平台回退。
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_produces_sixteen_lowercase_hex_chars() {
        let id = derive_device_id("test-serial-12345");
        assert_eq!(id.len(), DEVICE_ID_HEX_LEN);
        assert!(id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn derive_is_deterministic_for_same_input() {
        let a = derive_device_id("SERIAL-ABC");
        let b = derive_device_id("SERIAL-ABC");
        assert_eq!(a, b);
    }

    #[test]
    fn derive_differs_for_different_inputs() {
        assert_ne!(derive_device_id("serial-A"), derive_device_id("serial-B"),);
    }

    #[test]
    fn derive_handles_empty_string() {
        let id = derive_device_id("");
        assert_eq!(id.len(), DEVICE_ID_HEX_LEN);
    }

    #[test]
    fn fallback_fingerprint_is_non_empty() {
        let value = fallback_fingerprint();
        assert!(!value.is_empty());
        // 至少含两个 `-` 分隔符（host-arch-os）
        assert!(value.matches('-').count() >= 2);
    }

    #[test]
    fn collect_raw_never_returns_empty() {
        // 真实机器上可能成功采集，也可能走兜底；只要非空即可。
        let raw = collect_raw_fingerprint();
        assert!(!raw.is_empty());
    }

    #[test]
    fn get_device_id_length_is_stable_across_calls() {
        // 同台机器多次调用必须一致且长度 16。
        let a = get_device_id();
        let b = get_device_id();
        assert_eq!(a, b);
        assert_eq!(a.len(), DEVICE_ID_HEX_LEN);
    }

    #[test]
    fn first_non_header_line_skips_header_case_insensitively() {
        let input = "UUID\n\n  ABC-DEF-123  \n";
        assert_eq!(
            first_non_header_line(input, "UUID"),
            Some("ABC-DEF-123".to_string())
        );

        // 大小写不敏感：HEADER 行被跳过
        let input = "Uuid\nFooBar\n";
        assert_eq!(
            first_non_header_line(input, "UUID"),
            Some("FooBar".to_string())
        );
    }

    #[test]
    fn first_non_header_line_returns_none_when_only_header() {
        assert_eq!(first_non_header_line("UUID\n\n  ", "UUID"), None);
    }
}
