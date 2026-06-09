#![allow(non_snake_case)]

use crate::{components::{ItemForm,
                         ItemFormData,
                         VvkikBoard},
            models::{CreateItemRequest,
                     UpdateItemRequest,
                     VvkikItem},
            services::VvkikService};
use dioxus::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");

#[derive(Debug, Clone, PartialEq)]
enum AppView {
    Board,
    Add,
    Edit(Box<VvkikItem>),
}

async fn fetch_items(query: String) -> Result<Vec<VvkikItem>, String> {
    let query = query.trim().to_string();

    if query.is_empty() {
        VvkikService::list_items().await
    } else {
        VvkikService::search_items(query).await
    }
}

fn blank_to_none(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn parse_optional_f64(value: String, label: &str) -> Result<Option<f64>, String> {
    let Some(value) = blank_to_none(value) else {
        return Ok(None);
    };

    value.parse::<f64>().map(Some).map_err(|_| format!("{label}은 숫자로 입력하세요."))
}

fn parse_i64_or_zero(value: String) -> i64 { value.trim().parse::<i64>().unwrap_or_default() }

pub fn App() -> Element {
    let mut items = use_signal(Vec::<VvkikItem>::new);
    let mut current_view = use_signal(|| AppView::Board);
    let mut search_query = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);
    let mut pending_delete = use_signal(|| None::<VvkikItem>);

    use_effect(move || {
        spawn(async move {
            loading.set(true);
            match fetch_items(String::new()).await {
                | Ok(item_list) => {
                    items.set(item_list);
                    error_message.set(None);
                },
                | Err(e) => {
                    error_message.set(Some(format!("VVKIK 항목을 불러오지 못했습니다: {}", e)));
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
            match fetch_items(query).await {
                | Ok(item_list) => {
                    items.set(item_list);
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
            match fetch_items(String::new()).await {
                | Ok(item_list) => {
                    items.set(item_list);
                    error_message.set(None);
                },
                | Err(e) => {
                    error_message.set(Some(format!("VVKIK 항목을 불러오지 못했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    let handle_add_item = move |form_data: ItemFormData| {
        if *loading.read() {
            return;
        }

        let target_value = match parse_optional_f64(form_data.target_value.clone(), "목표값") {
            | Ok(value) => value,
            | Err(message) => {
                error_message.set(Some(message));
                return;
            },
        };
        let current_value = match parse_optional_f64(form_data.current_value.clone(), "현재값") {
            | Ok(value) => value,
            | Err(message) => {
                error_message.set(Some(message));
                return;
            },
        };

        loading.set(true);
        spawn(async move {
            let request = CreateItemRequest {
                kind: form_data.kind,
                parent_id: blank_to_none(form_data.parent_id),
                title: form_data.title.trim().to_string(),
                description: blank_to_none(form_data.description),
                target_value,
                current_value,
                unit: blank_to_none(form_data.unit),
                position: Some(parse_i64_or_zero(form_data.position)),
            };

            match VvkikService::create_item(request).await {
                | Ok(_) => {
                    current_view.set(AppView::Board);
                    search_query.set(String::new());
                    match fetch_items(String::new()).await {
                        | Ok(item_list) => {
                            items.set(item_list);
                            error_message.set(None);
                        },
                        | Err(e) => {
                            error_message.set(Some(format!("목록을 새로고침하지 못했습니다: {}", e)));
                        },
                    }
                },
                | Err(e) => {
                    error_message.set(Some(format!("항목 추가에 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    let handle_edit_item = move |form_data: ItemFormData| {
        if *loading.read() {
            return;
        }

        let target_value = match parse_optional_f64(form_data.target_value.clone(), "목표값") {
            | Ok(value) => value,
            | Err(message) => {
                error_message.set(Some(message));
                return;
            },
        };
        let current_value = match parse_optional_f64(form_data.current_value.clone(), "현재값") {
            | Ok(value) => value,
            | Err(message) => {
                error_message.set(Some(message));
                return;
            },
        };

        if let AppView::Edit(item) = current_view.read().clone() {
            loading.set(true);
            spawn(async move {
                let request = UpdateItemRequest {
                    id: item.id.clone(),
                    kind: Some(form_data.kind),
                    parent_id: Some(blank_to_none(form_data.parent_id)),
                    title: Some(form_data.title.trim().to_string()),
                    description: Some(form_data.description.trim().to_string()),
                    target_value: Some(target_value),
                    current_value: Some(current_value),
                    unit: Some(form_data.unit.trim().to_string()),
                    position: Some(parse_i64_or_zero(form_data.position)),
                    status: Some(form_data.status),
                };

                match VvkikService::update_item(request).await {
                    | Ok(_) => {
                        current_view.set(AppView::Board);
                        let query = search_query.read().clone();
                        match fetch_items(query).await {
                            | Ok(item_list) => {
                                items.set(item_list);
                                error_message.set(None);
                            },
                            | Err(e) => {
                                error_message.set(Some(format!("목록을 새로고침하지 못했습니다: {}", e)));
                            },
                        }
                    },
                    | Err(e) => {
                        error_message.set(Some(format!("항목 수정에 실패했습니다: {}", e)));
                    },
                }
                loading.set(false);
            });
        }
    };

    let handle_delete_item = move |id: String| {
        let query = search_query.read().clone();
        spawn(async move {
            loading.set(true);
            match VvkikService::delete_item(id).await {
                | Ok(_) => match fetch_items(query).await {
                    | Ok(item_list) => {
                        items.set(item_list);
                        error_message.set(None);
                    },
                    | Err(e) => {
                        error_message.set(Some(format!("목록을 새로고침하지 못했습니다: {}", e)));
                    },
                },
                | Err(e) => {
                    error_message.set(Some(format!("항목 삭제에 실패했습니다: {}", e)));
                },
            }
            loading.set(false);
        });
    };

    rsx! {
        link { rel: "stylesheet", href: CSS }
        main { class: "app",
            header { class: "app-header",
                div { class: "brand-block",
                    h1 { "VVKIK" }
                    p { "Value에서 KPI까지, 큰 그림을 실행과 피드백으로 연결합니다." }
                }

                if let AppView::Board = current_view.read().clone() {
                    div { class: "header-actions",
                        form { class: "search-form", onsubmit: handle_search,
                            input {
                                r#type: "text",
                                placeholder: "Value, Vision, KRA, IGT, KPI 검색...",
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
                            "새 항목"
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
                AppView::Board => rsx! {
                    VvkikBoard {
                        items: items.read().clone(),
                        is_filtering: !search_query.read().trim().is_empty(),
                        on_edit: move |item| {
                            error_message.set(None);
                            current_view.set(AppView::Edit(Box::new(item)));
                        },
                        on_delete: move |item| pending_delete.set(Some(item))
                    }
                },
                AppView::Add => rsx! {
                    ItemForm {
                        item: None,
                        items: items.read().clone(),
                        on_submit: handle_add_item,
                        on_cancel: move |_| {
                            error_message.set(None);
                            current_view.set(AppView::Board);
                        }
                    }
                },
                AppView::Edit(item) => rsx! {
                    ItemForm {
                        item: Some((*item).clone()),
                        items: items.read().clone(),
                        on_submit: handle_edit_item,
                        on_cancel: move |_| {
                            error_message.set(None);
                            current_view.set(AppView::Board);
                        }
                    }
                }
            }

            if let Some(item) = pending_delete.read().clone() {
                div { class: "confirm-backdrop",
                    div { class: "confirm-dialog", role: "dialog", aria_label: "VVKIK 항목 삭제 확인",
                        h2 { "항목 삭제" }
                        p { "\"{item.title}\" 항목을 삭제할까요? 하위 항목도 함께 삭제됩니다." }
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
                                    let item_id = item.id.clone();
                                    move |_| {
                                        pending_delete.set(None);
                                        handle_delete_item(item_id.clone());
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
