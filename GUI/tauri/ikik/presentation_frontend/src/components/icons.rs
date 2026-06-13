#![allow(non_snake_case)]

//! 헤더 토글이 쓰는 인라인 SVG 아이콘. 공통 stroke 스타일을 한곳에 모아
//! 마크업이 라우팅·핸들러 코드에 섞이지 않도록 분리한다.

use dioxus::prelude::*;

/// 18×24 좌표계의 stroke 아이콘 공통 골격. `children`에 path/circle 등을 둔다.
#[component]
fn StrokeIcon(children: Element) -> Element {
    rsx! {
        svg {
            width: "18",
            height: "18",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.8",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            {children}
        }
    }
}

/// 자물쇠. `unlocked`면 고리가 열린 모양(관리 모드), 아니면 닫힌 모양(사용
/// 모드).
#[component]
pub fn LockIcon(unlocked: bool) -> Element {
    rsx! {
        StrokeIcon {
            rect { x: "3", y: "11", width: "18", height: "10", rx: "2" }
            if unlocked {
                path { d: "M7 11V7a5 5 0 0 1 9.9-1" }
            } else {
                path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
            }
        }
    }
}

/// 해. 라이트 테마로 전환하는 토글에 쓴다(현재 다크일 때 노출).
#[component]
pub fn SunIcon() -> Element {
    rsx! {
        StrokeIcon {
            circle { cx: "12", cy: "12", r: "5" }
            path { d: "M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" }
        }
    }
}

/// 달. 다크 테마로 전환하는 토글에 쓴다(현재 라이트일 때 노출).
#[component]
pub fn MoonIcon() -> Element {
    rsx! {
        StrokeIcon {
            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
        }
    }
}
