//! 反风控管线集成测试。
//!
//! 对应 M1-06 卡片：把 `order_fetcher::retry_on_rate_limit` 与
//! `order_fetcher_risk::run_with_risk_fallback` 串起来跑，模拟"抓多页 + 限流
//! 重试 + 风控冷却 + 极速降级"的完整链路，确保 CI 每次回归都能在秒级完成。
//!
//! 执行速度：用 `#[tokio::test(start_paused = true)]` 让 tokio runtime 在所有
//! 任务都在 sleep 时自动 advance 虚拟时钟；60 秒冷却实际耗时 < 1 秒。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use desktop_services::order_fetcher::{
    classify_rate_limit, is_risk_control_result, retry_on_rate_limit, FetchError, LimitOutcome,
    ProgressCallback,
};
use desktop_services::order_fetcher_risk::{
    run_with_risk_fallback, FallbackOutcome, NormalOutcome, RiskOutcome,
};
use serde_json::{json, Value};

// ---- Mock 订单 API ----------------------------------------------------------

/// 单次请求的预设响应类型。
#[derive(Clone, Debug)]
enum Response {
    /// HTTP 429，payload 忽略。
    HttpRateLimit,
    /// HTTP 200 + body 是 API 429。
    ApiRateLimit,
    /// HTTP 200 + body 是 code=430 风控。
    RiskControl,
    /// HTTP 200 + body 是 msg 命中风控关键词。
    RiskByMessage(&'static str),
    /// HTTP 200 + 订单页。
    OrdersPage(Vec<u32>),
    /// HTTP 200 + 空页，标记结束。
    Empty,
}

struct MockApi {
    script: Mutex<std::collections::VecDeque<Response>>,
}

impl MockApi {
    fn new(responses: Vec<Response>) -> Self {
        Self {
            script: Mutex::new(responses.into()),
        }
    }

    /// 取下一条预设响应；脚本耗尽视为 Empty，避免测试意外死循环。
    fn next_response(&self) -> Response {
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Response::Empty)
    }
}

fn response_to_pair(r: Response) -> (u16, Option<Value>, Vec<u32>) {
    match r {
        Response::HttpRateLimit => (429, Some(json!({})), vec![]),
        Response::ApiRateLimit => (200, Some(json!({"code": 429, "msg": "too fast"})), vec![]),
        Response::RiskControl => (200, Some(json!({"code": 430, "msg": "风控"})), vec![]),
        Response::RiskByMessage(msg) => (200, Some(json!({"code": 0, "msg": msg})), vec![]),
        Response::OrdersPage(ids) => (
            200,
            Some(
                json!({"code": 0, "orderList": ids.iter().map(|id| json!({"id": id})).collect::<Vec<_>>()}),
            ),
            ids,
        ),
        Response::Empty => (200, Some(json!({"code": 0, "orderList": []})), vec![]),
    }
}

// ---- 业务层：单次请求 → LimitOutcome -----------------------------------------

/// 单次抓页的执行结果。外层 retry 循环据此判断是否重试。
#[derive(Debug)]
enum PageOutcome {
    /// 拿到订单页，可能为空（空页视为结束）。
    Orders(Vec<u32>),
    /// 平台风控，中断翻页并把 partial 带回。
    RiskControl,
}

/// 模拟业务层执行一次带限流重试的分页请求。
///
/// 把 classify_rate_limit + is_risk_control_result 的判定收口到业务层，
/// 把限流视为"请再试"，把风控视为"退出翻页"。
async fn fetch_page_with_retry(
    api: &MockApi,
    page_index: u32,
    stop_flag: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<PageOutcome, FetchError> {
    retry_on_rate_limit(page_index, stop_flag, on_progress, || async {
        let response = api.next_response();
        let (status, body, orders) = response_to_pair(response);
        let body_ref = body.as_ref();
        match classify_rate_limit(status, body_ref) {
            LimitOutcome::Ok(()) => {
                if let Some(payload) = body_ref {
                    if is_risk_control_result(payload) {
                        return Ok(LimitOutcome::Ok(PageOutcome::RiskControl));
                    }
                }
                Ok(LimitOutcome::Ok(PageOutcome::Orders(orders)))
            }
            LimitOutcome::RateLimited { api_level } => Ok(LimitOutcome::RateLimited { api_level }),
        }
    })
    .await
}

// ---- 顶层：抓多页并按风控早停 -----------------------------------------------

async fn run_pages_as_normal(
    api: &MockApi,
    max_pages: u32,
    stop_flag: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> NormalOutcome<u32> {
    let mut collected: Vec<u32> = Vec::new();
    for page in 1..=max_pages {
        match fetch_page_with_retry(api, page, stop_flag.clone(), on_progress.clone()).await {
            Ok(PageOutcome::Orders(ids)) if ids.is_empty() => return NormalOutcome::Ok(collected),
            Ok(PageOutcome::Orders(ids)) => collected.extend(ids),
            Ok(PageOutcome::RiskControl) => {
                return NormalOutcome::RiskControl { partial: collected };
            }
            Err(FetchError::RateLimitExhausted { retries: _ }) => {
                return NormalOutcome::Fatal(format!("第 {page} 页重试耗尽"));
            }
            Err(other) => return NormalOutcome::Fatal(other.to_string()),
        }
    }
    NormalOutcome::Ok(collected)
}

async fn run_pages_as_risk(
    api: &MockApi,
    max_pages: u32,
    stop_flag: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> RiskOutcome<u32> {
    let mut collected: Vec<u32> = Vec::new();
    for page in 1..=max_pages {
        match fetch_page_with_retry(api, page, stop_flag.clone(), on_progress.clone()).await {
            Ok(PageOutcome::Orders(ids)) if ids.is_empty() => return RiskOutcome::Ok(collected),
            Ok(PageOutcome::Orders(ids)) => collected.extend(ids),
            Ok(PageOutcome::RiskControl) => {
                return RiskOutcome::Failed { partial: collected };
            }
            Err(_) => {
                return RiskOutcome::Failed { partial: collected };
            }
        }
    }
    RiskOutcome::Ok(collected)
}

fn make_progress(sink: Arc<Mutex<Vec<String>>>) -> ProgressCallback {
    Arc::new(move |msg| sink.lock().unwrap().push(msg))
}

fn default_key(order: &u32) -> u32 {
    *order
}

// ---- 场景 1: 纯正常 ---------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_returns_all_orders_when_no_risk_or_limit() {
    let normal = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![1, 2, 3]),
        Response::OrdersPage(vec![4, 5]),
        Response::Empty,
    ]));
    let normal_api = normal.clone();
    let risk = Arc::new(MockApi::new(vec![]));

    let stop = Arc::new(AtomicBool::new(false));
    let msgs = Arc::new(Mutex::new(Vec::new()));

    let out: FallbackOutcome<u32> = run_with_risk_fallback(
        || async move {
            run_pages_as_normal(&normal_api, 10, stop.clone(), make_progress(msgs.clone())).await
        },
        || async move {
            run_pages_as_risk(
                &risk,
                10,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap();

    assert_eq!(out.orders, vec![1, 2, 3, 4, 5]);
    assert!(out.warnings.is_empty());
}

// ---- 场景 2: HTTP 429 退避后成功 -------------------------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_recovers_after_two_http_429() {
    let normal = Arc::new(MockApi::new(vec![
        Response::HttpRateLimit,
        Response::HttpRateLimit,
        Response::OrdersPage(vec![10, 11]),
        Response::Empty,
    ]));
    let normal_api = normal.clone();
    let risk = Arc::new(MockApi::new(vec![]));

    let stop = Arc::new(AtomicBool::new(false));
    let normal_msgs = Arc::new(Mutex::new(Vec::new()));
    let normal_msgs_read = normal_msgs.clone();

    let out = run_with_risk_fallback(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                stop.clone(),
                make_progress(normal_msgs.clone()),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap();

    assert_eq!(out.orders, vec![10, 11]);
    let msgs = normal_msgs_read.lock().unwrap();
    let backoff_msgs: Vec<&String> = msgs.iter().filter(|m| m.contains("等待")).collect();
    assert_eq!(backoff_msgs.len(), 2);
    assert!(backoff_msgs[0].contains("2 秒"));
    assert!(backoff_msgs[1].contains("4 秒"));
}

// ---- 场景 3: HTTP 429 连续 4 次 → 耗尽 --------------------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_reports_fatal_when_rate_limit_exhausted() {
    let normal = Arc::new(MockApi::new(vec![
        Response::HttpRateLimit,
        Response::HttpRateLimit,
        Response::HttpRateLimit,
        Response::HttpRateLimit,
    ]));
    let normal_api = normal.clone();
    let risk = Arc::new(MockApi::new(vec![]));

    let err = run_with_risk_fallback::<u32, _, _, _, _, _, _>(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap_err();

    match err {
        FetchError::Other(msg) => assert!(msg.contains("重试耗尽"), "实际: {msg}"),
        other => panic!("预期 Other，实际 {other:?}"),
    }
}

// ---- 场景 4: code=430 风控 → 冷却 → 极速模式成功 ----------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_cools_down_then_risk_mode_succeeds() {
    let normal = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![1, 2]),
        Response::RiskControl,
    ]));
    let risk = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![2, 3, 4]),
        Response::Empty,
    ]));
    let normal_api = normal.clone();
    let risk_api = risk.clone();

    let cooldown_msgs = Arc::new(Mutex::new(Vec::new()));
    let cooldown_msgs_read = cooldown_msgs.clone();

    let out = run_with_risk_fallback(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(cooldown_msgs),
    )
    .await
    .unwrap();

    // 合并去重：1,2 (normal) + 2,3,4 (risk) → 1,2,3,4
    assert_eq!(out.orders, vec![1, 2, 3, 4]);
    assert_eq!(out.warnings.len(), 1);
    assert!(out.warnings[0].contains("降级"));

    let msgs = cooldown_msgs_read.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("还剩 60 秒")));
    assert!(msgs.iter().any(|m| m.contains("还剩 10 秒")));
}

// ---- 场景 5: msg 风控关键词 → 冷却 → 极速也失败但有 partial -----------------

#[tokio::test(start_paused = true)]
async fn pipeline_returns_partial_when_risk_mode_also_hits_risk() {
    let normal = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![100]),
        Response::RiskByMessage("检测到异常行为，请稍后"),
    ]));
    let risk = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![200]),
        Response::RiskByMessage("拒绝访问"),
    ]));
    let normal_api = normal.clone();
    let risk_api = risk.clone();

    let out = run_with_risk_fallback(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap();

    assert_eq!(out.orders, vec![100, 200]);
    assert_eq!(out.warnings.len(), 2);
    assert!(out.warnings[0].contains("降级"));
    assert!(out.warnings[1].contains("部分窗口未完成"));
}

// ---- 场景 6: 混合 HTTP 429 + API 429 + 风控 ---------------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_handles_mixed_http_api_and_risk_signals() {
    // normal: HTTP 429 → API 429 → 订单 → RISK（中断）
    let normal = Arc::new(MockApi::new(vec![
        Response::HttpRateLimit,
        Response::ApiRateLimit,
        Response::OrdersPage(vec![7]),
        Response::RiskControl,
    ]));
    // risk: 纯正常 2 页
    let risk = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![8, 9]),
        Response::Empty,
    ]));
    let normal_api = normal.clone();
    let risk_api = risk.clone();

    let normal_msgs = Arc::new(Mutex::new(Vec::new()));
    let normal_msgs_read = normal_msgs.clone();

    let out = run_with_risk_fallback(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(normal_msgs.clone()),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap();

    assert_eq!(out.orders, vec![7, 8, 9]);
    assert_eq!(out.warnings.len(), 1);

    let msgs = normal_msgs_read.lock().unwrap();
    let api_msg = msgs.iter().find(|m| m.contains("(API)"));
    let http_msg = msgs
        .iter()
        .find(|m| m.contains("等待") && !m.contains("(API)"));
    assert!(api_msg.is_some(), "必须有一条 (API) 级限流提示");
    assert!(http_msg.is_some(), "必须有一条 HTTP 级限流提示");
}

// ---- 场景 7: 双路都没产出数据 → Fatal ---------------------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_escalates_to_fatal_when_no_partial_collected() {
    let normal = Arc::new(MockApi::new(vec![Response::RiskControl]));
    let risk = Arc::new(MockApi::new(vec![Response::RiskControl]));
    let normal_api = normal.clone();
    let risk_api = risk.clone();

    let err = run_with_risk_fallback::<u32, _, _, _, _, _, _>(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap_err();

    match err {
        FetchError::Other(msg) => assert!(msg.contains("平台风控持续触发")),
        other => panic!("预期 Other，实际 {other:?}"),
    }
}

// ---- 场景 8: 抓取途中用户停止 -----------------------------------------------

#[tokio::test(start_paused = true)]
async fn pipeline_respects_stop_flag_during_cooldown() {
    let normal = Arc::new(MockApi::new(vec![
        Response::OrdersPage(vec![1]),
        Response::RiskControl,
    ]));
    let risk = Arc::new(MockApi::new(vec![Response::OrdersPage(vec![2])]));
    let normal_api = normal.clone();
    let risk_api = risk.clone();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let trigger = tokio::spawn(async move {
        tokio::time::advance(std::time::Duration::from_millis(100)).await;
        stop_clone.store(true, Ordering::SeqCst);
        tokio::time::advance(std::time::Duration::from_secs(120)).await;
    });

    let result = run_with_risk_fallback::<u32, _, _, _, _, _, _>(
        || async move {
            run_pages_as_normal(
                &normal_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        || async move {
            run_pages_as_risk(
                &risk_api,
                5,
                Arc::new(AtomicBool::new(false)),
                make_progress(Arc::new(Mutex::new(vec![]))),
            )
            .await
        },
        default_key,
        stop,
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await;

    trigger.await.unwrap();
    assert!(matches!(result, Err(FetchError::Stopped)));
}

// ---- 总结：call_count 断言 --------------------------------------------------

/// 辅助：确认 normal_fn 仅调用一次、risk_fn 也只在需要时调用一次。
#[tokio::test(start_paused = true)]
async fn pipeline_invokes_fns_exactly_once() {
    let normal_calls = Arc::new(AtomicUsize::new(0));
    let risk_calls = Arc::new(AtomicUsize::new(0));

    let normal_calls_inner = normal_calls.clone();
    let risk_calls_inner = risk_calls.clone();

    let _ = run_with_risk_fallback(
        || async move {
            normal_calls_inner.fetch_add(1, Ordering::SeqCst);
            NormalOutcome::RiskControl {
                partial: vec![1u32],
            }
        },
        || async move {
            risk_calls_inner.fetch_add(1, Ordering::SeqCst);
            RiskOutcome::Ok(vec![2u32])
        },
        default_key,
        Arc::new(AtomicBool::new(false)),
        make_progress(Arc::new(Mutex::new(vec![]))),
    )
    .await
    .unwrap();

    assert_eq!(normal_calls.load(Ordering::SeqCst), 1);
    assert_eq!(risk_calls.load(Ordering::SeqCst), 1);
}
