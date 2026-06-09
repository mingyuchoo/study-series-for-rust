use super::validation::validate_measurement_value;
use domain::{DomainError,
             ItemKind,
             KpiMeasurement,
             VvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct RecordKpiMeasurementUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl RecordKpiMeasurementUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, kpi_id: Uuid, value: f64, note: Option<String>) -> Result<KpiMeasurement, DomainError> {
        validate_measurement_value(value)?;

        let mut kpi = self.repository.get_item_by_id(kpi_id).await?.ok_or(DomainError::ItemNotFound)?;
        if kpi.kind != ItemKind::Kpi {
            return Err(DomainError::InvalidVvkikData("KPI 항목에만 측정값을 기록할 수 있습니다.".to_string()));
        }

        kpi.update(None, None, None, None, None, Some(Some(value)), None, None, None);
        self.repository.update_item(kpi).await?;

        let measurement = KpiMeasurement::new(kpi_id, value, note);
        self.repository.record_kpi_measurement(measurement).await
    }
}
