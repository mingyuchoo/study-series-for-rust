use domain::{DomainError,
             ItemRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteItemUseCase {
    repository: Arc<dyn ItemRepository>,
}

impl DeleteItemUseCase {
    pub fn new(repository: Arc<dyn ItemRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid) -> Result<(), DomainError> { self.repository.delete(id).await }
}
