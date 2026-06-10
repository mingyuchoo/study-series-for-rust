#![allow(non_snake_case)]

use super::card::VvkikCard;
use crate::models::{ItemKind,
                    VvkikItem,
                    kind_description,
                    tree::{parent_path,
                           sort_items}};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikKindViewProps {
    pub kind: ItemKind,
    pub items: Vec<VvkikItem>,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
}

/// 한 단계의 항목들을 상위 경로별로 묶어 보여 주는 탭 화면.
pub fn VvkikKindView(props: VvkikKindViewProps) -> Element {
    let mut kind_items: Vec<VvkikItem> = props.items.iter().filter(|item| item.kind == props.kind).cloned().collect();
    sort_items(&mut kind_items);

    // 같은 상위 경로를 가진 항목끼리 묶는다.
    let mut groups: Vec<(Option<String>, Vec<VvkikItem>)> = Vec::new();
    for item in kind_items.iter() {
        let path = parent_path(item, &props.items);
        match groups.iter_mut().find(|(candidate, _)| *candidate == path) {
            | Some((_, group)) => group.push(item.clone()),
            | None => groups.push((path, vec![item.clone()])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));

    rsx! {
        section { class: "vvkik-lane",
            div { class: "lane-heading",
                div {
                    h2 { "{props.kind.label()}" }
                    p { "{kind_description(props.kind)}" }
                }
                span { class: "lane-count", "{kind_items.len()}" }
            }
            if kind_items.is_empty() {
                div { class: "lane-empty", "비어 있음" }
            } else {
                for (path, group_items) in groups {
                    {
                        let path_label = path.unwrap_or_else(|| "최상위".to_string());
                        rsx! {
                            div { class: "kind-group",
                                p { class: "group-path", "{path_label}" }
                                div { class: "item-grid",
                                    for item in group_items {
                                        VvkikCard {
                                            item,
                                            parent_title: None,
                                            on_edit: props.on_edit,
                                            on_delete: props.on_delete
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
