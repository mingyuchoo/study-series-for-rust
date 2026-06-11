use super::validation::{validate_kpi_values,
                        validate_parent,
                        validate_title};
use domain::{DomainError,
             IvkikItem,
             IvkikRepository,
             NewIvkikItem};
use std::sync::Arc;

pub struct CreateItemUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl CreateItemUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, draft: NewIvkikItem) -> Result<IvkikItem, DomainError> {
        validate_title(&draft.title)?;
        validate_kpi_values(draft.kind, draft.target_value, draft.current_value, draft.unit.as_deref())?;

        let parent = match draft.parent_id {
            | Some(parent_id) => Some(self.repository.get_item_by_id(parent_id).await?.ok_or(DomainError::ItemNotFound)?),
            | None => None,
        };
        validate_parent(draft.kind, parent.as_ref())?;

        let item = IvkikItem::new(draft);
        self.repository.create_item(item).await
    }
}
