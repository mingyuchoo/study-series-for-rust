#![allow(non_snake_case)]

use super::{measurement_stepper::MeasurementStepper,
            quick_record::use_quick_record,
            record_toast::RecordToastView};
use crate::{i18n::use_lang,
            models::{IkikItem,
                     ItemKind,
                     KpiAggregation,
                     aggregation_label,
                     kind_description,
                     kind_label,
                     status_label,
                     tree::{grouped_rows,
                            kpi_percent,
                            progress_text}}};
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
    let t = *use_lang().read();
    let is_kpi = props.kind == ItemKind::Kpi;
    // 기록·토스트·실행 취소는 퀵 기록 훅이 책임진다.
    let quick = use_quick_record();

    // 측정값 입력 팝오버가 열려 있는 Key Performance Indicator id. 한 번에 하나만
    // 연다.
    let mut open_record = use_signal(|| None::<String>);
    // 팝오버 스테퍼가 만드는 측정값. 팝오버를 열 때 직전 집계값으로 채운다.
    let mut record_value = use_signal(|| 0.0_f64);

    // 합계형 +1과 팝오버 저장이 모두 같은 기록 경로를 쓴다. 성공하면
    // 팝오버를 닫는다.
    let record_measurement = move |item: IkikItem, value: f64| {
        spawn(async move {
            if quick.record(&item, value).await {
                open_record.set(None);
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
                                                            disabled: quick.is_busy(),
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
                                                            disabled: quick.is_busy(),
                                                            onclick: move |evt| {
                                                                evt.stop_propagation();
                                                                if record_open {
                                                                    open_record.set(None);
                                                                } else {
                                                                    // 직전 집계값을 미리 채워 몇 클릭으로 고치면 되게 한다.
                                                                    record_value.set(quick_item.current_value.unwrap_or(0.0).max(0.0));
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
                                                        // 상세 패널과 같은 스테퍼 입력을 쓴다.
                                                        MeasurementStepper {
                                                            target_value: item.target_value,
                                                            aggregation: item.aggregation,
                                                            unit: item.unit.clone(),
                                                            value: record_value
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "btn btn-primary",
                                                            disabled: quick.is_busy(),
                                                            onclick: move |_| record_measurement(save_item.clone(), *record_value.peek()),
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

            RecordToastView { host: quick.toast, on_undo: move |pair| quick.undo(pair) }
        }
    }
}
