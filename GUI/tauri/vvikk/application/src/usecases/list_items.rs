use domain::{DomainError,
             VvkikItem,
             VvkikRepository};
use std::sync::Arc;

pub struct ListItemsUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl ListItemsUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self) -> Result<Vec<VvkikItem>, DomainError> { self.repository.list_items().await }
}
