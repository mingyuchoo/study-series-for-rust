#![windows_subsystem = "windows"]
use chrono::Local;
use gpui::{prelude::*, *};
use stillnote::{
    JournalStore,
    input::bind_input_keys,
    ui::{JournalView, journal_window_options},
};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let path = if let Some(index) = args.iter().position(|a| a == "--data-file") {
        args.get(index + 1).map(std::path::PathBuf::from).expect("--data-file requires a file path")
    } else {
        JournalStore::default_path().expect("User data directory unavailable")
    };
    Application::new().run(move |cx: &mut App| {
        bind_input_keys(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1360.), px(900.)), cx);
        cx.open_window(journal_window_options(bounds), move |_, cx| {
            cx.new(|cx| JournalView::new(path, Local::now().date_naive(), cx))
        })
        .expect("Unable to open GPUI window");
        cx.activate(true);
    });
}
