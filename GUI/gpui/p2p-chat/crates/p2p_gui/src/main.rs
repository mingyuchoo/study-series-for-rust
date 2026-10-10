#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! P2P Chat 데스크톱 앱 (gpui). 같은 네트워크의 피어를 UDP 멀티캐스트로 찾아
//! 채팅한다.

mod app;
mod model;
mod text_input;
mod view;

use app::AppRoot;
use gpui::{App,
           AppContext,
           Application,
           Bounds,
           KeyBinding,
           TitlebarOptions,
           WindowBounds,
           WindowOptions,
           actions,
           point,
           px,
           size};
use p2p_runtime::NodeSession;
use std::{cell::RefCell,
          rc::Rc};

actions!(p2p_chat_app, [Quit]);

fn main() {
    let session_slot: Rc<RefCell<Option<NodeSession>>> = Rc::new(RefCell::new(None));

    Application::new().run(move |cx: &mut App| {
        let quit_session_slot = session_slot.clone();
        // macOS는 run이 반환되기 전에 종료하므로 종료 콜백에서 노드를 정리한다.
        cx.on_app_quit(move |_| {
            if let Some(session) = quit_session_slot.borrow_mut().take() {
                session.shutdown();
            }
            std::future::ready(())
        })
        .detach();
        cx.bind_keys(text_input::key_bindings());
        cx.bind_keys([KeyBinding::new("enter", app::Submit, None), KeyBinding::new("secondary-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(960.), px(640.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(640.), px(420.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("P2P Chat".into()),
                        // macOS의 기본 창 버튼은 투명 제목 표시줄에서도 남는다.
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.), px(9.))),
                    }),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| AppRoot::new(session_slot, cx)),
            )
            .unwrap();

        window
            .update(cx, |root, window, cx| {
                root.focus_first_field(window, cx);
                cx.activate(true);
            })
            .unwrap();
    });
}
