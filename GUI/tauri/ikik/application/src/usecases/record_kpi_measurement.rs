use super::{recompute::recompute_kpi_current_value,
            validation::validate_measurement_value};
use domain::{DomainError,
             IkikRepository,
             ItemKind,
             KpiMeasurement};
use std::sync::Arc;
use uuid::Uuid;

pub struct RecordKpiMeasurementUseCase {
    repository: Arc<dyn IkikRepository>,
}

impl RecordKpiMeasurementUseCase {
    pub fn new(repository: Arc<dyn IkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// 측정 기록을 추가하고, 전체 기록을 Key Performance Indicator의 집계
    /// 방식(최신값·합계· 평균)대로 취합해 현재값을 갱신한다.
    pub async fn execute(&self, kpi_id: Uuid, value: f64, note: Option<String>) -> Result<KpiMeasurement, DomainError> {
        validate_measurement_value(value)?;

        let kpi = self.repository.get_item_by_id(kpi_id).await?.ok_or(DomainError::ItemNotFound)?;
        if kpi.kind != ItemKind::Kpi {
            return Err(DomainError::Validation(domain::ValidationIssue::MeasurementsRequireKpi));
        }

        let measurement = self.repository.record_kpi_measurement(KpiMeasurement::new(kpi_id, value, note)).await?;
        recompute_kpi_current_value(self.repository.as_ref(), kpi_id, true).await?;

        Ok(measurement)
    }
}
