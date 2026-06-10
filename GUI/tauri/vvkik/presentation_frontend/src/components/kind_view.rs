#![allow(non_snake_case)]

use crate::models::{ItemKind,
                    VvkikItem,
                    kind_description,
                    status_label,
                    tree::{kpi_percent,
                           parent_path,
                           progress_text,
                           short_parent_path}};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikKindViewProps {
    pub kind: ItemKind,
    pub items: Vec<VvkikItem>,
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
}

/// 한 단계의 항목들을 표로 보여 주는 탭 화면. 상위 경로를 컬럼으로
/// 노출해 모든 행에서 위상이 보이고, KPI 탭은 설명 대신 진행률을
/// 보여 준다.
pub fn VvkikKindView(props: VvkikKindViewProps) -> Element {
    let is_kpi = props.kind == ItemKind::Kpi;

    // (항목, 짧은 경로, 전체 경로 툴팁) — 경로 → 정렬값 → 제목 순으로
    // 정렬해 같은 가지의 항목이 모이게 한다.
    let mut rows: Vec<(VvkikItem, Option<String>, Option<String>)> = props
        .items
        .iter()
        .filter(|item| item.kind == props.kind)
        .map(|item| (item.clone(), short_parent_path(item, &props.items), parent_path(item, &props.items)))
        .collect();
    rows.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.position.cmp(&b.0.position)).then(a.0.title.cmp(&b.0.title)));

    rsx! {
        section { class: "vvkik-lane",
            div { class: "lane-heading",
                div {
                    h2 { "{props.kind.label()}" }
                    p { "{kind_description(props.kind)}" }
                }
                span { class: "lane-count", "{rows.len()}" }
            }
            if rows.is_empty() {
                div { class: "lane-empty", "비어 있음" }
            } else {
                table { class: "kind-table",
                    thead {
                        tr {
                            th { class: "col-title", "제목" }
                            th { class: "col-path", "상위 경로" }
                            if is_kpi {
                                th { class: "col-mid", "진행률" }
                            } else {
                                th { class: "col-mid col-desc", "설명" }
                            }
                            th { class: "col-status", "상태" }
                            th { class: "col-actions" }
                        }
                    }
                    tbody {
                        for (item, short_path, full_path) in rows {
                            {
                                let path_display = short_path.unwrap_or_else(|| "최상위".to_string());
                                let path_tooltip = full_path.unwrap_or_else(|| "최상위".to_string());
                                let description = item.description.clone().filter(|text| !text.is_empty()).unwrap_or_else(|| "—".to_string());
                                let progress = progress_text(&item);
                                let percent = kpi_percent(&item);
                                let row_item = item.clone();
                                let delete_item = item.clone();
                                rsx! {
                                    tr {
                                        // 행 전체가 수정 진입점이다. 삭제 버튼만 전파를
                                        // 차단해 행 클릭과 분리한다.
                                        onclick: move |_| props.on_edit.call(row_item.clone()),
                                        td { class: "cell-title", title: "{item.title}", "{item.title}" }
                                        td { class: "cell-path", title: "{path_tooltip}", "{path_display}" }
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
                                                    class: "btn row-btn",
                                                    onclick: move |evt| {
                                                        evt.stop_propagation();
                                                        props.on_delete.call(delete_item.clone());
                                                    },
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
        }
    }
}
