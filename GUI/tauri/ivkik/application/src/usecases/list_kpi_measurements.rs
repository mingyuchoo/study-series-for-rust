use domain::{DomainError,
             ItemKind,
             IvkikRepository,
             KpiMeasurement};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListKpiMeasurementsUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl ListKpiMeasurementsUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// KPI의 측정 기록을 최신순으로 돌려준다.
    pub async fn execute(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError> {
        let kpi = self.repository.get_item_by_id(kpi_id).await?.ok_or(DomainError::ItemNotFound)?;
        if kpi.kind != ItemKind::Kpi {
            return Err(DomainError::InvalidIvkikData("KPI 항목의 측정 기록만 조회할 수 있습니다.".to_string()));
        }

        self.repository.list_kpi_measurements(kpi_id).await
    }
}
