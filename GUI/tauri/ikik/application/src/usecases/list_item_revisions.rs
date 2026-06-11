use domain::{DomainError,
             ItemRevision,
             ItemRevisionRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListItemRevisionsUseCase {
    repository: Arc<dyn ItemRevisionRepository>,
}

impl ListItemRevisionsUseCase {
    pub fn new(repository: Arc<dyn ItemRevisionRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, item_id: Uuid) -> Result<Vec<ItemRevision>, DomainError> { self.repository.list_item_revisions(item_id).await }
}
