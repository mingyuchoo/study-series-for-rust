use domain::{DomainError,
             IkikItem,
             ItemRepository};
use std::sync::Arc;

pub struct SearchItemsUseCase {
    repository: Arc<dyn ItemRepository>,
}

impl SearchItemsUseCase {
    pub fn new(repository: Arc<dyn ItemRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, query: &str) -> Result<Vec<IkikItem>, DomainError> { self.repository.search_items(query).await }
}
