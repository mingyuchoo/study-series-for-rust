use super::{recompute::recompute_kpi_current_value,
            validation::{validate_due_date,
                         validate_kpi_values,
                         validate_parent,
                         validate_title}};
use domain::{DomainError,
             IkikItem,
             IkikRepository,
             ItemKind,
             ItemPatch,
             ValidationIssue,
             diff_item_revisions};
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateItemUseCase {
    repository: Arc<dyn IkikRepository>,
}

impl UpdateItemUseCase {
    pub fn new(repository: Arc<dyn IkikRepository>) -> Self {
        Self {
            repository,
        }
    }

    /// 항목 수정 트랜잭션: 검증 → 상위 항목 확정 → 적용 → 변경 이력 →
    /// Key Performance Indicator 현재값 재집계.
    pub async fn execute(&self, id: Uuid, patch: ItemPatch) -> Result<IkikItem, DomainError> {
        let mut item = self.repository.get_item_by_id(id).await?.ok_or(DomainError::ItemNotFound)?;
        validate_scalar_fields(&item, &patch)?;

        let next_kind = patch.kind.unwrap_or(item.kind);
        let next_parent_id = patch.parent_id.unwrap_or(item.parent_id);
        let parent = self.resolve_parent(id, next_parent_id).await?;
        validate_parent(next_kind, parent.as_ref())?;

        let (old_parent_title, new_parent_title) = self.parent_titles_for_revision(&item, next_parent_id, parent.as_ref()).await?;

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

        // 측정 기록이 있는 Key Performance Indicator는 현재값이 기록의 집계 결과여야
        // 한다. 집계 방식이 바뀐 경우에도 여기서 따라잡는다.
        if updated.kind == ItemKind::Kpi
            && let Some(rewritten) = recompute_kpi_current_value(self.repository.as_ref(), id, false).await?
        {
            return Ok(rewritten);
        }

        Ok(updated)
    }

    /// 확정된 상위 항목을 불러온다. 자기 자신을 상위로 고르는 것은
    /// 여기서 막는다.
    async fn resolve_parent(&self, id: Uuid, next_parent_id: Option<Uuid>) -> Result<Option<IkikItem>, DomainError> {
        match next_parent_id {
            | Some(parent_id) if parent_id == id => Err(DomainError::Validation(ValidationIssue::SelfParent)),
            | Some(parent_id) => Ok(Some(self.repository.get_item_by_id(parent_id).await?.ok_or(DomainError::ItemNotFound)?)),
            | None => Ok(None),
        }
    }

    /// 변경 이력에 기록할 이전·이후 상위 항목의 제목. 상위 항목이 실제로
    /// 바뀔 때만 이전 상위 항목을 찾아 둔다.
    async fn parent_titles_for_revision(
        &self,
        item: &IkikItem,
        next_parent_id: Option<Uuid>,
        parent: Option<&IkikItem>,
    ) -> Result<(Option<String>, Option<String>), DomainError> {
        let parent_changed = next_parent_id != item.parent_id;
        let old_parent_title = match (parent_changed, item.parent_id) {
            | (true, Some(old_parent_id)) => self.repository.get_item_by_id(old_parent_id).await?.map(|parent| parent.title),
            | _ => None,
        };
        Ok((old_parent_title, parent.map(|parent| parent.title.clone())))
    }
}

/// 패치를 적용했을 때의 값 기준으로 제목·Key Performance Indicator 전용
/// 필드·마감 기한을 검증한다.
fn validate_scalar_fields(item: &IkikItem, patch: &ItemPatch) -> Result<(), DomainError> {
    if let Some(title) = patch.title.as_ref() {
        validate_title(title)?;
    }

    let next_kind = patch.kind.unwrap_or(item.kind);
    let next_target_value = patch.target_value.unwrap_or(item.target_value);
    let next_current_value = patch.current_value.unwrap_or(item.current_value);
    let next_unit = patch.unit.as_deref().or(item.unit.as_deref());
    let next_due_date = patch.due_date.unwrap_or(item.due_date);

    validate_kpi_values(next_kind, next_target_value, next_current_value, next_unit)?;
    validate_due_date(next_kind, next_due_date)
}
