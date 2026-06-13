#![allow(non_snake_case)]

use super::{dashboard::IkikDashboard,
            item_form::AddPreset,
            kind_view::IkikKindView,
            quick_add::QuickAddData,
            tree::IkikTreeView};
use crate::{board_tab::BoardTab,
            i18n::use_lang,
            models::{IkikItem,
                     ItemKind,
                     kind_label}};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct IkikBoardProps {
    pub items: Vec<IkikItem>,
    pub is_filtering: bool,
    pub active_tab: Signal<BoardTab>,
    pub on_open: EventHandler<IkikItem>,
    pub on_delete: EventHandler<IkikItem>,
    pub on_quick_add: EventHandler<QuickAddData>,
    pub on_add_child: EventHandler<AddPreset>,
    pub on_reparent: EventHandler<(IkikItem, IkikItem)>,
}

/// 탭 바와 탭별 화면(전체 구조 트리 / 단계별 카드)을 배선한다.
pub fn IkikBoard(props: IkikBoardProps) -> Element {
    let t = *use_lang().read();
    let mut active_tab = props.active_tab;
    let current_tab = *active_tab.read();
    let is_dashboard = current_tab == BoardTab::Dashboard;
    let is_tree = current_tab == BoardTab::Structure;
    let kind_tab: Option<ItemKind> = match current_tab {
        | BoardTab::Kind(kind) => Some(kind),
        | _ => None,
    };

    rsx! {
        div { class: "ikik-board",
            nav { class: "board-tabs", role: "tablist", aria_label: t.tabs_aria(),
                button {
                    r#type: "button",
                    role: "tab",
                    aria_selected: is_dashboard,
                    class: if is_dashboard { "board-tab active" } else { "board-tab" },
                    onclick: move |_| active_tab.set(BoardTab::Dashboard),
                    {t.dashboard_tab()}
                }
                button {
                    r#type: "button",
                    role: "tab",
                    aria_selected: is_tree,
                    class: if is_tree { "board-tab active" } else { "board-tab" },
                    onclick: move |_| active_tab.set(BoardTab::Structure),
                    {t.structure_tab()}
                }
                // 단계별 수량은 대시보드 카드가 상태 분포와 함께 보여
                // 주므로, 탭은 이동 수단으로만 남긴다.
                for kind in ItemKind::ALL {
                    {
                        let is_active = kind_tab == Some(kind);
                        rsx! {
                            button {
                                r#type: "button",
                                role: "tab",
                                aria_selected: is_active,
                                class: if is_active { "board-tab active" } else { "board-tab" },
                                onclick: move |_| active_tab.set(BoardTab::Kind(kind)),
                                {kind_label(kind, t)}
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
                    IkikDashboard {
                        items: props.items.clone(),
                        is_filtering: props.is_filtering,
                        on_open: props.on_open,
                        on_kind_select: move |kind: ItemKind| active_tab.set(BoardTab::Kind(kind))
                    }
                } else {
                    match kind_tab {
                        Some(kind) => rsx! {
                            IkikKindView {
                                kind,
                                items: props.items.clone(),
                                on_open: props.on_open
                            }
                        },
                        None => rsx! {
                            IkikTreeView {
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
