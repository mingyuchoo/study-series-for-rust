use domain::{DomainError,
             IvkikRepository,
             KpiMeasurement};
use std::sync::Arc;

/// 모든 KPI의 측정 기록을 한 번에 돌려준다. 대시보드의 기록 잔디처럼
/// KPI를 가리지 않고 "기록하는 행위" 자체를 집계하는 화면이 쓴다.
pub struct ListAllKpiMeasurementsUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl ListAllKpiMeasurementsUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self) -> Result<Vec<KpiMeasurement>, DomainError> { self.repository.list_all_kpi_measurements().await }
}
