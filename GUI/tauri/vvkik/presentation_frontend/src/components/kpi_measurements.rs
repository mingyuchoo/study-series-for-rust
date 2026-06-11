#![allow(non_snake_case)]

use crate::{models::{KpiMeasurement,
                     RecordKpiMeasurementRequest,
                     aggregation_label},
            services::VvkikService};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct KpiMeasurementPanelProps {
    pub kpi_id: String,
    pub aggregation: crate::models::KpiAggregation,
    pub unit: Option<String>,
    /// 기록 유무를 폼과 공유해 현재값 입력을 잠근다.
    pub has_measurements: Signal<bool>,
    /// 집계된 현재값을 폼의 현재값 입력에도 반영한다.
    pub current_value: Signal<String>,
    /// 기록이 추가·삭제되면 호출된다. 목록 화면이 새 현재값을 반영하게
    /// 한다.
    #[props(default)]
    pub on_data_change: EventHandler<()>,
}

fn format_value(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

/// RFC3339 시각(UTC)에서 분 단위까지만 잘라 보여 준다.
fn format_measured_at(measured_at: &str) -> String { measured_at.chars().take(16).map(|ch| if ch == 'T' { ' ' } else { ch }).collect() }

/// KPI 수정 화면의 "실적 기록" 패널. 일기처럼 값과 메모를 기록하면
/// 백엔드가 집계 방식대로 현재값을 다시 계산한다.
pub fn KpiMeasurementPanel(props: KpiMeasurementPanelProps) -> Element {
    let kpi_id = use_signal(|| props.kpi_id.clone());
    let aggregation = props.aggregation;
    let on_data_change = props.on_data_change;
    let unit = props.unit.clone().unwrap_or_default();

    let mut measurements = use_signal(Vec::<KpiMeasurement>::new);
    let mut value_input = use_signal(String::new);
    let mut note_input = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut panel_error = use_signal(|| None::<String>);
    let mut has_measurements = props.has_measurements;
    let mut current_value = props.current_value;

    // 목록을 받아 패널 상태와 폼의 현재값 표시를 한꺼번에 갱신한다.
    let mut apply = move |list: Vec<KpiMeasurement>| {
        has_measurements.set(!list.is_empty());
        let values: Vec<f64> = list.iter().map(|measurement| measurement.value).collect();
        if let Some(aggregated) = aggregation.aggregate(&values) {
            current_value.set(format_value(aggregated));
        }
        measurements.set(list);
    };

    use_effect(move || {
        spawn(async move {
            match VvkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                | Ok(list) => apply(list),
                | Err(e) => panel_error.set(Some(format!("실적 기록을 불러오지 못했습니다: {e}"))),
            }
        });
    });

    let handle_add = move |_| {
        if *busy.read() {
            return;
        }

        let raw = value_input.read().trim().to_string();
        let Ok(value) = raw.parse::<f64>() else {
            panel_error.set(Some("측정값은 숫자로 입력하세요.".to_string()));
            return;
        };
        let note = note_input.read().trim().to_string();

        spawn(async move {
            busy.set(true);
            let request = RecordKpiMeasurementRequest {
                kpi_id: kpi_id.read().clone(),
                value,
                note: (!note.is_empty()).then_some(note),
            };
            match VvkikService::record_kpi_measurement(request).await {
                | Ok(_) => {
                    value_input.set(String::new());
                    note_input.set(String::new());
                    panel_error.set(None);
                    match VvkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                        | Ok(list) => apply(list),
                        | Err(e) => panel_error.set(Some(e)),
                    }
                    on_data_change.call(());
                },
                | Err(e) => panel_error.set(Some(format!("실적 기록 추가에 실패했습니다: {e}"))),
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
            match VvkikService::delete_kpi_measurement(kpi_id.read().clone(), measurement_id).await {
                | Ok(()) => {
                    panel_error.set(None);
                    match VvkikService::list_kpi_measurements(kpi_id.read().clone()).await {
                        | Ok(list) => {
                            // 마지막 기록을 지우면 집계값이 없어지므로 표시도 비운다.
                            if list.is_empty() {
                                current_value.set(String::new());
                            }
                            apply(list);
                        },
                        | Err(e) => panel_error.set(Some(e)),
                    }
                    on_data_change.call(());
                },
                | Err(e) => panel_error.set(Some(format!("실적 기록 삭제에 실패했습니다: {e}"))),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "measurement-panel",
            div { class: "measurement-heading",
                label { "실적 기록" }
                span { class: "measurement-hint", "{aggregation_label(aggregation)}(으)로 현재값에 자동 집계됩니다." }
            }

            if let Some(error) = panel_error.read().clone() {
                div { class: "form-error", "{error}" }
            }

            div { class: "measurement-add",
                input {
                    r#type: "number",
                    step: "any",
                    class: "measurement-value-input",
                    placeholder: "측정값",
                    value: "{value_input}",
                    oninput: move |evt| value_input.set(evt.value())
                }
                if !unit.is_empty() {
                    span { class: "measurement-unit", "{unit}" }
                }
                input {
                    r#type: "text",
                    class: "measurement-note-input",
                    placeholder: "메모 (선택)",
                    value: "{note_input}",
                    oninput: move |evt| note_input.set(evt.value())
                }
                button {
                    r#type: "button",
                    class: "btn btn-secondary",
                    disabled: *busy.read(),
                    onclick: handle_add,
                    "기록"
                }
            }

            if measurements.read().is_empty() {
                p { class: "measurement-empty", "아직 기록이 없습니다. 첫 실적을 기록해 보세요." }
            } else {
                ul { class: "measurement-list",
                    for measurement in measurements.read().iter() {
                        {
                            let measurement_id = measurement.id.clone();
                            let value_text = format_value(measurement.value);
                            let measured_at = format_measured_at(&measurement.measured_at);
                            let note = measurement.note.clone().unwrap_or_default();
                            rsx! {
                                li { class: "measurement-row",
                                    span { class: "measurement-value", "{value_text} {unit}" }
                                    span { class: "measurement-note", "{note}" }
                                    span { class: "measurement-date", title: "{measurement.measured_at}", "{measured_at}" }
                                    button {
                                        r#type: "button",
                                        class: "btn row-btn",
                                        disabled: *busy.read(),
                                        onclick: move |_| handle_delete(measurement_id.clone()),
                                        "삭제"
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
