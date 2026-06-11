use super::validation::{validate_due_date,
                        validate_kpi_values,
                        validate_parent,
                        validate_title};
use domain::{DomainError,
             IkikItem,
             ItemRepository,
             NewIkikItem};
use std::sync::Arc;

pub struct CreateItemUseCase {
    repository: Arc<dyn ItemRepository>,
}

impl CreateItemUseCase {
    pub fn new(repository: Arc<dyn ItemRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, draft: NewIkikItem) -> Result<IkikItem, DomainError> {
        validate_title(&draft.title)?;
        validate_kpi_values(draft.kind, draft.target_value, draft.current_value, draft.unit.as_deref())?;
        validate_due_date(draft.kind, draft.due_date)?;

        let parent = match draft.parent_id {
            | Some(parent_id) => Some(self.repository.get_item_by_id(parent_id).await?.ok_or(DomainError::ItemNotFound)?),
            | None => None,
        };
        validate_parent(draft.kind, parent.as_ref())?;

        let item = IkikItem::new(draft);
        self.repository.create_item(item).await
    }
}
