use super::validation::{validate_kpi_values,
                        validate_parent,
                        validate_title};
use domain::{DomainError,
             ItemPatch,
             VvkikItem,
             VvkikRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateItemUseCase {
    repository: Arc<dyn VvkikRepository>,
}

impl UpdateItemUseCase {
    pub fn new(repository: Arc<dyn VvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid, patch: ItemPatch) -> Result<VvkikItem, DomainError> {
        if let Some(title) = patch.title.as_ref() {
            validate_title(title)?;
        }

        let mut item = self.repository.get_item_by_id(id).await?.ok_or(DomainError::ItemNotFound)?;
        let next_kind = patch.kind.unwrap_or(item.kind);
        let next_parent_id = patch.parent_id.unwrap_or(item.parent_id);
        let next_target_value = patch.target_value.unwrap_or(item.target_value);
        let next_current_value = patch.current_value.unwrap_or(item.current_value);
        let next_unit = patch.unit.as_deref().or(item.unit.as_deref());

        validate_kpi_values(next_kind, next_target_value, next_current_value, next_unit)?;

        let parent = match next_parent_id {
            | Some(parent_id) if parent_id == id => {
                return Err(DomainError::InvalidVvkikData("자기 자신을 상위 항목으로 선택할 수 없습니다.".to_string()));
            },
            | Some(parent_id) => Some(self.repository.get_item_by_id(parent_id).await?.ok_or(DomainError::ItemNotFound)?),
            | None => None,
        };
        validate_parent(next_kind, parent.as_ref())?;

        // 상위 항목은 검증을 거친 확정값으로 항상 덮어쓴다.
        item.update(ItemPatch {
            parent_id: Some(next_parent_id),
            ..patch
        });
        self.repository.update_item(item).await
    }
}
