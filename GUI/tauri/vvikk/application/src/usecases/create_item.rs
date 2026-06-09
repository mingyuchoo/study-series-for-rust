use super::validation::{validate_kpi_values,
                        validate_parent,
                        validate_title};
use domain::{DomainError,
             ItemKind,
             VvkikItem,
             VvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateItemUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl CreateItemUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        &self,
        kind: ItemKind,
        parent_id: Option<Uuid>,
        title: String,
        description: Option<String>,
        target_value: Option<f64>,
        current_value: Option<f64>,
        unit: Option<String>,
        position: i64,
    ) -> Result<VvkikItem, DomainError> {
        validate_title(&title)?;
        validate_kpi_values(kind, target_value, current_value, unit.as_deref())?;

        let parent = match parent_id {
            | Some(parent_id) => Some(self.repository.get_item_by_id(parent_id).await?.ok_or(DomainError::ItemNotFound)?),
            | None => None,
        };
        validate_parent(kind, parent.as_ref())?;

        let item = VvkikItem::new(kind, parent_id, title, description, target_value, current_value, unit, position);
        self.repository.create_item(item).await
    }
}
