#![allow(non_snake_case)]

use super::{icons::PencilIcon,
            measurement_stepper::MeasurementStepper,
            record_grass::RecordGrass};
use crate::{i18n::use_lang,
            mode::use_mode,
            models::{KpiAggregation,
                     KpiMeasurement,
                     RecordKpiMeasurementRequest,
                     aggregation_label,
                     format_timestamp,
                     format_value},
            store::IkikStore};
use dioxus::prelude::*;

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
    // 삭제는 구조 변경이므로 관리 모드에서만 노출한다(사용 모드에선 숨김).
    let is_manage = use_mode().read().is_manage();
    let kpi_id = use_signal(|| props.kpi_id.clone());
    let aggregation = props.aggregation;
    // 측정값 추가·삭제는 스토어를 거쳐 목록(현재값·진행률)까지 함께
    // 새로고침한다.
    let store = use_context::<IkikStore>();
    let unit = props.unit.clone().unwrap_or_default();

    let mut measurements = use_signal(Vec::<KpiMeasurement>::new);
    // 스테퍼가 만드는 입력 중인 측정값. 시작값과 제출 후 초기화는
    // apply()가 결정한다.
    let mut step_value = use_signal(|| 0.0_f64);
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

    use_effect(move || {
        spawn(async move {
            match store.load_measurements(kpi_id.read().clone()).await {
                | Ok(list) => apply(list),
                | Err(e) => panel_error.set(Some(lang.peek().err_load_records(&e))),
            }
        });
    });

    // 버튼 클릭과 메모 입력의 ⌘+Enter가 같은 경로를 쓴다.
    let submit = move || {
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
                    match store.load_measurements(kpi_id.read().clone()).await {
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
                    match store.load_measurements(kpi_id.read().clone()).await {
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
            // 일기 우선: 머리말과 텍스트 영역이 패널의 주인공이고,
            // 측정값 스테퍼는 그 위에 작은 보조 컨트롤로 둔다.
            div { class: "measurement-heading",
                span { class: "diary-label",
                    PencilIcon {}
                    label { {t.diary_heading()} }
                }
            }

            if let Some(error) = panel_error.read().clone() {
                div { class: "form-error", "{error}" }
            }

            div { class: "measurement-add",
                // 직접 입력 대신 − / + 스테퍼로 측정값을 만든다. 한 줄짜리
                // 보조 컨트롤이라 텍스트 영역이 더 도드라진다.
                div { class: "measurement-controls",
                    MeasurementStepper {
                        target_value: props.target_value,
                        aggregation,
                        unit: props.unit.clone(),
                        value: step_value
                    }
                    // 집계 방식 안내는 그 결과가 만들어지는 스테퍼 곁에 둔다.
                    span { class: "measurement-hint", {t.agg_auto_hint(aggregation_label(aggregation, t))} }
                }
                textarea {
                    rows: "4",
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
                // 지난 기록은 연속된 일기 타임라인으로 읽힌다: 날짜·수치는
                // 흐린 머리줄로 두고 메모를 본문처럼 도드라지게 보여 준다.
                div { class: "timeline-divider",
                    span { {t.past_entries()} }
                    span { class: "rule" }
                }
                ul { class: "measurement-timeline",
                    for measurement in measurements.read().iter() {
                        {
                            let measurement_id = measurement.id.clone();
                            let value_text = format_value(measurement.value);
                            let measured_at = format_timestamp(&measurement.measured_at);
                            let note = measurement.note.clone().unwrap_or_default();
                            rsx! {
                                li { class: "timeline-entry",
                                    div { class: "timeline-entry-head",
                                        span { class: "timeline-date", title: "{measurement.measured_at}", "{measured_at}" }
                                        span { class: "timeline-value", "{value_text} {unit}" }
                                        if is_manage {
                                            button {
                                                r#type: "button",
                                                class: "btn row-btn timeline-del",
                                                disabled: *busy.read(),
                                                onclick: move |_| handle_delete(measurement_id.clone()),
                                                {t.delete()}
                                            }
                                        }
                                    }
                                    if !note.is_empty() {
                                        p { class: "timeline-note", "{note}" }
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
