#![allow(non_snake_case)]

use crate::{components::{AddPreset,
                         IkikBoard,
                         ItemDetail,
                         ItemForm,
                         ItemFormData,
                         QuickAddData},
            i18n,
            mode,
            models::{IkikItem,
                     ItemKind},
            store::{IkikStore,
                    use_ikik_store},
            theme::{self,
                    Theme}};
use dioxus::prelude::*;

static CSS: Asset = asset!("/assets/styles.css");

#[derive(Debug, Clone, PartialEq)]
enum AppView {
    Board,
    Add(Box<AddPreset>),
    /// 항목 상세 보기. 목록이 새로고침되어도 최신 항목을 보여 주도록
    /// 스냅숏 대신 id를 들고 매 렌더마다 스토어에서 찾는다.
    Detail(String),
    Edit(Box<IkikItem>),
}

pub fn App() -> Element {
    // 언어 컨텍스트는 스토어(오류 메시지)보다 먼저 제공해야 한다.
    let mut lang = use_signal(i18n::initial_lang);
    use_context_provider(|| lang);
    use_effect(move || i18n::apply_lang(*lang.read()));
    let t = *lang.read();

    let store: IkikStore = use_ikik_store();
    // 사용/관리 모드: 사용 모드(기본)에서는 구조 변경 진입점을 숨긴다.
    let mut mode = use_signal(mode::initial_mode);
    use_context_provider(|| mode);
    use_effect(move || mode::apply_mode(*mode.read()));
    let is_manage = mode.read().is_manage();
    let mut theme = use_signal(theme::initial_theme);
    // 시그널이 바뀔 때마다 <html data-theme>와 localStorage에 반영한다.
    use_effect(move || theme::apply_theme(*theme.read()));
    let mut current_view = use_signal(|| AppView::Board);
    let mut pending_delete = use_signal(|| None::<IkikItem>);
    let active_tab = use_signal(|| "dashboard".to_string());

    let items = store.items;
    let loading = store.loading;
    let error_message = store.error;
    let mut search_query = store.search_query;

    let handle_add_item = move |form_data: ItemFormData| {
        if store.is_busy() {
            return;
        }

        let position = store.next_position(form_data.kind, form_data.parent_id_opt().as_deref());
        match form_data.to_create_request(position, *lang.peek()) {
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
        let request = quick_add.to_create_request(position);
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
        match form_data.to_update_request(item.id.clone(), *lang.peek()) {
            | Ok(request) => {
                spawn(async move {
                    if store.update(request).await {
                        // 수정을 마치면 상세 보기로 돌아간다.
                        current_view.set(AppView::Detail(item.id.clone()));
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
    let handle_reparent = move |(item, new_parent): (IkikItem, IkikItem)| {
        spawn(async move {
            store.reparent(item, new_parent).await;
        });
    };

    rsx! {
        link { rel: "stylesheet", href: CSS }
        main { class: "app",
            header { class: "app-header",
                div { class: "header-controls",
                // 자물쇠 토글: 잠겨 있으면 사용 모드, 열려 있으면 관리 모드.
                button {
                    r#type: "button",
                    class: if is_manage { "mode-toggle active" } else { "mode-toggle" },
                    aria_label: if is_manage { t.to_use_mode() } else { t.to_manage_mode() },
                    title: if is_manage { t.to_use_mode() } else { t.to_manage_mode() },
                    onclick: move |_| {
                        let next = mode.read().toggled();
                        mode.set(next);
                    },
                    if is_manage {
                        svg {
                            width: "18",
                            height: "18",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.8",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            rect { x: "3", y: "11", width: "18", height: "10", rx: "2" }
                            path { d: "M7 11V7a5 5 0 0 1 9.9-1" }
                        }
                    } else {
                        svg {
                            width: "18",
                            height: "18",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.8",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            rect { x: "3", y: "11", width: "18", height: "10", rx: "2" }
                            path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "lang-toggle",
                    aria_label: t.to_other_lang(),
                    title: t.to_other_lang(),
                    onclick: move |_| {
                        let next = lang.read().toggled();
                        lang.set(next);
                    },
                    {t.lang_button()}
                }
                button {
                    r#type: "button",
                    class: "theme-toggle",
                    aria_label: if *theme.read() == Theme::Dark { t.to_light_theme() } else { t.to_dark_theme() },
                    title: if *theme.read() == Theme::Dark { t.to_light_theme() } else { t.to_dark_theme() },
                    onclick: move |_| {
                        let next = theme.read().toggled();
                        theme.set(next);
                    },
                    if *theme.read() == Theme::Dark {
                        svg {
                            width: "18",
                            height: "18",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.8",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            circle { cx: "12", cy: "12", r: "5" }
                            path { d: "M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" }
                        }
                    } else {
                        svg {
                            width: "18",
                            height: "18",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.8",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                        }
                    }
                }
                }
                div { class: "brand-block",
                    h1 { "IKIK" }
                    p { {t.tagline()} }
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
                                placeholder: t.search_placeholder(),
                                value: "{search_query}",
                                oninput: move |evt| search_query.set(evt.value())
                            }
                            button { r#type: "submit", class: "btn btn-secondary", {t.search()} }
                            if !search_query.read().trim().is_empty() {
                                button {
                                    r#type: "button",
                                    class: "btn btn-secondary",
                                    onclick: move |_| {
                                        spawn(async move { store.clear_search().await });
                                    },
                                    {t.reset()}
                                }
                            }
                        }
                        if is_manage {
                            button {
                                class: "btn btn-primary",
                                onclick: move |_| {
                                    store.clear_error();
                                    // 단계 탭을 보고 있었다면 그 단계를 기본 선택한다.
                                    let kind = active_tab.read().parse::<ItemKind>().unwrap_or(ItemKind::Identity);
                                    current_view.set(AppView::Add(Box::new(AddPreset { kind, parent: None, title: String::new() })));
                                },
                                {t.new_item()}
                            }
                        }
                    }
                }
            }

            if *loading.read() {
                div { class: "loading", {t.loading()} }
            }

            if let Some(error) = error_message.read().clone() {
                div { class: "error-message", "{error}" }
            }

            match current_view.read().clone() {
                AppView::Board => rsx! {
                    IkikBoard {
                        items: items.read().clone(),
                        is_filtering: !search_query.read().trim().is_empty(),
                        active_tab,
                        // 행 클릭은 수정이 아니라 읽기 전용 상세 보기를 연다.
                        on_open: move |item: IkikItem| {
                            store.clear_error();
                            current_view.set(AppView::Detail(item.id.clone()));
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
                AppView::Detail(id) => {
                    match items.read().iter().find(|item| item.id == id).cloned() {
                        Some(item) => rsx! {
                            ItemDetail {
                                // 브레드크럼으로 다른 항목 상세로 건너뛸 때
                                // 변경 이력 등이 새로 불러와지도록 강제 재마운트.
                                key: "{item.id}",
                                item,
                                items: items.read().clone(),
                                on_edit: move |target: IkikItem| {
                                    store.clear_error();
                                    current_view.set(AppView::Edit(Box::new(target)));
                                },
                                on_delete: move |target| pending_delete.set(Some(target)),
                                on_navigate: move |target: IkikItem| {
                                    store.clear_error();
                                    current_view.set(AppView::Detail(target.id.clone()));
                                },
                                on_back: move |_| {
                                    store.clear_error();
                                    current_view.set(AppView::Board);
                                }
                            }
                        },
                        // 삭제 등으로 사라진 항목이면 보드로 안내한다.
                        None => rsx! {
                            div { class: "empty-state",
                                p { {t.item_not_found()} }
                                button {
                                    r#type: "button",
                                    class: "btn btn-secondary",
                                    onclick: move |_| current_view.set(AppView::Board),
                                    {t.back_to_list()}
                                }
                            }
                        },
                    }
                },
                AppView::Edit(item) => rsx! {
                    ItemForm {
                        // 브레드크럼으로 다른 항목 수정으로 건너뛸 때 폼
                        // 시그널이 새 항목 값으로 초기화되도록 강제 재마운트.
                        key: "{item.id}",
                        item: Some((*item).clone()),
                        items: items.read().clone(),
                        on_submit: handle_edit_item,
                        on_navigate: move |target: IkikItem| {
                            store.clear_error();
                            current_view.set(AppView::Detail(target.id.clone()));
                        },
                        on_cancel: {
                            let item_id = item.id.clone();
                            move |_| {
                                store.clear_error();
                                current_view.set(AppView::Detail(item_id.clone()));
                            }
                        }
                    }
                }
            }

            if let Some(item) = pending_delete.read().clone() {
                div { class: "confirm-backdrop",
                    div { class: "confirm-dialog", role: "dialog", aria_label: t.confirm_delete_aria(),
                        h2 { {t.confirm_delete_title()} }
                        p { {t.confirm_delete_body(&item.title)} }
                        div { class: "confirm-actions",
                            button {
                                r#type: "button",
                                class: "btn btn-secondary",
                                onclick: move |_| pending_delete.set(None),
                                {t.cancel()}
                            }
                            button {
                                r#type: "button",
                                class: "btn btn-danger",
                                onclick: {
                                    let item_id = item.id.clone();
                                    move |_| {
                                        pending_delete.set(None);
                                        // 상세 화면에서 그 항목을 지우면 보드로 돌아간다.
                                        if matches!(current_view.read().clone(), AppView::Detail(id) if id == item_id) {
                                            current_view.set(AppView::Board);
                                        }
                                        handle_delete_item(item_id.clone());
                                    }
                                },
                                {t.delete()}
                            }
                        }
                    }
                }
            }
        }
    }
}
