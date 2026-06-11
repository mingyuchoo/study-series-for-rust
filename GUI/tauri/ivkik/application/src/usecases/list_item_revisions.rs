use domain::{DomainError,
             ItemRevision,
             IvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListItemRevisionsUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl ListItemRevisionsUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, item_id: Uuid) -> Result<Vec<ItemRevision>, DomainError> { self.repository.list_item_revisions(item_id).await }
}
