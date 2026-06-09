#![allow(non_snake_case)]

use crate::models::{VvkikItem,
                    allowed_parent_kinds,
                    kind_label};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ItemFormProps {
    pub item: Option<VvkikItem>,
    pub items: Vec<VvkikItem>,
    pub on_submit: EventHandler<ItemFormData>,
    pub on_cancel: EventHandler<()>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemFormData {
    pub kind: String,
    pub parent_id: String,
    pub title: String,
    pub description: String,
    pub target_value: String,
    pub current_value: String,
    pub unit: String,
    pub position: String,
    pub status: String,
}

fn number_to_string(value: Option<f64>) -> String { value.map(|value| value.to_string()).unwrap_or_default() }

pub fn ItemForm(props: ItemFormProps) -> Element {
    let mut kind = use_signal(|| props.item.as_ref().map(|item| item.kind.clone()).unwrap_or_else(|| "value".to_string()));
    let mut parent_id = use_signal(|| props.item.as_ref().and_then(|item| item.parent_id.clone()).unwrap_or_default());
    let mut title = use_signal(|| props.item.as_ref().map(|item| item.title.clone()).unwrap_or_default());
    let mut description = use_signal(|| props.item.as_ref().and_then(|item| item.description.clone()).unwrap_or_default());
    let mut target_value = use_signal(|| props.item.as_ref().map(|item| number_to_string(item.target_value)).unwrap_or_default());
    let mut current_value = use_signal(|| props.item.as_ref().map(|item| number_to_string(item.current_value)).unwrap_or_default());
    let mut unit = use_signal(|| props.item.as_ref().and_then(|item| item.unit.clone()).unwrap_or_default());
    let mut position = use_signal(|| props.item.as_ref().map(|item| item.position.to_string()).unwrap_or_else(|| "0".to_string()));
    let mut status = use_signal(|| props.item.as_ref().map(|item| item.status.clone()).unwrap_or_else(|| "active".to_string()));
    let mut form_error = use_signal(|| None::<String>);

    let is_edit = props.item.is_some();
    let selected_kind = kind.read().clone();
    let parent_kinds = allowed_parent_kinds(&selected_kind);
    let parent_options: Vec<VvkikItem> = props
        .items
        .iter()
        .filter(|candidate| parent_kinds.contains(&candidate.kind.as_str()) && props.item.as_ref().is_none_or(|current| current.id != candidate.id))
        .cloned()
        .collect();

    let handle_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if title.read().trim().is_empty() {
            form_error.set(Some("제목을 입력하세요.".to_string()));
            return;
        }
        if kind.read().as_str() != "value" && parent_id.read().trim().is_empty() {
            form_error.set(Some("상위 항목을 선택하세요.".to_string()));
            return;
        }

        form_error.set(None);
        props.on_submit.call(ItemFormData {
            kind: kind.read().clone(),
            parent_id: parent_id.read().clone(),
            title: title.read().clone(),
            description: description.read().clone(),
            target_value: target_value.read().clone(),
            current_value: current_value.read().clone(),
            unit: unit.read().clone(),
            position: position.read().clone(),
            status: status.read().clone(),
        });
    };

    rsx! {
        div { class: "item-form",
            h2 { if is_edit { "항목 수정" } else { "새 VVKIK 항목" } }
            if let Some(error) = form_error.read().clone() {
                div { class: "form-error", "{error}" }
            }
            form { onsubmit: handle_submit,
                div { class: "form-grid",
                    div { class: "form-group",
                        label { r#for: "kind", "단계" }
                        select {
                            id: "kind",
                            value: "{kind}",
                            onchange: move |evt| {
                                kind.set(evt.value());
                                parent_id.set(String::new());
                            },
                            option { value: "value", "Value 가치" }
                            option { value: "vision", "Vision 비전" }
                            option { value: "kra", "KRA 핵심 결과 영역" }
                            option { value: "igt", "IGT 소득 창출 업무" }
                            option { value: "kpi", "KPI 핵심 성과 지표" }
                        }
                    }

                    div { class: "form-group",
                        label { r#for: "parent", "상위 항목" }
                        select {
                            id: "parent",
                            value: "{parent_id}",
                            disabled: selected_kind == "value",
                            onchange: move |evt| parent_id.set(evt.value()),
                            option { value: "", if selected_kind == "value" { "최상위 Value" } else { "상위 항목 선택" } }
                            for parent in parent_options.iter() {
                                option { value: "{parent.id}", "{kind_label(&parent.kind)} · {parent.title}" }
                            }
                        }
                    }
                }

                div { class: "form-group",
                    label { r#for: "title", "제목 *" }
                    input {
                        id: "title",
                        r#type: "text",
                        required: true,
                        value: "{title}",
                        oninput: move |evt| title.set(evt.value())
                    }
                }

                div { class: "form-group",
                    label { r#for: "description", "설명" }
                    textarea {
                        id: "description",
                        rows: "4",
                        value: "{description}",
                        oninput: move |evt| description.set(evt.value())
                    }
                }

                if selected_kind == "kpi" {
                    div { class: "form-grid",
                        div { class: "form-group",
                            label { r#for: "current_value", "현재값" }
                            input {
                                id: "current_value",
                                r#type: "number",
                                step: "any",
                                value: "{current_value}",
                                oninput: move |evt| current_value.set(evt.value())
                            }
                        }
                        div { class: "form-group",
                            label { r#for: "target_value", "목표값" }
                            input {
                                id: "target_value",
                                r#type: "number",
                                step: "any",
                                value: "{target_value}",
                                oninput: move |evt| target_value.set(evt.value())
                            }
                        }
                        div { class: "form-group",
                            label { r#for: "unit", "단위" }
                            input {
                                id: "unit",
                                r#type: "text",
                                placeholder: "원, %, 건...",
                                value: "{unit}",
                                oninput: move |evt| unit.set(evt.value())
                            }
                        }
                    }
                }

                div { class: "form-grid compact",
                    div { class: "form-group",
                        label { r#for: "position", "정렬" }
                        input {
                            id: "position",
                            r#type: "number",
                            value: "{position}",
                            oninput: move |evt| position.set(evt.value())
                        }
                    }

                    if is_edit {
                        div { class: "form-group",
                            label { r#for: "status", "상태" }
                            select {
                                id: "status",
                                value: "{status}",
                                onchange: move |evt| status.set(evt.value()),
                                option { value: "active", "Active" }
                                option { value: "paused", "Paused" }
                                option { value: "completed", "Completed" }
                            }
                        }
                    }
                }

                div { class: "form-actions",
                    button { r#type: "submit", class: "btn btn-primary",
                        if is_edit { "수정" } else { "추가" }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        onclick: move |_| props.on_cancel.call(()),
                        "취소"
                    }
                }
            }
        }
    }
}
