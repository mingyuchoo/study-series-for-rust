#![allow(non_snake_case)]

use super::record_toast::{RecordToastView,
                          use_record_toast};
use crate::{i18n::use_lang,
            models::{IkikItem,
                     ItemKind,
                     KpiAggregation,
                     RecordKpiMeasurementRequest,
                     aggregation_label,
                     format_value,
                     kind_description,
                     kind_label,
                     status_label,
                     tree::{grouped_rows,
                            kpi_percent,
                            progress_text}},
            store::IkikStore};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct IkikKindViewProps {
    pub kind: ItemKind,
    pub items: Vec<IkikItem>,
    pub on_open: EventHandler<IkikItem>,
}

/// 한 단계의 항목들을 표로 보여 주는 탭 화면. 같은 상위 경로의 항목을
/// 묶고 경로를 그룹 헤더로 한 번만 보여 주어, 어떤 가치·목표 아래의
/// 항목인지 줄임 없이 드러낸다. Key Performance Indicator 탭은 설명 대신
/// 진행률을 보여 주고, 수정 화면에 들어가지 않고도 실적을 기록할 수 있도록 집계
/// 방식에 맞는 퀵 기록 버튼을 항상 노출한다.
pub fn IkikKindView(props: IkikKindViewProps) -> Element {
    let lang = use_lang();
    let t = *lang.read();
    let is_kpi = props.kind == ItemKind::Kpi;
    let store = use_context::<IkikStore>();
    let toast = use_record_toast();

    // 측정값 입력 팝오버가 열려 있는 Key Performance Indicator id. 한 번에 하나만
    // 연다.
    let mut open_record = use_signal(|| None::<String>);
    let mut record_input = use_signal(String::new);
    let mut record_busy = use_signal(|| false);

    // 측정값 한 건을 기록하고 토스트로 결과를 알린다. 합계형 +1과
    // 팝오버 저장이 모두 이 경로를 쓴다.
    let record_measurement = move |item: IkikItem, value: f64| {
        if *record_busy.read() {
            return;
        }

        spawn(async move {
            record_busy.set(true);
            let request = RecordKpiMeasurementRequest {
                kpi_id: item.id.clone(),
                value,
                note: None,
            };
            match store.record_measurement(request).await {
                | Ok(measurement) => {
                    open_record.set(None);
                    let unit = item.unit.clone().unwrap_or_default();
                    let amount = if item.aggregation == KpiAggregation::Sum {
                        format!("+{}", format_value(value))
                    } else {
                        format_value(value)
                    };
                    let message = lang.peek().recorded_toast(&item.title, &amount, &unit);
                    toast.show(message, Some((item.id.clone(), measurement.id)));
                },
                | Err(e) => toast.show(lang.peek().err_record(&e), None),
            }
            record_busy.set(false);
        });
    };

    // 팝오버 입력값을 검증해 기록한다.
    let submit_record = move |item: IkikItem| {
        let raw = record_input.read().trim().to_string();
        match raw.parse::<f64>() {
            | Ok(value) => record_measurement(item, value),
            | Err(_) => toast.show(lang.peek().value_must_be_number().to_string(), None),
        }
    };

    // 실행 취소: 방금 추가한 측정값을 지우고 목록을 새로고침한다.
    let handle_undo = move |(kpi_id, measurement_id): (String, String)| {
        spawn(async move {
            if let Err(e) = store.delete_measurement(kpi_id, measurement_id).await {
                toast.show(lang.peek().err_undo(&e), None);
            }
        });
    };

    let display = grouped_rows(props.kind, &props.items, t);
    let row_count = display.len();
    let any_grouped = display.iter().any(|(header, _)| header.is_some());

    rsx! {
        section { class: "ikik-lane",
            div { class: "lane-heading",
                div {
                    h2 { {kind_label(props.kind, t)} }
                    p { {kind_description(props.kind, t)} }
                }
                span { class: "lane-count", "{row_count}" }
            }
            if display.is_empty() {
                div { class: "lane-empty", {t.empty_lane()} }
            } else {
                table { class: "kind-table",
                    thead {
                        tr {
                            th { class: "col-title", {t.title_label()} }
                            if is_kpi {
                                th { class: "col-mid col-mid-kpi", {t.progress_label()} }
                            } else {
                                th { class: "col-mid col-desc", {t.description_label()} }
                            }
                            th { class: "col-status", {t.status_field()} }
                            // 동작 컬럼은 퀵 기록 버튼이 있는 Key Performance Indicator 탭에만 둔다.
                            if is_kpi {
                                th { class: "col-actions" }
                            }
                        }
                    }
                    tbody {
                        for (header, item) in display {
                            {
                                let description = item.description.clone().filter(|text| !text.is_empty()).unwrap_or_else(|| "—".to_string());
                                let progress = progress_text(&item);
                                let percent = kpi_percent(&item);
                                let unit = item.unit.clone().unwrap_or_default();
                                let is_sum = item.aggregation == KpiAggregation::Sum;
                                let record_open = is_kpi && *open_record.read() == Some(item.id.clone());
                                let title_class = if any_grouped { "cell-title grouped" } else { "cell-title" };
                                let row_item = item.clone();
                                let quick_item = item.clone();
                                let submit_item = item.clone();
                                let save_item = item.clone();
                                rsx! {
                                    if let Some(header) = header {
                                        tr { class: "group-row",
                                            td { colspan: if is_kpi { "4" } else { "3" }, title: "{header.tooltip}",
                                                if !header.prefix.is_empty() {
                                                    span { class: "group-path-prefix", "{header.prefix} › " }
                                                }
                                                span { class: "group-path-leaf", "{header.leaf}" }
                                            }
                                        }
                                    }
                                    tr {
                                        // 행 전체가 상세 보기 진입점이다. 행 안의 버튼들만
                                        // 전파를 차단해 행 클릭과 분리한다.
                                        onclick: move |_| props.on_open.call(row_item.clone()),
                                        td { class: title_class, title: "{item.title}", "{item.title}" }
                                        if is_kpi {
                                            td { class: "cell-kpi",
                                                if let Some(progress) = progress {
                                                    span { class: "cell-kpi-wrap",
                                                        if let Some(percent) = percent {
                                                            span { class: "kpi-track",
                                                                span { class: "kpi-fill", style: "width: {percent}%;" }
                                                            }
                                                        }
                                                        span { class: "cell-kpi-text", "{progress}" }
                                                    }
                                                } else {
                                                    span { class: "cell-empty", "—" }
                                                }
                                            }
                                        } else {
                                            td { class: "col-desc cell-desc", title: "{description}", "{description}" }
                                        }
                                        td {
                                            span { class: "status-pill {item.status}", "{status_label(item.status, t)}" }
                                        }
                                        // 삭제는 실수 클릭을 막기 위해 목록에서 빼고, 상세
                                        // 화면과 전체 구조 트리에서만 할 수 있다.
                                        if is_kpi {
                                            td { class: "cell-actions",
                                                div { class: "table-actions",
                                                    // 합계형은 한 번의 클릭으로 1을 누적하고,
                                                    // 최신값·평균형은 측정값 입력 팝오버를 연다.
                                                    if is_sum {
                                                        button {
                                                            r#type: "button",
                                                            class: "btn row-btn quick-record-btn",
                                                            disabled: *record_busy.read(),
                                                            onclick: move |evt| {
                                                                evt.stop_propagation();
                                                                record_measurement(quick_item.clone(), 1.0);
                                                            },
                                                            if unit.is_empty() { "+1" } else { "+1 {unit}" }
                                                        }
                                                    } else {
                                                        button {
                                                            r#type: "button",
                                                            class: "btn row-btn quick-record-btn",
                                                            disabled: *record_busy.read(),
                                                            onclick: move |evt| {
                                                                evt.stop_propagation();
                                                                if record_open {
                                                                    open_record.set(None);
                                                                } else {
                                                                    // 직전 집계값을 미리 채워 숫자만 고치면 되게 한다.
                                                                    record_input.set(quick_item.current_value.map(format_value).unwrap_or_default());
                                                                    open_record.set(Some(quick_item.id.clone()));
                                                                }
                                                            },
                                                            {t.record()}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if record_open {
                                        tr { class: "record-row",
                                            td { colspan: "4",
                                                div { class: "record-popover",
                                                    span { class: "record-popover-hint",
                                                        {t.record_popover_hint(aggregation_label(item.aggregation, t))}
                                                    }
                                                    div { class: "record-popover-controls",
                                                        input {
                                                            r#type: "number",
                                                            step: "any",
                                                            class: "record-popover-input",
                                                            autofocus: true,
                                                            placeholder: t.value_placeholder(),
                                                            value: "{record_input}",
                                                            oninput: move |evt| record_input.set(evt.value()),
                                                            onkeydown: move |evt| {
                                                                if evt.key() == Key::Enter {
                                                                    submit_record(submit_item.clone());
                                                                }
                                                            }
                                                        }
                                                        if !unit.is_empty() {
                                                            span { class: "record-popover-unit", "{unit}" }
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "btn btn-primary",
                                                            disabled: *record_busy.read(),
                                                            onclick: move |_| submit_record(save_item.clone()),
                                                            {t.save()}
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "btn btn-secondary",
                                                            onclick: move |_| open_record.set(None),
                                                            {t.cancel()}
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
            }

            RecordToastView { host: toast, on_undo: handle_undo }
        }
    }
}
