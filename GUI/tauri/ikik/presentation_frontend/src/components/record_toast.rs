#![allow(non_snake_case)]

use crate::i18n::use_lang;
use dioxus::prelude::*;

const TOAST_DURATION_MS: i32 = 4000;

/// wasm에는 스레드 sleep이 없어 setTimeout을 Future로 감싼다.
async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// 기록 직후 띄우는 토스트 한 건. `undo`가 있으면 실행 취소 버튼으로
/// 방금 추가한 측정값(kpi_id, measurement_id)을 지울 수 있다.
#[derive(Clone, PartialEq)]
pub struct RecordToast {
    pub message: String,
    pub undo: Option<(String, String)>,
}

/// 토스트 상태 핸들. 화면은 `show`로 띄우기만 하고, 자동 닫기 타이머가
/// 뒤늦게 다른 토스트를 닫지 않도록 세대 번호로 구분하는 일은 여기서
/// 책임진다.
#[derive(Clone, Copy, PartialEq)]
pub struct RecordToastHost {
    toast: Signal<Option<RecordToast>>,
    seq: Signal<u64>,
}

pub fn use_record_toast() -> RecordToastHost {
    RecordToastHost {
        toast: use_signal(|| None),
        seq: use_signal(|| 0u64),
    }
}

impl RecordToastHost {
    pub fn show(mut self, message: String, undo: Option<(String, String)>) {
        let seq = *self.seq.read() + 1;
        self.seq.set(seq);
        self.toast.set(Some(RecordToast {
            message,
            undo,
        }));
        spawn(async move {
            sleep_ms(TOAST_DURATION_MS).await;
            if *self.seq.read() == seq {
                self.toast.set(None);
            }
        });
    }

    pub fn dismiss(mut self) { self.toast.set(None); }

    fn current(&self) -> Option<RecordToast> { self.toast.read().clone() }
}

#[derive(Props, Clone, PartialEq)]
pub struct RecordToastViewProps {
    pub host: RecordToastHost,
    /// 실행 취소 버튼: (kpi_id, measurement_id)를 돌려준다.
    pub on_undo: EventHandler<(String, String)>,
}

pub fn RecordToastView(props: RecordToastViewProps) -> Element {
    let t = *use_lang().read();
    let Some(current) = props.host.current() else {
        return rsx! {};
    };

    rsx! {
        div { class: "record-toast", role: "status",
            span { "{current.message}" }
            if let Some(undo) = current.undo.clone() {
                button {
                    r#type: "button",
                    class: "btn row-btn",
                    onclick: move |_| {
                        props.host.dismiss();
                        props.on_undo.call(undo.clone());
                    },
                    {t.undo()}
                }
            }
        }
    }
}
