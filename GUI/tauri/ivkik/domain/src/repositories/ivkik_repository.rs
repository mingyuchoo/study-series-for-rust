use crate::{entities::{ItemRevision,
                       IvkikItem,
                       KpiMeasurement},
            errors::DomainError};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait IvkikRepository: Send + Sync {
    async fn create_item(&self, item: IvkikItem) -> Result<IvkikItem, DomainError>;
    async fn get_item_by_id(&self, id: Uuid) -> Result<Option<IvkikItem>, DomainError>;
    async fn list_items(&self) -> Result<Vec<IvkikItem>, DomainError>;
    async fn update_item(&self, item: IvkikItem) -> Result<IvkikItem, DomainError>;
    async fn delete(&self, id: Uuid) -> Result<(), DomainError>;
    async fn search_items(&self, query: &str) -> Result<Vec<IvkikItem>, DomainError>;
    async fn record_kpi_measurement(&self, measurement: KpiMeasurement) -> Result<KpiMeasurement, DomainError>;
    /// 측정 기록을 최신순(측정 시각 내림차순)으로 돌려준다.
    async fn list_kpi_measurements(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError>;
    /// 모든 KPI의 측정 기록을 최신순으로 돌려준다. 대시보드의 기록
    /// 잔디처럼 전체를 집계하는 화면이 쓴다.
    async fn list_all_kpi_measurements(&self) -> Result<Vec<KpiMeasurement>, DomainError>;
    /// `kpi_id`에 속한 측정 기록 하나를 지운다. 다른 KPI의 기록 id를
    /// 넘기면 아무것도 지우지 않는다.
    async fn delete_kpi_measurement(&self, kpi_id: Uuid, measurement_id: Uuid) -> Result<(), DomainError>;
    /// 항목 수정에서 생긴 변경 이력을 남긴다.
    async fn record_item_revisions(&self, revisions: Vec<ItemRevision>) -> Result<(), DomainError>;
    /// 변경 이력을 최신순(변경 시각 내림차순)으로 돌려준다.
    async fn list_item_revisions(&self, item_id: Uuid) -> Result<Vec<ItemRevision>, DomainError>;
}
