#![allow(non_snake_case)]

use crate::{models::{ItemKind,
                     KpiAggregation,
                     RecordKpiMeasurementRequest,
                     VvkikItem,
                     aggregation_label,
                     kind_description,
                     status_label,
                     tree::{kpi_percent,
                            parent_chain,
                            parent_path,
                            progress_text}},
            services::VvkikService};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikKindViewProps {
    pub kind: ItemKind,
    pub items: Vec<VvkikItem>,
    pub on_open: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
    /// 퀵 기록으로 측정값이 추가·삭제되면 호출된다. 목록이 새 현재값을
    /// 반영하게 한다.
    #[props(default)]
    pub on_data_change: EventHandler<()>,
}

/// 기록 직후 띄우는 토스트. `undo`가 있으면 실행 취소 버튼으로 방금
/// 추가한 측정값을 지울 수 있다.
#[derive(Clone, PartialEq)]
struct RecordToast {
    message: String,
    /// (kpi_id, measurement_id)
    undo: Option<(String, String)>,
}

/// 상위 경로 그룹 헤더 한 줄. 직계 부모(leaf)만 진하게 보여 준다.
#[derive(Clone, PartialEq)]
struct GroupHeader {
    prefix: String,
    leaf: String,
    tooltip: String,
}

fn format_value(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

/// wasm에는 스레드 sleep이 없어 setTimeout을 Future로 감싼다.
async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// 한 단계의 항목들을 표로 보여 주는 탭 화면. 같은 상위 경로의 항목을
/// 묶고 경로를 그룹 헤더로 한 번만 보여 주어, 어떤 가치·목표 아래의
/// 항목인지 줄임 없이 드러낸다. KPI 탭은 설명 대신 진행률을 보여 주고,
/// 수정 화면에 들어가지 않고도 실적을 기록할 수 있도록 집계 방식에
/// 맞는 퀵 기록 버튼을 항상 노출한다.
pub fn VvkikKindView(props: VvkikKindViewProps) -> Element {
    let is_kpi = props.kind == ItemKind::Kpi;
    let on_data_change = props.on_data_change;

    // 측정값 입력 팝오버가 열려 있는 KPI id. 한 번에 하나만 연다.
    let mut open_record = use_signal(|| None::<String>);
    let mut record_input = use_signal(String::new);
    let mut record_busy = use_signal(|| false);
    let mut toast = use_signal(|| None::<RecordToast>);
    // 토스트 자동 닫기 타이머가 뒤늦게 다른 토스트를 닫지 않도록
    // 세대 번호로 구분한다.
    let mut toast_seq = use_signal(|| 0u64);

    let mut show_toast = move |message: String, undo: Option<(String, String)>| {
        let seq = *toast_seq.read() + 1;
        toast_seq.set(seq);
        toast.set(Some(RecordToast { message, undo }));
        spawn(async move {
            sleep_ms(4000).await;
            if *toast_seq.read() == seq {
                toast.set(None);
            }
        });
    };

    // 측정값 한 건을 기록하고 토스트로 결과를 알린다. 합계형 +1과
    // 팝오버 저장이 모두 이 경로를 쓴다.
    let record_measurement = move |item: VvkikItem, value: f64| {
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
            match VvkikService::record_kpi_measurement(request).await {
                | Ok(measurement) => {
                    open_record.set(None);
                    let unit = item.unit.clone().unwrap_or_default();
                    let amount = if item.aggregation == KpiAggregation::Sum {
                        format!("+{}", format_value(value))
                    } else {
                        format_value(value)
                    };
                    let message = format!("\"{}\" {amount} {unit} 기록됨", item.title).trim_end().to_string();
                    show_toast(message, Some((item.id.clone(), measurement.id)));
                    on_data_change.call(());
                },
                | Err(e) => show_toast(format!("실적 기록에 실패했습니다: {e}"), None),
            }
            record_busy.set(false);
        });
    };

    // 팝오버 입력값을 검증해 기록한다.
    let mut submit_record = move |item: VvkikItem| {
        let raw = record_input.read().trim().to_string();
        match raw.parse::<f64>() {
            | Ok(value) => record_measurement(item, value),
            | Err(_) => show_toast("측정값은 숫자로 입력하세요.".to_string(), None),
        }
    };

    // 실행 취소: 방금 추가한 측정값을 지우고 목록을 새로고침한다.
    let handle_undo = move |_| {
        let Some(current) = toast.read().clone() else {
            return;
        };
        let Some((kpi_id, measurement_id)) = current.undo else {
            return;
        };

        spawn(async move {
            toast.set(None);
            match VvkikService::delete_kpi_measurement(kpi_id, measurement_id).await {
                | Ok(()) => on_data_change.call(()),
                | Err(e) => show_toast(format!("실행 취소에 실패했습니다: {e}"), None),
            }
        });
    };

    // (항목, 조상 제목들, 전체 경로) — 경로 → 정렬값 → 제목 순으로
    // 정렬해 같은 가지(같은 Value·Vision·KRA·IGT)의 항목이 모이게 한다.
    let mut rows: Vec<(VvkikItem, Vec<String>, Option<String>)> = props
        .items
        .iter()
        .filter(|item| item.kind == props.kind)
        .map(|item| {
            let chain: Vec<String> = parent_chain(item, &props.items).iter().map(|parent| parent.title.clone()).collect();
            (item.clone(), chain, parent_path(item, &props.items))
        })
        .collect();
    rows.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.position.cmp(&b.0.position)).then(a.0.title.cmp(&b.0.title)));

    // 최상위뿐인 탭(Value 등)은 그룹 헤더 없이 평평한 표로 보여 준다.
    let any_grouped = rows.iter().any(|(_, _, full)| full.is_some());
    let row_count = rows.len();

    // 경로가 바뀌는 행 앞에만 그룹 헤더를 끼워 넣는다.
    let mut display: Vec<(Option<GroupHeader>, VvkikItem)> = Vec::with_capacity(row_count);
    let mut previous_path: Option<Option<String>> = None;
    for (item, chain, full_path) in rows {
        let header = if any_grouped && previous_path.as_ref() != Some(&full_path) {
            let (prefix, leaf) = match chain.split_last() {
                | Some((leaf, ancestors)) => (ancestors.join(" › "), leaf.clone()),
                | None if item.parent_id.is_some() => (String::new(), "(연결된 상위 항목 없음)".to_string()),
                | None => (String::new(), "최상위".to_string()),
            };
            let tooltip = full_path.clone().unwrap_or_else(|| "최상위".to_string());
            Some(GroupHeader { prefix, leaf, tooltip })
        } else {
            None
        };
        previous_path = Some(full_path);
        display.push((header, item));
    }

    rsx! {
        section { class: "vvkik-lane",
            div { class: "lane-heading",
                div {
                    h2 { "{props.kind.label()}" }
                    p { "{kind_description(props.kind)}" }
                }
                span { class: "lane-count", "{row_count}" }
            }
            if display.is_empty() {
                div { class: "lane-empty", "비어 있음" }
            } else {
                table { class: "kind-table",
                    thead {
                        tr {
                            th { class: "col-title", "제목" }
                            if is_kpi {
                                th { class: "col-mid col-mid-kpi", "진행률" }
                            } else {
                                th { class: "col-mid col-desc", "설명" }
                            }
                            th { class: "col-status", "상태" }
                            th { class: if is_kpi { "col-actions col-actions-kpi" } else { "col-actions" } }
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
                                let delete_item = item.clone();
                                let quick_item = item.clone();
                                let submit_item = item.clone();
                                let save_item = item.clone();
                                rsx! {
                                    if let Some(header) = header {
                                        tr { class: "group-row",
                                            td { colspan: "4", title: "{header.tooltip}",
                                                if !header.prefix.is_empty() {
                                                    span { class: "group-path-prefix", "{header.prefix} › " }
                                                }
                                                span { class: "group-path-leaf", "{header.leaf}" }
                                            }
                                        }
                                    }
                                    tr {
                                        // 행 전체가 수정 진입점이다. 행 안의 버튼들만
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
                                            span { class: "status-pill {item.status}", "{status_label(item.status)}" }
                                        }
                                        td { class: "cell-actions",
                                            div { class: "table-actions",
                                                button {
                                                    r#type: "button",
                                                    class: "btn row-btn row-hover-btn",
                                                    onclick: move |evt| {
                                                        evt.stop_propagation();
                                                        props.on_delete.call(delete_item.clone());
                                                    },
                                                    "삭제"
                                                }
                                                if is_kpi {
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
                                                            "기록"
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
                                                        "측정값을 입력하면 {aggregation_label(item.aggregation)}(으)로 현재값에 바로 반영됩니다."
                                                    }
                                                    div { class: "record-popover-controls",
                                                        input {
                                                            r#type: "number",
                                                            step: "any",
                                                            class: "record-popover-input",
                                                            autofocus: true,
                                                            placeholder: "측정값",
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
                                                            "저장"
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "btn btn-secondary",
                                                            onclick: move |_| open_record.set(None),
                                                            "취소"
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

            if let Some(current) = toast.read().clone() {
                div { class: "record-toast", role: "status",
                    span { "{current.message}" }
                    if current.undo.is_some() {
                        button {
                            r#type: "button",
                            class: "btn row-btn",
                            onclick: handle_undo,
                            "실행 취소"
                        }
                    }
                }
            }
        }
    }
}
