#![allow(non_snake_case)]

use super::record_grass::RecordGrass;
use crate::{i18n::use_lang,
            models::{ItemKind,
                     ItemStatus,
                     IvkikItem,
                     kind_description,
                     status_label,
                     tree::{kpi_percent,
                            progress_text,
                            short_parent_path}},
            services::IvkikService};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct IvkikDashboardProps {
    pub items: Vec<IvkikItem>,
    pub is_filtering: bool,
    pub on_open: EventHandler<IvkikItem>,
}

/// Key Performance Indicator 목록 한 줄을 그리는 데 필요한 값 묶음.
struct KpiRow {
    item: IvkikItem,
    percent: Option<i64>,
    progress: Option<String>,
    path: Option<String>,
}

/// 대시보드 탭: 단계별 총 수량과 상태별 수량, Key Performance Indicator 달성 현황을
/// 한눈에 보여 준다. Key Performance Indicator는 주의가 필요한 지표가 먼저 보이도록
/// 달성률 낮은 순으로 나열한다.
pub fn IvkikDashboard(props: IvkikDashboardProps) -> Element {
    let t = *use_lang().read();
    let items = &props.items;

    // 전체 Key Performance Indicator의 측정 시각. "오늘 이 시스템과 마주했는가"를 보여 주는
    // 잔디의 재료라 Key Performance Indicator 구분 없이 한 번에 불러온다.
    let mut grass_timestamps = use_signal(Vec::<String>::new);
    use_effect(move || {
        spawn(async move {
            if let Ok(measurements) = IvkikService::list_all_kpi_measurements().await {
                grass_timestamps.set(measurements.into_iter().map(|measurement| measurement.measured_at).collect());
            }
        });
    });

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
            path: short_parent_path(item, items, t),
        })
        .collect();
    // 달성률 낮은 순. 목표가 없어 달성률을 계산할 수 없는 Key Performance Indicator는 뒤로 보낸다.
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
        section { class: "ivkik-dashboard",
            if props.is_filtering {
                p { class: "dash-notice", {t.dash_filter_notice()} }
            }

            div { class: "dash-cards",
                for (kind, total, by_status) in kind_cards {
                    div { class: "dash-card",
                        div { class: "dash-card-head",
                            span { class: "dash-card-kind", "{kind.label()}" }
                            span { class: "dash-card-total", "{total}" }
                        }
                        p { class: "dash-card-desc", {kind_description(kind, t)} }
                        div { class: "dash-card-statuses",
                            for (index, status) in ItemStatus::ALL.into_iter().enumerate() {
                                span { class: "dash-status {status}", "{status_label(status)} {by_status[index]}" }
                            }
                        }
                    }
                }
            }

            div { class: "dash-grass",
                RecordGrass { timestamps: grass_timestamps.read().clone(), scope: Some(t.all_kpis().to_string()) }
            }

            div { class: "dash-kpi-section",
                div { class: "lane-heading",
                    div {
                        h2 { {t.kpi_levels_heading()} }
                        p {
                            if let Some(average) = average_percent {
                                {t.dash_average(average)}
                            } else {
                                {t.dash_no_measurable()}
                            }
                            if unmeasured_count > 0 {
                                {t.dash_no_target_suffix(unmeasured_count)}
                            }
                        }
                    }
                    span { class: "lane-count", "{kpi_rows.len()}" }
                }

                if kpi_rows.is_empty() {
                    div { class: "lane-empty", {t.no_kpis()} }
                } else {
                    div { class: "dash-kpi-list",
                        for KpiRow { item, percent, progress, path } in kpi_rows {
                            {
                                let row_item = item.clone();
                                let path_display = path.unwrap_or_else(|| t.top_level().to_string());
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
                                                span { class: "dash-kpi-text", {t.no_target()} }
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
