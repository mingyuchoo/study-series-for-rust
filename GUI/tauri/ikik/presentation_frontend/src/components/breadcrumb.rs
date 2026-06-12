#![allow(non_snake_case)]

//! 조상 경로 브레드크럼. 상세 화면과 수정 폼이 같은 표기를 공유한다 —
//! 조상은 클릭하면 그 항목으로 이동하고, 현재 항목은 링크 없이 끝에
//! 머문다.

use crate::{i18n::use_lang,
            models::{IkikItem,
                     ItemKind,
                     kind_label}};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct BreadcrumbProps {
    /// 루트부터 직계 부모까지의 조상 목록(tree::parent_chain의 결과).
    pub chain: Vec<IkikItem>,
    pub current_kind: ItemKind,
    pub current_title: String,
    /// 조상을 클릭하면 그 항목으로 이동한다.
    pub on_navigate: EventHandler<IkikItem>,
}

pub fn Breadcrumb(props: BreadcrumbProps) -> Element {
    let t = *use_lang().read();

    rsx! {
        nav { class: "edit-breadcrumb", aria_label: t.breadcrumb_aria(),
            for ancestor in props.chain.iter() {
                {
                    let ancestor = ancestor.clone();
                    let ancestor_title = ancestor.title.clone();
                    let ancestor_kind = kind_label(ancestor.kind, t);
                    rsx! {
                        span { class: "breadcrumb-seg",
                            span { class: "breadcrumb-kind", "{ancestor_kind}" }
                            button {
                                r#type: "button",
                                class: "breadcrumb-link",
                                title: t.goto_detail(ancestor_kind),
                                onclick: move |_| props.on_navigate.call(ancestor.clone()),
                                "{ancestor_title}"
                            }
                        }
                        span { class: "breadcrumb-sep", "›" }
                    }
                }
            }
            span { class: "breadcrumb-seg",
                span { class: "breadcrumb-kind", {kind_label(props.current_kind, t)} }
                span { class: "breadcrumb-current", "{props.current_title}" }
            }
        }
    }
}
