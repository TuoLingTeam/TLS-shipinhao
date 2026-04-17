use tauri::State;

use crate::adapters::http_delivery_gateway::HttpDeliveryGateway;
use crate::commands::license::{authorize_runtime_task, ensure_feature_authorized};
use crate::error::AppError;
use crate::state::AppState;
use api_contracts::LICENSE_TASK_BATCH_DELIVERY;
use desktop_services::delivery_batch_runner::{BatchDeliveryItem, BatchDeliveryRuntimeGuard};
use domain_core::DeliveryUpdateResult;

#[tauri::command(rename_all = "snake_case")]
pub async fn update_delivery(
    state: State<'_, AppState>,
    order_id: String,
    tracking_number: String,
    carrier_code: String,
) -> Result<DeliveryUpdateResult, AppError> {
    ensure_feature_authorized(&state, "发货功能").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_BATCH_DELIVERY).await?;
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile.biz_magic.clone().unwrap_or_default();
    drop(cookie_profile);

    let gateway = HttpDeliveryGateway::new_with_grant(cookie, magic, Some(grant.grant_id));
    let request = domain_core::DeliveryUpdateRequest {
        order_id,
        tracking_number,
        carrier_code,
    };
    tokio::task::spawn_blocking(move || {
        use desktop_services::DeliveryGateway;
        gateway.update_delivery(&request)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn batch_delivery(
    state: State<'_, AppState>,
    items: Vec<BatchDeliveryInput>,
) -> Result<BatchDeliveryOutput, AppError> {
    ensure_feature_authorized(&state, "发货功能").await?;
    let grant = authorize_runtime_task(&state, LICENSE_TASK_BATCH_DELIVERY).await?;
    let cookie_profile = state.cookie_profile.lock().await;
    if cookie_profile.cookie_header.is_empty() {
        return Err(AppError::Message("请先在设置中配置 Cookie".to_string()));
    }
    let cookie = cookie_profile.cookie_header.clone();
    let magic = cookie_profile.biz_magic.clone().unwrap_or_default();
    drop(cookie_profile);

    let batch_items: Vec<BatchDeliveryItem> = items
        .iter()
        .map(|i| BatchDeliveryItem {
            order_id: i.order_id.clone(),
            tracking_number: i.tracking_number.clone(),
        })
        .collect();

    let report = tokio::task::spawn_blocking(move || {
        let mut gateway = HttpDeliveryGateway::new_with_grant(cookie, magic, Some(grant.grant_id));
        let mut guard = StaticGrantGuard { task_type: LICENSE_TASK_BATCH_DELIVERY.to_string() };
        desktop_services::run_batch_delivery_flow(&batch_items, &mut gateway, &mut guard)
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))?
    .map_err(AppError::Internal)?;

    Ok(BatchDeliveryOutput {
        total_count: report.total_count,
        success_count: report.success_count,
        failure_count: report.failure_count,
        fatal_error: report.fatal_error,
    })
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
pub struct BatchDeliveryInput {
    pub order_id: String,
    pub tracking_number: String,
}

#[derive(serde::Serialize)]
pub struct BatchDeliveryOutput {
    pub total_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub fatal_error: Option<String>,
}
