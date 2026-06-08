use domain::{ContactRepository,
             DomainError};
use std::sync::Arc;
use uuid::Uuid;

pub struct DeleteContactUseCase {
    repository: Arc<dyn ContactRepository>,
}

impl DeleteContactUseCase {
    pub fn new(repository: Arc<dyn ContactRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid) -> Result<(), DomainError> { self.repository.delete(id).await }
}
