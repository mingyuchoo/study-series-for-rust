#![allow(non_snake_case)]

use crate::{models::{CreateItemRequest,
                     ItemKind,
                     ItemStatus,
                     IvkikItem,
                     KpiAggregation,
                     UpdateItemRequest,
                     aggregation_description,
                     aggregation_label,
                     kind_description,
                     status_label,
                     tree::parent_chain},
            services::IvkikService};
use dioxus::prelude::*;

/// 폼을 여는 시점의 문맥. `parent`가 `Some`이면 트리에서 "+ 하위 추가"로
/// 진입한 것이므로 단계와 상위 항목을 잠근다.
#[derive(Debug, Clone, PartialEq)]
pub struct AddPreset {
    pub kind: ItemKind,
    pub parent: Option<IvkikItem>,
    pub title: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct ItemFormProps {
    pub item: Option<IvkikItem>,
    pub items: Vec<IvkikItem>,
    #[props(default)]
    pub preset: Option<AddPreset>,
    pub on_submit: EventHandler<ItemFormData>,
    pub on_cancel: EventHandler<()>,
    /// 브레드크럼에서 조상을 클릭하면 그 항목의 상세 화면으로 이동한다.
    #[props(default)]
    pub on_navigate: EventHandler<IvkikItem>,
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
    pub aggregation: KpiAggregation,
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
            aggregation: self.aggregation,
        })
    }

    /// 폼 입력을 수정 요청으로 변환한다. 구조(단계·상위·정렬)는 전체
    /// 구조 탭에서만 바꾸므로 여기서는 건드리지 않는다.
    pub fn to_update_request(&self, id: String) -> Result<UpdateItemRequest, String> {
        Ok(UpdateItemRequest {
            id,
            kind: None,
            parent_id: None,
            title: Some(self.title.trim().to_string()),
            description: Some(self.description.trim().to_string()),
            target_value: Some(parse_optional_f64(&self.target_value, "목표값")?),
            current_value: Some(parse_optional_f64(&self.current_value, "현재값")?),
            unit: Some(self.unit.trim().to_string()),
            position: None,
            status: Some(self.status),
            aggregation: Some(self.aggregation),
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
        .unwrap_or(ItemKind::Identity);
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
    let mut aggregation = use_signal(|| props.item.as_ref().map(|item| item.aggregation).unwrap_or_default());
    // 측정 기록이 있는 KPI는 현재값이 자동 집계되므로 입력을 잠근다.
    // 실적 기록 자체는 상세 화면이 담당하고, 폼은 잠금 여부만 조회한다.
    let mut has_measurements = use_signal(|| false);
    let mut form_error = use_signal(|| None::<String>);

    let measurement_kpi_id = props.item.as_ref().filter(|item| item.kind == ItemKind::Kpi).map(|item| item.id.clone());
    use_effect(move || {
        let Some(kpi_id) = measurement_kpi_id.clone() else {
            return;
        };
        spawn(async move {
            if let Ok(measurements) = IvkikService::list_kpi_measurements(kpi_id).await {
                has_measurements.set(!measurements.is_empty());
            }
        });
    });

    let is_edit = props.item.is_some();
    // 트리에서 하위 추가로 진입하면 단계와 상위 항목이 이미 결정되어 있다.
    let locked_parent = if is_edit {
        None
    } else {
        props.preset.as_ref().and_then(|preset| preset.parent.clone())
    };

    let selected_kind = *kind.read();
    let selected_status = *status.read();
    // 수정 모드: 구조는 브레드크럼으로만 보여 주고, 조상 클릭으로 그
    // 항목의 수정 화면으로 이동할 수 있다.
    let edit_context = props.item.as_ref().map(|item| (parent_chain(item, &props.items), item.title.clone()));
    let parent_kinds = selected_kind.allowed_parent_kinds();
    let parent_options: Vec<IvkikItem> = props
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
        if !is_edit && *kind.read() != ItemKind::Identity && parent_id.read().trim().is_empty() {
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
            aggregation: *aggregation.read(),
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
                    "새 IVKIK 항목"
                }
            }
            if let Some(error) = form_error.read().clone() {
                div { class: "form-error", "{error}" }
            }
            form { onsubmit: handle_submit,
                // 수정 모드: 어떤 항목을 고치는지 브레드크럼으로 보여 주고,
                // 조상을 누르면 그 항목의 수정 화면으로 이동한다.
                if let Some((chain, current_title)) = edit_context.as_ref() {
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
                            span { class: "breadcrumb-kind", "{selected_kind.label()}" }
                            span { class: "breadcrumb-current", "{current_title}" }
                        }
                    }
                }

                if !is_edit {
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

                        if selected_kind != ItemKind::Identity {
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
                                disabled: *has_measurements.read(),
                                value: "{current_value}",
                                oninput: move |evt| current_value.set(evt.value())
                            }
                            if *has_measurements.read() {
                                span { class: "field-hint", "실적 기록에서 자동 집계됩니다." }
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

                    div { class: "form-group",
                        label { "집계 방식" }
                        div { class: "kind-segment", role: "radiogroup", aria_label: "집계 방식 선택",
                            for aggregation_option in KpiAggregation::ALL {
                                button {
                                    r#type: "button",
                                    role: "radio",
                                    aria_checked: *aggregation.read() == aggregation_option,
                                    title: "{aggregation_description(aggregation_option)}",
                                    class: if *aggregation.read() == aggregation_option { "segment-btn active" } else { "segment-btn" },
                                    onclick: move |_| aggregation.set(aggregation_option),
                                    "{aggregation_label(aggregation_option)}"
                                }
                            }
                        }
                        span { class: "field-hint", "{aggregation_description(*aggregation.read())}" }
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
