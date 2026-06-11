use super::recompute::recompute_kpi_current_value;
use domain::{DomainError,
             IvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteKpiMeasurementUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl DeleteKpiMeasurementUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// 측정 기록을 지우고 남은 기록으로 현재값을 다시 집계한다.
    /// 마지막 기록을 지우면 현재값도 비운다.
    pub async fn execute(&self, kpi_id: Uuid, measurement_id: Uuid) -> Result<(), DomainError> {
        self.repository.delete_kpi_measurement(kpi_id, measurement_id).await?;
        recompute_kpi_current_value(self.repository.as_ref(), kpi_id, true).await?;
        Ok(())
    }
}
