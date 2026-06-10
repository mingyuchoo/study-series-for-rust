#![allow(non_snake_case)]

use crate::models::{CreateItemRequest,
                    ItemKind,
                    ItemStatus,
                    UpdateItemRequest,
                    VvkikItem,
                    kind_description,
                    status_label};
use dioxus::prelude::*;

/// 폼을 여는 시점의 문맥. `parent`가 `Some`이면 트리에서 "+ 하위 추가"로
/// 진입한 것이므로 단계와 상위 항목을 잠근다.
#[derive(Debug, Clone, PartialEq)]
pub struct AddPreset {
    pub kind: ItemKind,
    pub parent: Option<VvkikItem>,
    pub title: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct ItemFormProps {
    pub item: Option<VvkikItem>,
    pub items: Vec<VvkikItem>,
    #[props(default)]
    pub preset: Option<AddPreset>,
    pub on_submit: EventHandler<ItemFormData>,
    pub on_cancel: EventHandler<()>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemFormData {
    pub kind: ItemKind,
    pub parent_id: String,
    pub title: String,
    pub description: String,
    pub target_value: String,
    pub current_value: String,
    pub unit: String,
    pub status: ItemStatus,
}

fn blank_to_none(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_optional_f64(value: &str, label: &str) -> Result<Option<f64>, String> {
    let Some(value) = blank_to_none(value) else {
        return Ok(None);
    };

    value.parse::<f64>().map(Some).map_err(|_| format!("{label}은 숫자로 입력하세요."))
}

impl ItemFormData {
    pub fn parent_id_opt(&self) -> Option<String> { blank_to_none(&self.parent_id) }

    /// 폼 입력을 생성 요청으로 변환한다. 숫자 파싱 실패 시 사용자에게
    /// 보여줄 메시지를 돌려준다.
    pub fn to_create_request(&self, position: i64) -> Result<CreateItemRequest, String> {
        Ok(CreateItemRequest {
            kind: self.kind,
            parent_id: self.parent_id_opt(),
            title: self.title.trim().to_string(),
            description: blank_to_none(&self.description),
            target_value: parse_optional_f64(&self.target_value, "목표값")?,
            current_value: parse_optional_f64(&self.current_value, "현재값")?,
            unit: blank_to_none(&self.unit),
            position: Some(position),
        })
    }

    /// 폼 입력을 수정 요청으로 변환한다. 정렬값은 건드리지 않는다.
    pub fn to_update_request(&self, id: String) -> Result<UpdateItemRequest, String> {
        Ok(UpdateItemRequest {
            id,
            kind: Some(self.kind),
            parent_id: Some(self.parent_id_opt()),
            title: Some(self.title.trim().to_string()),
            description: Some(self.description.trim().to_string()),
            target_value: Some(parse_optional_f64(&self.target_value, "목표값")?),
            current_value: Some(parse_optional_f64(&self.current_value, "현재값")?),
            unit: Some(self.unit.trim().to_string()),
            position: None,
            status: Some(self.status),
        })
    }
}

fn number_to_string(value: Option<f64>) -> String { value.map(|value| value.to_string()).unwrap_or_default() }

pub fn ItemForm(props: ItemFormProps) -> Element {
    let initial_kind = props
        .item
        .as_ref()
        .map(|item| item.kind)
        .or_else(|| props.preset.as_ref().map(|preset| preset.kind))
        .unwrap_or(ItemKind::Value);
    let initial_parent_id = props
        .item
        .as_ref()
        .and_then(|item| item.parent_id.clone())
        .or_else(|| props.preset.as_ref().and_then(|preset| preset.parent.as_ref().map(|parent| parent.id.clone())))
        .unwrap_or_default();
    let initial_title = props
        .item
        .as_ref()
        .map(|item| item.title.clone())
        .or_else(|| props.preset.as_ref().map(|preset| preset.title.clone()))
        .unwrap_or_default();

    let mut kind = use_signal(|| initial_kind);
    let mut parent_id = use_signal(|| initial_parent_id);
    let mut title = use_signal(|| initial_title);
    let mut description = use_signal(|| props.item.as_ref().and_then(|item| item.description.clone()).unwrap_or_default());
    let mut target_value = use_signal(|| props.item.as_ref().map(|item| number_to_string(item.target_value)).unwrap_or_default());
    let mut current_value = use_signal(|| props.item.as_ref().map(|item| number_to_string(item.current_value)).unwrap_or_default());
    let mut unit = use_signal(|| props.item.as_ref().and_then(|item| item.unit.clone()).unwrap_or_default());
    let mut status = use_signal(|| props.item.as_ref().map(|item| item.status).unwrap_or(ItemStatus::Active));
    let mut form_error = use_signal(|| None::<String>);

    let is_edit = props.item.is_some();
    // 트리에서 하위 추가로 진입하면 단계와 상위 항목이 이미 결정되어 있다.
    let locked_parent = if is_edit {
        None
    } else {
        props.preset.as_ref().and_then(|preset| preset.parent.clone())
    };

    let selected_kind = *kind.read();
    let selected_status = *status.read();
    let parent_kinds = selected_kind.allowed_parent_kinds();
    let parent_options: Vec<VvkikItem> = props
        .items
        .iter()
        .filter(|candidate| parent_kinds.contains(&candidate.kind) && props.item.as_ref().is_none_or(|current| current.id != candidate.id))
        .cloned()
        .collect();

    let handle_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if title.read().trim().is_empty() {
            form_error.set(Some("제목을 입력하세요.".to_string()));
            return;
        }
        if *kind.read() != ItemKind::Value && parent_id.read().trim().is_empty() {
            form_error.set(Some("상위 항목을 선택하세요.".to_string()));
            return;
        }

        form_error.set(None);
        props.on_submit.call(ItemFormData {
            kind: *kind.read(),
            parent_id: parent_id.read().clone(),
            title: title.read().clone(),
            description: description.read().clone(),
            target_value: target_value.read().clone(),
            current_value: current_value.read().clone(),
            unit: unit.read().clone(),
            status: *status.read(),
        });
    };

    rsx! {
        div { class: "item-form",
            h2 {
                if is_edit {
                    "항목 수정"
                } else if locked_parent.is_some() {
                    "새 {selected_kind.label()} 항목"
                } else {
                    "새 VVKIK 항목"
                }
            }
            if let Some(error) = form_error.read().clone() {
                div { class: "form-error", "{error}" }
            }
            form { onsubmit: handle_submit,
                if let Some(parent) = locked_parent.as_ref() {
                    div { class: "form-grid",
                        div { class: "form-group",
                            label { "단계" }
                            div { class: "locked-field", "{selected_kind.label()}" }
                        }
                        div { class: "form-group",
                            label { "상위 항목" }
                            div { class: "locked-field", "{parent.kind.label()} · {parent.title}" }
                        }
                    }
                } else {
                    div { class: "form-group",
                        label { "단계" }
                        div { class: "kind-segment", role: "radiogroup", aria_label: "단계 선택",
                            for kind_option in ItemKind::ALL {
                                button {
                                    r#type: "button",
                                    role: "radio",
                                    aria_checked: selected_kind == kind_option,
                                    title: "{kind_description(kind_option)}",
                                    class: if selected_kind == kind_option { "segment-btn active" } else { "segment-btn" },
                                    onclick: move |_| {
                                        if *kind.read() != kind_option {
                                            kind.set(kind_option);
                                            parent_id.set(String::new());
                                        }
                                    },
                                    "{kind_option.label()}"
                                }
                            }
                        }
                    }

                    if selected_kind != ItemKind::Value {
                        div { class: "form-group",
                            label { r#for: "parent", "상위 항목" }
                            select {
                                id: "parent",
                                value: "{parent_id}",
                                onchange: move |evt| parent_id.set(evt.value()),
                                option { value: "", "상위 항목 선택" }
                                for parent in parent_options.iter() {
                                    option { value: "{parent.id}", "{parent.kind.label()} · {parent.title}" }
                                }
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
                        autofocus: true,
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

                if selected_kind == ItemKind::Kpi {
                    div { class: "form-grid compact",
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

                if is_edit {
                    div { class: "form-group",
                        label { "상태" }
                        div { class: "kind-segment", role: "radiogroup", aria_label: "상태 선택",
                            for status_option in ItemStatus::ALL {
                                button {
                                    r#type: "button",
                                    role: "radio",
                                    aria_checked: selected_status == status_option,
                                    class: if selected_status == status_option { "segment-btn active" } else { "segment-btn" },
                                    onclick: move |_| status.set(status_option),
                                    "{status_label(status_option)}"
                                }
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
