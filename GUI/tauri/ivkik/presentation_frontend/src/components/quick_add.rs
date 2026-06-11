#![allow(non_snake_case)]

use super::item_form::AddPreset;
use crate::models::{ItemKind,
                    IvkikItem};
use dioxus::prelude::*;

/// 인라인 빠른 추가가 만들어 내는 최소 입력. 설명·KPI 값은 비워 두고
/// 생성 후 필요하면 수정한다.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickAddData {
    pub kind: ItemKind,
    pub parent_id: Option<String>,
    pub title: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct QuickAddRowProps {
    pub kind: ItemKind,
    pub parent: Option<IvkikItem>,
    pub on_quick_add: EventHandler<QuickAddData>,
    pub on_add_child: EventHandler<AddPreset>,
    pub on_close: EventHandler<()>,
}

/// 제목만 입력해 바로 추가하는 인라인 행. "자세히 입력"은 입력한 제목을
/// 들고 전체 폼으로 이동한다.
pub fn QuickAddRow(props: QuickAddRowProps) -> Element {
    let mut title = use_signal(String::new);

    let handle_submit = {
        let parent_id = props.parent.as_ref().map(|parent| parent.id.clone());
        let kind = props.kind;
        move |evt: FormEvent| {
            evt.prevent_default();
            let value = title.read().trim().to_string();
            if value.is_empty() {
                return;
            }
            props.on_quick_add.call(QuickAddData {
                kind,
                parent_id: parent_id.clone(),
                title: value,
            });
            props.on_close.call(());
        }
    };

    let handle_detail = {
        let parent = props.parent.clone();
        let kind = props.kind;
        move |_| {
            props.on_add_child.call(AddPreset {
                kind,
                parent: parent.clone(),
                title: title.read().trim().to_string(),
            });
            props.on_close.call(());
        }
    };

    rsx! {
        form { class: "quick-add-row", onsubmit: handle_submit,
            input {
                r#type: "text",
                required: true,
                autofocus: true,
                placeholder: "{props.kind.label()} 제목을 입력하고 Enter",
                value: "{title}",
                oninput: move |evt| title.set(evt.value()),
                onkeydown: move |evt| {
                    if evt.key() == Key::Escape {
                        props.on_close.call(());
                    }
                }
            }
            button { r#type: "submit", class: "btn btn-sm btn-primary", "추가" }
            button {
                r#type: "button",
                class: "btn btn-sm btn-outline",
                onclick: handle_detail,
                "자세히 입력"
            }
            button {
                r#type: "button",
                class: "btn btn-sm btn-secondary",
                onclick: move |_| props.on_close.call(()),
                "취소"
            }
        }
    }
}
