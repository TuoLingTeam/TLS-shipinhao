//! 反风控的冷却 + 极速降级编排。
//!
//! 对应 PRD §7.1.4 / M1-05。核心思想：
//!
//! 1. 业务层先以「正常模式」抓取（多 worker 高频）。
//! 2. 命中风控时返回 `NormalOutcome::RiskControl { partial }`，编排层自动进入
//!    60 秒冷却，并每 10 秒向前端广播剩余时间，便于用户看到进度且可随时中断。
//! 3. 冷却完成后切换「极速模式」（单 worker 低频），再尝试一次。
//! 4. 极速模式也失败时，若已有部分数据就返回 + 警告，否则上抛致命错误。
//!
//! 该文件**不依赖 HTTP 客户端**——把真正的 HTTP 请求收口到调用侧的两个 async
//! closure。好处：单元测试无需 mock HTTP，直接构造 outcome 就能覆盖所有分支，
//! 业务侧接入时想跑 normal_fn / risk_fn 就跑，解耦度高。

use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::order_fetcher::{FetchError, ProgressCallback};

// ---- 常量（与 Python 4.3.0 对齐） -------------------------------------------

/// 正常模式并发 worker 数。
pub const ORDER_WINDOW_WORKERS: usize = 3;
/// 正常模式每页请求间隔。
pub const FETCH_PAGE_INTERVAL: Duration = Duration::from_millis(300);

/// 极速（降级）模式并发 worker 数。单 worker 串行化以避免再次触发风控。
pub const ORDER_RISK_WINDOW_WORKERS: usize = 1;
/// 极速模式每页请求间隔，明显慢于正常模式。
pub const ORDER_RISK_PAGE_INTERVAL: Duration = Duration::from_secs(2);

/// 命中风控后的冷却总时长。
pub const RISK_COOLDOWN_SECS: u64 = 60;
/// 冷却倒计时打印步长。
pub const RISK_COOLDOWN_STEP_SECS: u64 = 10;

const STOP_POLL_INTERVAL_MS: u64 = 100;

const WARNING_DEGRADED: &str = "本次抓取触发平台风控，已自动降级到极速模式";
const WARNING_PARTIAL: &str = "仍有部分窗口未完成，结果可能不完整";
const FATAL_PERSISTENT_RISK: &str = "平台风控持续触发，请稍后重试";

// ---- 业务层与编排层的通信语义 -----------------------------------------------

/// 正常模式的抓取结果。
///
/// 之所以不复用 `Result<Vec<T>, FetchError>`，是因为「风控」不是错误——它是
/// 「请走降级路径」的控制信号，需要携带已抓到的部分数据以便后续合并去重。
#[derive(Debug)]
pub enum NormalOutcome<T> {
    Ok(Vec<T>),
    /// 命中风控，已抓到 `partial` 份数据，请降级后再补抓。
    RiskControl {
        partial: Vec<T>,
    },
    /// 遇到非风控类致命错误，不会进入降级路径。
    Fatal(String),
}

/// 极速模式的抓取结果。
#[derive(Debug)]
pub enum RiskOutcome<T> {
    Ok(Vec<T>),
    /// 极速模式仍命中风控或其他限制，携带本次已获取的 `partial`。
    Failed {
        partial: Vec<T>,
    },
    /// 致命错误，立即上抛。
    Fatal(String),
}

/// 编排层的最终输出。
#[derive(Debug)]
pub struct FallbackOutcome<T> {
    pub orders: Vec<T>,
    pub warnings: Vec<String>,
}

// ---- 通用工具 ---------------------------------------------------------------

/// 按给定 key 提取器稳定去重。保留首次出现，顺序保留。
///
/// 用法：`deduplicate_by(orders, |o| o.order_id.clone())`。
pub fn deduplicate_by<T, K, F>(items: Vec<T>, key_of: F) -> Vec<T>
where
    K: Eq + Hash,
    F: Fn(&T) -> K,
{
    let mut seen: HashSet<K> = HashSet::with_capacity(items.len());
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let key = key_of(&it);
        if seen.insert(key) {
            out.push(it);
        }
    }
    out
}

/// 冷却倒计时：按 `step` 分片推进，期间每 100ms 轮询 `stop_flag`，保证停止响应 <200ms。
///
/// 进度消息：
/// - 开始：`"⚠️ 检测到平台风控，等待 60 秒冷却后切换到极速模式..."`
/// - 每步：`"[风控冷却] 还剩 N 秒..."`，N ∈ {60, 50, 40, 30, 20, 10}（取决于 total/step）
pub async fn cool_down_with_countdown(
    total_secs: u64,
    step_secs: u64,
    stop_flag: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<(), FetchError> {
    on_progress(format!(
        "⚠️ 检测到平台风控，等待 {total_secs} 秒冷却后切换到极速模式（单线程 + 更慢间隔）。"
    ));
    let step = step_secs.max(1);
    let mut remaining = total_secs;
    while remaining > 0 {
        if stop_flag.load(Ordering::Relaxed) {
            return Err(FetchError::Stopped);
        }
        on_progress(format!("[风控冷却] 还剩 {remaining} 秒..."));
        let chunk = step.min(remaining);
        sleep_interruptible(Duration::from_secs(chunk), &stop_flag).await?;
        remaining = remaining.saturating_sub(chunk);
    }
    Ok(())
}

async fn sleep_interruptible(total: Duration, stop_flag: &AtomicBool) -> Result<(), FetchError> {
    if total.is_zero() {
        return Ok(());
    }
    let step = Duration::from_millis(STOP_POLL_INTERVAL_MS);
    let deadline = tokio::time::Instant::now() + total;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            return Err(FetchError::Stopped);
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(());
        }
        let remaining = deadline - now;
        tokio::time::sleep(remaining.min(step)).await;
    }
}

// ---- 编排函数 ---------------------------------------------------------------

/// 以「正常模式 → 风控冷却 → 极速模式」的顺序编排一次抓取。
///
/// 入参：
/// - `normal_fn`：执行正常模式抓取，返回 `NormalOutcome<T>`
/// - `risk_fn`：执行极速模式抓取，返回 `RiskOutcome<T>`
/// - `key_of`：订单去重 key（通常是 `order_id` 克隆）
/// - `stop_flag`：用户停止信号
/// - `on_progress`：进度推送
///
/// 语义：
/// - normal_fn 直接成功 → 返回结果，无 warning
/// - normal_fn 风控 → 冷却 60s → risk_fn 尝试：
///     - risk_fn 成功 → 合并去重 + warning「已降级到极速模式」
///     - risk_fn 失败但已有 partial → 合并去重 + 两条 warning
///     - risk_fn 失败且 partial 为空 → `FetchError::Other("平台风控持续触发...")`
/// - normal_fn Fatal / risk_fn Fatal → 直接上抛 `FetchError::Other(msg)`
/// - 冷却期间被 stop_flag 中断 → `FetchError::Stopped`
pub async fn run_with_risk_fallback<T, K, KF, N, Nfut, R, Rfut>(
    normal_fn: N,
    risk_fn: R,
    key_of: KF,
    stop_flag: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<FallbackOutcome<T>, FetchError>
where
    K: Eq + Hash,
    KF: Fn(&T) -> K,
    N: FnOnce() -> Nfut,
    Nfut: Future<Output = NormalOutcome<T>>,
    R: FnOnce() -> Rfut,
    Rfut: Future<Output = RiskOutcome<T>>,
{
    match normal_fn().await {
        NormalOutcome::Ok(orders) => Ok(FallbackOutcome {
            orders,
            warnings: vec![],
        }),
        NormalOutcome::Fatal(msg) => Err(FetchError::Other(msg)),
        NormalOutcome::RiskControl { partial } => {
            cool_down_with_countdown(
                RISK_COOLDOWN_SECS,
                RISK_COOLDOWN_STEP_SECS,
                stop_flag,
                on_progress,
            )
            .await?;

            let warning = WARNING_DEGRADED.to_string();
            match risk_fn().await {
                RiskOutcome::Ok(risk_orders) => {
                    let merged =
                        deduplicate_by(partial.into_iter().chain(risk_orders).collect(), |item| {
                            key_of(item)
                        });
                    Ok(FallbackOutcome {
                        orders: merged,
                        warnings: vec![warning],
                    })
                }
                RiskOutcome::Failed {
                    partial: risk_partial,
                } => {
                    let merged =
                        deduplicate_by(partial.into_iter().chain(risk_partial).collect(), |item| {
                            key_of(item)
                        });
                    if merged.is_empty() {
                        Err(FetchError::Other(FATAL_PERSISTENT_RISK.to_string()))
                    } else {
                        Ok(FallbackOutcome {
                            orders: merged,
                            warnings: vec![warning, WARNING_PARTIAL.to_string()],
                        })
                    }
                }
                RiskOutcome::Fatal(msg) => Err(FetchError::Other(msg)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn make_progress(sink: Arc<Mutex<Vec<String>>>) -> ProgressCallback {
        Arc::new(move |msg| sink.lock().unwrap().push(msg))
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Order {
        id: u32,
        tag: &'static str,
    }

    fn order_key(order: &Order) -> u32 {
        order.id
    }

    // --- deduplicate_by ---

    #[test]
    fn deduplicate_preserves_first_occurrence_and_order() {
        let input = vec![
            Order { id: 1, tag: "a" },
            Order { id: 2, tag: "b" },
            Order { id: 1, tag: "c" }, // 重复，保留首次
            Order { id: 3, tag: "d" },
            Order { id: 2, tag: "e" }, // 重复
        ];
        let out = deduplicate_by(input, order_key);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].tag, "a");
        assert_eq!(out[1].tag, "b");
        assert_eq!(out[2].tag, "d");
    }

    #[test]
    fn deduplicate_handles_empty_input() {
        let out: Vec<Order> = deduplicate_by(vec![], order_key);
        assert!(out.is_empty());
    }

    // --- cool_down_with_countdown ---

    #[tokio::test(start_paused = true)]
    async fn cooldown_prints_ten_second_countdown_ticks() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        cool_down_with_countdown(60, 10, stop, on_progress)
            .await
            .unwrap();

        let msgs = messages.lock().unwrap();
        assert!(msgs[0].contains("60 秒冷却"));
        // 剩余秒数提示：60, 50, 40, 30, 20, 10（共 6 条）。
        let tick_msgs: Vec<_> = msgs.iter().filter(|m| m.contains("还剩")).collect();
        assert_eq!(tick_msgs.len(), 6);
        assert!(tick_msgs[0].contains("还剩 60"));
        assert!(tick_msgs[1].contains("还剩 50"));
        assert!(tick_msgs[5].contains("还剩 10"));
    }

    #[tokio::test(start_paused = true)]
    async fn cooldown_can_be_interrupted_by_stop_flag() {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let trigger = tokio::spawn(async move {
            tokio::time::advance(Duration::from_millis(50)).await;
            stop_clone.store(true, Ordering::SeqCst);
            tokio::time::advance(Duration::from_secs(120)).await;
        });

        let result = cool_down_with_countdown(60, 10, stop, on_progress).await;
        trigger.await.unwrap();
        assert!(matches!(result, Err(FetchError::Stopped)));
    }

    // --- run_with_risk_fallback ---

    fn orders(range: impl Iterator<Item = u32>, tag: &'static str) -> Vec<Order> {
        range.map(|id| Order { id, tag }).collect()
    }

    #[tokio::test(start_paused = true)]
    async fn normal_mode_success_returns_directly_without_warning() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let out = run_with_risk_fallback(
            || async { NormalOutcome::Ok(orders(1..=3, "n")) },
            || async { panic!("risk_fn 不应被调用") },
            order_key,
            stop,
            on_progress,
        )
        .await
        .unwrap();

        assert_eq!(out.orders.len(), 3);
        assert!(out.warnings.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn normal_fatal_propagates_without_entering_cooldown() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let err = run_with_risk_fallback::<Order, _, _, _, _, _, _>(
            || async { NormalOutcome::Fatal("boom".into()) },
            || async { panic!("不应被调用") },
            order_key,
            stop,
            on_progress,
        )
        .await
        .unwrap_err();

        match err {
            FetchError::Other(msg) => assert_eq!(msg, "boom"),
            other => panic!("预期 Other，实际 {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn risk_control_then_successful_risk_mode_merges_and_warns() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let out = run_with_risk_fallback(
            || async {
                NormalOutcome::RiskControl {
                    partial: orders(1..=3, "n"),
                }
            },
            || async { RiskOutcome::Ok(orders(3..=5, "r")) }, // 3 重复
            order_key,
            stop,
            on_progress,
        )
        .await
        .unwrap();

        assert_eq!(out.orders.len(), 5, "应去重后保留 5 条");
        // 重复的 id=3 保留 normal 的版本（首次出现）。
        let third = out.orders.iter().find(|o| o.id == 3).unwrap();
        assert_eq!(third.tag, "n");
        assert_eq!(out.warnings.len(), 1);
        assert!(out.warnings[0].contains("已自动降级到极速模式"));

        // 冷却阶段必须打印倒计时。
        let msgs = messages.lock().unwrap();
        assert!(msgs.iter().any(|m| m.contains("还剩 60 秒")));
    }

    #[tokio::test(start_paused = true)]
    async fn risk_mode_failure_with_partial_returns_merged_with_two_warnings() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let out = run_with_risk_fallback(
            || async {
                NormalOutcome::RiskControl {
                    partial: orders(1..=2, "n"),
                }
            },
            || async {
                RiskOutcome::Failed {
                    partial: orders(2..=4, "r"),
                }
            },
            order_key,
            stop,
            on_progress,
        )
        .await
        .unwrap();

        assert_eq!(out.orders.len(), 4);
        assert_eq!(out.warnings.len(), 2);
        assert!(out.warnings[0].contains("降级"));
        assert!(out.warnings[1].contains("部分窗口未完成"));
    }

    #[tokio::test(start_paused = true)]
    async fn risk_mode_failure_without_any_partial_returns_fatal() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let err = run_with_risk_fallback::<Order, _, _, _, _, _, _>(
            || async { NormalOutcome::RiskControl { partial: vec![] } },
            || async { RiskOutcome::Failed { partial: vec![] } },
            order_key,
            stop,
            on_progress,
        )
        .await
        .unwrap_err();

        match err {
            FetchError::Other(msg) => assert!(msg.contains("平台风控持续触发")),
            other => panic!("预期 Other，实际 {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn cooldown_stop_interrupts_fallback_pipeline() {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let trigger = tokio::spawn(async move {
            tokio::time::advance(Duration::from_millis(100)).await;
            stop_clone.store(true, Ordering::SeqCst);
            tokio::time::advance(Duration::from_secs(120)).await;
        });

        let result = run_with_risk_fallback::<Order, _, _, _, _, _, _>(
            || async {
                NormalOutcome::RiskControl {
                    partial: orders(1..=2, "n"),
                }
            },
            || async { panic!("stop 后不应进入 risk_fn") },
            order_key,
            stop,
            on_progress,
        )
        .await;

        trigger.await.unwrap();
        assert!(matches!(result, Err(FetchError::Stopped)));
    }

    #[tokio::test(start_paused = true)]
    async fn risk_mode_fatal_propagates_as_other_error() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let err = run_with_risk_fallback::<Order, _, _, _, _, _, _>(
            || async { NormalOutcome::RiskControl { partial: vec![] } },
            || async { RiskOutcome::Fatal("network gone".into()) },
            order_key,
            stop,
            on_progress,
        )
        .await
        .unwrap_err();

        match err {
            FetchError::Other(msg) => assert_eq!(msg, "network gone"),
            other => panic!("预期 Other，实际 {other:?}"),
        }
    }
}
