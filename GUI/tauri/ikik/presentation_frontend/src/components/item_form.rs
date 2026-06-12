#![allow(non_snake_case)]

use super::breadcrumb::Breadcrumb;
use crate::{i18n::{Lang,
                   use_lang},
            models::{CreateItemRequest,
                     IkikItem,
                     ItemKind,
                     ItemStatus,
                     KpiAggregation,
                     UpdateItemRequest,
                     aggregation_description,
                     aggregation_label,
                     kind_description,
                     kind_label,
                     status_label,
                     tree::parent_chain}};
use dioxus::prelude::*;

/// 폼을 여는 시점의 문맥. `parent`가 `Some`이면 트리에서 "+ 하위 추가"로
/// 진입한 것이므로 단계와 상위 항목을 잠근다.
#[derive(Debug, Clone, PartialEq)]
pub struct AddPreset {
    pub kind: ItemKind,
    pub parent: Option<IkikItem>,
    pub title: String,
}

#[derive(Props, Clone, PartialEq)]
pub struct ItemFormProps {
    pub item: Option<IkikItem>,
    pub items: Vec<IkikItem>,
    #[props(default)]
    pub preset: Option<AddPreset>,
    pub on_submit: EventHandler<ItemFormData>,
    pub on_cancel: EventHandler<()>,
    /// 브레드크럼에서 조상을 클릭하면 그 항목의 상세 화면으로 이동한다.
    #[props(default)]
    pub on_navigate: EventHandler<IkikItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemFormData {
    pub kind: ItemKind,
    pub parent_id: String,
    pub title: String,
    pub description: String,
    pub target_value: String,
    pub unit: String,
    pub status: ItemStatus,
    pub aggregation: KpiAggregation,
    /// "YYYY-MM-DD" 또는 빈 문자열(마감 없음).
    pub due_date: String,
}

fn blank_to_none(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_optional_f64(value: &str, label: &str, lang: Lang) -> Result<Option<f64>, String> {
    let Some(value) = blank_to_none(value) else {
        return Ok(None);
    };

    value.parse::<f64>().map(Some).map_err(|_| lang.err_must_be_number(label))
}

impl ItemFormData {
    pub fn parent_id_opt(&self) -> Option<String> { blank_to_none(&self.parent_id) }

    /// 폼 입력을 생성 요청으로 변환한다. 숫자 파싱 실패 시 사용자에게
    /// 보여줄 메시지를 돌려준다.
    pub fn to_create_request(&self, position: i64, lang: Lang) -> Result<CreateItemRequest, String> {
        Ok(CreateItemRequest {
            kind: self.kind,
            parent_id: self.parent_id_opt(),
            title: self.title.trim().to_string(),
            description: blank_to_none(&self.description),
            target_value: parse_optional_f64(&self.target_value, lang.target_label(), lang)?,
            // 현재값은 실적 기록으로만 집계되므로 폼에서 받지 않는다.
            current_value: None,
            unit: blank_to_none(&self.unit),
            position: Some(position),
            aggregation: self.aggregation,
            due_date: blank_to_none(&self.due_date),
        })
    }

    /// 폼 입력을 수정 요청으로 변환한다. 구조(단계·상위·정렬)는 전체
    /// 구조 탭에서만 바꾸므로 여기서는 건드리지 않는다.
    pub fn to_update_request(&self, id: String, lang: Lang) -> Result<UpdateItemRequest, String> {
        Ok(UpdateItemRequest {
            id,
            kind: None,
            parent_id: None,
            title: Some(self.title.trim().to_string()),
            description: Some(self.description.trim().to_string()),
            target_value: Some(parse_optional_f64(&self.target_value, lang.target_label(), lang)?),
            // None이면 백엔드가 현재값을 건드리지 않는다(실적 기록 집계값 보존).
            current_value: None,
            unit: Some(self.unit.trim().to_string()),
            position: None,
            status: Some(self.status),
            aggregation: Some(self.aggregation),
            due_date: Some(blank_to_none(&self.due_date)),
        })
    }
}

fn number_to_string(value: Option<f64>) -> String { value.map(|value| value.to_string()).unwrap_or_default() }

#[derive(Props, Clone, PartialEq)]
struct StructureFieldsProps {
    kind: Signal<ItemKind>,
    parent_id: Signal<String>,
    due_date: Signal<String>,
    /// 트리의 "+ 하위 추가"로 진입했으면 단계와 상위 항목이 잠긴다.
    locked_parent: Option<IkikItem>,
    parent_options: Vec<IkikItem>,
}

/// 생성 모드의 구조 선택: 단계 세그먼트와 상위 항목. 단계를 바꾸면
/// 상위 항목 선택이 초기화되고, Identity는 마감을 갖지 않으므로 마감
/// 입력값도 비운다.
fn StructureFields(props: StructureFieldsProps) -> Element {
    let t = *use_lang().read();
    let mut kind = props.kind;
    let mut parent_id = props.parent_id;
    let mut due_date = props.due_date;
    let selected_kind = *kind.read();

    if let Some(parent) = props.locked_parent.as_ref() {
        return rsx! {
            div { class: "form-grid",
                div { class: "form-group",
                    label { {t.stage_label()} }
                    div { class: "locked-field", {kind_label(selected_kind, t)} }
                }
                div { class: "form-group",
                    label { {t.parent_label()} }
                    div { class: "locked-field", "{kind_label(parent.kind, t)} · {parent.title}" }
                }
            }
        };
    }

    rsx! {
        div { class: "form-group",
            label { {t.stage_label()} }
            div { class: "kind-segment", role: "radiogroup", aria_label: t.stage_select_aria(),
                for kind_option in ItemKind::ALL {
                    button {
                        r#type: "button",
                        role: "radio",
                        aria_checked: selected_kind == kind_option,
                        title: kind_description(kind_option, t),
                        class: if selected_kind == kind_option { "segment-btn active" } else { "segment-btn" },
                        onclick: move |_| {
                            if *kind.read() != kind_option {
                                kind.set(kind_option);
                                parent_id.set(String::new());
                                // Identity는 마감을 갖지 않으므로 입력값을 비운다.
                                if kind_option == ItemKind::Identity {
                                    due_date.set(String::new());
                                }
                            }
                        },
                        {kind_label(kind_option, t)}
                    }
                }
            }
        }

        if selected_kind != ItemKind::Identity {
            div { class: "form-group",
                label { r#for: "parent", {t.parent_label()} }
                select {
                    id: "parent",
                    value: "{parent_id}",
                    onchange: move |evt| parent_id.set(evt.value()),
                    option { value: "", {t.parent_select_placeholder()} }
                    for parent in props.parent_options.iter() {
                        option { value: "{parent.id}", "{kind_label(parent.kind, t)} · {parent.title}" }
                    }
                }
            }
        }
    }
}

pub fn ItemForm(props: ItemFormProps) -> Element {
    let lang = use_lang();
    let t = *lang.read();
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

    let kind = use_signal(|| initial_kind);
    let parent_id = use_signal(|| initial_parent_id);
    let mut title = use_signal(|| initial_title);
    let mut description = use_signal(|| props.item.as_ref().and_then(|item| item.description.clone()).unwrap_or_default());
    let mut target_value = use_signal(|| props.item.as_ref().map(|item| number_to_string(item.target_value)).unwrap_or_default());
    let mut unit = use_signal(|| props.item.as_ref().and_then(|item| item.unit.clone()).unwrap_or_default());
    let mut status = use_signal(|| props.item.as_ref().map(|item| item.status).unwrap_or(ItemStatus::Active));
    let mut aggregation = use_signal(|| props.item.as_ref().map(|item| item.aggregation).unwrap_or_default());
    let mut due_date = use_signal(|| props.item.as_ref().and_then(|item| item.due_date.clone()).unwrap_or_default());
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
    // 수정 모드: 구조는 브레드크럼으로만 보여 주고, 조상 클릭으로 그
    // 항목의 수정 화면으로 이동할 수 있다.
    let edit_context = props.item.as_ref().map(|item| (parent_chain(item, &props.items), item.title.clone()));
    let parent_kinds = selected_kind.allowed_parent_kinds();
    let parent_options: Vec<IkikItem> = props
        .items
        .iter()
        .filter(|candidate| parent_kinds.contains(&candidate.kind) && props.item.as_ref().is_none_or(|current| current.id != candidate.id))
        .cloned()
        .collect();

    let handle_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if title.read().trim().is_empty() {
            form_error.set(Some(lang.peek().err_title_required().to_string()));
            return;
        }
        if !is_edit && *kind.read() != ItemKind::Identity && parent_id.read().trim().is_empty() {
            form_error.set(Some(lang.peek().err_parent_required().to_string()));
            return;
        }

        form_error.set(None);
        props.on_submit.call(ItemFormData {
            kind: *kind.read(),
            parent_id: parent_id.read().clone(),
            title: title.read().clone(),
            description: description.read().clone(),
            target_value: target_value.read().clone(),
            unit: unit.read().clone(),
            status: *status.read(),
            aggregation: *aggregation.read(),
            due_date: due_date.read().clone(),
        });
    };

    rsx! {
        div { class: "item-form",
            h2 {
                if is_edit {
                    {t.form_edit_title()}
                } else if locked_parent.is_some() {
                    {t.form_new_kind_title(kind_label(selected_kind, t))}
                } else {
                    {t.form_new_title()}
                }
            }
            if let Some(error) = form_error.read().clone() {
                div { class: "form-error", "{error}" }
            }
            form { onsubmit: handle_submit,
                // 수정 모드: 어떤 항목을 고치는지 브레드크럼으로 보여 주고,
                // 조상을 누르면 그 항목의 수정 화면으로 이동한다.
                if let Some((chain, current_title)) = edit_context.as_ref() {
                    Breadcrumb {
                        chain: chain.clone(),
                        current_kind: selected_kind,
                        current_title: current_title.clone(),
                        on_navigate: props.on_navigate
                    }
                }

                if !is_edit {
                    StructureFields {
                        kind,
                        parent_id,
                        due_date,
                        locked_parent: locked_parent.clone(),
                        parent_options: parent_options.clone()
                    }
                }

                div { class: "form-group",
                    label { r#for: "title", "{t.title_label()} *" }
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
                    label { r#for: "description", {t.description_label()} }
                    textarea {
                        id: "description",
                        rows: "4",
                        value: "{description}",
                        oninput: move |evt| description.set(evt.value())
                    }
                }

                // Identity는 마감 없이 지속되는 단계라 필드 자체를 보여 주지
                // 않는다. Key Performance Indicator의 날짜는 목표값 옆에서
                // "목표 달성일"로 따로 받는다.
                if selected_kind == ItemKind::Kra || selected_kind == ItemKind::Igt {
                    div { class: "form-group",
                        label { r#for: "due_date", {t.due_date_label()} }
                        input {
                            id: "due_date",
                            r#type: "date",
                            value: "{due_date}",
                            oninput: move |evt| due_date.set(evt.value())
                        }
                        span { class: "field-hint", {t.due_date_hint()} }
                    }
                }

                if selected_kind == ItemKind::Kpi {
                    div { class: "form-grid compact",
                        div { class: "form-group",
                            label { r#for: "target_value", {t.target_label()} }
                            input {
                                id: "target_value",
                                r#type: "number",
                                step: "any",
                                value: "{target_value}",
                                oninput: move |evt| target_value.set(evt.value())
                            }
                        }
                        div { class: "form-group",
                            label { r#for: "unit", {t.unit_label()} }
                            input {
                                id: "unit",
                                r#type: "text",
                                placeholder: t.unit_placeholder(),
                                value: "{unit}",
                                oninput: move |evt| unit.set(evt.value())
                            }
                        }
                    }
                    span { class: "field-hint", {t.current_value_hint()} }

                    div { class: "form-group",
                        label { r#for: "target_date", {t.target_date_label()} }
                        input {
                            id: "target_date",
                            r#type: "date",
                            value: "{due_date}",
                            oninput: move |evt| due_date.set(evt.value())
                        }
                        span { class: "field-hint", {t.target_date_hint()} }
                    }

                    div { class: "form-group",
                        label { {t.aggregation_field()} }
                        div { class: "kind-segment", role: "radiogroup", aria_label: t.aggregation_select_aria(),
                            for aggregation_option in KpiAggregation::ALL {
                                button {
                                    r#type: "button",
                                    role: "radio",
                                    aria_checked: *aggregation.read() == aggregation_option,
                                    title: aggregation_description(aggregation_option, t),
                                    class: if *aggregation.read() == aggregation_option { "segment-btn active" } else { "segment-btn" },
                                    onclick: move |_| aggregation.set(aggregation_option),
                                    {aggregation_label(aggregation_option, t)}
                                }
                            }
                        }
                        span { class: "field-hint", {aggregation_description(*aggregation.read(), t)} }
                    }

                }

                if is_edit {
                    div { class: "form-group",
                        label { {t.status_field()} }
                        div { class: "kind-segment", role: "radiogroup", aria_label: t.status_select_aria(),
                            for status_option in ItemStatus::ALL {
                                button {
                                    r#type: "button",
                                    role: "radio",
                                    aria_checked: selected_status == status_option,
                                    class: if selected_status == status_option { "segment-btn active" } else { "segment-btn" },
                                    onclick: move |_| status.set(status_option),
                                    "{status_label(status_option, t)}"
                                }
                            }
                        }
                    }
                }

                div { class: "form-actions",
                    button { r#type: "submit", class: "btn btn-primary",
                        if is_edit { {t.save()} } else { {t.add()} }
                    }
                    button {
                        r#type: "button",
                        class: "btn btn-secondary",
                        onclick: move |_| props.on_cancel.call(()),
                        {t.cancel()}
                    }
                }
            }
        }
    }
}
