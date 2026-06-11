use super::{recompute::recompute_kpi_current_value,
            validation::{validate_kpi_values,
                         validate_parent,
                         validate_title}};
use domain::{DomainError,
             ItemKind,
             ItemPatch,
             IvkikItem,
             IvkikRepository,
             diff_item_revisions};
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateItemUseCase {
    repository: Arc<dyn IvkikRepository>,
}

impl UpdateItemUseCase {
    pub fn new(repository: Arc<dyn IvkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    pub async fn execute(&self, id: Uuid, patch: ItemPatch) -> Result<IvkikItem, DomainError> {
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
                return Err(DomainError::InvalidIvkikData("자기 자신을 상위 항목으로 선택할 수 없습니다.".to_string()));
            },
            | Some(parent_id) => Some(self.repository.get_item_by_id(parent_id).await?.ok_or(DomainError::ItemNotFound)?),
            | None => None,
        };
        validate_parent(next_kind, parent.as_ref())?;

        // 상위 항목이 실제로 바뀔 때만 이전 상위 항목의 제목을 찾아 둔다.
        let parent_changed = next_parent_id != item.parent_id;
        let old_parent_title = match (parent_changed, item.parent_id) {
            | (true, Some(old_parent_id)) => self.repository.get_item_by_id(old_parent_id).await?.map(|parent| parent.title),
            | _ => None,
        };
        let new_parent_title = parent.as_ref().map(|parent| parent.title.clone());

        // 상위 항목은 검증을 거친 확정값으로 항상 덮어쓴다.
        let before = item.clone();
        item.update(ItemPatch {
            parent_id: Some(next_parent_id),
            ..patch
        });
        let updated = self.repository.update_item(item).await?;

        // 무엇이 어떻게 바뀌었는지 변경 이력으로 남긴다.
        let revisions = diff_item_revisions(&before, &updated, old_parent_title, new_parent_title);
        if !revisions.is_empty() {
            self.repository.record_item_revisions(revisions).await?;
        }

        // 측정 기록이 있는 Key Performance Indicator는 현재값이 기록의 집계 결과여야 한다.
        // 집계 방식이 바뀐 경우에도 여기서 따라잡는다.
        if updated.kind == ItemKind::Kpi
            && let Some(rewritten) = recompute_kpi_current_value(self.repository.as_ref(), id, false).await?
        {
            return Ok(rewritten);
        }

        Ok(updated)
    }
}
