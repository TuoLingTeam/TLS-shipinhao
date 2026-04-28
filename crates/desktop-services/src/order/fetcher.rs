//! 订单抓取的反风控工具箱。
//!
//! 对应 PRD §7.1 的可复用「限流 + 重试」核心：所有业务层（order / review /
//! quality_refund）只需把单次 HTTP 请求 + 结果判定封装成一个 `operation`，
//! 就能获得统一的指数退避、用户进度反馈与可中断等待能力。
//!
//! 设计要点：
//! - 「核心是高阶函数」而非具体的 HTTP 客户端，避免把重试逻辑耦合到 reqwest
//!   类型，方便单元测试。
//! - HTTP 层 429 与 API 层 429 合并计数（不超过 `RATE_LIMIT_RETRY_COUNT`），
//!   但进度消息里用 `(API)` 后缀区分，方便用户/日志定位来源。
//! - sleep 使用可中断分段实现，确保用户点「停止」时立刻响应，而不是等
//!   8 秒的最大退避窗口结束。

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

/// 允许的退避重试次数（首次调用之后的额外尝试次数）。
///
/// 总请求次数 = 1 + `RATE_LIMIT_RETRY_COUNT`。当 `RATE_LIMIT_RETRY_COUNT=3`，
/// 退避序列为 2s → 4s → 8s，与 Python 4.3.0 对齐。
pub const RATE_LIMIT_RETRY_COUNT: u32 = 3;

/// 单步轮询期间检查停止标记的间隔，100ms 兼顾响应性与 CPU 占用。
const STOP_POLL_INTERVAL_MS: u64 = 100;

/// 前端进度回调。进度消息应可直接展示给用户。
///
/// 使用 `Arc<dyn Fn ...>` 以便在多 worker 场景下低成本克隆。
pub type ProgressCallback = Arc<dyn Fn(String) + Send + Sync>;

/// 单次 `operation` 执行后的粗粒度结果。
///
/// 之所以不直接返回 `Result`，是因为「限流」不是错误——它是"请再试一次"的
/// 控制信号。业务层把它和普通成功分开，让退避策略可以集中处理。
#[derive(Debug)]
pub enum LimitOutcome<T> {
    /// 成功，带出业务结果。
    Ok(T),
    /// 触发限流。`api_level=true` 表示 JSON body 里的 `code=429`，
    /// 否则为 HTTP 层 429。
    RateLimited { api_level: bool },
}

/// 抓取过程中可能抛出的错误。
///
/// `RateLimitExhausted` 专门把重试次数带出来，方便上层日志定位。
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("持续触发频率限制（已重试 {retries} 次），请稍后再试")]
    RateLimitExhausted { retries: u32 },
    #[error("抓取已被用户中止")]
    Stopped,
    #[error("{0}")]
    Other(String),
}

impl FetchError {
    pub fn other(message: impl Into<String>) -> Self {
        FetchError::Other(message.into())
    }
}

/// 针对单次请求执行限流重试。
///
/// # 行为
///
/// 1. 调用 `operation` 一次；若返回 `LimitOutcome::Ok(value)` 立即返回。
/// 2. 若返回 `LimitOutcome::RateLimited { api_level }`：
///    - 若重试次数已达 `RATE_LIMIT_RETRY_COUNT`，返回 `RateLimitExhausted`。
///    - 否则等待 `2^(attempt+1)` 秒（2/4/8…）并推送进度消息，然后重试。
/// 3. 等待期间若 `stop_flag` 被置位，立刻返回 `Stopped`。
///
/// # 进度消息格式
///
/// - HTTP 层限流：`"第 X 页触发频率限制，等待 Y 秒后重试..."`
/// - API 层限流：`"第 X 页触发频率限制(API)，等待 Y 秒后重试..."`
pub async fn retry_on_rate_limit<T, F, Fut>(
    page_index: u32,
    stop_flag: Arc<AtomicBool>,
    on_progress: ProgressCallback,
    mut operation: F,
) -> Result<T, FetchError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<LimitOutcome<T>, FetchError>>,
{
    let mut attempt: u32 = 0;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            return Err(FetchError::Stopped);
        }

        match operation().await? {
            LimitOutcome::Ok(value) => return Ok(value),
            LimitOutcome::RateLimited { api_level } => {
                if attempt >= RATE_LIMIT_RETRY_COUNT {
                    return Err(FetchError::RateLimitExhausted { retries: attempt });
                }
                let wait_secs = backoff_seconds(attempt);
                let limit_type = if api_level { "(API)" } else { "" };
                on_progress(format!(
                    "第 {page_index} 页触发频率限制{limit_type}，等待 {wait_secs} 秒后重试..."
                ));
                interruptible_sleep(Duration::from_secs(wait_secs), &stop_flag).await?;
                attempt += 1;
            }
        }
    }
}

/// 计算第 `attempt` 次重试应等待的秒数。
///
/// 序列 2, 4, 8, 16 …；保持与 Python 4.3.0 一致，不做抖动，便于回归对比。
pub fn backoff_seconds(attempt: u32) -> u64 {
    2u64.saturating_pow(attempt.saturating_add(1))
}

/// 判断 HTTP 响应状态是否为平台频率限制。
///
/// 用 `u16` 而不是 `reqwest::StatusCode`，避免 `desktop-services` 被迫依赖 reqwest；
/// 业务层通常写：`if is_http_rate_limited(response.status().as_u16()) { ... }`。
pub fn is_http_rate_limited(status_code: u16) -> bool {
    status_code == 429
}

/// 判断 JSON 响应体是否为 API 层频率限制。
///
/// 平台在某些接口上即便 HTTP 返回 200，JSON 里也会带 `code: 429` 或
/// `respStatusCode: 429` 表示被限流。这里把两种字段都覆盖，避免漏检。
///
/// 备注：`msg` 中的"异常行为"/"拒绝访问"被归类为风控（M1-04），不在本函数范围内。
pub fn is_api_rate_limited(payload: &Value) -> bool {
    for field in ["code", "respStatusCode"] {
        if payload.get(field).and_then(Value::as_i64) == Some(429) {
            return true;
        }
    }
    false
}

/// 风控关键字。命中任意一条即视为触发平台风控。
///
/// 关键字选自 Python 4.3.0 线上抓包样本，默认区分大小写（中文语境下无副作用，
/// 英文关键字需在此处同时提供大小写形式）。
const RISK_CONTROL_MESSAGE_MARKERS: &[&str] = &["异常行为", "拒绝访问"];

/// 判断响应是否为平台风控信号（PRD §14.1、PRD §7.1.3）。
///
/// 满足任一条件即判风控：
/// - `code == 430`
/// - `respStatusCode == 430`
/// - `msg` 中包含 `异常行为` 或 `拒绝访问`
///
/// 命中后业务层应跳过重试，直接进入冷却 + 降级流程（M1-05）。
pub fn is_risk_control_result(payload: &Value) -> bool {
    for field in ["code", "respStatusCode"] {
        if payload.get(field).and_then(Value::as_i64) == Some(430) {
            return true;
        }
    }
    let message = payload
        .get("msg")
        .or_else(|| payload.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if message.is_empty() {
        return false;
    }
    RISK_CONTROL_MESSAGE_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
}

/// 业务层辅助：把「HTTP 响应 + JSON payload」映射成 `LimitOutcome`。
///
/// 使用示例（业务层）：
/// ```ignore
/// let response = client.send().await?;
/// let status = response.status().as_u16();
/// if is_http_rate_limited(status) {
///     return Ok(LimitOutcome::RateLimited { api_level: false });
/// }
/// let payload: Value = response.json().await?;
/// classify_rate_limit(status, Some(&payload))
///     .map(|outcome| match outcome {
///         LimitOutcome::Ok(()) => LimitOutcome::Ok(payload),
///         LimitOutcome::RateLimited { api_level } => LimitOutcome::RateLimited { api_level },
///     })
/// ```
pub fn classify_rate_limit(status_code: u16, payload: Option<&Value>) -> LimitOutcome<()> {
    if is_http_rate_limited(status_code) {
        return LimitOutcome::RateLimited { api_level: false };
    }
    if let Some(body) = payload {
        if is_api_rate_limited(body) {
            return LimitOutcome::RateLimited { api_level: true };
        }
    }
    LimitOutcome::Ok(())
}

/// 可中断的 sleep。每 100ms 检查一次 `stop_flag`。
async fn interruptible_sleep(total: Duration, stop_flag: &AtomicBool) -> Result<(), FetchError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    fn make_progress(sink: Arc<Mutex<Vec<String>>>) -> ProgressCallback {
        Arc::new(move |msg| {
            sink.lock().unwrap().push(msg);
        })
    }

    // --- is_http_rate_limited / is_api_rate_limited / classify_rate_limit ---

    #[test]
    fn http_rate_limit_detection_only_matches_429() {
        assert!(is_http_rate_limited(429));
        assert!(!is_http_rate_limited(200));
        assert!(!is_http_rate_limited(500));
        assert!(!is_http_rate_limited(430));
    }

    #[test]
    fn api_rate_limit_detects_code_field() {
        assert!(is_api_rate_limited(&json!({"code": 429})));
    }

    #[test]
    fn api_rate_limit_detects_resp_status_code_field() {
        assert!(is_api_rate_limited(&json!({"respStatusCode": 429})));
    }

    #[test]
    fn api_rate_limit_ignores_normal_responses() {
        assert!(!is_api_rate_limited(&json!({"code": 0, "msg": "ok"})));
        assert!(!is_api_rate_limited(&json!({"code": 10003})));
        assert!(!is_api_rate_limited(&json!({})));
    }

    #[test]
    fn api_rate_limit_ignores_risk_control_messages() {
        // 风控信号由 M1-04 的 is_risk_control_result 处理，不应被误判为限流。
        assert!(!is_api_rate_limited(&json!({
            "code": 430,
            "msg": "检测到异常行为"
        })));
    }

    // --- is_risk_control_result（M1-04） ---

    #[test]
    fn risk_control_detects_code_430() {
        assert!(is_risk_control_result(&json!({"code": 430})));
    }

    #[test]
    fn risk_control_detects_resp_status_code_430() {
        assert!(is_risk_control_result(&json!({"respStatusCode": 430})));
    }

    #[test]
    fn risk_control_detects_abnormal_behavior_message() {
        assert!(is_risk_control_result(&json!({
            "code": 0,
            "msg": "检测到异常行为，请联系客服"
        })));
    }

    #[test]
    fn risk_control_detects_access_denied_message() {
        assert!(is_risk_control_result(&json!({
            "code": 0,
            "msg": "拒绝访问，请稍后"
        })));
    }

    #[test]
    fn risk_control_accepts_message_field_alias() {
        // 部分接口用 `message` 字段而非 `msg`；都要覆盖。
        assert!(is_risk_control_result(&json!({
            "code": 0,
            "message": "请求被拒绝访问"
        })));
    }

    #[test]
    fn risk_control_rejects_normal_responses() {
        assert!(!is_risk_control_result(&json!({"code": 0, "msg": "ok"})));
        assert!(!is_risk_control_result(&json!({"code": 429})));
        assert!(!is_risk_control_result(&json!({"code": 10003})));
        assert!(!is_risk_control_result(&json!({})));
    }

    #[test]
    fn risk_control_is_separate_from_rate_limit() {
        // 风控与限流需要保持分类互斥，避免上层用错分支。
        let risk = json!({"code": 430, "msg": "拒绝访问"});
        assert!(is_risk_control_result(&risk));
        assert!(!is_api_rate_limited(&risk));

        let rate_limit = json!({"code": 429, "msg": "请稍后再试"});
        assert!(is_api_rate_limited(&rate_limit));
        assert!(!is_risk_control_result(&rate_limit));
    }

    #[test]
    fn classify_rate_limit_prioritizes_http_over_body() {
        let outcome = classify_rate_limit(429, Some(&json!({"code": 0})));
        assert!(matches!(
            outcome,
            LimitOutcome::RateLimited { api_level: false }
        ));
    }

    #[test]
    fn classify_rate_limit_falls_back_to_api_level_when_http_ok() {
        let outcome = classify_rate_limit(200, Some(&json!({"code": 429})));
        assert!(matches!(
            outcome,
            LimitOutcome::RateLimited { api_level: true }
        ));
    }

    #[test]
    fn classify_rate_limit_returns_ok_when_both_normal() {
        let outcome = classify_rate_limit(200, Some(&json!({"code": 0})));
        assert!(matches!(outcome, LimitOutcome::Ok(())));
    }

    #[test]
    fn classify_rate_limit_handles_missing_body() {
        let outcome = classify_rate_limit(200, None);
        assert!(matches!(outcome, LimitOutcome::Ok(())));
    }

    // --- retry_on_rate_limit 与 backoff 行为（M1-02） ---

    #[test]
    fn backoff_sequence_matches_python_reference() {
        assert_eq!(backoff_seconds(0), 2);
        assert_eq!(backoff_seconds(1), 4);
        assert_eq!(backoff_seconds(2), 8);
        assert_eq!(backoff_seconds(3), 16);
    }

    #[tokio::test(start_paused = true)]
    async fn returns_success_on_first_try() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let result = retry_on_rate_limit(7, stop, on_progress, || async {
            Ok(LimitOutcome::Ok(42_u32))
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert!(messages.lock().unwrap().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn succeeds_after_three_rate_limits() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let attempt_counter = Arc::new(AtomicBool::new(false));
        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_inner = call_count.clone();
        let _ = attempt_counter;

        let result = retry_on_rate_limit::<&'static str, _, _>(3, stop, on_progress, move || {
            let count = call_count_inner.clone();
            async move {
                let n = count.fetch_add(1, Ordering::SeqCst);
                if n < 3 {
                    Ok(LimitOutcome::RateLimited { api_level: false })
                } else {
                    Ok(LimitOutcome::Ok("ok"))
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
        let msgs = messages.lock().unwrap();
        assert_eq!(msgs.len(), 3);
        assert!(msgs[0].contains("2 秒"));
        assert!(msgs[1].contains("4 秒"));
        assert!(msgs[2].contains("8 秒"));
        assert!(msgs[0].contains("第 3 页"));
    }

    #[tokio::test(start_paused = true)]
    async fn exhausts_after_four_rate_limits() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let result = retry_on_rate_limit::<(), _, _>(9, stop, on_progress, || async {
            Ok(LimitOutcome::RateLimited { api_level: false })
        })
        .await;

        let err = result.expect_err("应当耗尽重试");
        match err {
            FetchError::RateLimitExhausted { retries } => {
                assert_eq!(retries, RATE_LIMIT_RETRY_COUNT);
            }
            other => panic!("预期 RateLimitExhausted，实际 {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn api_level_rate_limit_marks_message() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();
        let _ = retry_on_rate_limit::<u32, _, _>(5, stop, on_progress, move || {
            let cc = cc.clone();
            async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Ok(LimitOutcome::RateLimited { api_level: true })
                } else {
                    Ok(LimitOutcome::Ok(1))
                }
            }
        })
        .await
        .unwrap();

        let msgs = messages.lock().unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(
            msgs[0].contains("(API)"),
            "API 级限流必须带 (API) 标记: {}",
            msgs[0]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn http_and_api_rate_limits_share_retry_budget() {
        // 混合场景：HTTP 429 → API 429 → HTTP 429 → 成功。
        // 验证「合并计数不超过 3 次」：4 次尝试中前 3 次限流仍能成功。
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();
        let result = retry_on_rate_limit::<u32, _, _>(11, stop, on_progress, move || {
            let cc = cc.clone();
            async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                match n {
                    0 => Ok(LimitOutcome::RateLimited { api_level: false }),
                    1 => Ok(LimitOutcome::RateLimited { api_level: true }),
                    2 => Ok(LimitOutcome::RateLimited { api_level: false }),
                    _ => Ok(LimitOutcome::Ok(77)),
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 77);
        let msgs = messages.lock().unwrap();
        assert_eq!(msgs.len(), 3);
        assert!(msgs[0].contains("第 11 页"));
        assert!(!msgs[0].contains("(API)"));
        assert!(msgs[1].contains("(API)"));
        assert!(!msgs[2].contains("(API)"));
        // 时长依次递增：HTTP 429 与 API 429 共享 attempt 计数。
        assert!(msgs[0].contains("2 秒"));
        assert!(msgs[1].contains("4 秒"));
        assert!(msgs[2].contains("8 秒"));
    }

    #[tokio::test(start_paused = true)]
    async fn stops_immediately_when_flag_set_before_first_call() {
        let stop = Arc::new(AtomicBool::new(true));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();
        let result = retry_on_rate_limit::<(), _, _>(1, stop, on_progress, move || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(LimitOutcome::Ok(()))
            }
        })
        .await;

        assert!(matches!(result, Err(FetchError::Stopped)));
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn stops_during_backoff_sleep() {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = stop.clone();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let cc = call_count.clone();

        // 在启动 sleep 期间翻转 stop_flag：手动推进 Tokio 暂停时钟 50ms 后置位。
        let driver = tokio::spawn(async move {
            tokio::time::advance(Duration::from_millis(50)).await;
            stop_clone.store(true, Ordering::SeqCst);
            tokio::time::advance(Duration::from_secs(2)).await;
        });

        let result = retry_on_rate_limit::<(), _, _>(1, stop, on_progress, move || {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(LimitOutcome::RateLimited { api_level: false })
            }
        })
        .await;

        driver.await.unwrap();
        assert!(matches!(result, Err(FetchError::Stopped)));
        // 至少发生过一次调用（触发了第一次退避），但不会耗尽 3 次重试。
        let calls = call_count.load(Ordering::SeqCst);
        assert!((1..=RATE_LIMIT_RETRY_COUNT).contains(&calls));
    }

    #[tokio::test(start_paused = true)]
    async fn operation_error_is_propagated() {
        let stop = Arc::new(AtomicBool::new(false));
        let messages = Arc::new(Mutex::new(Vec::new()));
        let on_progress = make_progress(messages.clone());

        let result = retry_on_rate_limit::<u32, _, _>(1, stop, on_progress, || async {
            Err(FetchError::other("custom failure"))
        })
        .await;

        match result {
            Err(FetchError::Other(msg)) => assert_eq!(msg, "custom failure"),
            other => panic!("期望 Other，实际 {other:?}"),
        }
    }
}
