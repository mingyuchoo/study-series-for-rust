use super::IvkikItem;
use chrono::{DateTime,
             Utc};
use serde::{Deserialize,
            Serialize};
use uuid::Uuid;

/// 항목 정의가 어떻게 바뀌었는지 남기는 변경 이력 한 건. 필드 하나의
/// 이전→이후 값을 기록한다. `None`은 "값 없음(비어 있음)"이다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemRevision {
    pub id: Uuid,
    pub item_id: Uuid,
    /// 와이어 필드 이름: kind, parent, title, description, target_value,
    /// unit, status, aggregation. 화면 레이블은 프런트엔드가 매긴다.
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_at: DateTime<Utc>,
}

impl ItemRevision {
    pub fn new(item_id: Uuid, field: impl Into<String>, old_value: Option<String>, new_value: Option<String>, changed_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            item_id,
            field: field.into(),
            old_value,
            new_value,
            changed_at,
        }
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

/// 두 스냅숏을 비교해 바뀐 필드만 변경 이력으로 만든다. 측정 기록이
/// 결정하는 `current_value`와 정렬용 `position`은 잡음이라 제외하고,
/// 상위 항목은 id 대신 호출자가 찾아 준 제목으로 기록한다.
pub fn diff_item_revisions(before: &IvkikItem, after: &IvkikItem, old_parent_title: Option<String>, new_parent_title: Option<String>) -> Vec<ItemRevision> {
    let mut revisions = Vec::new();
    let mut push = |field: &str, old_value: Option<String>, new_value: Option<String>| {
        revisions.push(ItemRevision::new(after.id, field, old_value, new_value, after.updated_at));
    };

    if before.kind != after.kind {
        push("kind", Some(before.kind.as_str().to_string()), Some(after.kind.as_str().to_string()));
    }
    if before.parent_id != after.parent_id {
        push("parent", old_parent_title, new_parent_title);
    }
    if before.title != after.title {
        push("title", Some(before.title.clone()), Some(after.title.clone()));
    }
    if before.description != after.description {
        push("description", before.description.clone(), after.description.clone());
    }
    if before.target_value != after.target_value {
        push("target_value", before.target_value.map(format_number), after.target_value.map(format_number));
    }
    if before.unit != after.unit {
        push("unit", before.unit.clone(), after.unit.clone());
    }
    if before.status != after.status {
        push("status", Some(before.status.as_str().to_string()), Some(after.status.as_str().to_string()));
    }
    if before.aggregation != after.aggregation {
        push(
            "aggregation",
            Some(before.aggregation.as_str().to_string()),
            Some(after.aggregation.as_str().to_string()),
        );
    }

    revisions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemKind,
                ItemPatch,
                KpiAggregation,
                NewIvkikItem};

    #[test]
    fn diff_records_only_changed_fields_and_skips_noise() {
        let before = IvkikItem::new(NewIvkikItem {
            kind: ItemKind::Kpi,
            parent_id: None,
            title: "Monthly commits".to_string(),
            description: None,
            target_value: Some(40.0),
            current_value: Some(0.0),
            unit: Some("회".to_string()),
            position: 0,
            aggregation: KpiAggregation::Latest,
        });

        let mut after = before.clone();
        after.update(ItemPatch {
            title: Some("Weekly commits".to_string()),
            target_value: Some(Some(60.0)),
            current_value: Some(Some(12.0)),
            position: Some(7),
            aggregation: Some(KpiAggregation::Sum),
            ..ItemPatch::default()
        });

        let revisions = diff_item_revisions(&before, &after, None, None);
        let fields: Vec<&str> = revisions.iter().map(|revision| revision.field.as_str()).collect();

        // current_value(측정 집계)와 position(정렬)은 기록하지 않는다.
        assert_eq!(fields, vec!["title", "target_value", "aggregation"]);
        let target = revisions.iter().find(|revision| revision.field == "target_value").unwrap();
        assert_eq!(target.old_value.as_deref(), Some("40"));
        assert_eq!(target.new_value.as_deref(), Some("60"));
        assert!(
            revisions
                .iter()
                .all(|revision| revision.item_id == after.id && revision.changed_at == after.updated_at)
        );
    }

    #[test]
    fn diff_records_parent_move_with_titles() {
        let before = IvkikItem::new(NewIvkikItem {
            kind: ItemKind::Kpi,
            parent_id: Some(Uuid::new_v4()),
            title: "KPI".to_string(),
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: KpiAggregation::default(),
        });
        let mut after = before.clone();
        after.update(ItemPatch {
            parent_id: Some(Some(Uuid::new_v4())),
            ..ItemPatch::default()
        });

        let revisions = diff_item_revisions(&before, &after, Some("강의 제작".to_string()), Some("출강 영업".to_string()));

        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions[0].field, "parent");
        assert_eq!(revisions[0].old_value.as_deref(), Some("강의 제작"));
        assert_eq!(revisions[0].new_value.as_deref(), Some("출강 영업"));
    }

    #[test]
    fn diff_returns_empty_when_nothing_changed() {
        let item = IvkikItem::new(NewIvkikItem {
            kind: ItemKind::Identity,
            parent_id: None,
            title: "Freedom".to_string(),
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: 0,
            aggregation: KpiAggregation::default(),
        });

        assert!(diff_item_revisions(&item, &item.clone(), None, None).is_empty());
    }
}
