#![allow(non_snake_case)]

//! 되돌릴 수 없는 동작을 한 번 더 확인받는 모달. 표시 문구는 모두 호출자가
//! 넘기므로 i18n·도메인에 의존하지 않고, 삭제 외의 확인에도 재사용할 수 있다.

use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ConfirmDialogProps {
    /// 스크린리더용 대화상자 레이블.
    pub aria_label: String,
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub cancel_label: String,
    /// 확인(위험) 버튼을 누르면 호출된다.
    pub on_confirm: EventHandler<()>,
    /// 취소 또는 배경을 누르면 호출된다.
    pub on_cancel: EventHandler<()>,
}

pub fn ConfirmDialog(props: ConfirmDialogProps) -> Element {
    rsx! {
        div { class: "confirm-backdrop",
            div { class: "confirm-dialog", role: "dialog", aria_label: props.aria_label,
                h2 { {props.title} }
                p { {props.body} }
                div { class: "confirm-actions",
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        onclick: move |_| props.on_cancel.call(()),
                        {props.cancel_label}
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-danger",
                        onclick: move |_| props.on_confirm.call(()),
                        {props.confirm_label}
                    }
                }
            }
        }
    }
}
