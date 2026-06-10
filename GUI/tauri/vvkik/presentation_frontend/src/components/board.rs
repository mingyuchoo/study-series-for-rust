#![allow(non_snake_case)]

use super::{dashboard::VvkikDashboard,
            item_form::AddPreset,
            kind_view::VvkikKindView,
            quick_add::QuickAddData,
            tree::VvkikTreeView};
use crate::models::{ItemKind,
                    VvkikItem};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikBoardProps {
    pub items: Vec<VvkikItem>,
    pub is_filtering: bool,
    pub active_tab: Signal<String>,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
    pub on_quick_add: EventHandler<QuickAddData>,
    pub on_add_child: EventHandler<AddPreset>,
    pub on_reparent: EventHandler<(VvkikItem, VvkikItem)>,
}

/// 탭 바와 탭별 화면(전체 구조 트리 / 단계별 카드)을 배선한다.
pub fn VvkikBoard(props: VvkikBoardProps) -> Element {
    let mut active_tab = props.active_tab;
    let current_tab = active_tab.read().clone();
    let is_dashboard = current_tab == "dashboard";
    let kind_tab: Option<ItemKind> = current_tab.parse().ok();
    let is_tree = !is_dashboard && kind_tab.is_none();

    rsx! {
        div { class: "vvkik-board",
            nav { class: "board-tabs", role: "tablist", aria_label: "VVKIK 단계 탭",
                button {
                    r#type: "button",
                    role: "tab",
                    aria_selected: is_dashboard,
                    class: if is_dashboard { "board-tab active" } else { "board-tab" },
                    onclick: move |_| active_tab.set("dashboard".to_string()),
                    "대시보드"
                }
                button {
                    r#type: "button",
                    role: "tab",
                    aria_selected: is_tree,
                    class: if is_tree { "board-tab active" } else { "board-tab" },
                    onclick: move |_| active_tab.set("tree".to_string()),
                    "전체 구조"
                }
                for kind in ItemKind::ALL {
                    {
                        let count = props.items.iter().filter(|item| item.kind == kind).count();
                        let is_active = kind_tab == Some(kind);
                        rsx! {
                            button {
                                r#type: "button",
                                role: "tab",
                                aria_selected: is_active,
                                class: if is_active { "board-tab active" } else { "board-tab" },
                                onclick: move |_| active_tab.set(kind.as_str().to_string()),
                                "{kind.label()}"
                                span { class: "tab-count", "{count}" }
                            }
                        }
                    }
                }
            }

            if props.items.is_empty() && props.is_filtering {
                div { class: "empty-state",
                    p { "검색 결과가 없습니다." }
                }
            } else {
                if is_dashboard {
                    VvkikDashboard {
                        items: props.items.clone(),
                        is_filtering: props.is_filtering,
                        on_edit: props.on_edit
                    }
                } else {
                    match kind_tab {
                        Some(kind) => rsx! {
                            VvkikKindView {
                                kind,
                                items: props.items.clone(),
                                on_edit: props.on_edit,
                                on_delete: props.on_delete
                            }
                        },
                        None => rsx! {
                            VvkikTreeView {
                                items: props.items.clone(),
                                on_edit: props.on_edit,
                                on_delete: props.on_delete,
                                on_quick_add: props.on_quick_add,
                                on_add_child: props.on_add_child,
                                on_reparent: props.on_reparent
                            }
                        },
                    }
                }
            }
        }
    }
}
