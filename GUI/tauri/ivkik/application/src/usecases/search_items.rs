use domain::{DomainError,
             IvkikItem,
             IvkikRepository};
use std::sync::Arc;

pub struct SearchItemsUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl SearchItemsUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, query: &str) -> Result<Vec<IvkikItem>, DomainError> { self.repository.search_items(query).await }
}
