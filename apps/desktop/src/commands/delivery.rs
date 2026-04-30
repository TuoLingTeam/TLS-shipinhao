use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, State};

use crate::adapters::delivery::HttpDeliveryGateway;
use crate::commands::license::{authorize_runtime_task, ensure_feature_authorized};
use crate::commands::shared::require_cookie_credentials;
use crate::error::AppError;
use crate::state::AppState;
use api_contracts::LICENSE_TASK_BATCH_DELIVERY;
use desktop::domain::DeliveryUpdateResult;
use desktop::services::delivery_batch_runner::{
    run_batch_delivery_with_hooks, BatchDeliveryItem, BatchDeliveryReport,
    BatchDeliveryRuntimeGuard, BatchDeliveryStepResult,
};

#[tauri::command(rename_all = "snake_case")]
pub async fn update_delivery(
    state: State<'_, AppState>,
    order_id: String,
    tracking_number: String,
    carrier_code: String,
) -> Result<DeliveryUpdateResult, AppError> {
    ensure_feature_authorized(&state, "发货功能").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_BATCH_DELIVERY).await?;
    let creds = require_cookie_credentials(&state).await?;

    let gateway =
        HttpDeliveryGateway::new_with_grant(creds.cookie, creds.magic, Some(grant.grant_id));
    let request = desktop::domain::DeliveryUpdateRequest {
        order_id,
        tracking_number,
        carrier_code,
    };
    gateway
        .update_delivery_async(&request)
        .await
        .map_err(AppError::Internal)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn batch_delivery(
    app: AppHandle,
    state: State<'_, AppState>,
    items: Vec<Bdi>,
) -> Result<BatchDeliveryOutput, AppError> {
    ensure_feature_authorized(&state, "发货功能").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_BATCH_DELIVERY).await?;
    let creds = require_cookie_credentials(&state).await?;
    let cookie = creds.cookie;
    let magic = creds.magic;

    let batch_items: Vec<BatchDeliveryItem> = items
        .iter()
        .map(|i| BatchDeliveryItem {
            order_id: i.order_id.clone(),
            tracking_number: i.tracking_number.clone(),
        })
        .collect();

    // 每次开始批量前重置取消标志，避免上一次残留。
    state.batch_delivery_cancel.store(false, Ordering::Relaxed);
    let cancel_flag = state.batch_delivery_cancel.clone();
    let total = batch_items.len();

    // 启动事件：让前端立即切换到进度视图。
    emit_batch_progress(
        &app,
        &BatchDeliveryProgressEvent {
            phase: BatchDeliveryPhase::Started,
            total_count: total,
            success_count: 0,
            failure_count: 0,
            processed_count: 0,
            step: None,
            fatal_error: None,
            stopped: false,
        },
    );

    let app_clone = app.clone();
    let cancel_for_task = cancel_flag.clone();
    let report = tokio::task::spawn_blocking(move || {
        let mut gateway = HttpDeliveryGateway::new_with_grant(cookie, magic, Some(grant.grant_id));
        let mut guard = StaticGrantGuard {
            task_type: LICENSE_TASK_BATCH_DELIVERY.to_string(),
        };

        run_batch_delivery_with_hooks(
            &batch_items,
            &mut gateway,
            &mut guard,
            |step, progress| {
                let processed = progress.success_count + progress.failure_count;
                emit_batch_progress(
                    &app_clone,
                    &BatchDeliveryProgressEvent {
                        phase: BatchDeliveryPhase::Step,
                        total_count: progress.total_count,
                        success_count: progress.success_count,
                        failure_count: progress.failure_count,
                        processed_count: processed,
                        step: Some(step.clone()),
                        fatal_error: None,
                        stopped: false,
                    },
                );
            },
            || cancel_for_task.load(Ordering::Relaxed),
        )
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?;

    emit_batch_progress(
        &app,
        &BatchDeliveryProgressEvent {
            phase: BatchDeliveryPhase::Completed,
            total_count: report.total_count,
            success_count: report.success_count,
            failure_count: report.failure_count,
            processed_count: report.success_count + report.failure_count,
            step: None,
            fatal_error: report.fatal_error.clone(),
            stopped: report.stopped,
        },
    );

    Ok(BatchDeliveryOutput::from_report(report))
}

/// 请求取消当前正在进行的批量发货。已派发的条目会跑完，但剩余条目会被跳过。
#[tauri::command(rename_all = "snake_case")]
pub async fn cancel_batch_delivery(state: State<'_, AppState>) -> Result<bool, AppError> {
    state.batch_delivery_cancel.store(true, Ordering::Relaxed);
    Ok(true)
}

fn emit_batch_progress(app: &AppHandle, event: &BatchDeliveryProgressEvent) {
    let _ = app.emit("batch-delivery-progress", event);
}

struct StaticGrantGuard {
    task_type: String,
}
impl BatchDeliveryRuntimeGuard for StaticGrantGuard {
    fn authorize(&mut self, task_type: &str) -> anyhow::Result<()> {
        if task_type != self.task_type {
            anyhow::bail!("运行时授权任务不匹配：{task_type}");
        }
        Ok(())
    }
    fn validate_continuity(&mut self, _task_type: &str, _index: usize) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(serde::Deserialize)]
pub struct Bdi {
    pub order_id: String,
    pub tracking_number: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BatchDeliveryOutput {
    pub total_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub stopped: bool,
    pub fatal_error: Option<String>,
    /// 逐条发货结果，按提交顺序排列。前端据此渲染失败明细、导出 CSV 与"仅重试失败项"。
    pub steps: Vec<BatchDeliveryStepResult>,
}

impl BatchDeliveryOutput {
    fn from_report(report: BatchDeliveryReport) -> Self {
        Self {
            total_count: report.total_count,
            success_count: report.success_count,
            failure_count: report.failure_count,
            stopped: report.stopped,
            fatal_error: report.fatal_error,
            steps: report.steps,
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum BatchDeliveryPhase {
    Started,
    Step,
    Completed,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
struct BatchDeliveryProgressEvent {
    phase: BatchDeliveryPhase,
    total_count: usize,
    success_count: usize,
    failure_count: usize,
    processed_count: usize,
    step: Option<BatchDeliveryStepResult>,
    fatal_error: Option<String>,
    stopped: bool,
}
