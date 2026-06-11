use domain::{DomainError,
             IvkikItem,
             IvkikRepository};
use std::sync::Arc;

pub struct ListItemsUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl ListItemsUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self) -> Result<Vec<IvkikItem>, DomainError> { self.repository.list_items().await }
}
