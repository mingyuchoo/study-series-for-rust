use domain::{DomainError,
             IvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteItemUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl DeleteItemUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid) -> Result<(), DomainError> { self.repository.delete(id).await }
}
