#![allow(non_snake_case)]

use super::kpi_measurements::KpiMeasurementPanel;
use crate::{models::{ItemKind,
                     ItemRevision,
                     VvkikItem,
                     aggregation_label,
                     revision_field_label,
                     revision_value_label,
                     status_label,
                     tree::{kpi_percent,
                            parent_chain,
                            progress_text}},
            services::VvkikService};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ItemDetailProps {
    pub item: VvkikItem,
    pub items: Vec<VvkikItem>,
    /// 「수정」 버튼: 수정 폼으로 진입한다.
    pub on_edit: EventHandler<VvkikItem>,
    pub on_delete: EventHandler<VvkikItem>,
    /// 브레드크럼에서 조상을 클릭하면 그 항목의 상세 화면으로 이동한다.
    #[props(default)]
    pub on_navigate: EventHandler<VvkikItem>,
    /// 「목록으로」: 보드로 돌아간다.
    pub on_back: EventHandler<()>,
    /// 측정 기록 추가·삭제로 데이터가 바뀌면 호출된다.
    #[props(default)]
    pub on_data_change: EventHandler<()>,
}

/// RFC3339 시각(UTC)에서 분 단위까지만 잘라 보여 준다.
fn format_timestamp(timestamp: &str) -> String { timestamp.chars().take(16).map(|ch| if ch == 'T' { ' ' } else { ch }).collect() }

/// 항목 상세 보기. 등록된 내용은 기준 문서이므로 읽기 전용으로 보여
/// 주고, 수정은 별도 버튼으로만 진입한다. KPI는 실적 기록을 여기서
/// 바로 남길 수 있고, 하단에 정의 변경 이력이 쌓인다.
pub fn ItemDetail(props: ItemDetailProps) -> Element {
    let item = props.item.clone();
    let is_kpi = item.kind == ItemKind::Kpi;

    let mut revisions = use_signal(Vec::<ItemRevision>::new);
    let mut revisions_error = use_signal(|| None::<String>);
    // KpiMeasurementPanel과 공유하는 시그널. 상세 화면에서는 패널이
    // 집계한 현재값을 별도로 쓰지 않지만 패널 계약상 필요하다.
    let has_measurements = use_signal(|| false);
    let panel_current_value = use_signal(String::new);

    let revision_item_id = use_signal(|| item.id.clone());
    use_effect(move || {
        spawn(async move {
            match VvkikService::list_item_revisions(revision_item_id.read().clone()).await {
                | Ok(list) => revisions.set(list),
                | Err(e) => revisions_error.set(Some(format!("변경 이력을 불러오지 못했습니다: {e}"))),
            }
        });
    });

    let chain = parent_chain(&item, &props.items);
    let progress = progress_text(&item);
    let percent = kpi_percent(&item);
    let edit_item = item.clone();
    let delete_item = item.clone();
    let description = item.description.clone().filter(|text| !text.is_empty());
    let created_at = format_timestamp(&item.created_at);

    rsx! {
        div { class: "item-detail",
            nav { class: "edit-breadcrumb", aria_label: "상위 항목 경로",
                for ancestor in chain.iter() {
                    {
                        let ancestor = ancestor.clone();
                        let ancestor_title = ancestor.title.clone();
                        let ancestor_kind = ancestor.kind.label();
                        rsx! {
                            span { class: "breadcrumb-seg",
                                span { class: "breadcrumb-kind", "{ancestor_kind}" }
                                button {
                                    r#type: "button",
                                    class: "breadcrumb-link",
                                    title: "{ancestor_kind} 상세로 이동",
                                    onclick: move |_| props.on_navigate.call(ancestor.clone()),
                                    "{ancestor_title}"
                                }
                            }
                            span { class: "breadcrumb-sep", "›" }
                        }
                    }
                }
                span { class: "breadcrumb-seg",
                    span { class: "breadcrumb-kind", "{item.kind.label()}" }
                    span { class: "breadcrumb-current", "{item.title}" }
                }
            }

            div { class: "detail-heading",
                div { class: "detail-title-block",
                    h2 { "{item.title}" }
                    span { class: "status-pill {item.status}", "{status_label(item.status)}" }
                }
                div { class: "detail-actions",
                    button {
                        r#type: "button",
                        class: "btn btn-primary",
                        onclick: move |_| props.on_edit.call(edit_item.clone()),
                        "수정"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        onclick: move |_| props.on_delete.call(delete_item.clone()),
                        "삭제"
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        onclick: move |_| props.on_back.call(()),
                        "목록으로"
                    }
                }
            }

            if let Some(description) = description {
                p { class: "detail-description", "{description}" }
            } else {
                p { class: "detail-description empty", "설명 없음" }
            }

            if is_kpi {
                div { class: "detail-progress",
                    if let Some(progress) = progress {
                        if let Some(percent) = percent {
                            span { class: "kpi-track detail-kpi-track",
                                span { class: "kpi-fill", style: "width: {percent}%;" }
                            }
                            span { class: "detail-progress-text", "{progress}" }
                            span { class: "detail-progress-hint", "{aggregation_label(item.aggregation)} 집계 · {percent}% 달성" }
                        } else {
                            span { class: "detail-progress-text", "{progress}" }
                            span { class: "detail-progress-hint", "{aggregation_label(item.aggregation)} 집계" }
                        }
                    } else {
                        span { class: "detail-progress-hint", "아직 기록이 없습니다. 아래에서 첫 실적을 기록해 보세요." }
                    }
                }

                KpiMeasurementPanel {
                    kpi_id: item.id.clone(),
                    aggregation: item.aggregation,
                    unit: item.unit.clone(),
                    has_measurements,
                    current_value: panel_current_value,
                    on_data_change: props.on_data_change
                }
            }

            div { class: "detail-revisions",
                h3 { "변경 이력" }
                if let Some(error) = revisions_error.read().clone() {
                    div { class: "form-error", "{error}" }
                }
                ul { class: "revision-list",
                    for revision in revisions.read().iter() {
                        {
                            let field_label = revision_field_label(&revision.field);
                            let old_value = revision_value_label(&revision.field, revision.old_value.as_deref());
                            let new_value = revision_value_label(&revision.field, revision.new_value.as_deref());
                            let changed_at = format_timestamp(&revision.changed_at);
                            rsx! {
                                li { class: "revision-row",
                                    span { class: "revision-change",
                                        span { class: "revision-field", "{field_label}" }
                                        span { class: "revision-old", "{old_value}" }
                                        span { class: "revision-arrow", "→" }
                                        span { class: "revision-new", "{new_value}" }
                                    }
                                    span { class: "revision-date", title: "{revision.changed_at}", "{changed_at}" }
                                }
                            }
                        }
                    }
                    li { class: "revision-row",
                        span { class: "revision-change",
                            span { class: "revision-field", "항목 생성" }
                        }
                        span { class: "revision-date", title: "{item.created_at}", "{created_at}" }
                    }
                }
            }
        }
    }
}
