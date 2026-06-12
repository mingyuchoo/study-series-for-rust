//! 트리 화면이 쓰는 순수 로직. 컴포넌트와 분리되어 있어 단위 테스트가
//! 가능하다.

use super::{format::format_value,
            ikik::{IkikItem,
                   ItemKind,
                   kind_label}};
use crate::i18n::Lang;
use std::collections::HashSet;

/// 순환 참조에 대비한 안전 깊이. 정상 데이터는 4단계를 넘지 않는다.
pub const MAX_TREE_DEPTH: usize = 5;

/// 단계 순서 → 정렬값 → 제목 순으로 정렬한다.
pub fn sort_items(items: &mut [IkikItem]) {
    items.sort_by(|a, b| a.kind.rank().cmp(&b.kind.rank()).then(a.position.cmp(&b.position)).then(a.title.cmp(&b.title)));
}

pub fn sorted_children(parent_id: &str, items: &[IkikItem]) -> Vec<IkikItem> {
    let mut children: Vec<IkikItem> = items.iter().filter(|item| item.parent_id.as_deref() == Some(parent_id)).cloned().collect();
    sort_items(&mut children);
    children
}

/// 트리의 루트: 상위 항목이 없거나, (검색 필터 등으로) 상위 항목이 목록에
/// 없는 항목.
pub fn root_items(items: &[IkikItem]) -> Vec<IkikItem> {
    let ids: HashSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
    let mut roots: Vec<IkikItem> = items
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

/// 조상 걷기 결과: 루트부터 직계 부모까지(루트 우선)와, 부모 id가
/// 있는데 목록에 없어(검색 필터 등) 걷기가 끊겼는지 여부.
struct AncestorWalk<'a> {
    chain: Vec<&'a IkikItem>,
    broken: bool,
}

/// 조상 순회의 단일 구현. 깊이 가드(순환 방어)와 끊긴 부모 처리를
/// 여기 한 곳에서 결정하고, 경로 표기들은 결과를 가공만 한다.
fn walk_ancestors<'a>(item: &IkikItem, items: &'a [IkikItem]) -> AncestorWalk<'a> {
    let mut chain: Vec<&'a IkikItem> = Vec::new();
    let mut broken = false;
    let mut current = item.parent_id.as_deref();

    while let Some(parent_id) = current {
        match items.iter().find(|candidate| candidate.id == parent_id) {
            | Some(parent) => {
                chain.push(parent);
                current = parent.parent_id.as_deref();
            },
            | None => {
                broken = true;
                break;
            },
        }
        if chain.len() >= MAX_TREE_DEPTH {
            break;
        }
    }

    chain.reverse();
    AncestorWalk {
        chain,
        broken,
    }
}

/// 상위 항목 경로를 "Identity · 제목 › Key Result Area · 제목" 형태로 만든다.
pub fn parent_path(item: &IkikItem, items: &[IkikItem], lang: Lang) -> Option<String> {
    let walk = walk_ancestors(item, items);
    let mut segments: Vec<String> = Vec::with_capacity(walk.chain.len() + 1);
    if walk.broken {
        segments.push(lang.missing_parent().to_string());
    }
    segments.extend(walk.chain.iter().map(|parent| format!("{} · {}", kind_label(parent.kind, lang), parent.title)));

    if segments.is_empty() { None } else { Some(segments.join(" › ")) }
}

/// 루트부터 직계 부모까지의 조상 목록. 브레드크럼처럼 조상으로
/// 이동하는 UI에 쓴다. 끊긴 부모를 만나면 거기서 멈춘다.
pub fn parent_chain(item: &IkikItem, items: &[IkikItem]) -> Vec<IkikItem> { walk_ancestors(item, items).chain.into_iter().cloned().collect() }

/// 표 표시용 짧은 경로: 부모 제목만 모으고, 3단계 이상이면 가운데를
/// 줄여 "최상위 › … › 직계 부모" 형태로 만든다.
pub fn short_parent_path(item: &IkikItem, items: &[IkikItem], lang: Lang) -> Option<String> {
    let walk = walk_ancestors(item, items);
    let mut titles: Vec<String> = Vec::with_capacity(walk.chain.len() + 1);
    if walk.broken {
        titles.push(lang.missing_parent().to_string());
    }
    titles.extend(walk.chain.iter().map(|parent| parent.title.clone()));

    if titles.is_empty() {
        return None;
    }

    if titles.len() > 2 {
        Some(format!("{} › … › {}", titles.first().unwrap(), titles.last().unwrap()))
    } else {
        Some(titles.join(" › "))
    }
}

/// 단계 탭 표의 그룹 헤더 한 줄. 직계 부모(leaf)만 진하게 강조한다.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupHeader {
    pub prefix: String,
    pub leaf: String,
    pub tooltip: String,
}

/// 단계 탭 표시용 행 목록. 경로 → 정렬값 → 제목 순으로 정렬해 같은
/// 가지(같은 Identity·Key Result Area·Income Generating Task)의 항목이 모이게
/// 하고, 경로가 바뀌는 행 앞에만 그룹 헤더를 끼워 넣는다. 최상위뿐인
/// 단계(Identity 등)는 헤더 없이 평평한 목록으로 돌려준다.
pub fn grouped_rows(kind: ItemKind, items: &[IkikItem], lang: Lang) -> Vec<(Option<GroupHeader>, IkikItem)> {
    let mut rows: Vec<(IkikItem, Vec<String>, Option<String>)> = items
        .iter()
        .filter(|item| item.kind == kind)
        .map(|item| {
            let chain: Vec<String> = parent_chain(item, items).iter().map(|parent| parent.title.clone()).collect();
            (item.clone(), chain, parent_path(item, items, lang))
        })
        .collect();
    rows.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.position.cmp(&b.0.position)).then(a.0.title.cmp(&b.0.title)));

    let any_grouped = rows.iter().any(|(_, _, full_path)| full_path.is_some());

    let mut display = Vec::with_capacity(rows.len());
    let mut previous_path: Option<Option<String>> = None;
    for (item, chain, full_path) in rows {
        let header = if any_grouped && previous_path.as_ref() != Some(&full_path) {
            let (prefix, leaf) = match chain.split_last() {
                | Some((leaf, ancestors)) => (ancestors.join(" › "), leaf.clone()),
                | None if item.parent_id.is_some() => (String::new(), lang.missing_parent().to_string()),
                | None => (String::new(), lang.top_level().to_string()),
            };
            let tooltip = full_path.clone().unwrap_or_else(|| lang.top_level().to_string());
            Some(GroupHeader {
                prefix,
                leaf,
                tooltip,
            })
        } else {
            None
        };
        previous_path = Some(full_path);
        display.push((header, item));
    }
    display
}

pub fn progress_text(item: &IkikItem) -> Option<String> {
    if item.kind != ItemKind::Kpi {
        return None;
    }

    match (item.current_value, item.target_value, item.unit.as_deref()) {
        | (Some(current), Some(target), Some(unit)) => Some(format!("{} / {} {unit}", format_value(current), format_value(target))),
        | (Some(current), Some(target), None) => Some(format!("{} / {}", format_value(current), format_value(target))),
        | (Some(current), None, Some(unit)) => Some(format!("{} {unit}", format_value(current))),
        | (Some(current), None, None) => Some(format_value(current)),
        | _ => None,
    }
}

/// Key Performance Indicator 달성률(0~100). 목표값이 있을 때만 의미가 있다.
pub fn kpi_percent(item: &IkikItem) -> Option<i64> {
    if item.kind != ItemKind::Kpi {
        return None;
    }

    match (item.current_value, item.target_value) {
        | (Some(current), Some(target)) if target > 0.0 => Some((current / target * 100.0).clamp(0.0, 100.0).round() as i64),
        | _ => None,
    }
}

/// 기본 펼침 규칙: Identity는 펼쳐서 Key Result Area까지 뼈대를 보여
/// 주고, 그 아래(Key Result Area·Income Generating Task)는 접어 둔다.
pub fn default_open(kind: ItemKind) -> bool { matches!(kind, ItemKind::Identity) }

pub fn count_descendants(id: &str, items: &[IkikItem]) -> usize {
    items
        .iter()
        .filter(|item| item.parent_id.as_deref() == Some(id))
        .map(|child| 1 + count_descendants(&child.id, items))
        .sum()
}

pub fn has_children(id: &str, items: &[IkikItem]) -> bool { items.iter().any(|item| item.parent_id.as_deref() == Some(id)) }

/// 드래그한 항목을 `target` 위에 놓아 상위를 바꿀 수 있는지.
/// 자기 자신·현재 부모·계층 규칙에 어긋나는 단계는 제외한다.
/// (자손은 항상 하위 단계라 계층 규칙만으로 순환이 차단된다.)
pub fn is_valid_drop(dragged: &IkikItem, target: &IkikItem) -> bool {
    dragged.id != target.id && dragged.parent_id.as_deref() != Some(target.id.as_str()) && dragged.kind.allowed_parent_kinds().contains(&target.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ItemStatus;

    fn item(id: &str, kind: ItemKind, parent_id: Option<&str>, position: i64, title: &str) -> IkikItem {
        IkikItem {
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
            aggregation: contracts::KpiAggregation::default(),
            due_date: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn sample() -> Vec<IkikItem> {
        vec![
            item("v1", ItemKind::Identity, None, 0, "경제적 자유"),
            item("k1", ItemKind::Kra, Some("v1"), 0, "온라인 강의"),
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
            item("v1", ItemKind::Identity, None, 0, "Identity"),
            item("k1", ItemKind::Kra, Some("vi1"), 1, "나중"),
            item("k2", ItemKind::Kra, Some("vi1"), 0, "먼저"),
            item("i1", ItemKind::Igt, Some("vi1"), 0, "다른 단계"),
        ];
        let children = sorted_children("vi1", &items);
        let ids: Vec<&str> = children.iter().map(|item| item.id.as_str()).collect();
        // Key Result Area(rank 1)가 Income Generating Task(rank 2)보다 먼저, Key Result
        // Area끼리는 position 순.
        assert_eq!(ids, vec!["k2", "k1", "i1"]);
    }

    #[test]
    fn parent_path_walks_up_to_the_root() {
        let items = sample();
        let kpi = items.iter().find(|item| item.id == "p1").unwrap();
        assert_eq!(
            parent_path(kpi, &items, Lang::Ko).unwrap(),
            "정체성 · 경제적 자유 › 핵심 결과 영역 · 온라인 강의 › 소득 창출 업무 · 강의 제작"
        );
        assert_eq!(
            parent_path(kpi, &items, Lang::En).unwrap(),
            "Identity · 경제적 자유 › Key Result Area · 온라인 강의 › Income Generating Task · 강의 제작"
        );
    }

    #[test]
    fn parent_path_marks_missing_parent() {
        let items = sample();
        let orphan = items.iter().find(|item| item.id == "orphan").unwrap();
        assert_eq!(parent_path(orphan, &items, Lang::Ko).unwrap(), "(연결된 상위 항목 없음)");
    }

    #[test]
    fn parent_chain_lists_ancestors_root_first() {
        let items = sample();
        let find = |id: &str| items.iter().find(|item| item.id == id).unwrap();

        let chain_ids: Vec<String> = parent_chain(find("p1"), &items).iter().map(|item| item.id.clone()).collect();
        assert_eq!(chain_ids, vec!["v1", "k1", "i1"]);

        assert!(parent_chain(find("v1"), &items).is_empty(), "루트는 조상이 없다");
        assert!(parent_chain(find("orphan"), &items).is_empty(), "끊긴 부모에서 멈춘다");
    }

    #[test]
    fn short_parent_path_collapses_middle_segments() {
        let items = sample();
        let find = |id: &str| items.iter().find(|item| item.id == id).unwrap();

        // 3단계 경로는 가운데가 줄어든다.
        assert_eq!(short_parent_path(find("p1"), &items, Lang::Ko).unwrap(), "경제적 자유 › … › 강의 제작");
        // 2단계 이하는 그대로 보여 준다.
        assert_eq!(short_parent_path(find("i1"), &items, Lang::Ko).unwrap(), "경제적 자유 › 온라인 강의");
        assert_eq!(short_parent_path(find("k1"), &items, Lang::Ko).unwrap(), "경제적 자유");
        // 루트와 고아 항목.
        assert_eq!(short_parent_path(find("v1"), &items, Lang::Ko), None);
        assert_eq!(short_parent_path(find("orphan"), &items, Lang::Ko).unwrap(), "(연결된 상위 항목 없음)");
    }

    #[test]
    fn parent_path_is_none_for_roots() {
        let items = sample();
        let root = items.iter().find(|item| item.id == "v1").unwrap();
        assert_eq!(parent_path(root, &items, Lang::Ko), None);
    }

    #[test]
    fn descendants_count_recursively() {
        let items = sample();
        assert_eq!(count_descendants("v1", &items), 4);
        assert_eq!(count_descendants("k1", &items), 3);
        assert_eq!(count_descendants("p1", &items), 0);
        assert!(has_children("v1", &items));
        assert!(!has_children("p1", &items));
    }

    #[test]
    fn grouped_rows_insert_headers_only_where_the_path_changes() {
        let mut items = sample();
        items.push(item("p2", ItemKind::Kpi, Some("i1"), 1, "월 수강생"));
        items.push(item("p3", ItemKind::Kpi, Some("i2"), 0, "월 영업 건수"));

        let rows = grouped_rows(ItemKind::Kpi, &items, Lang::Ko);
        let ids: Vec<&str> = rows.iter().map(|(_, item)| item.id.as_str()).collect();
        assert_eq!(ids, vec!["p1", "p2", "p3"]);

        // 같은 Income Generating Task(i1)의 두 Key Performance Indicator는 한 헤더 아래
        // 묶인다.
        let first = rows[0].0.as_ref().expect("first row should open a group");
        assert_eq!(first.prefix, "경제적 자유 › 온라인 강의");
        assert_eq!(first.leaf, "강의 제작");
        assert!(rows[1].0.is_none(), "same path should not repeat the header");

        // 경로가 바뀌면(i2) 새 헤더가 열린다.
        let third = rows[2].0.as_ref().expect("path change should open a group");
        assert_eq!(third.leaf, "출강 영업");
    }

    #[test]
    fn grouped_rows_stay_flat_for_top_level_kinds() {
        let items = vec![
            item("v1", ItemKind::Identity, None, 0, "경제적 자유"),
            item("v2", ItemKind::Identity, None, 1, "건강과 성장"),
        ];

        let rows = grouped_rows(ItemKind::Identity, &items, Lang::Ko);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(header, _)| header.is_none()), "value tab has no parent paths to group by");
    }

    #[test]
    fn grouped_rows_label_orphans() {
        let items = sample();
        let rows = grouped_rows(ItemKind::Kra, &items, Lang::Ko);

        let orphan_header = rows
            .iter()
            .find(|(_, item)| item.id == "orphan")
            .and_then(|(header, _)| header.as_ref())
            .expect("orphan should open its own group");
        assert_eq!(orphan_header.leaf, "(연결된 상위 항목 없음)");
        assert!(orphan_header.prefix.is_empty());
    }

    #[test]
    fn kpi_percent_clamps_and_rounds() {
        let mut kpi = item("p", ItemKind::Kpi, None, 0, "Key Performance Indicator");
        kpi.current_value = Some(8_000_000.0);
        kpi.target_value = Some(25_000_000.0);
        assert_eq!(kpi_percent(&kpi), Some(32));

        kpi.current_value = Some(40.0);
        kpi.target_value = Some(30.0);
        assert_eq!(kpi_percent(&kpi), Some(100));

        kpi.target_value = None;
        assert_eq!(kpi_percent(&kpi), None);

        let not_kpi = item("v", ItemKind::Identity, None, 0, "Identity");
        assert_eq!(kpi_percent(&not_kpi), None);
    }

    #[test]
    fn progress_text_formats_values() {
        let mut kpi = item("p", ItemKind::Kpi, None, 0, "Key Performance Indicator");
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
        assert!(is_valid_drop(igt, find("orphan")), "다른 Key Result Area로는 이동 가능");
        assert!(!is_valid_drop(igt, find("k1")), "현재 부모로는 이동 불가(무의미)");
        assert!(!is_valid_drop(igt, find("v1")), "Identity는 Income Generating Task의 부모가 될 수 없음");
        assert!(!is_valid_drop(igt, igt), "자기 자신 불가");

        let kpi = find("p1"); // 현재 부모: i1
        assert!(
            is_valid_drop(kpi, find("i2")),
            "Key Performance Indicator는 다른 Income Generating Task 아래로 이동 가능"
        );
        assert!(!is_valid_drop(kpi, find("i1")), "현재 부모 제외");
        assert!(
            !is_valid_drop(kpi, find("k1")),
            "Key Result Area는 더 이상 Key Performance Indicator의 부모가 될 수 없음"
        );
        assert!(!is_valid_drop(kpi, find("v1")), "Identity는 Key Performance Indicator의 부모가 될 수 없음");
    }

    #[test]
    fn default_open_covers_identity_only() {
        assert!(default_open(ItemKind::Identity));
        assert!(!default_open(ItemKind::Kra));
        assert!(!default_open(ItemKind::Igt));
        assert!(!default_open(ItemKind::Kpi));
    }
}
