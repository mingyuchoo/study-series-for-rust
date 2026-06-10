//! 트리 화면이 쓰는 순수 로직. 컴포넌트와 분리되어 있어 단위 테스트가
//! 가능하다.

use super::vvkik::{ItemKind,
                   VvkikItem};
use std::collections::HashSet;

/// 순환 참조에 대비한 안전 깊이. 정상 데이터는 5단계를 넘지 않는다.
pub const MAX_TREE_DEPTH: usize = 6;

/// 단계 순서 → 정렬값 → 제목 순으로 정렬한다.
pub fn sort_items(items: &mut [VvkikItem]) {
    items.sort_by(|a, b| {
        a.kind
            .rank()
            .cmp(&b.kind.rank())
            .then(a.position.cmp(&b.position))
            .then(a.title.cmp(&b.title))
    });
}

pub fn sorted_children(parent_id: &str, items: &[VvkikItem]) -> Vec<VvkikItem> {
    let mut children: Vec<VvkikItem> = items.iter().filter(|item| item.parent_id.as_deref() == Some(parent_id)).cloned().collect();
    sort_items(&mut children);
    children
}

/// 트리의 루트: 상위 항목이 없거나, (검색 필터 등으로) 상위 항목이 목록에
/// 없는 항목.
pub fn root_items(items: &[VvkikItem]) -> Vec<VvkikItem> {
    let ids: HashSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
    let mut roots: Vec<VvkikItem> = items
        .iter()
        .filter(|item| match item.parent_id.as_deref() {
            | Some(parent_id) => !ids.contains(parent_id),
            | None => true,
        })
        .cloned()
        .collect();
    sort_items(&mut roots);
    roots
}

/// 상위 항목 경로를 "Value · 제목 › Vision · 제목" 형태로 만든다.
pub fn parent_path(item: &VvkikItem, items: &[VvkikItem]) -> Option<String> {
    let mut segments = Vec::new();
    let mut current = item.parent_id.clone();

    while let Some(parent_id) = current {
        match items.iter().find(|candidate| candidate.id == parent_id) {
            | Some(parent) => {
                segments.push(format!("{} · {}", parent.kind.label(), parent.title));
                current = parent.parent_id.clone();
            },
            | None => {
                segments.push("(연결된 상위 항목 없음)".to_string());
                break;
            },
        }
        if segments.len() >= MAX_TREE_DEPTH {
            break;
        }
    }

    if segments.is_empty() {
        None
    } else {
        segments.reverse();
        Some(segments.join(" › "))
    }
}

pub fn progress_text(item: &VvkikItem) -> Option<String> {
    if item.kind != ItemKind::Kpi {
        return None;
    }

    let format_number = |value: f64| {
        if value.fract() == 0.0 {
            format!("{}", value as i64)
        } else {
            value.to_string()
        }
    };

    match (item.current_value, item.target_value, item.unit.as_deref()) {
        | (Some(current), Some(target), Some(unit)) => Some(format!("{} / {} {unit}", format_number(current), format_number(target))),
        | (Some(current), Some(target), None) => Some(format!("{} / {}", format_number(current), format_number(target))),
        | (Some(current), None, Some(unit)) => Some(format!("{} {unit}", format_number(current))),
        | (Some(current), None, None) => Some(format_number(current)),
        | _ => None,
    }
}

/// KPI 달성률(0~100). 목표값이 있을 때만 의미가 있다.
pub fn kpi_percent(item: &VvkikItem) -> Option<i64> {
    if item.kind != ItemKind::Kpi {
        return None;
    }

    match (item.current_value, item.target_value) {
        | (Some(current), Some(target)) if target > 0.0 => Some((current / target * 100.0).clamp(0.0, 100.0).round() as i64),
        | _ => None,
    }
}

/// 기본 펼침 규칙: Value와 Vision까지는 펼쳐서 뼈대를 보여 주고,
/// 그 아래(KRA·IGT)는 접어 둔다.
pub fn default_open(kind: ItemKind) -> bool { matches!(kind, ItemKind::Value | ItemKind::Vision) }

pub fn count_descendants(id: &str, items: &[VvkikItem]) -> usize {
    items
        .iter()
        .filter(|item| item.parent_id.as_deref() == Some(id))
        .map(|child| 1 + count_descendants(&child.id, items))
        .sum()
}

pub fn has_children(id: &str, items: &[VvkikItem]) -> bool { items.iter().any(|item| item.parent_id.as_deref() == Some(id)) }

/// 드래그한 항목을 `target` 위에 놓아 상위를 바꿀 수 있는지.
/// 자기 자신·현재 부모·계층 규칙에 어긋나는 단계는 제외한다.
/// (자손은 항상 하위 단계라 계층 규칙만으로 순환이 차단된다.)
pub fn is_valid_drop(dragged: &VvkikItem, target: &VvkikItem) -> bool {
    dragged.id != target.id
        && dragged.parent_id.as_deref() != Some(target.id.as_str())
        && dragged.kind.allowed_parent_kinds().contains(&target.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ItemStatus;

    fn item(id: &str, kind: ItemKind, parent_id: Option<&str>, position: i64, title: &str) -> VvkikItem {
        VvkikItem {
            id: id.to_string(),
            kind,
            parent_id: parent_id.map(str::to_string),
            title: title.to_string(),
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position,
            status: ItemStatus::Active,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn sample() -> Vec<VvkikItem> {
        vec![
            item("v1", ItemKind::Value, None, 0, "경제적 자유"),
            item("vi1", ItemKind::Vision, Some("v1"), 0, "지식기업"),
            item("k1", ItemKind::Kra, Some("vi1"), 0, "온라인 강의"),
            item("i1", ItemKind::Igt, Some("k1"), 0, "강의 제작"),
            item("i2", ItemKind::Igt, Some("k1"), 1, "출강 영업"),
            item("p1", ItemKind::Kpi, Some("i1"), 0, "월 매출"),
            item("orphan", ItemKind::Kra, Some("missing"), 0, "고아 항목"),
        ]
    }

    #[test]
    fn roots_include_parentless_and_orphans() {
        let items = sample();
        let roots = root_items(&items);
        let ids: Vec<&str> = roots.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["v1", "orphan"]);
    }

    #[test]
    fn children_sort_by_kind_rank_then_position_then_title() {
        let items = vec![
            item("v1", ItemKind::Value, None, 0, "Value"),
            item("k1", ItemKind::Kra, Some("vi1"), 1, "나중"),
            item("k2", ItemKind::Kra, Some("vi1"), 0, "먼저"),
            item("i1", ItemKind::Igt, Some("vi1"), 0, "다른 단계"),
        ];
        let children = sorted_children("vi1", &items);
        let ids: Vec<&str> = children.iter().map(|item| item.id.as_str()).collect();
        // KRA(rank 2)가 IGT(rank 3)보다 먼저, KRA끼리는 position 순.
        assert_eq!(ids, vec!["k2", "k1", "i1"]);
    }

    #[test]
    fn parent_path_walks_up_to_the_root() {
        let items = sample();
        let kpi = items.iter().find(|item| item.id == "p1").unwrap();
        assert_eq!(
            parent_path(kpi, &items).unwrap(),
            "Value · 경제적 자유 › Vision · 지식기업 › KRA · 온라인 강의 › IGT · 강의 제작"
        );
    }

    #[test]
    fn parent_path_marks_missing_parent() {
        let items = sample();
        let orphan = items.iter().find(|item| item.id == "orphan").unwrap();
        assert_eq!(parent_path(orphan, &items).unwrap(), "(연결된 상위 항목 없음)");
    }

    #[test]
    fn parent_path_is_none_for_roots() {
        let items = sample();
        let root = items.iter().find(|item| item.id == "v1").unwrap();
        assert_eq!(parent_path(root, &items), None);
    }

    #[test]
    fn descendants_count_recursively() {
        let items = sample();
        assert_eq!(count_descendants("v1", &items), 5);
        assert_eq!(count_descendants("k1", &items), 3);
        assert_eq!(count_descendants("p1", &items), 0);
        assert!(has_children("v1", &items));
        assert!(!has_children("p1", &items));
    }

    #[test]
    fn kpi_percent_clamps_and_rounds() {
        let mut kpi = item("p", ItemKind::Kpi, None, 0, "KPI");
        kpi.current_value = Some(8_000_000.0);
        kpi.target_value = Some(25_000_000.0);
        assert_eq!(kpi_percent(&kpi), Some(32));

        kpi.current_value = Some(40.0);
        kpi.target_value = Some(30.0);
        assert_eq!(kpi_percent(&kpi), Some(100));

        kpi.target_value = None;
        assert_eq!(kpi_percent(&kpi), None);

        let not_kpi = item("v", ItemKind::Value, None, 0, "Value");
        assert_eq!(kpi_percent(&not_kpi), None);
    }

    #[test]
    fn progress_text_formats_values() {
        let mut kpi = item("p", ItemKind::Kpi, None, 0, "KPI");
        kpi.current_value = Some(18.0);
        kpi.target_value = Some(30.0);
        kpi.unit = Some("km".to_string());
        assert_eq!(progress_text(&kpi).unwrap(), "18 / 30 km");

        kpi.unit = None;
        assert_eq!(progress_text(&kpi).unwrap(), "18 / 30");

        kpi.current_value = None;
        assert_eq!(progress_text(&kpi), None);
    }

    #[test]
    fn drop_targets_follow_hierarchy_rules() {
        let items = sample();
        let find = |id: &str| items.iter().find(|item| item.id == id).unwrap();

        let igt = find("i1"); // 현재 부모: k1
        assert!(is_valid_drop(igt, find("orphan")), "다른 KRA로는 이동 가능");
        assert!(!is_valid_drop(igt, find("k1")), "현재 부모로는 이동 불가(무의미)");
        assert!(!is_valid_drop(igt, find("vi1")), "Vision은 IGT의 부모가 될 수 없음");
        assert!(!is_valid_drop(igt, igt), "자기 자신 불가");

        let kpi = find("p1"); // 현재 부모: i1
        assert!(is_valid_drop(kpi, find("i2")), "KPI는 다른 IGT 아래로 이동 가능");
        assert!(!is_valid_drop(kpi, find("i1")), "현재 부모 제외");
        assert!(!is_valid_drop(kpi, find("k1")), "KRA는 더 이상 KPI의 부모가 될 수 없음");
        assert!(!is_valid_drop(kpi, find("v1")), "Value는 KPI의 부모가 될 수 없음");
    }

    #[test]
    fn default_open_covers_value_and_vision_only() {
        assert!(default_open(ItemKind::Value));
        assert!(default_open(ItemKind::Vision));
        assert!(!default_open(ItemKind::Kra));
        assert!(!default_open(ItemKind::Igt));
        assert!(!default_open(ItemKind::Kpi));
    }
}
