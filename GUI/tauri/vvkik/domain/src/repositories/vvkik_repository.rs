use crate::{entities::{KpiMeasurement,
                       VvkikItem},
            errors::DomainError};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait VvkikRepository: Send + Sync {
    async fn create_item(&self, item: VvkikItem) -> Result<VvkikItem, DomainError>;
    async fn get_item_by_id(&self, id: Uuid) -> Result<Option<VvkikItem>, DomainError>;
    async fn list_items(&self) -> Result<Vec<VvkikItem>, DomainError>;
    async fn update_item(&self, item: VvkikItem) -> Result<VvkikItem, DomainError>;
    async fn delete(&self, id: Uuid) -> Result<(), DomainError>;
    async fn search_items(&self, query: &str) -> Result<Vec<VvkikItem>, DomainError>;
    async fn record_kpi_measurement(&self, measurement: KpiMeasurement) -> Result<KpiMeasurement, DomainError>;
    async fn list_kpi_measurements(&self, kpi_id: Uuid) -> Result<Vec<KpiMeasurement>, DomainError>;
}
