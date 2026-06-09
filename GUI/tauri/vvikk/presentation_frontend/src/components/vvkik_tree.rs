#![allow(non_snake_case)]

use crate::models::{VvkikItem,
                    kind_description,
                    kind_label};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikBoardProps {
    pub items: Vec<VvkikItem>,
    pub is_filtering: bool,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
}

const KIND_ORDER: [&str; 5] = ["value", "vision", "kra", "igt", "kpi"];

fn parent_title(item: &VvkikItem, items: &[VvkikItem]) -> Option<String> {
    let parent_id = item.parent_id.as_ref()?;
    items.iter().find(|candidate| &candidate.id == parent_id).map(|parent| parent.title.clone())
}

fn progress_text(item: &VvkikItem) -> Option<String> {
    if item.kind != "kpi" {
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

pub fn VvkikBoard(props: VvkikBoardProps) -> Element {
    rsx! {
        div { class: "vvkik-board",
            if props.items.is_empty() {
                div { class: "empty-state",
                    if props.is_filtering {
                        p { "검색 결과가 없습니다." }
                    } else {
                        p { "아직 VVKIK 항목이 없습니다. Value부터 추가해 보세요." }
                    }
                }
            } else {
                for kind in KIND_ORDER {
                    VvkikLane {
                        kind: kind.to_string(),
                        items: props.items.iter().filter(|item| item.kind == kind).cloned().collect::<Vec<_>>(),
                        all_items: props.items.clone(),
                        on_edit: props.on_edit,
                        on_delete: props.on_delete
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct VvkikLaneProps {
    kind: String,
    items: Vec<VvkikItem>,
    all_items: Vec<VvkikItem>,
    on_edit: EventHandler<VvkikItem>,
    on_delete: EventHandler<VvkikItem>,
}

fn VvkikLane(props: VvkikLaneProps) -> Element {
    rsx! {
        section { class: "vvkik-lane",
            div { class: "lane-heading",
                div {
                    h2 { "{kind_label(&props.kind)}" }
                    p { "{kind_description(&props.kind)}" }
                }
                span { class: "lane-count", "{props.items.len()}" }
            }
            if props.items.is_empty() {
                div { class: "lane-empty", "비어 있음" }
            } else {
                div { class: "item-grid",
                    for item in props.items.iter() {
                        VvkikCard {
                            item: item.clone(),
                            parent_title: parent_title(item, &props.all_items),
                            on_edit: props.on_edit,
                            on_delete: props.on_delete
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct VvkikCardProps {
    pub item: VvkikItem,
    pub parent_title: Option<String>,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
}

pub fn VvkikCard(props: VvkikCardProps) -> Element {
    let item = props.item.clone();
    let progress = progress_text(&item);

    rsx! {
        article { class: "vvkik-card",
            div { class: "item-header",
                div {
                    span { class: "item-kind", "{kind_label(&item.kind)}" }
                    h3 { "{item.title}" }
                }
                div { class: "item-actions",
                    button {
                        class: "btn btn-sm btn-outline",
                        onclick: {
                            let item = item.clone();
                            move |_| props.on_edit.call(item.clone())
                        },
                        "수정"
                    }
                    button {
                        class: "btn btn-sm btn-danger",
                        onclick: {
                            let item = item.clone();
                            move |_| props.on_delete.call(item.clone())
                        },
                        "삭제"
                    }
                }
            }

            div { class: "item-meta",
                if let Some(parent_title) = &props.parent_title {
                    span { "상위: {parent_title}" }
                }
                span { "상태: {item.status}" }
            }

            if let Some(description) = &item.description {
                if !description.is_empty() {
                    p { class: "item-description", "{description}" }
                }
            }

            if let Some(progress) = progress {
                div { class: "kpi-progress",
                    span { "KPI" }
                    strong { "{progress}" }
                }
            }
        }
    }
}
