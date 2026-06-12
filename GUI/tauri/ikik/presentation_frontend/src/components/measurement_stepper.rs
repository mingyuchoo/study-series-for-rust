#![allow(non_snake_case)]

//! 측정값 스테퍼: − / 숫자(직접 입력 불가) / + 와 스텝 칩.
//! 상세 패널과 단계 탭의 퀵 기록 팝오버가 같은 입력 경험을 공유한다.
//! 값 시그널은 부모가 소유한다 — 시작값과 제출·초기화는 화면의 결정이고,
//! 이 컴포넌트는 값을 만드는 방법(클릭·길게 누르기·스텝 칩)만 책임진다.

use crate::{i18n::use_lang,
            models::{KpiAggregation,
                     format_value,
                     stepper::{bump_value,
                               default_step,
                               step_chips}}};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

/// 길게 누르기 자동 반복: 시작 지연(ms)과 반복 간격(ms).
const HOLD_DELAY_MS: u32 = 450;
const HOLD_REPEAT_MS: u32 = 90;

#[derive(Props, Clone, PartialEq)]
pub struct MeasurementStepperProps {
    /// 스텝 칩 구성을 정하는 목표값.
    #[props(default)]
    pub target_value: Option<f64>,
    pub aggregation: KpiAggregation,
    #[props(default)]
    pub unit: Option<String>,
    /// 입력 중인 측정값. 부모가 소유한다.
    pub value: Signal<f64>,
}

pub fn MeasurementStepper(props: MeasurementStepperProps) -> Element {
    let t = *use_lang().read();
    let mut value = props.value;
    let unit = props.unit.clone().unwrap_or_default();
    let chips = step_chips(props.target_value, props.aggregation);

    // 한 클릭의 변화량과 길게 누르기 토큰. 토큰은 누를 때마다 증가하고,
    // 반복 루프는 자기 토큰이 최신일 때만 돈다 — 손을 떼거나 다른
    // 버튼을 누르면 이전 루프가 멈춘다.
    let mut step_size = use_signal(|| default_step(&step_chips(props.target_value, props.aggregation)));
    let mut hold_token = use_signal(|| 0_u32);

    let mut bump = move |direction: f64| {
        let next = bump_value(*value.peek(), *step_size.peek(), direction);
        value.set(next);
    };

    // 누르는 즉시 한 번 움직이고, 잠시 후 자동 반복으로 빨라진다.
    let mut start_hold = move |direction: f64| {
        let token = *hold_token.peek() + 1;
        hold_token.set(token);
        spawn(async move {
            bump(direction);
            TimeoutFuture::new(HOLD_DELAY_MS).await;
            while *hold_token.peek() == token {
                bump(direction);
                TimeoutFuture::new(HOLD_REPEAT_MS).await;
            }
        });
    };
    let mut stop_hold = move || {
        let token = *hold_token.peek() + 1;
        hold_token.set(token);
    };

    rsx! {
        div { class: "measurement-add-value",
            div { class: "measurement-stepper",
                button {
                    r#type: "button",
                    class: "step-btn",
                    aria_label: t.step_decrease(),
                    disabled: *value.read() <= 0.0,
                    onmousedown: move |_| start_hold(-1.0),
                    onmouseup: move |_| stop_hold(),
                    onmouseleave: move |_| stop_hold(),
                    "−"
                }
                span { class: "step-num", "{format_value(*value.read())}" }
                button {
                    r#type: "button",
                    class: "step-btn",
                    aria_label: t.step_increase(),
                    onmousedown: move |_| start_hold(1.0),
                    onmouseup: move |_| stop_hold(),
                    onmouseleave: move |_| stop_hold(),
                    "+"
                }
            }
            if !unit.is_empty() {
                span { class: "measurement-unit", "{unit}" }
            }
        }
        if chips.len() > 1 {
            div { class: "step-chips", role: "radiogroup", aria_label: t.step_size_aria(),
                for chip in chips.clone() {
                    button {
                        r#type: "button",
                        role: "radio",
                        aria_checked: *step_size.read() == chip,
                        class: if *step_size.read() == chip { "step-chip active" } else { "step-chip" },
                        onclick: move |_| step_size.set(chip),
                        "±{format_value(chip)}"
                    }
                }
            }
        }
    }
}
