use domain::{Contact,
             ContactRepository,
             DomainError};
use std::sync::Arc;

pub struct ListContactsUseCase {
    repository: Arc<dyn ContactRepository>,
}

impl ListContactsUseCase {
    pub fn new(repository: Arc<dyn ContactRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self) -> Result<Vec<Contact>, DomainError> { self.repository.get_all().await }
}
