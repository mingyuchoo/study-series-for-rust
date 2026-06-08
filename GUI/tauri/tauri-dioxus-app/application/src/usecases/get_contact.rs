use domain::{Contact,
             ContactRepository,
             DomainError};
use std::sync::Arc;
use uuid::Uuid;

pub struct GetContactUseCase {
    repository: Arc<dyn ContactRepository>,
}

impl GetContactUseCase {
    pub fn new(repository: Arc<dyn ContactRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid) -> Result<Contact, DomainError> { self.repository.get_by_id(id).await?.ok_or(DomainError::ContactNotFound) }
}
