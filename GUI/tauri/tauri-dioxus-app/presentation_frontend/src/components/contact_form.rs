#![allow(non_snake_case)]

use crate::models::Contact;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct ContactFormProps {
    pub contact: Option<Contact>,
    pub on_submit: EventHandler<ContactFormData>,
    pub on_cancel: EventHandler<()>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContactFormData {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub address: String,
}

pub fn ContactForm(props: ContactFormProps) -> Element {
    let mut name = use_signal(|| props.contact.as_ref().map(|c| c.name.clone()).unwrap_or_default());
    let mut email = use_signal(|| props.contact.as_ref().and_then(|c| c.email.clone()).unwrap_or_default());
    let mut phone = use_signal(|| props.contact.as_ref().and_then(|c| c.phone.clone()).unwrap_or_default());
    let mut address = use_signal(|| props.contact.as_ref().and_then(|c| c.address.clone()).unwrap_or_default());
    let mut form_error = use_signal(|| None::<String>);

    let is_edit = props.contact.is_some();
    let title = if is_edit { "연락처 수정" } else { "새 연락처 추가" };

    let handle_submit = move |evt: FormEvent| {
        evt.prevent_default();
        if name.read().trim().is_empty() {
            form_error.set(Some("이름을 입력하세요.".to_string()));
            return;
        }

        form_error.set(None);
        let form_data = ContactFormData {
            name: name.read().clone(),
            email: email.read().clone(),
            phone: phone.read().clone(),
            address: address.read().clone(),
        };
        props.on_submit.call(form_data);
    };

    rsx! {
        div { class: "contact-form",
            h2 { "{title}" }
            if let Some(error) = form_error.read().clone() {
                div { class: "form-error", "{error}" }
            }
            form { onsubmit: handle_submit,
                div { class: "form-group",
                    label { r#for: "name", "이름 *" }
                    input {
                        id: "name",
                        r#type: "text",
                        required: true,
                        value: "{name}",
                        oninput: move |evt| name.set(evt.value())
                    }
                }

                div { class: "form-group",
                    label { r#for: "email", "이메일" }
                    input {
                        id: "email",
                        r#type: "email",
                        value: "{email}",
                        oninput: move |evt| email.set(evt.value())
                    }
                }

                div { class: "form-group",
                    label { r#for: "phone", "전화번호" }
                    input {
                        id: "phone",
                        r#type: "tel",
                        placeholder: "010-1234-5678",
                        value: "{phone}",
                        oninput: move |evt| phone.set(evt.value())
                    }
                }

                div { class: "form-group",
                    label { r#for: "address", "주소" }
                    textarea {
                        id: "address",
                        rows: "3",
                        value: "{address}",
                        oninput: move |evt| address.set(evt.value())
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
