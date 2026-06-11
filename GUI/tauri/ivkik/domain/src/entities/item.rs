use super::{ItemKind,
            ItemStatus,
            KpiAggregation};
use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IvkikItem {
    pub id: Uuid,
    pub kind: ItemKind,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: i64,
    pub status: ItemStatus,
    pub aggregation: KpiAggregation,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 새 항목을 만들 때 호출자가 결정하는 값들. id·status·시각은 엔티티가
/// 부여한다.
#[derive(Debug, Clone, PartialEq)]
pub struct NewIvkikItem {
    pub kind: ItemKind,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: i64,
    pub aggregation: KpiAggregation,
}

/// 부분 수정. `None`은 "변경하지 않음", `Some(None)`은 "값을 비움"이다.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemPatch {
    pub kind: Option<ItemKind>,
    pub parent_id: Option<Option<Uuid>>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub target_value: Option<Option<f64>>,
    pub current_value: Option<Option<f64>>,
    pub unit: Option<String>,
    pub position: Option<i64>,
    pub status: Option<ItemStatus>,
    pub aggregation: Option<KpiAggregation>,
}

impl IvkikItem {
    pub fn new(draft: NewIvkikItem) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            kind: draft.kind,
            parent_id: draft.parent_id,
            title: draft.title.trim().to_string(),
            description: normalize_optional_field(draft.description),
            target_value: draft.target_value,
            current_value: draft.current_value,
            unit: normalize_optional_field(draft.unit),
            position: draft.position,
            status: ItemStatus::Active,
            aggregation: draft.aggregation,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update(&mut self, patch: ItemPatch) {
        if let Some(kind) = patch.kind {
            self.kind = kind;
        }
        if let Some(parent_id) = patch.parent_id {
            self.parent_id = parent_id;
        }
        if let Some(title) = patch.title {
            self.title = title.trim().to_string();
        }
        if let Some(description) = patch.description {
            self.description = normalize_optional_field(Some(description));
        }
        if let Some(target_value) = patch.target_value {
            self.target_value = target_value;
        }
        if let Some(current_value) = patch.current_value {
            self.current_value = current_value;
        }
        if let Some(unit) = patch.unit {
            self.unit = normalize_optional_field(Some(unit));
        }
        if let Some(position) = patch.position {
            self.position = position;
        }
        if let Some(status) = patch.status {
            self.status = status;
        }
        if let Some(aggregation) = patch.aggregation {
            self.aggregation = aggregation;
        }
        self.updated_at = Utc::now();
    }

    pub fn is_kpi(&self) -> bool { self.kind == ItemKind::Kpi }
}

pub(crate) fn normalize_optional_field(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_trims_title_and_discards_blank_optional_fields() {
        let item = IvkikItem::new(NewIvkikItem {
            kind: ItemKind::Vision,
            parent_id: None,
            title: "  Build a focused practice  ".to_string(),
            description: Some("  ".to_string()),
            target_value: None,
            current_value: None,
            unit: Some("  sessions  ".to_string()),
            position: 0,
            aggregation: KpiAggregation::default(),
        });

        assert_eq!(item.title, "Build a focused practice");
        assert_eq!(item.description, None);
        assert_eq!(item.unit, Some("sessions".to_string()));
        assert_eq!(item.status, ItemStatus::Active);
    }

    #[test]
    fn update_applies_only_patched_fields() {
        let mut item = IvkikItem::new(NewIvkikItem {
            kind: ItemKind::Identity,
            parent_id: None,
            title: "Freedom".to_string(),
            description: Some("First".to_string()),
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: KpiAggregation::default(),
        });

        item.update(ItemPatch {
            title: Some("  Creative freedom  ".to_string()),
            position: Some(3),
            ..ItemPatch::default()
        });

        assert_eq!(item.title, "Creative freedom");
        assert_eq!(item.position, 3);
        assert_eq!(item.description, Some("First".to_string()));
        assert_eq!(item.kind, ItemKind::Identity);
    }

    #[test]
    fn item_kind_knows_allowed_hierarchy() {
        assert!(ItemKind::Vision.allows_parent(ItemKind::Identity));
        assert!(ItemKind::Kra.allows_parent(ItemKind::Vision));
        assert!(ItemKind::Igt.allows_parent(ItemKind::Kra));
        assert!(ItemKind::Kpi.allows_parent(ItemKind::Igt));
        assert!(!ItemKind::Identity.allows_parent(ItemKind::Vision));
    }
}
