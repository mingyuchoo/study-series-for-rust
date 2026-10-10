//! 화면 렌더링: 설정 화면과 채팅 화면.

use crate::{app::{AppRoot,
                  ChatState},
            text_input::TextInput};
use chrono::{DateTime,
             Local};
use gpui::{AnyElement,
           App,
           ClickEvent,
           Context,
           Div,
           Entity,
           IntoElement,
           Render,
           Window,
           WindowControlArea,
           div,
           prelude::*,
           px,
           rgb};
use p2p_core::{ChatEntry,
               Speaker};
use std::time::SystemTime;

const BG: u32 = 0x1e1e2e;
const PANEL: u32 = 0x181825;
const SURFACE: u32 = 0x313244;
const TEXT: u32 = 0xcdd6f4;
const MUTED: u32 = 0x7f849c;
const ACCENT: u32 = 0x89b4fa;
const ACCENT_HOVER: u32 = 0xb4befe;
const ERROR: u32 = 0xf38ba8;
const WHITE: u32 = 0xffffff;
/// Windows 기본 닫기 버튼의 hover 배경색
const CLOSE_HOVER: u32 = 0xe81123;
const TITLEBAR_HEIGHT: f32 = 32.;
/// 한글 글리프를 확실히 그리기 위한 Windows 기본 한글 글꼴
const FONT: &str = "Malgun Gothic";
/// 캡션 버튼 아이콘을 그리는 Windows 기본 아이콘 글꼴
const ICON_FONT: &str = "Segoe MDL2 Assets";

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = match self.chat.as_ref() {
            | Some(chat) => self.render_chat(chat, cx).into_any_element(),
            | None => self.render_setup(cx).into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_size(px(14.))
            .font_family(FONT)
            .on_action(cx.listener(Self::submit))
            .child(titlebar(window.is_maximized()))
            .child(div().flex_1().min_h_0().child(screen))
    }
}

impl AppRoot {
    fn render_setup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let start = button(
            "start-button",
            "시작",
            cx.listener(|this, _: &ClickEvent, window, cx| this.start_session(window, cx)),
        );
        let error = self.setup_error.clone().map(|message| div().text_sm().text_color(rgb(ERROR)).child(message));

        div().flex().items_center().justify_center().size_full().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w(px(360.))
                .p(px(24.))
                .rounded_lg()
                .bg(rgb(PANEL))
                .child(div().text_xl().child("P2P Chat"))
                .child(div().text_sm().text_color(rgb(MUTED)).child("같은 네트워크의 피어를 자동으로 찾습니다."))
                .child(field_label("닉네임"))
                .child(input_frame(self.nickname_input.clone()))
                .child(field_label("수신 포트 (UDP)"))
                .child(input_frame(self.port_input.clone()))
                .children(error)
                .child(start),
        )
    }

    fn render_chat(&self, chat: &ChatState, cx: &mut Context<Self>) -> impl IntoElement {
        let send = button("send-button", "보내기", cx.listener(|this, _: &ClickEvent, _, cx| this.send_chat(cx)));

        div().flex().size_full().child(sidebar(chat)).child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .h_full()
                .child(header(chat))
                .child(timeline(chat))
                .child(status_line(chat))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .p_3()
                        .bg(rgb(PANEL))
                        .child(input_frame(chat.message_input.clone()).flex_1())
                        .child(send),
                ),
        )
    }
}

/// macOS는 기본 창 버튼 공간을 남기고, Windows는 `WindowControlArea`로
/// 캡션 버튼을 처리한다(Windows 11 스냅 레이아웃 포함).
fn titlebar(is_maximized: bool) -> impl IntoElement {
    let max_icon = if is_maximized { "\u{E923}" } else { "\u{E922}" };
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .bg(rgb(PANEL))
        .border_b_1()
        .border_color(rgb(SURFACE))
        .child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .h_full()
                .px_3()
                .when(cfg!(target_os = "macos"), |title| title.pl(px(80.)))
                .text_sm()
                .text_color(rgb(MUTED))
                .window_control_area(WindowControlArea::Drag)
                .child("P2P Chat"),
        )
        .when(cfg!(target_os = "windows"), |titlebar| {
            titlebar
                .child(caption_button("titlebar-min", "\u{E921}", WindowControlArea::Min, SURFACE, TEXT))
                .child(caption_button("titlebar-max", max_icon, WindowControlArea::Max, SURFACE, TEXT))
                .child(caption_button("titlebar-close", "\u{E8BB}", WindowControlArea::Close, CLOSE_HOVER, WHITE))
        })
}

/// 제목 표시줄의 최소화·최대화·닫기 버튼. 클릭 동작은 gpui가 아니라 Windows가
/// 처리한다.
fn caption_button(id: &'static str, icon: &'static str, area: WindowControlArea, hover_bg: u32, hover_text: u32) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(46.))
        .h_full()
        .font_family(ICON_FONT)
        .text_size(px(10.))
        .text_color(rgb(TEXT))
        .hover(move |style| style.bg(rgb(hover_bg)).text_color(rgb(hover_text)))
        .window_control_area(area)
        .child(icon)
}

/// 입력창을 둘러싸는 테두리 상자
fn input_frame(input: Entity<TextInput>) -> Div { div().w_full().rounded_md().overflow_hidden().bg(rgb(SURFACE)).child(input) }

fn field_label(text: &'static str) -> impl IntoElement { div().text_sm().text_color(rgb(MUTED)).child(text) }

fn button(id: &'static str, label: &'static str, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> impl IntoElement {
    div()
        .id(id)
        .px_4()
        .py_2()
        .rounded_md()
        .bg(rgb(ACCENT))
        .text_color(rgb(BG))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(ACCENT_HOVER)))
        .child(label)
        .on_click(on_click)
}

fn header(chat: &ChatState) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(48.))
        .px_4()
        .border_b_1()
        .border_color(rgb(SURFACE))
        .child(div().text_lg().child("채팅"))
        .child(div().text_xs().text_color(rgb(MUTED)).child(format!("UDP {}", chat.listen_port)))
}

/// 왼쪽 사이드바: 발견된 피어 목록과 내 정보
fn sidebar(chat: &ChatState) -> impl IntoElement {
    let peers: Vec<Div> = chat
        .model
        .peers
        .iter()
        .map(|peer| {
            div()
                .flex()
                .flex_col()
                .px_2()
                .py_1()
                .rounded_md()
                .hover(|style| style.bg(rgb(SURFACE)))
                .child(peer.nickname.clone())
                .child(div().text_xs().text_color(rgb(MUTED)).child(peer.endpoint.to_string()))
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .w(px(220.))
        .h_full()
        .p_3()
        .gap_2()
        .bg(rgb(PANEL))
        .child(div().text_sm().text_color(rgb(MUTED)).child(format!("피어 {}명", chat.model.peers.len())))
        .when(chat.model.peers.is_empty(), |list| {
            list.child(div().text_xs().text_color(rgb(MUTED)).child("주변 피어를 찾는 중입니다..."))
        })
        .children(peers)
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .bg(rgb(SURFACE))
                .child(chat.nickname.clone())
                .child(div().text_xs().text_color(rgb(MUTED)).child(format!("ID {}", short_id(&chat.node_id)))),
        )
}

/// 대화 기록 영역. 새 항목이 생기면 `scroll_to_bottom`으로 맨 아래를 보여 준다.
fn timeline(chat: &ChatState) -> impl IntoElement {
    div()
        .id("timeline")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&chat.scroll)
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .when(chat.model.entries.is_empty(), |list| {
            list.child(div().text_color(rgb(MUTED)).child("아직 메시지가 없습니다."))
        })
        .children(chat.model.entries.iter().map(entry_row))
}

fn entry_row(entry: &ChatEntry) -> AnyElement {
    match entry {
        | ChatEntry::Notice {
            text,
            at,
        } => div()
            .w_full()
            .flex()
            .justify_center()
            .text_xs()
            .text_color(rgb(MUTED))
            .child(format!("{text} · {}", clock(at)))
            .into_any_element(),
        | ChatEntry::Message {
            speaker,
            content,
            at,
        } => {
            let (name, bubble, text_color, is_me) = match speaker {
                | Speaker::Me => ("나".to_string(), ACCENT, BG, true),
                | Speaker::Peer {
                    nickname, ..
                } => (nickname.clone(), SURFACE, TEXT, false),
            };
            let column = div()
                .flex()
                .flex_col()
                .gap_1()
                .max_w(px(480.))
                .child(div().text_xs().text_color(rgb(MUTED)).child(format!("{name}  {}", clock(at))))
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(rgb(bubble))
                        .text_color(rgb(text_color))
                        .child(content.clone()),
                );
            let row = div().w_full().flex();
            if is_me {
                row.justify_end().child(column.items_end()).into_any_element()
            } else {
                row.justify_start().child(column.items_start()).into_any_element()
            }
        },
    }
}

/// 하단 상태 줄: 종료 또는 전송 오류를 보여 준다.
fn status_line(chat: &ChatState) -> impl IntoElement {
    let message = if chat.model.stopped {
        Some("노드가 종료되었습니다.".to_string())
    } else {
        chat.model.error.clone()
    };
    div()
        .h(px(24.))
        .px_4()
        .flex()
        .items_center()
        .text_sm()
        .text_color(rgb(ERROR))
        .child(message.unwrap_or_default())
}

fn clock(at: &SystemTime) -> String { DateTime::<Local>::from(*at).format("%H:%M").to_string() }

fn short_id(node_id: &str) -> &str { node_id.get(.. 8).unwrap_or(node_id) }
