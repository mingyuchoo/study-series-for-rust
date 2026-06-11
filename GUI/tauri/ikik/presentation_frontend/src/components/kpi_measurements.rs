#![allow(non_snake_case)]

use super::record_grass::RecordGrass;
use crate::{i18n::use_lang,
            models::{KpiAggregation,
                     KpiMeasurement,
                     RecordKpiMeasurementRequest,
                     aggregation_label,
                     format_timestamp,
                     format_value,
                     stepper::{bump_value,
                               default_step,
                               step_chips}},
            services::IkikService,
            store::IkikStore};
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

/// 길게 누르기 자동 반복: 시작 지연(ms)과 반복 간격(ms).
const HOLD_DELAY_MS: u32 = 450;
const HOLD_REPEAT_MS: u32 = 90;

#[derive(Props, Clone, PartialEq)]
pub struct KpiMeasurementPanelProps {
    pub kpi_id: String,
    pub aggregation: crate::models::KpiAggregation,
    pub unit: Option<String>,
    /// 스텝 칩 구성을 정하는 목표값.
    #[props(default)]
    pub target_value: Option<f64>,
    /// 기록 유무를 폼과 공유해 현재값 입력을 잠근다.
    pub has_measurements: Signal<bool>,
    /// 집계된 현재값을 폼의 현재값 입력에도 반영한다.
    pub current_value: Signal<String>,
}

/// Key Performance Indicator 상세 화면의 "실적 기록" 패널. 값과 함께 그날의
/// 느낌·감상을 일기처럼 남기면 백엔드가 집계 방식대로 현재값을 다시 계산하고,
/// 기록의 꾸준함은 잔디 그래프로 쌓인다.
pub fn KpiMeasurementPanel(props: KpiMeasurementPanelProps) -> Element {
    let lang = use_lang();
    let t = *lang.read();
    let kpi_id = use_signal(|| props.kpi_id.clone());
    let aggregation = props.aggregation;
    // 측정값 추가·삭제는 스토어를 거쳐 목록(현재값·진행률)까지 함께
    // 새로고침한다.
    let store = use_context::<IkikStore>();
    let unit = props.unit.clone().unwrap_or_default();

    let chips = step_chips(props.target_value, aggregation);

    let mut measurements = use_signal(Vec::<KpiMeasurement>::new);
    // 스테퍼 상태: 입력 중인 측정값, 한 클릭의 변화량, 길게 누르기 토큰.
    // 토큰은 누를 때마다 증가하고, 반복 루프는 자기 토큰이 최신일 때만
    // 돈다 — 손을 떼거나 다른 버튼을 누르면 이전 루프가 멈춘다.
    let mut step_value = use_signal(|| 0.0_f64);
    let mut step_size = use_signal(|| default_step(&step_chips(props.target_value, aggregation)));
    let mut hold_token = use_signal(|| 0_u32);
    let mut note_input = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut panel_error = use_signal(|| None::<String>);
    let mut has_measurements = props.has_measurements;
    let mut current_value = props.current_value;

    // 목록을 받아 패널 상태와 폼의 현재값 표시를 한꺼번에 갱신한다.
    // 스테퍼 시작값도 여기서 정한다: 합계형은 증분 기록이라 0에서,
    // 최신값·평균형은 직전 기록값에서 이어서 고친다.
    let mut apply = move |list: Vec<KpiMeasurement>| {
        has_measurements.set(!list.is_empty());
        let values: Vec<f64> = list.iter().map(|measurement| measurement.value).collect();
        if let Some(aggregated) = aggregation.aggregate(&values) {
            current_value.set(format_value(aggregated));
        }
        let start = if aggregation == KpiAggregation::Sum {
            0.0
        } else {
            list.first().map(|measurement| measurement.value).unwrap_or(0.0)
        };
        step_value.set(start.max(0.0));
        measurements.set(list);
    };

    let mut bump = move |direction: f64| {
        let next = bump_value(*step_value.peek(), *step_size.peek(), direction);
        step_value.set(next);
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

    use_effect(move || {
        spawn(async move {
            match IkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                | Ok(list) => apply(list),
                | Err(e) => panel_error.set(Some(lang.peek().err_load_records(&e))),
            }
        });
    });

    // 버튼 클릭과 메모 입력의 ⌘+Enter가 같은 경로를 쓴다.
    let mut submit = move || {
        if *busy.read() {
            return;
        }

        let value = *step_value.peek();
        let note = note_input.read().trim().to_string();

        spawn(async move {
            busy.set(true);
            let request = RecordKpiMeasurementRequest {
                kpi_id: kpi_id.read().clone(),
                value,
                note: (!note.is_empty()).then_some(note),
            };
            match store.record_measurement(request).await {
                | Ok(_) => {
                    note_input.set(String::new());
                    panel_error.set(None);
                    match IkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                        | Ok(list) => apply(list),
                        | Err(e) => panel_error.set(Some(e)),
                    }
                },
                | Err(e) => panel_error.set(Some(lang.peek().err_add_record(&e))),
            }
            busy.set(false);
        });
    };

    let handle_delete = move |measurement_id: String| {
        if *busy.read() {
            return;
        }

        spawn(async move {
            busy.set(true);
            match store.delete_measurement(kpi_id.read().clone(), measurement_id).await {
                | Ok(()) => {
                    panel_error.set(None);
                    match IkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                        | Ok(list) => {
                            // 마지막 기록을 지우면 집계값이 없어지므로 표시도 비운다.
                            if list.is_empty() {
                                current_value.set(String::new());
                            }
                            apply(list);
                        },
                        | Err(e) => panel_error.set(Some(e)),
                    }
                },
                | Err(e) => panel_error.set(Some(lang.peek().err_delete_record(&e))),
            }
            busy.set(false);
        });
    };

    let grass_timestamps: Vec<String> = measurements.read().iter().map(|measurement| measurement.measured_at.clone()).collect();

    rsx! {
        div { class: "measurement-panel",
            div { class: "measurement-heading",
                label { {t.records_heading()} }
                span { class: "measurement-hint", {t.agg_auto_hint(aggregation_label(aggregation, t))} }
            }

            if let Some(error) = panel_error.read().clone() {
                div { class: "form-error", "{error}" }
            }

            div { class: "measurement-add",
                // 직접 입력 대신 − / + 스테퍼로 측정값을 만든다. 길게
                // 누르면 자동 반복되고, 칩은 한 클릭의 크기를 바꾼다.
                div { class: "measurement-add-value",
                    div { class: "measurement-stepper",
                        button {
                            r#type: "button",
                            class: "step-btn",
                            aria_label: t.step_decrease(),
                            disabled: *step_value.read() <= 0.0,
                            onmousedown: move |_| start_hold(-1.0),
                            onmouseup: move |_| stop_hold(),
                            onmouseleave: move |_| stop_hold(),
                            "−"
                        }
                        span { class: "step-num", "{format_value(*step_value.read())}" }
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
                textarea {
                    rows: "2",
                    class: "measurement-note-input",
                    placeholder: t.note_placeholder(),
                    value: "{note_input}",
                    oninput: move |evt| note_input.set(evt.value()),
                    onkeydown: move |evt| {
                        let modifiers = evt.modifiers();
                        if evt.key() == Key::Enter && (modifiers.meta() || modifiers.ctrl()) {
                            submit();
                        }
                    }
                }
                div { class: "measurement-add-actions",
                    span { class: "measurement-submit-hint", {t.cmd_enter_hint()} }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        disabled: *busy.read(),
                        onclick: move |_| submit(),
                        {t.record()}
                    }
                }
            }

            RecordGrass { timestamps: grass_timestamps }

            if measurements.read().is_empty() {
                p { class: "measurement-empty", {t.no_records_yet_panel()} }
            } else {
                ul { class: "measurement-list",
                    for measurement in measurements.read().iter() {
                        {
                            let measurement_id = measurement.id.clone();
                            let value_text = format_value(measurement.value);
                            let measured_at = format_timestamp(&measurement.measured_at);
                            let note = measurement.note.clone().unwrap_or_default();
                            rsx! {
                                li { class: "measurement-row",
                                    div { class: "measurement-row-head",
                                        span { class: "measurement-value", "{value_text} {unit}" }
                                        span { class: "measurement-meta",
                                            span { class: "measurement-date", title: "{measurement.measured_at}", "{measured_at}" }
                                            button {
                                                r#type: "button",
                                                class: "btn row-btn",
                                                disabled: *busy.read(),
                                                onclick: move |_| handle_delete(measurement_id.clone()),
                                                {t.delete()}
                                            }
                                        }
                                    }
                                    if !note.is_empty() {
                                        p { class: "measurement-note", "{note}" }
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
