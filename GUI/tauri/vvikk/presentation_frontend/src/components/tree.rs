#![allow(non_snake_case)]

use super::{item_form::AddPreset,
            quick_add::{QuickAddData,
                        QuickAddRow}};
use crate::models::{ItemKind,
                    ItemStatus,
                    VvkikItem,
                    kind_description,
                    tree::{MAX_TREE_DEPTH,
                           count_descendants,
                           default_open,
                           has_children,
                           kpi_percent,
                           progress_text,
                           root_items,
                           sorted_children}};
use dioxus::prelude::*;
use std::collections::HashSet;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikTreeViewProps {
    pub items: Vec<VvkikItem>,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
    pub on_quick_add: EventHandler<QuickAddData>,
    pub on_add_child: EventHandler<AddPreset>,
}

/// 전체 구조 탭: 접기/펼치기가 가능한 컴팩트 트리.
pub fn VvkikTreeView(props: VvkikTreeViewProps) -> Element {
    let mut adding_value = use_signal(|| false);
    // 기본 펼침 상태에서 뒤집힌 노드 집합. 펼침 여부 = default_open XOR
    // 포함 여부라서, 항목이 추가·삭제돼도 나머지 노드의 상태가 유지된다.
    let mut toggled = use_signal(HashSet::<String>::new);
    let roots = root_items(&props.items);

    let expand_all = {
        let items = props.items.clone();
        move |_| {
            let flipped: HashSet<String> = items
                .iter()
                .filter(|item| !default_open(item.kind) && has_children(&item.id, &items))
                .map(|item| item.id.clone())
                .collect();
            toggled.set(flipped);
        }
    };

    let collapse_all = {
        let items = props.items.clone();
        move |_| {
            let flipped: HashSet<String> = items
                .iter()
                .filter(|item| default_open(item.kind) && has_children(&item.id, &items))
                .map(|item| item.id.clone())
                .collect();
            toggled.set(flipped);
        }
    };

    rsx! {
        div { class: "vvkik-tree",
            if props.items.is_empty() {
                p { class: "tree-hint",
                    "아직 VVKIK 항목이 없습니다. 아래에서 Value부터 추가해 보세요."
                }
            } else {
                div { class: "tree-toolbar",
                    button { r#type: "button", class: "btn btn-sm btn-outline", onclick: expand_all, "모두 펼치기" }
                    button { r#type: "button", class: "btn btn-sm btn-outline", onclick: collapse_all, "모두 접기" }
                    span { class: "tree-flow", "Value → Vision → KRA → IGT → KPI" }
                }
            }
            for item in roots {
                VvkikTreeNode {
                    item,
                    all_items: props.items.clone(),
                    depth: 0,
                    toggled,
                    on_edit: props.on_edit,
                    on_delete: props.on_delete,
                    on_quick_add: props.on_quick_add,
                    on_add_child: props.on_add_child
                }
            }

            if *adding_value.read() {
                QuickAddRow {
                    kind: ItemKind::Value,
                    parent: None,
                    on_quick_add: props.on_quick_add,
                    on_add_child: props.on_add_child,
                    on_close: move |_| adding_value.set(false)
                }
            } else {
                button {
                    r#type: "button",
                    class: "btn tree-add-root",
                    onclick: move |_| adding_value.set(true),
                    "+ Value 추가"
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct VvkikTreeNodeProps {
    item: VvkikItem,
    all_items: Vec<VvkikItem>,
    depth: usize,
    toggled: Signal<HashSet<String>>,
    on_edit: EventHandler<VvkikItem>,
    on_delete: EventHandler<VvkikItem>,
    on_quick_add: EventHandler<QuickAddData>,
    on_add_child: EventHandler<AddPreset>,
}

fn VvkikTreeNode(props: VvkikTreeNodeProps) -> Element {
    let mut quick_add_kind = use_signal(|| None::<ItemKind>);
    let mut toggled = props.toggled;

    let item = props.item.clone();
    let children = if props.depth >= MAX_TREE_DEPTH {
        Vec::new()
    } else {
        sorted_children(&item.id, &props.all_items)
    };
    let has_kids = !children.is_empty();
    let is_open = default_open(item.kind) != toggled.read().contains(&item.id);
    let child_kinds = item.kind.allowed_child_kinds();
    let progress = progress_text(&item);
    let percent = kpi_percent(&item);
    let descendant_count = count_descendants(&item.id, &props.all_items);

    let toggle = {
        let item_id = item.id.clone();
        move |_| {
            let mut flipped = toggled.write();
            if !flipped.remove(&item_id) {
                flipped.insert(item_id.clone());
            }
        }
    };
    let toggle_from_count = {
        let item_id = item.id.clone();
        move |_| {
            let mut flipped = toggled.write();
            if !flipped.remove(&item_id) {
                flipped.insert(item_id.clone());
            }
        }
    };

    rsx! {
        div { class: "tree-node",
            div { class: "tree-row",
                if has_kids {
                    button {
                        r#type: "button",
                        class: "chevron-btn",
                        aria_expanded: is_open,
                        aria_label: if is_open { "접기" } else { "펼치기" },
                        onclick: toggle,
                        if is_open { "▾" } else { "▸" }
                    }
                } else {
                    span { class: "chevron-spacer" }
                }

                span { class: "row-kind", "{item.kind.label()}" }
                span { class: "row-title", title: "{item.title}", "{item.title}" }

                if let Some(description) = &item.description {
                    if !description.is_empty() {
                        span { class: "row-desc", title: "{description}", "{description}" }
                    }
                }

                if item.status != ItemStatus::Active {
                    span { class: "row-status", "{item.status}" }
                }

                if let Some(progress) = progress {
                    span { class: "row-kpi",
                        if let Some(percent) = percent {
                            span { class: "kpi-track",
                                span { class: "kpi-fill", style: "width: {percent}%;" }
                            }
                        }
                        span { class: "row-kpi-text", "{progress}" }
                    }
                }

                if has_kids && !is_open {
                    button {
                        r#type: "button",
                        class: "row-count",
                        onclick: toggle_from_count,
                        "하위 {descendant_count}"
                    }
                }

                div { class: "row-actions",
                    button {
                        r#type: "button",
                        class: "btn row-btn",
                        onclick: {
                            let item = item.clone();
                            move |_| props.on_edit.call(item.clone())
                        },
                        "수정"
                    }
                    button {
                        r#type: "button",
                        class: "btn row-btn",
                        onclick: {
                            let item = item.clone();
                            move |_| props.on_delete.call(item.clone())
                        },
                        "삭제"
                    }
                    for child_kind in child_kinds.iter().copied() {
                        button {
                            r#type: "button",
                            class: "btn row-btn",
                            title: "{kind_description(child_kind)}",
                            onclick: move |_| quick_add_kind.set(Some(child_kind)),
                            "+ {child_kind.label()}"
                        }
                    }
                }
            }

            if let Some(child_kind) = *quick_add_kind.read() {
                div { class: "quick-add-indent",
                    QuickAddRow {
                        kind: child_kind,
                        parent: Some(item.clone()),
                        on_quick_add: props.on_quick_add,
                        on_add_child: props.on_add_child,
                        on_close: move |_| quick_add_kind.set(None)
                    }
                }
            }

            if has_kids && is_open {
                div { class: "tree-children",
                    for child in children {
                        VvkikTreeNode {
                            item: child,
                            all_items: props.all_items.clone(),
                            depth: props.depth + 1,
                            toggled,
                            on_edit: props.on_edit,
                            on_delete: props.on_delete,
                            on_quick_add: props.on_quick_add,
                            on_add_child: props.on_add_child
                        }
                    }
                }
            }
        }
    }
}
