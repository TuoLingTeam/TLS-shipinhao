use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
use domain_core::DeliveryUpdateResult;

#[tauri::command]
pub async fn update_delivery(
    state: State<'_, AppState>,
    order_id: String,
    tracking_number: String,
    carrier_code: String,
) -> Result<DeliveryUpdateResult, AppError> {
    let request = domain_core::DeliveryUpdateRequest {
        order_id,
        tracking_number,
        carrier_code,
    };
    let services = state.services.clone();
    tokio::task::spawn_blocking(move || services.update_delivery(&request))
        .await
        .map_err(|e| AppError::Message(e.to_string()))?
        .map_err(AppError::Internal)
}

#[tauri::command]
pub async fn batch_delivery(
    items: Vec<BatchDeliveryInput>,
) -> Result<BatchDeliveryOutput, AppError> {
    use desktop_services::delivery_batch_runner::{
        BatchDeliveryGateway, BatchDeliveryItem, BatchDeliveryRuntimeGuard,
    };

    struct NoopGateway;
    impl BatchDeliveryGateway for NoopGateway {
        fn update_single_order(
            &mut self,
            _order_id: &str,
            _tracking_number: &str,
        ) -> anyhow::Result<Option<String>> {
            Ok(None)
        }
    }

    struct NoopGuard;
    impl BatchDeliveryRuntimeGuard for NoopGuard {
        fn authorize(&mut self, _task_type: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn validate_continuity(
            &mut self,
            _task_type: &str,
            _index: usize,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    let batch_items: Vec<BatchDeliveryItem> = items
        .iter()
        .map(|i| BatchDeliveryItem {
            order_id: i.order_id.clone(),
            tracking_number: i.tracking_number.clone(),
        })
        .collect();

    let report = tokio::task::spawn_blocking(move || {
        desktop_services::run_batch_delivery_flow(
            &batch_items,
            &mut NoopGateway,
            &mut NoopGuard,
        )
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
