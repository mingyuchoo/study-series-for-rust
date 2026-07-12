#![allow(non_snake_case)]

use crate::{components::{ContactForm,
                         ContactFormData,
                         ContactList},
            models::{Contact,
                     CreateContactRequest,
                     UpdateContactRequest},
            services::ContactService};
use dioxus::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");

#[derive(Debug, Clone, PartialEq)]
enum AppView {
    List,
    Add,
    Edit(Contact),
}

async fn fetch_contacts(query: String) -> Result<Vec<Contact>, String> {
    let query = query.trim().to_string();

    if query.is_empty() {
        ContactService::list_contacts().await
    } else {
        ContactService::search_contacts(query).await
    }
}

fn blank_to_none(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub fn App() -> Element {
    let mut contacts = use_signal(Vec::<Contact>::new);
    let mut current_view = use_signal(|| AppView::List);
    let mut search_query = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);
    let mut pending_delete = use_signal(|| None::<Contact>);

    // Load contacts on app start
    use_effect(move || {
        spawn(async move {
            loading.set(true);
            match fetch_contacts(String::new()).await {
                | Ok(contact_list) => {
                    contacts.set(contact_list);
                    error_message.set(None);
                },
                | Err(e) => {
                    error_message.set(Some(format!("연락처를 불러오는데 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    });

    let handle_search = move |evt: FormEvent| {
        evt.prevent_default();
        let query = search_query.read().clone();
        spawn(async move {
            loading.set(true);
            match fetch_contacts(query).await {
                | Ok(contact_list) => {
                    contacts.set(contact_list);
                    error_message.set(None);
                },
                | Err(e) => {
                    error_message.set(Some(format!("검색에 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    let handle_clear_search = move |_| {
        search_query.set(String::new());
        spawn(async move {
            loading.set(true);
            match fetch_contacts(String::new()).await {
                | Ok(contact_list) => {
                    contacts.set(contact_list);
                    error_message.set(None);
                },
                | Err(e) => {
                    error_message.set(Some(format!("연락처를 불러오는데 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    let handle_add_contact = move |form_data: ContactFormData| {
        // Re-entry guard: ignore a second submit (e.g. a rapid double-click)
        // while a request is already in flight. `loading` is set synchronously
        // here, before the spawn, so the guard sees it on the next event.
        if *loading.read() {
            return;
        }
        loading.set(true);
        spawn(async move {
            let request = CreateContactRequest {
                name: form_data.name.trim().to_string(),
                email: blank_to_none(form_data.email),
                phone: blank_to_none(form_data.phone),
                memo: blank_to_none(form_data.memo),
            };

            match ContactService::create_contact(request).await {
                | Ok(_) => {
                    current_view.set(AppView::List);
                    search_query.set(String::new());
                    match fetch_contacts(String::new()).await {
                        | Ok(contact_list) => {
                            contacts.set(contact_list);
                            error_message.set(None);
                        },
                        | Err(e) => {
                            error_message.set(Some(format!("목록을 새로고침하지 못했습니다: {}", e)));
                        },
                    }
                },
                | Err(e) => {
                    error_message.set(Some(format!("연락처 추가에 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    let handle_edit_contact = move |form_data: ContactFormData| {
        // Re-entry guard, matching `handle_add_contact` above.
        if *loading.read() {
            return;
        }
        if let AppView::Edit(contact) = current_view.read().clone() {
            loading.set(true);
            spawn(async move {
                let request = UpdateContactRequest {
                    id: contact.id,
                    name: Some(form_data.name.trim().to_string()),
                    email: Some(form_data.email.trim().to_string()),
                    phone: Some(form_data.phone.trim().to_string()),
                    memo: Some(form_data.memo.trim().to_string()),
                };

                match ContactService::update_contact(request).await {
                    | Ok(_) => {
                        current_view.set(AppView::List);
                        let query = search_query.read().clone();
                        match fetch_contacts(query).await {
                            | Ok(contact_list) => {
                                contacts.set(contact_list);
                                error_message.set(None);
                            },
                            | Err(e) => {
                                error_message.set(Some(format!("목록을 새로고침하지 못했습니다: {}", e)));
                            },
                        }
                    },
                    | Err(e) => {
                        error_message.set(Some(format!("연락처 수정에 실패했습니다: {}", e)));
                    },
                }
                loading.set(false);
            });
        }
    };

    let handle_delete_contact = move |id: String| {
        let query = search_query.read().clone();
        spawn(async move {
            loading.set(true);
            match ContactService::delete_contact(id).await {
                | Ok(_) => match fetch_contacts(query).await {
                    | Ok(contact_list) => {
                        contacts.set(contact_list);
                        error_message.set(None);
                    },
                    | Err(e) => {
                        error_message.set(Some(format!("목록을 새로고침하지 못했습니다: {}", e)));
                    },
                },
                | Err(e) => {
                    error_message.set(Some(format!("연락처 삭제에 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    rsx! {
        link { rel: "stylesheet", href: CSS }
        main { class: "app",
            header { class: "app-header",
                h1 { "주소록" }

                if let AppView::List = current_view.read().clone() {
                    div { class: "header-actions",
                        form { class: "search-form", onsubmit: handle_search,
                            input {
                                r#type: "text",
                                placeholder: "연락처 검색...",
                                value: "{search_query}",
                                oninput: move |evt| search_query.set(evt.value())
                            }
                            button { r#type: "submit", class: "btn btn-secondary", "검색" }
                            if !search_query.read().trim().is_empty() {
                                button {
                                    r#type: "button",
                                    class: "btn btn-secondary",
                                    onclick: handle_clear_search,
                                    "초기화"
                                }
                            }
                        }
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                error_message.set(None);
                                current_view.set(AppView::Add);
                            },
                            "새 연락처"
                        }
                    }
                }
            }

            if *loading.read() {
                div { class: "loading", "로딩 중..." }
            }

            if let Some(error) = error_message.read().clone() {
                div { class: "error-message", "{error}" }
            }

            match current_view.read().clone() {
                AppView::List => rsx! {
                    ContactList {
                        contacts: contacts.read().clone(),
                        is_filtering: !search_query.read().trim().is_empty(),
                        on_edit: move |contact| {
                            error_message.set(None);
                            current_view.set(AppView::Edit(contact));
                        },
                        on_delete: move |contact| pending_delete.set(Some(contact))
                    }
                },
                AppView::Add => rsx! {
                    ContactForm {
                        contact: None,
                        on_submit: handle_add_contact,
                        on_cancel: move |_| {
                            error_message.set(None);
                            current_view.set(AppView::List);
                        }
                    }
                },
                AppView::Edit(contact) => rsx! {
                    ContactForm {
                        contact: Some(contact),
                        on_submit: handle_edit_contact,
                        on_cancel: move |_| {
                            error_message.set(None);
                            current_view.set(AppView::List);
                        }
                    }
                }
            }

            if let Some(contact) = pending_delete.read().clone() {
                div { class: "confirm-backdrop",
                    div { class: "confirm-dialog", role: "dialog", aria_label: "연락처 삭제 확인",
                        h2 { "연락처 삭제" }
                        p { "\"{contact.name}\" 연락처를 삭제할까요?" }
                        div { class: "confirm-actions",
                            button {
                                r#type: "button",
                                class: "btn btn-secondary",
                                onclick: move |_| pending_delete.set(None),
                                "취소"
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-danger",
                                onclick: {
                                    let contact_id = contact.id.clone();
                                    move |_| {
                                        pending_delete.set(None);
                                        handle_delete_contact(contact_id.clone());
                                    }
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
