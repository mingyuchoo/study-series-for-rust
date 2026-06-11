#![allow(non_snake_case)]

use super::{dashboard::IvkikDashboard,
            item_form::AddPreset,
            kind_view::IvkikKindView,
            quick_add::QuickAddData,
            tree::IvkikTreeView};
use crate::{i18n::use_lang,
            models::{ItemKind,
                     IvkikItem}};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct IvkikBoardProps {
    pub items: Vec<IvkikItem>,
    pub is_filtering: bool,
    pub active_tab: Signal<String>,
    pub on_open: EventHandler<IvkikItem>,
    pub on_delete: EventHandler<IvkikItem>,
    pub on_quick_add: EventHandler<QuickAddData>,
    pub on_add_child: EventHandler<AddPreset>,
    pub on_reparent: EventHandler<(IvkikItem, IvkikItem)>,
}

/// 탭 바와 탭별 화면(전체 구조 트리 / 단계별 카드)을 배선한다.
pub fn IvkikBoard(props: IvkikBoardProps) -> Element {
    let t = *use_lang().read();
    let mut active_tab = props.active_tab;
    let current_tab = active_tab.read().clone();
    let is_dashboard = current_tab == "dashboard";
    let kind_tab: Option<ItemKind> = current_tab.parse().ok();
    let is_tree = !is_dashboard && kind_tab.is_none();

    rsx! {
        div { class: "ivkik-board",
            nav { class: "board-tabs", role: "tablist", aria_label: t.tabs_aria(),
                button {
                    r#type: "button",
                    role: "tab",
                    aria_selected: is_dashboard,
                    class: if is_dashboard { "board-tab active" } else { "board-tab" },
                    onclick: move |_| active_tab.set("dashboard".to_string()),
                    {t.dashboard_tab()}
                }
                button {
                    r#type: "button",
                    role: "tab",
                    aria_selected: is_tree,
                    class: if is_tree { "board-tab active" } else { "board-tab" },
                    onclick: move |_| active_tab.set("tree".to_string()),
                    {t.structure_tab()}
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
                    p { {t.no_search_results()} }
                }
            } else {
                if is_dashboard {
                    IvkikDashboard {
                        items: props.items.clone(),
                        is_filtering: props.is_filtering,
                        on_open: props.on_open
                    }
                } else {
                    match kind_tab {
                        Some(kind) => rsx! {
                            IvkikKindView {
                                kind,
                                items: props.items.clone(),
                                on_open: props.on_open
                            }
                        },
                        None => rsx! {
                            IvkikTreeView {
                                items: props.items.clone(),
                                on_open: props.on_open,
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
