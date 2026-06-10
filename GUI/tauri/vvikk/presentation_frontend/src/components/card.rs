#![allow(non_snake_case)]

use crate::models::{VvkikItem,
                    tree::progress_text};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikCardProps {
    pub item: VvkikItem,
    pub parent_title: Option<String>,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
}

/// 단계별 탭에서 쓰는 상세 카드. 설명 전문과 KPI 진행 상황을 보여 준다.
pub fn VvkikCard(props: VvkikCardProps) -> Element {
    let item = props.item.clone();
    let progress = progress_text(&item);

    rsx! {
        article { class: "vvkik-card",
            div { class: "item-header",
                div {
                    span { class: "item-kind", "{item.kind.label()}" }
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
