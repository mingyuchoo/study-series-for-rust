use domain::{DomainError,
             VvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteItemUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl DeleteItemUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid) -> Result<(), DomainError> { self.repository.delete(id).await }
}
