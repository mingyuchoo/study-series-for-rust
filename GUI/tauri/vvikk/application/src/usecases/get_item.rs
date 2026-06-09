use domain::{DomainError,
             VvkikItem,
             VvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct GetItemUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl GetItemUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid) -> Result<VvkikItem, DomainError> { self.repository.get_item_by_id(id).await?.ok_or(DomainError::ItemNotFound) }
}
