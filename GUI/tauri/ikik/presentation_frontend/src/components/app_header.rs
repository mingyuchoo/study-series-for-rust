#![allow(non_snake_case)]

//! 앱 상단 헤더: 모드·언어·테마 토글, 브랜드 블록, 그리고 보드 화면에서만
//! 보이는 검색·신규 동작. 토글 3종은 시그널을 직접 갱신하고, 검색·신규처럼
//! 스토어·뷰 전환이 얽힌 동작은 핸들러 prop으로 받아 컨트롤러(`App`)에
//! 남겨 둔다 — 헤더는 그리기만 하고 데이터 흐름은 모른다.

use super::icons::{LockIcon,
                   MoonIcon,
                   SunIcon};
use crate::{i18n::Lang,
            mode::Mode,
            theme::Theme};
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct AppHeaderProps {
    pub lang: Signal<Lang>,
    pub mode: Signal<Mode>,
    pub theme: Signal<Theme>,
    /// 보드 화면일 때만 검색·신규 동작을 보여 준다.
    pub is_board: bool,
    pub search_query: Signal<String>,
    /// 브랜드 타이틀 클릭 시 홈(보드)으로 이동한다.
    pub on_home: EventHandler<()>,
    pub on_search: EventHandler<()>,
    pub on_clear_search: EventHandler<()>,
    pub on_new: EventHandler<()>,
}

pub fn AppHeader(props: AppHeaderProps) -> Element {
    let mut lang = props.lang;
    let mut mode = props.mode;
    let mut theme = props.theme;
    let mut search_query = props.search_query;

    let t = *lang.read();
    let is_manage = mode.read().is_manage();
    let is_dark = *theme.read() == Theme::Dark;

    rsx! {
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
                    LockIcon { unlocked: is_manage }
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
                    aria_label: if is_dark { t.to_light_theme() } else { t.to_dark_theme() },
                    title: if is_dark { t.to_light_theme() } else { t.to_dark_theme() },
                    onclick: move |_| {
                        let next = theme.read().toggled();
                        theme.set(next);
                    },
                    if is_dark {
                        SunIcon {}
                    } else {
                        MoonIcon {}
                    }
                }
            }
            div { class: "brand-block",
                h1 {
                    button {
                        r#type: "button",
                        class: "brand-home",
                        aria_label: t.to_home(),
                        title: t.to_home(),
                        onclick: move |_| props.on_home.call(()),
                        "IKIK"
                    }
                }
                p { {t.tagline()} }
            }

            if props.is_board {
                div { class: "header-actions",
                    form {
                        class: "search-form",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            props.on_search.call(());
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
                                onclick: move |_| props.on_clear_search.call(()),
                                {t.reset()}
                            }
                        }
                    }
                    if is_manage {
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| props.on_new.call(()),
                            {t.new_item()}
                        }
                    }
                }
            }
        }
    }
}
