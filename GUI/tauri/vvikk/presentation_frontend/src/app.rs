#![allow(non_snake_case)]

use crate::{components::{AddPreset,
                         ItemForm,
                         ItemFormData,
                         QuickAddData,
                         VvkikBoard},
            models::{CreateItemRequest,
                     ItemKind,
                     UpdateItemRequest,
                     VvkikItem},
            store::{VvkikStore,
                    use_vvkik_store}};
use dioxus::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");

#[derive(Debug, Clone, PartialEq)]
enum AppView {
    Board,
    Add(Box<AddPreset>),
    Edit(Box<VvkikItem>),
}

pub fn App() -> Element {
    let store: VvkikStore = use_vvkik_store();
    let mut current_view = use_signal(|| AppView::Board);
    let mut pending_delete = use_signal(|| None::<VvkikItem>);
    let active_tab = use_signal(|| "tree".to_string());

    let items = store.items;
    let loading = store.loading;
    let error_message = store.error;
    let mut search_query = store.search_query;

    let handle_add_item = move |form_data: ItemFormData| {
        if store.is_busy() {
            return;
        }

        let position = store.next_position(form_data.kind, form_data.parent_id_opt().as_deref());
        match form_data.to_create_request(position) {
            | Ok(request) => {
                spawn(async move {
                    if store.create(request).await {
                        current_view.set(AppView::Board);
                    }
                });
            },
            | Err(message) => store.set_error(message),
        }
    };

    let handle_quick_add = move |quick_add: QuickAddData| {
        if store.is_busy() {
            return;
        }

        let position = store.next_position(quick_add.kind, quick_add.parent_id.as_deref());
        let request = CreateItemRequest {
            kind: quick_add.kind,
            parent_id: quick_add.parent_id,
            title: quick_add.title,
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: Some(position),
        };
        spawn(async move {
            store.create(request).await;
        });
    };

    let handle_edit_item = move |form_data: ItemFormData| {
        if store.is_busy() {
            return;
        }

        let AppView::Edit(item) = current_view.read().clone() else {
            return;
        };
        match form_data.to_update_request(item.id.clone()) {
            | Ok(request) => {
                spawn(async move {
                    if store.update(request).await {
                        current_view.set(AppView::Board);
                    }
                });
            },
            | Err(message) => store.set_error(message),
        }
    };

    let handle_delete_item = move |id: String| {
        spawn(async move {
            store.delete(id).await;
        });
    };

    // 트리에서 행을 드래그해 새 상위 항목 위에 놓으면 그 아래 맨 뒤로 이동한다.
    let handle_reparent = move |(item, new_parent): (VvkikItem, VvkikItem)| {
        if store.is_busy() {
            return;
        }

        let position = store.next_position(item.kind, Some(new_parent.id.as_str()));
        let request = UpdateItemRequest {
            id: item.id.clone(),
            kind: None,
            parent_id: Some(Some(new_parent.id.clone())),
            title: None,
            description: None,
            target_value: None,
            current_value: None,
            unit: None,
            position: Some(position),
            status: None,
        };
        spawn(async move {
            store.update(request).await;
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
                        form {
                            class: "search-form",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                spawn(async move { store.search().await });
                            },
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
                                    onclick: move |_| {
                                        spawn(async move { store.clear_search().await });
                                    },
                                    "초기화"
                                }
                            }
                        }
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                store.clear_error();
                                // 단계 탭을 보고 있었다면 그 단계를 기본 선택한다.
                                let kind = active_tab.read().parse::<ItemKind>().unwrap_or(ItemKind::Value);
                                current_view.set(AppView::Add(Box::new(AddPreset { kind, parent: None, title: String::new() })));
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
                        active_tab,
                        on_edit: move |item| {
                            store.clear_error();
                            current_view.set(AppView::Edit(Box::new(item)));
                        },
                        on_delete: move |item| pending_delete.set(Some(item)),
                        on_quick_add: handle_quick_add,
                        on_add_child: move |preset: AddPreset| {
                            store.clear_error();
                            current_view.set(AppView::Add(Box::new(preset)));
                        },
                        on_reparent: handle_reparent
                    }
                },
                AppView::Add(preset) => rsx! {
                    ItemForm {
                        item: None,
                        items: items.read().clone(),
                        preset: Some(*preset),
                        on_submit: handle_add_item,
                        on_cancel: move |_| {
                            store.clear_error();
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
                            store.clear_error();
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
