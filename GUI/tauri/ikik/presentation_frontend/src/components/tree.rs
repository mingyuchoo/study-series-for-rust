#![allow(non_snake_case)]

use super::{item_form::AddPreset,
            quick_add::{QuickAddData,
                        QuickAddRow},
            tree_drag::{TreeDrag,
                        use_tree_drag}};
use crate::{i18n::use_lang,
            models::{IkikItem,
                     ItemKind,
                     ItemStatus,
                     deadline::{due_chip,
                                local_today},
                     kind_description,
                     kind_label,
                     status_label,
                     tree::{MAX_TREE_DEPTH,
                            count_descendants,
                            default_open,
                            has_children,
                            kpi_percent,
                            progress_text,
                            root_items,
                            sorted_children}}};
use dioxus::prelude::*;
use std::collections::HashSet;

#[derive(Props, Clone, PartialEq)]
pub struct IkikTreeViewProps {
    pub items: Vec<IkikItem>,
    pub on_open: EventHandler<IkikItem>,
    pub on_delete: EventHandler<IkikItem>,
    pub on_quick_add: EventHandler<QuickAddData>,
    pub on_add_child: EventHandler<AddPreset>,
    /// (드래그한 항목, 새 상위 항목)
    pub on_reparent: EventHandler<(IkikItem, IkikItem)>,
}

/// 전체 구조 탭: 접기/펼치기가 가능한 컴팩트 트리. 행을 끌어 유효한
/// 상위 항목 위에 놓으면 그 아래로 이동한다.
pub fn IkikTreeView(props: IkikTreeViewProps) -> Element {
    let t = *use_lang().read();
    let mut adding_value = use_signal(|| false);
    // 기본 펼침 상태에서 뒤집힌 노드 집합. 펼침 여부 = default_open XOR
    // 포함 여부라서, 항목이 추가·삭제돼도 나머지 노드의 상태가 유지된다.
    let mut toggled = use_signal(HashSet::<String>::new);
    let drag = use_tree_drag();
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
        div { class: "ikik-tree",
            if props.items.is_empty() {
                p { class: "tree-hint",
                    {t.tree_empty_hint()}
                }
            } else {
                div { class: "tree-toolbar",
                    button { r#type: "button", class: "btn btn-sm btn-outline", onclick: expand_all, {t.expand_all()} }
                    button { r#type: "button", class: "btn btn-sm btn-outline", onclick: collapse_all, {t.collapse_all()} }
                    span { class: "tree-flow", {t.tree_flow_hint()} }
                }
            }
            for item in roots {
                IkikTreeNode {
                    item,
                    all_items: props.items.clone(),
                    depth: 0,
                    toggled,
                    drag,
                    on_open: props.on_open,
                    on_delete: props.on_delete,
                    on_quick_add: props.on_quick_add,
                    on_add_child: props.on_add_child,
                    on_reparent: props.on_reparent
                }
            }

            if *adding_value.read() {
                QuickAddRow {
                    kind: ItemKind::Identity,
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
                    {t.add_identity()}
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct IkikTreeNodeProps {
    item: IkikItem,
    all_items: Vec<IkikItem>,
    depth: usize,
    toggled: Signal<HashSet<String>>,
    drag: TreeDrag,
    on_open: EventHandler<IkikItem>,
    on_delete: EventHandler<IkikItem>,
    on_quick_add: EventHandler<QuickAddData>,
    on_add_child: EventHandler<AddPreset>,
    on_reparent: EventHandler<(IkikItem, IkikItem)>,
}

fn IkikTreeNode(props: IkikTreeNodeProps) -> Element {
    let t = *use_lang().read();
    let mut quick_add_kind = use_signal(|| None::<ItemKind>);
    let mut toggled = props.toggled;
    let drag = props.drag;

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
    let deadline = item
        .due_date
        .as_deref()
        .and_then(|due| local_today().and_then(|today| due_chip(due, item.kind, t, today)));

    let row_class = drag.row_class(&item);

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

    let handle_drag_start = {
        let item = item.clone();
        move |_| drag.start(item.clone())
    };
    let handle_drag_end = move |_| drag.reset();
    let handle_drag_over = {
        let item = item.clone();
        move |evt: DragEvent| drag.hover(&evt, &item)
    };
    let handle_drag_leave = {
        let item_id = item.id.clone();
        move |_| drag.leave(&item_id)
    };
    let handle_drop = {
        let item = item.clone();
        move |evt: DragEvent| {
            if let Some(pair) = drag.drop_on(&evt, &item) {
                props.on_reparent.call(pair);
            }
        }
    };

    rsx! {
        div { class: "tree-node",
            div { class: "{row_class}",
                draggable: true,
                ondragstart: handle_drag_start,
                ondragend: handle_drag_end,
                ondragover: handle_drag_over,
                ondragleave: handle_drag_leave,
                ondrop: handle_drop,
                if has_kids {
                    button {
                        r#type: "button",
                        class: "chevron-btn",
                        aria_expanded: is_open,
                        aria_label: if is_open { t.collapse() } else { t.expand() },
                        onclick: toggle,
                        if is_open { "▾" } else { "▸" }
                    }
                } else {
                    span { class: "chevron-spacer" }
                }

                span { class: "row-kind", {kind_label(item.kind, t)} }
                span { class: "row-title", title: "{item.title}", "{item.title}" }

                if let Some(description) = &item.description {
                    if !description.is_empty() {
                        span { class: "row-desc", title: "{description}", "{description}" }
                    }
                }

                if item.status != ItemStatus::Active {
                    span { class: "row-status", "{status_label(item.status, t)}" }
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

                if let Some(deadline) = deadline {
                    span { class: "due-chip {deadline.class}", "{deadline.text}" }
                }

                if has_kids && !is_open {
                    button {
                        r#type: "button",
                        class: "row-count",
                        onclick: toggle_from_count,
                        {t.nested_count(descendant_count)}
                    }
                }

                div { class: "row-actions",
                    button {
                        r#type: "button",
                        class: "btn row-btn",
                        onclick: {
                            let item = item.clone();
                            move |_| props.on_open.call(item.clone())
                        },
                        // on_open은 수정 화면이 아니라 읽기 전용 상세로 간다.
                        {t.detail()}
                    }
                    button {
                        r#type: "button",
                        class: "btn row-btn",
                        onclick: {
                            let item = item.clone();
                            move |_| props.on_delete.call(item.clone())
                        },
                        {t.delete()}
                    }
                    for child_kind in child_kinds.iter().copied() {
                        button {
                            r#type: "button",
                            class: "btn row-btn",
                            title: kind_description(child_kind, t),
                            onclick: move |_| quick_add_kind.set(Some(child_kind)),
                            "+ {kind_label(child_kind, t)}"
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
                        IkikTreeNode {
                            item: child,
                            all_items: props.all_items.clone(),
                            depth: props.depth + 1,
                            toggled,
                            drag,
                            on_open: props.on_open,
                            on_delete: props.on_delete,
                            on_quick_add: props.on_quick_add,
                            on_add_child: props.on_add_child,
                            on_reparent: props.on_reparent
                        }
                    }
                }
            }
        }
    }
}
