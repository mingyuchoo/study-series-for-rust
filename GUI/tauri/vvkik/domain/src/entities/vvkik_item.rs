use chrono::{DateTime,
             Utc};
// 단계/상태 enum은 공유 와이어 계약(contracts)이 단일 정의를 갖고,
// 도메인은 이를 재노출한다.
pub use contracts::{ItemKind,
                    ItemStatus};
use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VvkikItem {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 새 항목을 만들 때 호출자가 결정하는 값들. id·status·시각은 엔티티가
/// 부여한다.
#[derive(Debug, Clone, PartialEq)]
pub struct NewVvkikItem {
    pub kind: ItemKind,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub target_value: Option<f64>,
    pub current_value: Option<f64>,
    pub unit: Option<String>,
    pub position: i64,
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
}

impl VvkikItem {
    pub fn new(draft: NewVvkikItem) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            kind: draft.kind,
            parent_id: draft.parent_id,
            title: draft.title.trim().to_string(),
            description: Self::normalize_optional_field(draft.description),
            target_value: draft.target_value,
            current_value: draft.current_value,
            unit: Self::normalize_optional_field(draft.unit),
            position: draft.position,
            status: ItemStatus::Active,
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
            self.description = Self::normalize_optional_field(Some(description));
        }
        if let Some(target_value) = patch.target_value {
            self.target_value = target_value;
        }
        if let Some(current_value) = patch.current_value {
            self.current_value = current_value;
        }
        if let Some(unit) = patch.unit {
            self.unit = Self::normalize_optional_field(Some(unit));
        }
        if let Some(position) = patch.position {
            self.position = position;
        }
        if let Some(status) = patch.status {
            self.status = status;
        }
        self.updated_at = Utc::now();
    }

    pub fn is_kpi(&self) -> bool { self.kind == ItemKind::Kpi }

    fn normalize_optional_field(value: Option<String>) -> Option<String> { value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpiMeasurement {
    pub id: Uuid,
    pub kpi_id: Uuid,
    pub value: f64,
    pub measured_at: DateTime<Utc>,
    pub note: Option<String>,
}

impl KpiMeasurement {
    pub fn new(kpi_id: Uuid, value: f64, note: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            kpi_id,
            value,
            measured_at: Utc::now(),
            note: VvkikItem::normalize_optional_field(note),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_trims_title_and_discards_blank_optional_fields() {
        let item = VvkikItem::new(NewVvkikItem {
            kind: ItemKind::Vision,
            parent_id: None,
            title: "  Build a focused practice  ".to_string(),
            description: Some("  ".to_string()),
            target_value: None,
            current_value: None,
            unit: Some("  sessions  ".to_string()),
            position: 0,
        });

        assert_eq!(item.title, "Build a focused practice");
        assert_eq!(item.description, None);
        assert_eq!(item.unit, Some("sessions".to_string()));
        assert_eq!(item.status, ItemStatus::Active);
    }

    #[test]
    fn update_applies_only_patched_fields() {
        let mut item = VvkikItem::new(NewVvkikItem {
            kind: ItemKind::Value,
            parent_id: None,
            title: "Freedom".to_string(),
            description: Some("First".to_string()),
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
        });

        item.update(ItemPatch {
            title: Some("  Creative freedom  ".to_string()),
            position: Some(3),
            ..ItemPatch::default()
        });

        assert_eq!(item.title, "Creative freedom");
        assert_eq!(item.position, 3);
        assert_eq!(item.description, Some("First".to_string()));
        assert_eq!(item.kind, ItemKind::Value);
    }

    #[test]
    fn item_kind_knows_allowed_hierarchy() {
        assert!(ItemKind::Vision.allows_parent(ItemKind::Value));
        assert!(ItemKind::Kra.allows_parent(ItemKind::Vision));
        assert!(ItemKind::Igt.allows_parent(ItemKind::Kra));
        assert!(ItemKind::Kpi.allows_parent(ItemKind::Igt));
        assert!(!ItemKind::Value.allows_parent(ItemKind::Vision));
    }
}
