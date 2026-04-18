use anyhow::{anyhow, Context, Result};
use desktop_services::review_batch_match::{match_orders_with_evaluations, EvaluationRecord};
use desktop_services::review_candidate_scoring::{
    score_candidate_order, CandidateOrder, EvaluationMatchContext,
};
use desktop_services::review_index::{build_product_sku_index, collect_candidate_orders};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_FIXTURE_DIR: &str = "tests/fixtures/real_user_snapshot";
const DEFAULT_OUTPUT_DIR: &str = "tests/artifacts/bench_match";
const MAX_SCORE_DIFF: i32 = 2;
const MAX_STRATEGY_GAP: i32 = 1;
const MAX_MISMATCH_RATE: f64 = 0.02;
const MIN_REQUIRED_EVALUATIONS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MatchBenchmarkFixture {
    pub snapshot_name: String,
    #[serde(default)]
    pub notes: Vec<String>,
    pub orders: Vec<CandidateOrder>,
    pub evaluations: Vec<EvaluationRecord>,
    pub python_results: Vec<PythonMatchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PythonMatchResult {
    pub evaluation_id: String,
    pub order_id: Option<String>,
    pub matched: bool,
    pub match_score: i32,
    pub match_strategy: Option<String>,
    #[serde(default)]
    pub replyable: Option<bool>,
    #[serde(default)]
    pub top_candidates: Vec<PythonTopCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PythonTopCandidate {
    pub order_id: String,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct BenchmarkDiffRow {
    pub evaluation_id: String,
    pub python_order_id: Option<String>,
    pub rust_order_id: Option<String>,
    pub python_match_score: i32,
    pub rust_match_score: i32,
    pub score_diff: i32,
    pub python_strategy: Option<String>,
    pub rust_strategy: Option<String>,
    pub strategy_gap: i32,
    pub python_top5_avg_score: Option<f64>,
    pub rust_top5_avg_score: Option<f64>,
    pub top5_avg_diff: Option<f64>,
    pub mismatch_reason: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct BenchmarkSummary {
    pub snapshot_name: String,
    pub total_evaluations: usize,
    pub mismatched_rows: usize,
    pub mismatch_rate: f64,
    pub allowed_mismatch_rate: f64,
    pub score_diff_limit: i32,
    pub strategy_gap_limit: i32,
    pub passed: bool,
    #[serde(default)]
    pub mismatch_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
struct BenchmarkArtifacts {
    summary: BenchmarkSummary,
    #[serde(default)]
    diffs: Vec<BenchmarkDiffRow>,
}

pub fn run_bench_match_command(args: &[std::ffi::OsString]) -> Result<()> {
    let fixture_dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FIXTURE_DIR));
    let output_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR));

    let fixture = load_fixture(&fixture_dir)?;
    let artifacts = benchmark_fixture(&fixture);
    write_artifacts(&output_dir, &fixture, &artifacts)?;

    println!(
        "bench-match completed: snapshot={} total={} mismatched={} rate={:.2}% passed={}",
        artifacts.summary.snapshot_name,
        artifacts.summary.total_evaluations,
        artifacts.summary.mismatched_rows,
        artifacts.summary.mismatch_rate * 100.0,
        artifacts.summary.passed,
    );
    println!(
        "report markdown: {}",
        output_dir.join("summary.md").display()
    );
    println!("report csv: {}", output_dir.join("diff.csv").display());

    if !artifacts.summary.passed {
        return Err(anyhow!(
            "bench-match found {} mismatches; see {}",
            artifacts.summary.mismatched_rows,
            output_dir.join("summary.md").display()
        ));
    }

    Ok(())
}

fn load_fixture(dir: &Path) -> Result<MatchBenchmarkFixture> {
    let path = dir.join("snapshot.json");
    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "读取真实数据快照失败：{}。请先准备脱敏后的 tests/fixtures/real_user_snapshot/snapshot.json",
            path.display()
        )
    })?;
    let fixture: MatchBenchmarkFixture =
        serde_json::from_str(&raw).context("解析 snapshot.json 失败，请检查字段格式")?;
    if fixture.evaluations.is_empty()
        || fixture.orders.is_empty()
        || fixture.python_results.is_empty()
    {
        return Err(anyhow!(
            "snapshot.json 数据不完整：orders/evaluations/python_results 不能为空"
        ));
    }
    if fixture.python_results.len() < MIN_REQUIRED_EVALUATIONS {
        return Err(anyhow!(
            "真实数据比对至少需要 {MIN_REQUIRED_EVALUATIONS} 条评价样本，当前只有 {} 条",
            fixture.python_results.len()
        ));
    }
    Ok(fixture)
}

fn benchmark_fixture(fixture: &MatchBenchmarkFixture) -> BenchmarkArtifacts {
    let rust_results = match_orders_with_evaluations(&fixture.evaluations, &fixture.orders);
    let mut diffs = Vec::new();
    let mut mismatch_reasons = Vec::new();

    for python in &fixture.python_results {
        let Some(rust) = rust_results
            .iter()
            .find(|item| item.evaluation_id == python.evaluation_id)
        else {
            diffs.push(BenchmarkDiffRow {
                evaluation_id: python.evaluation_id.clone(),
                python_order_id: python.order_id.clone(),
                rust_order_id: None,
                python_match_score: python.match_score,
                rust_match_score: 0,
                score_diff: python.match_score,
                python_strategy: python.match_strategy.clone(),
                rust_strategy: None,
                strategy_gap: 99,
                python_top5_avg_score: top5_average(&python.top_candidates),
                rust_top5_avg_score: None,
                top5_avg_diff: None,
                mismatch_reason: vec!["Rust 结果缺失".into()],
            });
            mismatch_reasons.push("Rust 结果缺失".to_string());
            continue;
        };

        let rust_top_candidates = collect_rust_top_candidates(fixture, &python.evaluation_id);
        let row = build_diff_row(python, rust, &rust_top_candidates);
        if !row.mismatch_reason.is_empty() {
            mismatch_reasons.extend(row.mismatch_reason.clone());
            diffs.push(row);
        }
    }

    let total = fixture.python_results.len();
    let mismatched_rows = diffs.len();
    let mismatch_rate = if total == 0 {
        0.0
    } else {
        mismatched_rows as f64 / total as f64
    };

    BenchmarkArtifacts {
        summary: BenchmarkSummary {
            snapshot_name: fixture.snapshot_name.clone(),
            total_evaluations: total,
            mismatched_rows,
            mismatch_rate,
            allowed_mismatch_rate: MAX_MISMATCH_RATE,
            score_diff_limit: MAX_SCORE_DIFF,
            strategy_gap_limit: MAX_STRATEGY_GAP,
            passed: total >= MIN_REQUIRED_EVALUATIONS && mismatch_rate <= MAX_MISMATCH_RATE,
            mismatch_reasons: dedup_strings(mismatch_reasons),
        },
        diffs,
    }
}

fn collect_rust_top_candidates(
    fixture: &MatchBenchmarkFixture,
    evaluation_id: &str,
) -> Vec<PythonTopCandidate> {
    let Some(evaluation) = fixture
        .evaluations
        .iter()
        .find(|item| item.evaluation_id == evaluation_id)
    else {
        return Vec::new();
    };

    let index = build_product_sku_index(&fixture.orders);
    let context = EvaluationMatchContext {
        buyer_nickname: evaluation.buyer_nickname.clone(),
        product_id: evaluation.product_id.clone(),
        sku_id: evaluation.sku_id.clone(),
        product_name: evaluation.product_name.clone(),
        eval_time: evaluation.eval_time,
    };
    let mut scored = collect_candidate_orders(&index, &context, &evaluation.sku_name)
        .into_iter()
        .filter_map(|order| score_candidate_order(&order, &context))
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.confirm_diff.cmp(&right.confirm_diff))
            .then_with(|| left.time_diff.cmp(&right.time_diff))
            .then_with(|| left.order.order_id.cmp(&right.order.order_id))
    });

    scored
        .into_iter()
        .take(5)
        .map(|item| PythonTopCandidate {
            order_id: item.order.order_id,
            score: item.score,
        })
        .collect()
}

fn build_diff_row(
    python: &PythonMatchResult,
    rust: &desktop_services::review_batch_match::MatchedEvaluationResult,
    rust_top_candidates: &[PythonTopCandidate],
) -> BenchmarkDiffRow {
    let rust_strategy = rust.match_strategy.map(match_strategy_to_string);
    let rust_order_id = rust.order_id.clone();
    let score_diff = (python.match_score - rust.match_score).abs();
    let strategy_gap = strategy_gap(python.match_strategy.as_deref(), rust_strategy.as_deref());
    let python_top5_avg_score = top5_average(&python.top_candidates);
    let rust_top5_avg_score = top5_average(rust_top_candidates);
    let top5_avg_diff = match (python_top5_avg_score, rust_top5_avg_score) {
        (Some(left), Some(right)) => Some((left - right).abs()),
        _ => None,
    };

    let mut mismatch_reason = Vec::new();
    if python.order_id != rust_order_id {
        mismatch_reason.push("其他：命中订单不一致".to_string());
    }
    if python.matched != rust.matched {
        mismatch_reason.push("其他：matched 标记不一致".to_string());
    }
    if score_diff > MAX_SCORE_DIFF {
        mismatch_reason.push(format!("昵称算法差异：评分差异超阈值（{}）", score_diff));
    }
    if strategy_gap > MAX_STRATEGY_GAP {
        mismatch_reason.push(format!(
            "昵称算法差异：strategy 分档差异超阈值（{}）",
            strategy_gap
        ));
    }
    if let Some(replyable) = python.replyable {
        if replyable != rust.matched && python.order_id == rust_order_id {
            mismatch_reason.push("可回复期差异：replyable 判断不一致".to_string());
        }
    }
    if let Some(diff) = top5_avg_diff {
        if diff > MAX_SCORE_DIFF as f64 {
            mismatch_reason.push(format!("昵称算法差异：Top5 平均分差异超阈值（{diff:.2}）"));
        }
    }

    BenchmarkDiffRow {
        evaluation_id: python.evaluation_id.clone(),
        python_order_id: python.order_id.clone(),
        rust_order_id,
        python_match_score: python.match_score,
        rust_match_score: rust.match_score,
        score_diff,
        python_strategy: python.match_strategy.clone(),
        rust_strategy,
        strategy_gap,
        python_top5_avg_score,
        rust_top5_avg_score,
        top5_avg_diff,
        mismatch_reason,
    }
}

fn top5_average(candidates: &[PythonTopCandidate]) -> Option<f64> {
    if candidates.is_empty() {
        return None;
    }
    let sum: i32 = candidates.iter().take(5).map(|item| item.score).sum();
    let count = candidates.len().min(5) as f64;
    Some(sum as f64 / count)
}

fn strategy_gap(left: Option<&str>, right: Option<&str>) -> i32 {
    let left = left.map(strategy_rank).unwrap_or(0);
    let right = right.map(strategy_rank).unwrap_or(0);
    (left - right).abs()
}

fn strategy_rank(value: &str) -> i32 {
    match value {
        "exact_match" => 4,
        "high_confidence" => 3,
        "probable_match" => 2,
        "fallback" => 1,
        _ => 0,
    }
}

fn match_strategy_to_string(
    strategy: desktop_services::review_match_flow::MatchStrategy,
) -> String {
    match strategy {
        desktop_services::review_match_flow::MatchStrategy::ExactMatch => "exact_match".into(),
        desktop_services::review_match_flow::MatchStrategy::HighConfidence => {
            "high_confidence".into()
        }
        desktop_services::review_match_flow::MatchStrategy::ProbableMatch => {
            "probable_match".into()
        }
        desktop_services::review_match_flow::MatchStrategy::Fallback => "fallback".into(),
        desktop_services::review_match_flow::MatchStrategy::None => "none".into(),
    }
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut set = values;
    set.sort();
    set.dedup();
    set
}

fn write_artifacts(
    output_dir: &Path,
    fixture: &MatchBenchmarkFixture,
    artifacts: &BenchmarkArtifacts,
) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("创建输出目录失败：{}", output_dir.display()))?;

    fs::write(
        output_dir.join("summary.json"),
        serde_json::to_vec_pretty(&artifacts.summary)?,
    )?;
    fs::write(
        output_dir.join("diff.json"),
        serde_json::to_vec_pretty(&artifacts.diffs)?,
    )?;
    fs::write(output_dir.join("diff.csv"), build_csv(&artifacts.diffs))?;
    fs::write(
        output_dir.join("rust_results.json"),
        serde_json::to_vec_pretty(&match_orders_with_evaluations(
            &fixture.evaluations,
            &fixture.orders,
        ))?,
    )?;
    fs::write(
        output_dir.join("summary.md"),
        build_markdown_summary(fixture, artifacts),
    )?;
    Ok(())
}

fn build_csv(rows: &[BenchmarkDiffRow]) -> String {
    let mut csv = String::from("evaluation_id,python_order_id,rust_order_id,python_match_score,rust_match_score,score_diff,python_strategy,rust_strategy,strategy_gap,python_top5_avg_score,rust_top5_avg_score,top5_avg_diff,mismatch_reason\n");
    for row in rows {
        let reason = row.mismatch_reason.join(" | ").replace(',', "，");
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            row.evaluation_id,
            row.python_order_id.clone().unwrap_or_default(),
            row.rust_order_id.clone().unwrap_or_default(),
            row.python_match_score,
            row.rust_match_score,
            row.score_diff,
            row.python_strategy.clone().unwrap_or_default(),
            row.rust_strategy.clone().unwrap_or_default(),
            row.strategy_gap,
            opt_f64(row.python_top5_avg_score),
            opt_f64(row.rust_top5_avg_score),
            opt_f64(row.top5_avg_diff),
            reason,
        ));
    }
    csv
}

fn build_markdown_summary(
    fixture: &MatchBenchmarkFixture,
    artifacts: &BenchmarkArtifacts,
) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# 真实数据比对摘要：{}\n\n",
        fixture.snapshot_name
    ));
    if !fixture.notes.is_empty() {
        md.push_str("## 快照备注\n\n");
        for note in &fixture.notes {
            md.push_str(&format!("- {}\n", note));
        }
        md.push('\n');
    }
    md.push_str("## 结果概览\n\n");
    md.push_str("| 指标 | 值 |\n|---|---:|\n");
    md.push_str(&format!(
        "| 总评价数 | {} |\n",
        artifacts.summary.total_evaluations
    ));
    md.push_str(&format!(
        "| 最低样本门槛 | {} |\n",
        MIN_REQUIRED_EVALUATIONS
    ));
    md.push_str(&format!(
        "| 不一致条数 | {} |\n",
        artifacts.summary.mismatched_rows
    ));
    md.push_str(&format!(
        "| 不一致率 | {:.2}% |\n",
        artifacts.summary.mismatch_rate * 100.0
    ));
    md.push_str(&format!(
        "| 允许上限 | {:.2}% |\n",
        artifacts.summary.allowed_mismatch_rate * 100.0
    ));
    md.push_str(&format!(
        "| 结论 | {} |\n\n",
        if artifacts.summary.passed {
            "通过"
        } else {
            "未通过"
        }
    ));

    if !artifacts.summary.mismatch_reasons.is_empty() {
        md.push_str("## 差异归因\n\n");
        for reason in &artifacts.summary.mismatch_reasons {
            md.push_str(&format!("- {}\n", reason));
        }
        md.push('\n');
    }

    md.push_str("## 不一致条目\n\n");
    if artifacts.diffs.is_empty() {
        md.push_str("本次未发现不一致条目。\n");
        return md;
    }

    md.push_str(
        "| evaluation_id | Python 订单 | Rust 订单 | 分数差 | strategy 差 | Top5 均分差 | 原因 |\n",
    );
    md.push_str("|---|---|---|---:|---:|---:|---|\n");
    for row in &artifacts.diffs {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            row.evaluation_id,
            row.python_order_id.clone().unwrap_or_else(|| "-".into()),
            row.rust_order_id.clone().unwrap_or_else(|| "-".into()),
            row.score_diff,
            row.strategy_gap,
            opt_f64(row.top5_avg_diff),
            row.mismatch_reason.join("；")
        ));
    }
    md
}

fn opt_f64(value: Option<f64>) -> String {
    value.map(|item| format!("{item:.2}")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_fixture() -> MatchBenchmarkFixture {
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
                evaluation_content: format!("有点痒-{index}"),
                default_content: String::new(),
                evaluation_star: 1,
                can_reply_expire_time: 0,
            })
            .collect::<Vec<_>>();
        let python_results = (0..100)
            .map(|index| PythonMatchResult {
                evaluation_id: format!("eval-{index}"),
                order_id: Some("order-1".into()),
                matched: true,
                match_score: 95,
                match_strategy: Some("probable_match".into()),
                replyable: Some(true),
                top_candidates: vec![PythonTopCandidate {
                    order_id: "order-1".into(),
                    score: 95,
                }],
            })
            .collect::<Vec<_>>();
        MatchBenchmarkFixture {
            snapshot_name: "sample".into(),
            notes: vec!["脱敏样板数据，仅用于验证 bench-match 工作流".into()],
            orders,
            evaluations,
            python_results,
        }
    }

    #[test]
    fn benchmark_fixture_passes_for_sample_snapshot() {
        let artifacts = benchmark_fixture(&sample_fixture());
        assert!(artifacts.summary.passed);
        assert!(artifacts.diffs.is_empty());
    }

    #[test]
    fn write_artifacts_outputs_report_files() {
        let dir = tempdir().unwrap();
        let fixture = sample_fixture();
        let artifacts = benchmark_fixture(&fixture);
        write_artifacts(dir.path(), &fixture, &artifacts).unwrap();
        assert!(dir.path().join("summary.md").exists());
        assert!(dir.path().join("diff.csv").exists());
        assert!(dir.path().join("rust_results.json").exists());
    }
}
