use domain::{Contact,
             ContactRepository,
             DomainError};
use std::sync::Arc;

pub struct SearchContactsUseCase {
    repository: Arc<dyn ContactRepository>,
}

impl SearchContactsUseCase {
    pub fn new(repository: Arc<dyn ContactRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, query: &str) -> Result<Vec<Contact>, DomainError> { self.repository.search(query).await }
}
