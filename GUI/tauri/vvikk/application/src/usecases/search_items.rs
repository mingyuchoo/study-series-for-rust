use domain::{DomainError,
             VvkikItem,
             VvkikRepository};
use std::sync::Arc;

pub struct SearchItemsUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl SearchItemsUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, query: &str) -> Result<Vec<VvkikItem>, DomainError> { self.repository.search_items(query).await }
}
