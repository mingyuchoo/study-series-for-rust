#![allow(non_snake_case)]

use crate::models::{ItemKind,
                    ItemStatus,
                    VvkikItem,
                    kind_description,
                    status_label,
                    tree::{kpi_percent,
                           progress_text,
                           short_parent_path}};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct VvkikDashboardProps {
    pub items: Vec<VvkikItem>,
    pub is_filtering: bool,
    pub on_open: EventHandler<VvkikItem>,
}

/// KPI 목록 한 줄을 그리는 데 필요한 값 묶음.
struct KpiRow {
    item: VvkikItem,
    percent: Option<i64>,
    progress: Option<String>,
    path: Option<String>,
}

/// 대시보드 탭: 단계별 총 수량과 상태별 수량, KPI 달성 현황을
/// 한눈에 보여 준다. KPI는 주의가 필요한 지표가 먼저 보이도록
/// 달성률 낮은 순으로 나열한다.
pub fn VvkikDashboard(props: VvkikDashboardProps) -> Element {
    let items = &props.items;

    // 단계별 (총 수량, 상태별 수량). 상태 순서는 ItemStatus::ALL을 따른다.
    let kind_cards: Vec<(ItemKind, usize, [usize; 3])> = ItemKind::ALL
        .iter()
        .map(|&kind| {
            let mut by_status = [0usize; 3];
            let mut total = 0usize;
            for item in items.iter().filter(|item| item.kind == kind) {
                total += 1;
                if let Some(index) = ItemStatus::ALL.iter().position(|status| *status == item.status) {
                    by_status[index] += 1;
                }
            }
            (kind, total, by_status)
        })
        .collect();

    let mut kpi_rows: Vec<KpiRow> = items
        .iter()
        .filter(|item| item.kind == ItemKind::Kpi)
        .map(|item| KpiRow {
            item: item.clone(),
            percent: kpi_percent(item),
            progress: progress_text(item),
            path: short_parent_path(item, items),
        })
        .collect();
    // 달성률 낮은 순. 목표가 없어 달성률을 계산할 수 없는 KPI는 뒤로 보낸다.
    kpi_rows.sort_by(|a, b| match (a.percent, b.percent) {
        | (Some(left), Some(right)) => left.cmp(&right).then_with(|| a.item.title.cmp(&b.item.title)),
        | (Some(_), None) => std::cmp::Ordering::Less,
        | (None, Some(_)) => std::cmp::Ordering::Greater,
        | (None, None) => a.item.title.cmp(&b.item.title),
    });

    let percents: Vec<i64> = kpi_rows.iter().filter_map(|row| row.percent).collect();
    let average_percent = (!percents.is_empty()).then(|| (percents.iter().sum::<i64>() as f64 / percents.len() as f64).round() as i64);
    let unmeasured_count = kpi_rows.len() - percents.len();

    rsx! {
        section { class: "vvkik-dashboard",
            if props.is_filtering {
                p { class: "dash-notice", "검색 결과 기준 집계입니다." }
            }

            div { class: "dash-cards",
                for (kind, total, by_status) in kind_cards {
                    div { class: "dash-card",
                        div { class: "dash-card-head",
                            span { class: "dash-card-kind", "{kind.label()}" }
                            span { class: "dash-card-total", "{total}" }
                        }
                        p { class: "dash-card-desc", "{kind_description(kind)}" }
                        div { class: "dash-card-statuses",
                            for (index, status) in ItemStatus::ALL.into_iter().enumerate() {
                                span { class: "dash-status {status}", "{status_label(status)} {by_status[index]}" }
                            }
                        }
                    }
                }
            }

            div { class: "dash-kpi-section",
                div { class: "lane-heading",
                    div {
                        h2 { "KPI 현재 수준" }
                        p {
                            if let Some(average) = average_percent {
                                "평균 달성률 {average}%"
                            } else {
                                "달성률을 계산할 KPI가 없습니다."
                            }
                            if unmeasured_count > 0 {
                                " · 목표 미설정 {unmeasured_count}개"
                            }
                        }
                    }
                    span { class: "lane-count", "{kpi_rows.len()}" }
                }

                if kpi_rows.is_empty() {
                    div { class: "lane-empty", "KPI가 없습니다." }
                } else {
                    div { class: "dash-kpi-list",
                        for KpiRow { item, percent, progress, path } in kpi_rows {
                            {
                                let row_item = item.clone();
                                let path_display = path.unwrap_or_else(|| "최상위".to_string());
                                rsx! {
                                    button {
                                        r#type: "button",
                                        class: "dash-kpi-row",
                                        onclick: move |_| props.on_open.call(row_item.clone()),
                                        div { class: "dash-kpi-info",
                                            span { class: "dash-kpi-title", "{item.title}" }
                                            span { class: "dash-kpi-path", "{path_display}" }
                                        }
                                        div { class: "dash-kpi-progress",
                                            if let Some(percent) = percent {
                                                span { class: "kpi-track dash-kpi-track",
                                                    span { class: "kpi-fill", style: "width: {percent}%;" }
                                                }
                                                span { class: "dash-kpi-percent", "{percent}%" }
                                            }
                                            if let Some(progress) = progress {
                                                span { class: "dash-kpi-text", "{progress}" }
                                            } else {
                                                span { class: "dash-kpi-text", "목표 미설정" }
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
