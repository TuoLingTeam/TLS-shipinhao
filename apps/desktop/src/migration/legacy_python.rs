//! 从 Python 4.3.0 旧版目录迁移数据到新版 Rust/Tauri 目录。
//!
//! 迁移对象：
//! - 订单缓存三件套：`order_cache.sqlite3` / `-wal` / `-shm`
//! - Cookie：`cookie.txt`
//! - 授权：`license.json`（本版本仅做备份 + 标记，真实换取新 Lease 依赖 M2-11b）
//! - 配置指针：`selected_config_dir.txt`
//!
//! 设计要点：
//! - 旧目录不存在 → 直接返回空 `MigrationReport`
//! - 新目录已有同名文件 → **不动**（避免覆盖用户已在使用的数据）
//! - 单步失败不致命：写入 `errors` 后继续，上层选择如何告知用户
//! - 备份目录 `<backup_root>/<yyyy-mm-dd>/` 先写入副本再迁移，确保失败可回滚

use crate::state;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const LEGACY_DIR_NAME: &str = ".tls-shipinhao";
pub const BACKUP_DIR_NAME: &str = "legacy_backup";

/// 订单缓存主文件（SQLite 的 WAL/SHM 辅助文件在 open 时自动生成，迁移主文件即可）。
const ORDER_CACHE_FILES: &[&str] = &[
    "order_cache.sqlite3",
    "order_cache.sqlite3-wal",
    "order_cache.sqlite3-shm",
];
const RUNTIME_HOME_FILES: &[(&str, &str)] = &[
    ("cookie.txt", "migrate_cookie"),
    ("license.json", "migrate_license"),
    ("selected_config_dir.txt", "migrate_config_pointer"),
];

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct MigrationError {
    pub step: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct MigrationReport {
    pub legacy_detected: bool,
    pub cache_migrated: bool,
    pub cookie_migrated: bool,
    pub license_migrated: bool,
    pub config_pointer_migrated: bool,
    pub backup_dir: Option<String>,
    pub errors: Vec<MigrationError>,
}

impl MigrationReport {
    pub fn push_error(&mut self, step: impl Into<String>, message: impl ToString) {
        self.errors.push(MigrationError {
            step: step.into(),
            message: message.to_string(),
        });
    }
}

/// 迁移涉及的三组路径。生产环境通过 `default_platform` 解析，
/// 单元测试用 `with_roots` 注入临时目录。
#[derive(Debug, Clone)]
pub struct MigrationPaths {
    /// Python 旧版遗留目录：主要作为历史订单缓存来源。
    pub legacy_root: PathBuf,
    /// 当前桌面端真正读取 Cookie / 授权 / 店铺注册表的目录。
    pub runtime_root: PathBuf,
    /// 历史错误迁移落点：旧版本曾把 Cookie / 授权错误写到这里。
    pub misplaced_runtime_root: PathBuf,
    /// 当前全局订单缓存目录（无 active store 时的 fallback）。
    pub cache_root: PathBuf,
    pub backup_root: PathBuf,
}

impl MigrationPaths {
    /// 生产默认路径：
    /// - 订单缓存：`~/.tls-shipinhao/order_cache.sqlite3` → `$LOCALAPPDATA/TLS-shipinhao`
    /// - Cookie / 授权 / 配置指针：若历史版本误落到 `$LOCALAPPDATA/TLS-shipinhao`，
    ///   则回迁到当前运行时目录 `~/.tls-shipinhao`
    pub fn default_platform() -> anyhow::Result<Self> {
        let home = dirs::home_dir().context("无法解析用户 home 目录")?;
        let legacy_root = home.join(LEGACY_DIR_NAME);
        let data_local = dirs::data_local_dir().context("无法解析 data_local_dir")?;
        let runtime_root = state::app_home_dir();
        let cache_root = data_local.join("TLS-shipinhao");
        let backup_root = runtime_root.join(BACKUP_DIR_NAME);
        Ok(Self {
            legacy_root,
            runtime_root,
            misplaced_runtime_root: cache_root.clone(),
            cache_root,
            backup_root,
        })
    }

    #[cfg(test)]
    pub fn with_roots(
        legacy_root: PathBuf,
        runtime_root: PathBuf,
        misplaced_runtime_root: PathBuf,
        cache_root: PathBuf,
        backup_root: PathBuf,
    ) -> Self {
        Self {
            legacy_root,
            runtime_root,
            misplaced_runtime_root,
            cache_root,
            backup_root,
        }
    }
}

pub struct LegacyPythonMigrator {
    paths: MigrationPaths,
    today: String,
}

impl LegacyPythonMigrator {
    pub fn new(paths: MigrationPaths) -> Self {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        Self { paths, today }
    }

    /// 测试用入口：固定"今天"的字符串避免时区/时间敏感。
    #[cfg(test)]
    pub fn with_today(paths: MigrationPaths, today: impl Into<String>) -> Self {
        Self {
            paths,
            today: today.into(),
        }
    }

    pub fn run(&self) -> MigrationReport {
        let mut report = MigrationReport::default();
        let cache_needs_migration = self.resolve_order_cache_source_root().is_some();
        let runtime_files_need_migration = RUNTIME_HOME_FILES
            .iter()
            .any(|(file_name, _)| self.runtime_file_source(file_name).is_some());
        if !cache_needs_migration && !runtime_files_need_migration {
            return report;
        }
        report.legacy_detected = true;

        let backup_dir = self.paths.backup_root.join(&self.today);
        if let Err(err) = std::fs::create_dir_all(&backup_dir) {
            report.push_error("setup_backup_dir", err);
            return report;
        }
        report.backup_dir = Some(backup_dir.to_string_lossy().into_owned());

        if let Err(err) = std::fs::create_dir_all(&self.paths.runtime_root) {
            report.push_error("setup_runtime_root", err);
            return report;
        }
        if let Err(err) = std::fs::create_dir_all(&self.paths.cache_root) {
            report.push_error("setup_cache_root", err);
            return report;
        }

        report.cache_migrated = self.migrate_order_cache(&backup_dir, &mut report);
        report.cookie_migrated = self.migrate_runtime_home_file(
            "cookie.txt",
            &backup_dir,
            &mut report,
            "migrate_cookie",
        );
        report.license_migrated = self.migrate_runtime_home_file(
            "license.json",
            &backup_dir,
            &mut report,
            "migrate_license",
        );
        report.config_pointer_migrated = self.migrate_runtime_home_file(
            "selected_config_dir.txt",
            &backup_dir,
            &mut report,
            "migrate_config_pointer",
        );

        report
    }

    fn resolve_order_cache_source_root(&self) -> Option<PathBuf> {
        let destination = self.paths.cache_root.join(ORDER_CACHE_FILES[0]);
        [
            self.paths.legacy_root.clone(),
            self.paths.misplaced_runtime_root.clone(),
        ]
        .into_iter()
        .find(|root| {
            let source = root.join(ORDER_CACHE_FILES[0]);
            source.exists() && source != destination
        })
    }

    fn runtime_file_source(&self, file_name: &str) -> Option<PathBuf> {
        let destination = self.paths.runtime_root.join(file_name);
        if destination.exists() {
            return None;
        }

        [
            self.paths.misplaced_runtime_root.clone(),
            self.paths.legacy_root.clone(),
        ]
        .into_iter()
        .map(|root| root.join(file_name))
        .find(|source| source.exists() && *source != destination)
    }

    /// 返回 true 当且仅当主 `.sqlite3` 文件成功迁移（-wal / -shm 为可选附件）。
    fn migrate_order_cache(&self, backup_dir: &Path, report: &mut MigrationReport) -> bool {
        let Some(source_root) = self.resolve_order_cache_source_root() else {
            return false;
        };
        let mut main_file_migrated = false;
        for file_name in ORDER_CACHE_FILES {
            let outcome = copy_with_backup(
                &source_root.join(file_name),
                &self.paths.cache_root.join(file_name),
                &backup_dir.join(file_name),
            );
            match outcome {
                FileOutcome::Migrated => {
                    if *file_name == ORDER_CACHE_FILES[0] {
                        main_file_migrated = true;
                    }
                }
                FileOutcome::DestinationExists => {
                    // 新库已有同名文件 → 保守跳过，日志记到 errors 便于 UI 展示
                    if *file_name == ORDER_CACHE_FILES[0] {
                        report.push_error(
                            "migrate_order_cache",
                            format!("目标文件已存在，跳过：{file_name}"),
                        );
                    }
                }
                FileOutcome::SourceMissing => {}
                FileOutcome::Failure { step, error } => {
                    let full_step = format!("migrate_order_cache:{step}:{file_name}");
                    report.push_error(full_step, error);
                }
            }
        }
        main_file_migrated
    }

    fn migrate_runtime_home_file(
        &self,
        file_name: &str,
        backup_dir: &Path,
        report: &mut MigrationReport,
        step_prefix: &str,
    ) -> bool {
        let Some(source_path) = self.runtime_file_source(file_name) else {
            return false;
        };
        match copy_with_backup(
            &source_path,
            &self.paths.runtime_root.join(file_name),
            &backup_dir.join(file_name),
        ) {
            FileOutcome::Migrated => true,
            FileOutcome::DestinationExists => {
                report.push_error(step_prefix, format!("目标文件已存在，跳过：{file_name}"));
                false
            }
            FileOutcome::SourceMissing => false,
            FileOutcome::Failure { step, error } => {
                report.push_error(format!("{step_prefix}:{step}"), error);
                false
            }
        }
    }
}

enum FileOutcome {
    Migrated,
    DestinationExists,
    SourceMissing,
    Failure { step: String, error: String },
}

/// 先备份后迁移：任一步失败都能保证用户原始数据可恢复。
fn copy_with_backup(src: &Path, dest: &Path, backup: &Path) -> FileOutcome {
    if !src.exists() {
        return FileOutcome::SourceMissing;
    }
    if dest.exists() {
        return FileOutcome::DestinationExists;
    }
    if let Err(err) = ensure_parent(backup) {
        return FileOutcome::Failure {
            step: "prepare_backup_dir".into(),
            error: err.to_string(),
        };
    }
    if let Err(err) = std::fs::copy(src, backup) {
        return FileOutcome::Failure {
            step: "backup".into(),
            error: err.to_string(),
        };
    }
    if let Err(err) = ensure_parent(dest) {
        return FileOutcome::Failure {
            step: "prepare_dest_dir".into(),
            error: err.to_string(),
        };
    }
    if let Err(err) = std::fs::copy(src, dest) {
        return FileOutcome::Failure {
            step: "copy".into(),
            error: err.to_string(),
        };
    }
    FileOutcome::Migrated
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_paths(root: &Path) -> MigrationPaths {
        let legacy = root.join("legacy");
        let runtime_root = root.join("runtime");
        let misplaced_runtime_root = root.join("misplaced-runtime");
        let cache_root = root.join("cache");
        let backup = runtime_root.join("legacy_backup");
        MigrationPaths::with_roots(
            legacy,
            runtime_root,
            misplaced_runtime_root,
            cache_root,
            backup,
        )
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn returns_empty_report_when_legacy_root_missing() {
        let tmp = tempdir().unwrap();
        let paths = setup_paths(tmp.path());
        let report = LegacyPythonMigrator::with_today(paths, "2026-04-17").run();
        assert!(!report.legacy_detected);
        assert!(!report.cache_migrated);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn migrates_all_files_when_new_root_empty() {
        let tmp = tempdir().unwrap();
        let paths = setup_paths(tmp.path());
        write(
            &paths.legacy_root.join("order_cache.sqlite3"),
            "sqlite-main",
        );
        write(&paths.legacy_root.join("order_cache.sqlite3-wal"), "wal");
        write(&paths.misplaced_runtime_root.join("cookie.txt"), "cookie");
        write(
            &paths.misplaced_runtime_root.join("license.json"),
            "{\"plan\":\"basic\"}",
        );
        write(
            &paths.misplaced_runtime_root.join("selected_config_dir.txt"),
            "/old/path",
        );

        let report = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-17").run();
        assert!(report.legacy_detected);
        assert!(report.cache_migrated);
        assert!(report.cookie_migrated);
        assert!(report.license_migrated);
        assert!(report.config_pointer_migrated);
        assert!(
            report.errors.is_empty(),
            "unexpected errors: {:?}",
            report.errors
        );

        assert_eq!(
            read(&paths.cache_root.join("order_cache.sqlite3")),
            "sqlite-main"
        );
        assert_eq!(read(&paths.runtime_root.join("cookie.txt")), "cookie");

        let backup_dir = paths.backup_root.join("2026-04-17");
        assert_eq!(
            read(&backup_dir.join("order_cache.sqlite3")),
            "sqlite-main",
            "备份必须与源字节级一致"
        );
        assert_eq!(read(&backup_dir.join("cookie.txt")), "cookie");
    }

    #[test]
    fn skips_when_destination_already_exists() {
        let tmp = tempdir().unwrap();
        let paths = setup_paths(tmp.path());
        write(
            &paths.legacy_root.join("order_cache.sqlite3"),
            "legacy-data",
        );
        write(
            &paths.cache_root.join("order_cache.sqlite3"),
            "existing-data",
        );

        let report = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-17").run();
        assert!(!report.legacy_detected);
        assert!(!report.cache_migrated);
        // 必须保留新库原数据不被覆盖
        assert_eq!(
            read(&paths.cache_root.join("order_cache.sqlite3")),
            "existing-data"
        );
        assert!(report.errors.is_empty());
    }

    #[test]
    fn partial_failure_does_not_abort_other_files() {
        let tmp = tempdir().unwrap();
        let paths = setup_paths(tmp.path());
        // 只有 cookie 是 legacy 文件，order_cache 不存在 → 跳过但不失败
        write(
            &paths.misplaced_runtime_root.join("cookie.txt"),
            "cookie-data",
        );

        let report = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-17").run();
        assert!(report.legacy_detected);
        assert!(!report.cache_migrated);
        assert!(report.cookie_migrated);
        assert!(
            report.errors.is_empty(),
            "缺文件不应产生错误：{:?}",
            report.errors
        );
        assert_eq!(read(&paths.runtime_root.join("cookie.txt")), "cookie-data");
    }

    #[test]
    fn backup_dir_uses_today_subfolder() {
        let tmp = tempdir().unwrap();
        let paths = setup_paths(tmp.path());
        write(&paths.misplaced_runtime_root.join("cookie.txt"), "x");

        let report = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-17").run();
        let expected = paths.backup_root.join("2026-04-17");
        assert_eq!(report.backup_dir.unwrap(), expected.to_string_lossy());
        assert!(expected.join("cookie.txt").exists());
    }

    #[test]
    fn running_twice_is_idempotent_because_destination_exists() {
        let tmp = tempdir().unwrap();
        let paths = setup_paths(tmp.path());
        write(&paths.misplaced_runtime_root.join("cookie.txt"), "v1");

        let first = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-17").run();
        assert!(first.cookie_migrated);

        // 第二次：遗留文件仍在，但运行时目录已有同名文件，不应再重复提示 legacy
        let second = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-18").run();
        assert!(!second.legacy_detected);
        assert!(!second.cookie_migrated);
        assert_eq!(read(&paths.runtime_root.join("cookie.txt")), "v1");
    }

    #[test]
    fn current_runtime_home_is_not_misclassified_as_legacy_cookie_source() {
        let tmp = tempdir().unwrap();
        let shared_root = tmp.path().join("shared-home");
        let paths = MigrationPaths::with_roots(
            shared_root.clone(),
            shared_root.clone(),
            tmp.path().join("misplaced-runtime"),
            tmp.path().join("cache"),
            shared_root.join("legacy_backup"),
        );
        write(&paths.runtime_root.join("cookie.txt"), "already-live");

        let report = LegacyPythonMigrator::with_today(paths.clone(), "2026-04-17").run();

        assert!(!report.legacy_detected);
        assert!(!report.cookie_migrated);
        assert_eq!(read(&paths.runtime_root.join("cookie.txt")), "already-live");
    }
}
