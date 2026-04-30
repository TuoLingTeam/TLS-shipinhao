use super::update::is_non_retryable_delivery_error;
use serde::{Deserialize, Serialize};

pub const BATCH_DELIVERY_TASK_TYPE: &str = "batch_delivery";
pub const BATCH_DELIVERY_CONTINUITY_STEP: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeliveryItem {
    pub order_id: String,
    pub tracking_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BatchDeliveryStepStatus {
    #[default]
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeliveryStepResult {
    pub index: usize,
    pub order_id: String,
    pub tracking_number: String,
    pub status: BatchDeliveryStepStatus,
    #[serde(default = "default_retryable")]
    pub retryable: bool,
    pub old_waybill: Option<String>,
    pub error_message: Option<String>,
}

fn default_retryable() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeliveryReport {
    pub success_count: usize,
    pub failure_count: usize,
    pub total_count: usize,
    pub stopped: bool,
    #[serde(default)]
    pub fatal_error: Option<String>,
    #[serde(default)]
    pub steps: Vec<BatchDeliveryStepResult>,
}

pub trait BatchDeliveryGateway {
    fn update_single_order(
        &mut self,
        order_id: &str,
        tracking_number: &str,
    ) -> anyhow::Result<Option<String>>;
}

pub trait BatchDeliveryRuntimeGuard {
    fn authorize(&mut self, task_type: &str) -> anyhow::Result<()>;
    fn validate_continuity(&mut self, task_type: &str, index: usize) -> anyhow::Result<()>;
}

pub fn run_batch_delivery<G, R>(
    items: &[BatchDeliveryItem],
    gateway: &mut G,
    runtime_guard: &mut R,
) -> BatchDeliveryReport
where
    G: BatchDeliveryGateway,
    R: BatchDeliveryRuntimeGuard,
{
    run_batch_delivery_with_hooks(items, gateway, runtime_guard, |_, _| {}, || false)
}

/// 与 `run_batch_delivery` 行为完全一致，但允许调用方观察每条发货的结果并主动请求取消。
///
/// - `on_step`：每条发货完成后被调用一次，参数为最新步骤结果和截至当前的 `BatchDeliveryReport`；
/// - `should_cancel`：每次循环前被调用，返回 `true` 则提前中止剩余条目，`report.stopped = true`。
pub fn run_batch_delivery_with_hooks<G, R, O, C>(
    items: &[BatchDeliveryItem],
    gateway: &mut G,
    runtime_guard: &mut R,
    mut on_step: O,
    should_cancel: C,
) -> BatchDeliveryReport
where
    G: BatchDeliveryGateway,
    R: BatchDeliveryRuntimeGuard,
    O: FnMut(&BatchDeliveryStepResult, &BatchDeliveryReport),
    C: Fn() -> bool,
{
    let total_count = items.len();
    let mut report = BatchDeliveryReport {
        total_count,
        ..BatchDeliveryReport::default()
    };

    if let Err(error) = runtime_guard.authorize(BATCH_DELIVERY_TASK_TYPE) {
        report.failure_count = total_count;
        report.fatal_error = Some(error.to_string());
        return report;
    }

    for (offset, item) in items.iter().enumerate() {
        let index = offset + 1;
        if should_cancel() {
            report.stopped = true;
            break;
        }
        if index == 1 || offset % BATCH_DELIVERY_CONTINUITY_STEP == 0 {
            if let Err(error) = runtime_guard.validate_continuity(BATCH_DELIVERY_TASK_TYPE, index) {
                report.failure_count = total_count.saturating_sub(report.success_count);
                report.fatal_error = Some(error.to_string());
                break;
            }
        }

        let step = match gateway.update_single_order(&item.order_id, &item.tracking_number) {
            Ok(old_waybill) => {
                report.success_count += 1;
                BatchDeliveryStepResult {
                    index,
                    order_id: item.order_id.clone(),
                    tracking_number: item.tracking_number.clone(),
                    status: BatchDeliveryStepStatus::Success,
                    retryable: false,
                    old_waybill,
                    error_message: None,
                }
            }
            Err(error) => {
                report.failure_count += 1;
                let error_message = error.to_string();
                BatchDeliveryStepResult {
                    index,
                    order_id: item.order_id.clone(),
                    tracking_number: item.tracking_number.clone(),
                    status: BatchDeliveryStepStatus::Failed,
                    retryable: !is_non_retryable_delivery_error(&error_message),
                    old_waybill: None,
                    error_message: Some(error_message),
                }
            }
        };

        report.steps.push(step.clone());
        on_step(&step, &report);
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeGateway {
        results: Vec<anyhow::Result<Option<String>>>,
        calls: Vec<(String, String)>,
    }

    impl BatchDeliveryGateway for FakeGateway {
        fn update_single_order(
            &mut self,
            order_id: &str,
            tracking_number: &str,
        ) -> anyhow::Result<Option<String>> {
            self.calls
                .push((order_id.to_string(), tracking_number.to_string()));
            if self.results.is_empty() {
                Ok(None)
            } else {
                self.results.remove(0)
            }
        }
    }

    struct FakeRuntimeGuard {
        authorize_result: anyhow::Result<()>,
        continuity_results: Vec<anyhow::Result<()>>,
        continuity_calls: Vec<usize>,
    }

    impl Default for FakeRuntimeGuard {
        fn default() -> Self {
            Self {
                authorize_result: Ok(()),
                continuity_results: Vec::new(),
                continuity_calls: Vec::new(),
            }
        }
    }

    impl BatchDeliveryRuntimeGuard for FakeRuntimeGuard {
        fn authorize(&mut self, _task_type: &str) -> anyhow::Result<()> {
            self.authorize_result.take()
        }

        fn validate_continuity(&mut self, _task_type: &str, index: usize) -> anyhow::Result<()> {
            self.continuity_calls.push(index);
            if self.continuity_results.is_empty() {
                Ok(())
            } else {
                self.continuity_results.remove(0)
            }
        }
    }

    trait TakeResult {
        fn take(&mut self) -> anyhow::Result<()>;
    }

    impl TakeResult for anyhow::Result<()> {
        fn take(&mut self) -> anyhow::Result<()> {
            std::mem::replace(self, Ok(()))
        }
    }

    fn item(order_id: &str, tracking_number: &str) -> BatchDeliveryItem {
        BatchDeliveryItem {
            order_id: order_id.into(),
            tracking_number: tracking_number.into(),
        }
    }

    #[test]
    fn authorize_failure_short_circuits_batch() {
        let items = vec![item("o-1", "t-1"), item("o-2", "t-2")];
        let mut gateway = FakeGateway::default();
        let mut guard = FakeRuntimeGuard {
            authorize_result: Err(anyhow::anyhow!("授权租约已失效，请联网后重试。")),
            ..Default::default()
        };
        let report = run_batch_delivery(&items, &mut gateway, &mut guard);
        assert_eq!(report.success_count, 0);
        assert_eq!(report.failure_count, 2);
        assert!(report.fatal_error.unwrap().contains("授权租约已失效"));
        assert!(gateway.calls.is_empty());
    }

    #[test]
    fn batch_collects_step_success_and_failures() {
        let items = vec![item("o-1", "t-1"), item("o-2", "t-2")];
        let mut gateway = FakeGateway {
            results: vec![
                Ok(Some("old-1".into())),
                Err(anyhow::anyhow!("更新物流信息失败")),
            ],
            ..Default::default()
        };
        let mut guard = FakeRuntimeGuard::default();
        let report = run_batch_delivery(&items, &mut gateway, &mut guard);
        assert_eq!(report.success_count, 1);
        assert_eq!(report.failure_count, 1);
        assert_eq!(report.steps.len(), 2);
        assert_eq!(report.steps[0].status, BatchDeliveryStepStatus::Success);
        assert!(!report.steps[0].retryable);
        assert_eq!(report.steps[0].old_waybill.as_deref(), Some("old-1"));
        assert_eq!(report.steps[1].status, BatchDeliveryStepStatus::Failed);
        assert!(report.steps[1].retryable);
        assert!(report.steps[1]
            .error_message
            .as_deref()
            .unwrap()
            .contains("更新物流信息失败"));
    }

    #[test]
    fn confirmed_receipt_failure_is_not_retryable() {
        let items = vec![item("o-1", "SF0001")];
        let mut gateway = FakeGateway {
            results: vec![Err(anyhow::anyhow!(
                "更新物流信息失败：订单已确认收货，不支持修改物流"
            ))],
            ..Default::default()
        };
        let mut guard = FakeRuntimeGuard::default();
        let report = run_batch_delivery(&items, &mut gateway, &mut guard);

        assert_eq!(report.failure_count, 1);
        assert_eq!(report.steps[0].status, BatchDeliveryStepStatus::Failed);
        assert!(!report.steps[0].retryable);
    }

    #[test]
    fn continuity_failure_stops_remaining_steps() {
        let items = (1..=12)
            .map(|index| item(&format!("o-{index}"), &format!("t-{index}")))
            .collect::<Vec<_>>();
        let mut gateway = FakeGateway {
            results: (0..12).map(|_| Ok(None)).collect(),
            ..Default::default()
        };
        let mut guard = FakeRuntimeGuard {
            authorize_result: Ok(()),
            continuity_results: vec![
                Ok(()),
                Err(anyhow::anyhow!("授权租约已失效，请联网后重试。")),
            ],
            ..Default::default()
        };
        let report = run_batch_delivery(&items, &mut gateway, &mut guard);
        assert_eq!(guard.continuity_calls, vec![1, 11]);
        assert_eq!(report.success_count, 10);
        assert_eq!(report.failure_count, 2);
        assert!(report.fatal_error.unwrap().contains("授权租约已失效"));
        assert_eq!(gateway.calls.len(), 10);
    }

    // ---- run_batch_delivery_with_hooks：cancel 与 on_step 回调行为 ------------

    #[test]
    fn hooks_should_cancel_stops_before_dispatching_remaining_items() {
        // 5 条任务，第 3 条开始取消：只执行前两条，剩余条目不派发给 gateway
        let items = (1..=5)
            .map(|index| item(&format!("o-{index}"), &format!("t-{index}")))
            .collect::<Vec<_>>();
        let mut gateway = FakeGateway {
            results: (0..5).map(|_| Ok(None)).collect(),
            ..Default::default()
        };
        let mut guard = FakeRuntimeGuard::default();
        let triggered = std::cell::RefCell::new(0_usize);
        let report = run_batch_delivery_with_hooks(
            &items,
            &mut gateway,
            &mut guard,
            |_step, _progress| {},
            || {
                let mut n = triggered.borrow_mut();
                *n += 1;
                // 第 1、2 次循环前未取消；第 3 次开始返回 true
                *n >= 3
            },
        );
        assert!(report.stopped, "取消后 report.stopped 必须为 true");
        assert_eq!(report.success_count, 2);
        assert_eq!(report.failure_count, 0);
        assert_eq!(report.steps.len(), 2);
        assert_eq!(gateway.calls.len(), 2, "取消后剩余条目不应被派发到 gateway");
    }

    #[test]
    fn hooks_on_step_fires_once_per_completed_item_with_latest_report() {
        let items = vec![item("o-1", "t-1"), item("o-2", "t-2"), item("o-3", "t-3")];
        let mut gateway = FakeGateway {
            results: vec![
                Ok(None),
                Err(anyhow::anyhow!("模拟失败")),
                Ok(Some("old-3".into())),
            ],
            ..Default::default()
        };
        let mut guard = FakeRuntimeGuard::default();
        let snapshots: std::cell::RefCell<Vec<(usize, usize, usize)>> =
            std::cell::RefCell::new(Vec::new());
        let report = run_batch_delivery_with_hooks(
            &items,
            &mut gateway,
            &mut guard,
            |step, progress| {
                snapshots.borrow_mut().push((
                    step.index,
                    progress.success_count,
                    progress.failure_count,
                ));
            },
            || false,
        );
        let snaps = snapshots.borrow().clone();
        assert_eq!(snaps.len(), 3, "3 条应触发 3 次 on_step");
        // 每次回调时 progress 是到当前为止的累计值
        assert_eq!(snaps[0], (1, 1, 0));
        assert_eq!(snaps[1], (2, 1, 1));
        assert_eq!(snaps[2], (3, 2, 1));
        assert!(!report.stopped);
    }

    #[test]
    fn hooks_should_cancel_checked_before_first_item_keeps_report_empty() {
        // 极端场景：第一条循环前 should_cancel 就已 true —— 完整 0 派发
        let items = vec![item("o-1", "t-1")];
        let mut gateway = FakeGateway::default();
        let mut guard = FakeRuntimeGuard::default();
        let report =
            run_batch_delivery_with_hooks(&items, &mut gateway, &mut guard, |_, _| {}, || true);
        assert!(report.stopped);
        assert_eq!(report.success_count, 0);
        assert_eq!(report.failure_count, 0);
        assert!(report.steps.is_empty());
        assert!(gateway.calls.is_empty());
    }
}
