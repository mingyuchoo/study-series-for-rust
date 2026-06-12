#![allow(non_snake_case)]

use super::{breadcrumb::Breadcrumb,
            kpi_measurements::KpiMeasurementPanel};
use crate::{i18n::use_lang,
            models::{IkikItem,
                     ItemKind,
                     ItemRevision,
                     aggregation_label,
                     deadline::{due_chip,
                                local_today},
                     format_timestamp,
                     kind_label,
                     revision_field_label,
                     revision_value_label,
                     status_label,
                     tree::{MAX_TREE_DEPTH,
                            has_children,
                            kpi_percent,
                            parent_chain,
                            progress_text,
                            sorted_children}},
            store::IkikStore};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ItemDetailProps {
    pub item: IkikItem,
    pub items: Vec<IkikItem>,
    /// 「수정」 버튼: 수정 폼으로 진입한다.
    pub on_edit: EventHandler<IkikItem>,
    pub on_delete: EventHandler<IkikItem>,
    /// 브레드크럼에서 조상을 클릭하면 그 항목의 상세 화면으로 이동한다.
    #[props(default)]
    pub on_navigate: EventHandler<IkikItem>,
    /// 「목록으로」: 보드로 돌아간다.
    pub on_back: EventHandler<()>,
}

/// 항목 상세 보기. 등록된 내용은 기준 문서이므로 읽기 전용으로 보여
/// 주고, 수정은 별도 버튼으로만 진입한다. Key Performance Indicator는 실적
/// 기록을 여기서 바로 남길 수 있고, 하단에 정의 변경 이력이 쌓인다.
pub fn ItemDetail(props: ItemDetailProps) -> Element {
    let lang = use_lang();
    let t = *lang.read();
    let item = props.item.clone();
    let is_kpi = item.kind == ItemKind::Kpi;

    let mut revisions = use_signal(Vec::<ItemRevision>::new);
    let mut revisions_error = use_signal(|| None::<String>);
    // KpiMeasurementPanel과 공유하는 시그널. 상세 화면에서는 패널이
    // 집계한 현재값을 별도로 쓰지 않지만 패널 계약상 필요하다.
    let has_measurements = use_signal(|| false);
    let panel_current_value = use_signal(String::new);

    let store = use_context::<IkikStore>();
    let revision_item_id = use_signal(|| item.id.clone());
    use_effect(move || {
        spawn(async move {
            match store.load_revisions(revision_item_id.read().clone()).await {
                | Ok(list) => revisions.set(list),
                | Err(e) => revisions_error.set(Some(lang.peek().err_load_revisions(&e))),
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
    let deadline = item
        .due_date
        .as_deref()
        .and_then(|due| local_today().and_then(|today| due_chip(due, item.kind, t, today)));

    rsx! {
        div { class: "item-detail",
            Breadcrumb {
                chain,
                current_kind: item.kind,
                current_title: item.title.clone(),
                on_navigate: props.on_navigate
            }

            div { class: "detail-heading",
                div { class: "detail-title-block",
                    h2 { "{item.title}" }
                    span { class: "status-pill {item.status}", "{status_label(item.status, t)}" }
                    if let Some(deadline) = deadline {
                        span { class: "due-chip {deadline.class}", "{deadline.text}" }
                    }
                }
            }

            if let Some(description) = description {
                p { class: "detail-description", "{description}" }
            } else {
                p { class: "detail-description empty", {t.no_description()} }
            }

            div { class: "detail-actions",
                button {
                    r#type: "button",
                    class: "btn btn-primary",
                    onclick: move |_| props.on_edit.call(edit_item.clone()),
                    {t.edit()}
                }
                button {
                    r#type: "button",
                    class: "btn btn-secondary",
                    onclick: move |_| props.on_delete.call(delete_item.clone()),
                    {t.delete()}
                }
                button {
                    r#type: "button",
                    class: "btn btn-secondary",
                    onclick: move |_| props.on_back.call(()),
                    {t.back_to_list()}
                }
            }

            if is_kpi {
                div { class: "detail-progress",
                    if let Some(progress) = progress {
                        if let Some(percent) = percent {
                            span { class: "kpi-track detail-kpi-track",
                                span { class: "kpi-fill", style: "width: {percent}%;" }
                            }
                            span { class: "detail-progress-text", "{progress}" }
                            span { class: "detail-progress-hint", {t.agg_hint_percent(aggregation_label(item.aggregation, t), percent)} }
                        } else {
                            span { class: "detail-progress-text", "{progress}" }
                            span { class: "detail-progress-hint", {t.agg_hint(aggregation_label(item.aggregation, t))} }
                        }
                    } else {
                        span { class: "detail-progress-hint", {t.no_records_yet_detail()} }
                    }
                }

                KpiMeasurementPanel {
                    kpi_id: item.id.clone(),
                    aggregation: item.aggregation,
                    unit: item.unit.clone(),
                    target_value: item.target_value,
                    has_measurements,
                    current_value: panel_current_value
                }
            }

            if !is_kpi {
                div { class: "detail-descendants",
                    h3 { {t.sub_items()} }
                    if has_children(&item.id, &props.items) {
                        DescendantList {
                            parent_id: item.id.clone(),
                            items: props.items.clone(),
                            depth: 0,
                            on_navigate: props.on_navigate
                        }
                    } else {
                        p { class: "detail-descendants-empty", {t.no_sub_items()} }
                    }
                }
            }

            div { class: "detail-revisions",
                h3 { {t.change_history()} }
                if let Some(error) = revisions_error.read().clone() {
                    div { class: "form-error", "{error}" }
                }
                ul { class: "revision-list",
                    for revision in revisions.read().iter() {
                        {
                            let field_label = revision_field_label(&revision.field, t);
                            let old_value = revision_value_label(&revision.field, revision.old_value.as_deref(), t);
                            let new_value = revision_value_label(&revision.field, revision.new_value.as_deref(), t);
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
                            span { class: "revision-field", {t.item_created()} }
                        }
                        span { class: "revision-date", title: "{item.created_at}", "{created_at}" }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct DescendantListProps {
    parent_id: String,
    items: Vec<IkikItem>,
    depth: usize,
    on_navigate: EventHandler<IkikItem>,
}

/// 상세 화면의 하위 항목 트리. 브레드크럼이 조상으로 올라가는 길이라면
/// 이 트리는 자손으로 내려가는 길로, 노드를 누르면 그 항목의 상세로
/// 이동한다. 깊이 제한은 순환 참조 방어용.
fn DescendantList(props: DescendantListProps) -> Element {
    let t = *use_lang().read();
    if props.depth >= MAX_TREE_DEPTH {
        return rsx! {};
    }
    let children = sorted_children(&props.parent_id, &props.items);
    if children.is_empty() {
        return rsx! {};
    }

    rsx! {
        ul { class: "descendant-list",
            for child in children {
                {
                    let child_kind = kind_label(child.kind, t);
                    let child_title = child.title.clone();
                    let progress = progress_text(&child);
                    let nav_child = child.clone();
                    rsx! {
                        li { class: "descendant-node",
                            div { class: "descendant-row",
                                span { class: "descendant-kind", "{child_kind}" }
                                button {
                                    r#type: "button",
                                    class: "descendant-link",
                                    title: t.goto_detail(child_kind),
                                    onclick: move |_| props.on_navigate.call(nav_child.clone()),
                                    "{child_title}"
                                }
                                if let Some(progress) = progress {
                                    span { class: "descendant-progress", "{progress}" }
                                } else {
                                    span { class: "status-pill {child.status}", "{status_label(child.status, t)}" }
                                }
                            }
                            DescendantList {
                                parent_id: child.id.clone(),
                                items: props.items.clone(),
                                depth: props.depth + 1,
                                on_navigate: props.on_navigate
                            }
                        }
                    }
                }
            }
        }
    }
}
