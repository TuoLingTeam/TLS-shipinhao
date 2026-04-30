use anyhow::{Context, Result};
use desktop_services::review_batch_match::{match_orders_with_evaluations, EvaluationRecord};
use desktop_services::review_candidate_scoring::CandidateOrder;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DEFAULT_REPORT_PATH: &str = "target/perf-report-2026-04-17.md";
const MATCH_TARGET_SECS: f64 = 5.0;
const PACKAGE_TARGET_MB: f64 = 30.0;
const MEMORY_TARGET_MB: f64 = 200.0;
const STARTUP_TARGET_SECS: f64 = 2.0;
const SYNC_TARGET_SECS: f64 = 60.0;
const SINGLE_DELIVERY_TARGET_SECS: f64 = 3.0;
const BATCH_DELIVERY_TARGET_SECS: f64 = 300.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PerfMetric {
    pub name: String,
    pub target: String,
    pub actual: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PerfReport {
    pub generated_at: String,
    pub metrics: Vec<PerfMetric>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct PerfOverrides {
    startup_secs: Option<f64>,
    sync_secs: Option<f64>,
    match_secs: Option<f64>,
    single_delivery_secs: Option<f64>,
    batch_delivery_secs: Option<f64>,
    memory_mb: Option<f64>,
    package_mb: Option<f64>,
}

pub fn run_perf_command(args: &[OsString]) -> Result<()> {
    let output = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_PATH));
    let overrides = parse_overrides(&args[1..])?;
    let report = collect_perf_report(&overrides);
    write_report(&output, &report)?;
    println!("perf report written: {}", output.display());
    Ok(())
}

fn collect_perf_report(overrides: &PerfOverrides) -> PerfReport {
    let measured_match_secs = overrides.match_secs.unwrap_or_else(benchmark_match_100);
    let release_binary_size_mb = overrides.package_mb.or_else(find_release_binary_size_mb);

    let metrics = vec![
        build_metric(
            "冷启动时间",
            STARTUP_TARGET_SECS,
            overrides.startup_secs,
            "秒",
            "推荐使用 hyperfine 或真机桌面启动测量",
        ),
        build_metric(
            "订单同步 1000 条",
            SYNC_TARGET_SECS,
            overrides.sync_secs,
            "秒",
            "需使用真实 Cookie 与外部接口实测",
        ),
        PerfMetric {
            name: "评价匹配 100 条".into(),
            target: format!("< {MATCH_TARGET_SECS:.0} 秒"),
            actual: format!("{measured_match_secs:.4} 秒"),
            status: if measured_match_secs < MATCH_TARGET_SECS {
                "pass"
            } else {
                "fail"
            }
            .into(),
            evidence: if overrides.match_secs.is_some() {
                "人工录入实测值".into()
            } else {
                "xtask perf 内部 synthetic benchmark".into()
            },
        },
        build_metric(
            "单条发货",
            SINGLE_DELIVERY_TARGET_SECS,
            overrides.single_delivery_secs,
            "秒",
            "需要真实接口与可用订单号",
        ),
        build_metric(
            "批量发货 100 条",
            BATCH_DELIVERY_TARGET_SECS,
            overrides.batch_delivery_secs,
            "秒",
            "需要真实接口与可用订单号",
        ),
        build_metric(
            "运行内存",
            MEMORY_TARGET_MB,
            overrides.memory_mb,
            "MB",
            "推荐使用 Instruments / Activity Monitor 采样",
        ),
        PerfMetric {
            name: "安装包体积".into(),
            target: format!("< {PACKAGE_TARGET_MB:.0} MB"),
            actual: release_binary_size_mb
                .map(|size| format!("{size:.2} MB"))
                .unwrap_or_else(|| "待生成 release 产物后复测".into()),
            status: release_binary_size_mb
                .map(|size| {
                    if size < PACKAGE_TARGET_MB {
                        "pass"
                    } else {
                        "fail"
                    }
                })
                .unwrap_or("manual")
                .into(),
            evidence: if overrides.package_mb.is_some() {
                "人工录入安装包体积".into()
            } else {
                "target/release/desktop 二进制大小".into()
            },
        },
    ];

    PerfReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        metrics,
        notes: vec![
            "本报告用于建立 M6-03 性能基线；涉及外部接口和桌面 GUI 的指标保留人工复测。".into(),
            "当前仓库内已自动采集 100 条评价匹配样板基准，并补充 release 二进制体积探测。".into(),
            "正式发版前需在 macOS / Windows 打包产物上补齐冷启动、内存、同步与发货实测。".into(),
        ],
    }
}

fn build_metric(
    name: &str,
    target: f64,
    actual: Option<f64>,
    unit: &str,
    evidence: &str,
) -> PerfMetric {
    PerfMetric {
        name: name.into(),
        target: format!("< {:.0} {}", target, unit),
        actual: actual
            .map(|value| format!("{value:.4} {unit}"))
            .unwrap_or_else(|| "待人工执行".into()),
        status: actual
            .map(|value| if value < target { "pass" } else { "fail" })
            .unwrap_or("manual")
            .into(),
        evidence: evidence.into(),
    }
}

fn parse_overrides(args: &[OsString]) -> Result<PerfOverrides> {
    let mut overrides = PerfOverrides::default();
    for arg in args {
        let Some(raw) = arg.to_str() else {
            continue;
        };
        let Some((key, value)) = raw.split_once('=') else {
            continue;
        };
        let parsed = value
            .parse::<f64>()
            .with_context(|| format!("无法解析性能参数：{raw}"))?;
        match key {
            "--startup" => overrides.startup_secs = Some(parsed),
            "--sync" => overrides.sync_secs = Some(parsed),
            "--match" => overrides.match_secs = Some(parsed),
            "--single-delivery" => overrides.single_delivery_secs = Some(parsed),
            "--batch-delivery" => overrides.batch_delivery_secs = Some(parsed),
            "--memory" => overrides.memory_mb = Some(parsed),
            "--package" => overrides.package_mb = Some(parsed),
            _ => {}
        }
    }
    Ok(overrides)
}

fn benchmark_match_100() -> f64 {
    let orders = vec![CandidateOrder {
        order_id: "order-1".into(),
        buyer_nickname: "赵亮".into(),
        product_id: "p1".into(),
        sku_id: "s1".into(),
        product_name: "仁和洗发水".into(),
        create_time: 1_712_910_000 - 172800,
        confirm_receipt_time: 0,
        is_waybill_received: false,
        waybill_received_time: 0,
        sale_param: "默认规格".into(),
    }];
    let evaluations = (0..100)
        .map(|index| EvaluationRecord {
            buyer_nickname: "赵亮6057".into(),
            product_id: "p1".into(),
            sku_id: "s1".into(),
            sku_name: "默认规格".into(),
            product_name: "仁和洗发水".into(),
            eval_time: 1_712_910_000 + index as i64,
            attitude_name: "差评".into(),
            evaluation_content: "有点痒".into(),
            default_content: String::new(),
            evaluation_star: 1,
            can_reply_expire_time: 0,
        })
        .collect::<Vec<_>>();

    let start = Instant::now();
    let _ = match_orders_with_evaluations(&evaluations, &orders);
    start.elapsed().as_secs_f64()
}

fn find_release_binary_size_mb() -> Option<f64> {
    [PathBuf::from("target/release/desktop")]
        .into_iter()
        .find_map(|path| fs::metadata(path).ok())
        .map(|meta| meta.len() as f64 / 1024.0 / 1024.0)
}

fn write_report(path: &Path, report: &PerfReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建性能报告目录失败：{}", parent.display()))?;
    }
    fs::write(path, render_markdown(report))
        .with_context(|| format!("写入性能报告失败：{}", path.display()))?;
    Ok(())
}

fn render_markdown(report: &PerfReport) -> String {
    let mut md = String::new();
    md.push_str("# TLS-shipinhao 性能报告（2026-04-17）\n\n");
    md.push_str(&format!("- 生成时间：`{}`\n", report.generated_at));
    md.push_str("- 基线版本：`5.0.0`\n");
    md.push_str("- 对应卡片：`M6-03 性能压测与指标校验`\n\n");
    md.push_str("## 指标矩阵\n\n");
    md.push_str("| 指标 | 目标 | 实测 | 状态 | 证据/备注 |\n|---|---|---|---|---|\n");
    for metric in &report.metrics {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            metric.name, metric.target, metric.actual, metric.status, metric.evidence
        ));
    }
    md.push_str("\n## 备注\n\n");
    for note in &report.notes {
        md.push_str(&format!("- {}\n", note));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn synthetic_match_benchmark_is_fast_enough_for_report_generation() {
        let seconds = benchmark_match_100();
        assert!(seconds >= 0.0);
        assert!(seconds < 1.0);
    }

    #[test]
    fn perf_report_can_be_written() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("perf.md");
        let report = collect_perf_report(&PerfOverrides::default());
        write_report(&output, &report).unwrap();
        let content = fs::read_to_string(output).unwrap();
        assert!(content.contains("性能报告"));
        assert!(content.contains("评价匹配 100 条"));
    }
}
