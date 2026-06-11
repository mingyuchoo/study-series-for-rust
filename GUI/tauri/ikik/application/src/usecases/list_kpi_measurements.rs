use domain::{DomainError,
             IkikRepository,
             ItemKind,
             KpiMeasurement};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListKpiMeasurementsUseCase {
    repository: Arc<dyn IkikRepository>,
}

impl ListKpiMeasurementsUseCase {
    pub fn new(repository: Arc<dyn IkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// Key Performance Indicator의 측정 기록을 최신순으로 돌려준다.
    pub async fn execute(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError> {
        let kpi = self.repository.get_item_by_id(kpi_id).await?.ok_or(DomainError::ItemNotFound)?;
        if kpi.kind != ItemKind::Kpi {
            return Err(DomainError::InvalidIkikData(
                "Key Performance Indicator 항목의 측정 기록만 조회할 수 있습니다.".to_string(),
            ));
        }

        self.repository.list_kpi_measurements(kpi_id).await
    }
}
