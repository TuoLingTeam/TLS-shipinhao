use anyhow::{Context, Result};
use desktop_services::review_batch_match::{match_orders_with_evaluations, EvaluationRecord};
use desktop_services::review_candidate_scoring::CandidateOrder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DEFAULT_REPORT_PATH: &str = "docs/perf-report-2026-04-17.md";
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

pub fn run_perf_command(args: &[std::ffi::OsString]) -> Result<()> {
    let output = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_REPORT_PATH));
    let report = collect_perf_report();
    write_report(&output, &report)?;
    println!("perf report written: {}", output.display());
    Ok(())
}

fn collect_perf_report() -> PerfReport {
    let match_elapsed = benchmark_match_100();
    let release_binary_size_mb = find_release_binary_size_mb();

    let metrics = vec![
        PerfMetric {
            name: "冷启动时间".into(),
            target: format!("< {STARTUP_TARGET_SECS:.0} 秒"),
            actual: "待人工执行（推荐 hyperfine / 真机桌面启动）".into(),
            status: "manual".into(),
            evidence: "需在 M6-04 打包产物上复测".into(),
        },
        PerfMetric {
            name: "订单同步 1000 条".into(),
            target: format!("< {SYNC_TARGET_SECS:.0} 秒"),
            actual: "待人工执行（真实 Cookie / 外部接口）".into(),
            status: "manual".into(),
            evidence: "外部依赖强，留待发布前环境跑实测".into(),
        },
        PerfMetric {
            name: "评价匹配 100 条".into(),
            target: format!("< {MATCH_TARGET_SECS:.0} 秒"),
            actual: format!("{match_elapsed:.4} 秒（样板数据基准）"),
            status: if match_elapsed < MATCH_TARGET_SECS { "pass" } else { "fail" }.into(),
            evidence: "xtask perf 内部 synthetic benchmark".into(),
        },
        PerfMetric {
            name: "单条发货".into(),
            target: format!("< {SINGLE_DELIVERY_TARGET_SECS:.0} 秒"),
            actual: "待人工执行（真实接口）".into(),
            status: "manual".into(),
            evidence: "需要外部接口与可用订单号".into(),
        },
        PerfMetric {
            name: "批量发货 100 条".into(),
            target: format!("< {BATCH_DELIVERY_TARGET_SECS:.0} 秒"),
            actual: "待人工执行（真实接口）".into(),
            status: "manual".into(),
            evidence: "需要外部接口与可用订单号".into(),
        },
        PerfMetric {
            name: "运行内存".into(),
            target: format!("< {MEMORY_TARGET_MB:.0} MB"),
            actual: "待人工执行（推荐 Instruments / Activity Monitor）".into(),
            status: "manual".into(),
            evidence: "需在桌面产物运行态采样".into(),
        },
        PerfMetric {
            name: "安装包体积".into(),
            target: format!("< {PACKAGE_TARGET_MB:.0} MB"),
            actual: release_binary_size_mb
                .map(|size| format!("{size:.2} MB（当前 release 二进制）"))
                .unwrap_or_else(|| "待生成 release 产物后复测".into()),
            status: release_binary_size_mb
                .map(|size| if size < PACKAGE_TARGET_MB { "pass" } else { "fail" })
                .unwrap_or("manual")
                .into(),
            evidence: "target/release/desktop-app 或 desktop 二进制大小".into(),
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
            evaluation_id: format!("eval-{index}"),
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
    [
        PathBuf::from("target/release/desktop-app"),
        PathBuf::from("target/release/desktop"),
    ]
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
    md.push_str("- 基线版本：`5.1.0`\n");
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
        let report = collect_perf_report();
        write_report(&output, &report).unwrap();
        let content = fs::read_to_string(output).unwrap();
        assert!(content.contains("性能报告"));
        assert!(content.contains("评价匹配 100 条"));
    }
}
