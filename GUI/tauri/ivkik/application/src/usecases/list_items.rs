use domain::{DomainError,
             ItemRepository,
             IvkikItem};
use std::sync::Arc;

pub struct ListItemsUseCase {
    repository: Arc<dyn ItemRepository>,
}

impl ListItemsUseCase {
    pub fn new(repository: Arc<dyn ItemRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self) -> Result<Vec<IvkikItem>, DomainError> { self.repository.list_items().await }
}
