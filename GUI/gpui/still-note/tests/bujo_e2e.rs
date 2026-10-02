use gpui::{
    AppContext, ClipboardItem, Entity, EntityInputHandler, Focusable, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px,
    size,
};
use std::fs;
use stillnote::{Entry, Filter, Journal, Kind, Log, Status, input::bind_input_keys, parse_date, ui::JournalView};
use tempfile::tempdir;

// Snapshot reads cross the test context's App boundary; no mutation or UI
// commands are exposed through this assertion-only helper.
struct Snapshot {
    journal: Journal,
    visible: Vec<Entry>,
    date: chrono::NaiveDate,
    log: Log,
    error: Option<String>,
    filter: Filter,
    index: bool,
    menu_open: bool,
    entry_input: Entity<stillnote::input::TextInput>,
    entry_text: String,
}
impl Snapshot {
    fn journal(&self) -> &Journal {
        &self.journal
    }
    fn visible_entries(&self) -> Vec<&Entry> {
        self.visible.iter().collect()
    }
}
fn snapshot(view: &Entity<JournalView>, cx: &VisualTestContext) -> Snapshot {
    cx.read(|app| {
        let view = view.read(app);
        Snapshot {
            journal: view.journal().clone(),
            visible: view.visible_entries(app).into_iter().cloned().collect(),
            date: view.date,
            log: view.log.clone(),
            error: view.error.clone(),
            filter: view.filter,
            index: view.index,
            menu_open: view.menu_open,
            entry_input: view.entry_input.clone(),
            entry_text: view.entry_input.read(app).content.to_string(),
        }
    })
}

// Every click follows a draw of the production root view, not a domain command.
fn render(cx: &mut VisualTestContext) {
    cx.update(|window, app| window.draw(app)).clear();
    cx.run_until_parked();
}

fn click(cx: &mut VisualTestContext, selector: impl Into<String>) {
    render(cx);
    let selector: &'static str = Box::leak(selector.into().into_boxed_str());
    let bounds = cx.debug_bounds(selector).unwrap_or_else(|| panic!("rendered control missing: {selector}"));
    assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.), "zero-area control: {selector}");
    cx.simulate_click(bounds.center(), Modifiers::none());
    render(cx);
}

fn type_in(cx: &mut VisualTestContext, selector: &str, value: &str) {
    click(cx, selector);
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input(value);
    render(cx);
}

fn add(cx: &mut VisualTestContext, value: &str) {
    type_in(cx, "entry-input", value);
    cx.simulate_keystrokes("enter");
    render(cx);
}

fn set_date(cx: &mut VisualTestContext, value: &str) {
    type_in(cx, "date-input", value);
    cx.simulate_keystrokes("enter");
    render(cx);
}

fn reveal_settings(cx: &mut VisualTestContext) {
    render(cx);
    if cx.debug_bounds("language-en").is_none() {
        click(cx, "nav-menu-toggle");
    }
}

#[gpui::test]
fn lt01_lt02_lt03_lt05_real_controls_preserve_draft_selection_and_restore_settings(cx: &mut TestAppContext) {
    use stillnote::settings::{Language, ThemeMode};
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    render(cx);
    cx.read(|app| {
        assert_eq!(view.read(app).settings.language, Language::Korean);
        assert_eq!(view.read(app).settings.theme, ThemeMode::System);
    });
    add(cx, "사용자 기록 English 그대로");
    let journal = snapshot(&view, cx).journal().clone();
    let bytes = fs::read(&path).unwrap();
    let id = journal.entries[0].id;
    click(cx, format!("edit-{id}"));
    type_in(cx, "entry-input", "한글🙂 draft");
    cx.simulate_keystrokes("home right shift-right");
    render(cx);
    let input = snapshot(&view, cx).entry_input;
    let before_selection = cx.update(|window, app| input.update(app, |input, cx| input.selected_text_range(false, window, cx).unwrap().range));
    reveal_settings(cx);
    for selector in ["language-en", "theme-light", "theme-dark", "language-ko", "language-en"] {
        click(cx, selector);
    }
    assert_eq!(snapshot(&view, cx).entry_input, input, "changing presentation must retain input Entity");
    assert_eq!(snapshot(&view, cx).entry_text, "한글🙂 draft");
    assert_eq!(snapshot(&view, cx).journal(), &journal);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(cx.window_title().as_deref(), Some("Stillnote · My bullet journal"));
    cx.read(|app| {
        let view = view.read(app);
        assert_eq!(view.editing, Some(id));
        assert_eq!(view.date, parse_date("2026-10-02").unwrap());
        assert_eq!(view.settings.language, Language::English);
        assert_eq!(view.settings.theme, ThemeMode::Dark);
        assert_eq!(view.window_title, "Stillnote · My bullet journal");
        assert_eq!(view.entry_input.read(app).palette, stillnote::theme::Palette::DARK);
        assert_eq!(view.entry_input.read(app).placeholder.as_ref(), "Capture a thought in one line");
        assert_eq!(view.search_input.read(app).placeholder.as_ref(), "Search all entries…");
        assert_eq!(view.collection_input.read(app).placeholder.as_ref(), "New collection name");
        assert_eq!(view.target_input.read(app).placeholder.as_ref(), "Target date YYYY-MM-DD");
    });
    cx.update(|window, app| {
        input.update(app, |input, cx| {
            assert_eq!(input.selected_text_range(false, window, cx).unwrap().range, before_selection);
            window.focus(&input.focus_handle(cx));
        })
    });
    cx.simulate_input("X");
    render(cx);
    assert_eq!(
        snapshot(&view, cx).entry_text,
        "한X🙂 draft",
        "typing must replace original selection after toggles"
    );
    // Unsaved draft is independent of persisted presentation settings.
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    let (reopened, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    render(cx);
    cx.read(|app| {
        assert_eq!(reopened.read(app).settings.language, Language::English);
        assert_eq!(reopened.read(app).settings.theme, ThemeMode::Dark);
        assert_eq!(reopened.read(app).entry_input.read(app).palette, stillnote::theme::Palette::DARK);
    });
    assert_eq!(snapshot(&reopened, cx).journal(), &journal);
}

#[gpui::test]
fn lt04_lt09_shared_appearance_handler_updates_system_and_all_inputs(cx: &mut TestAppContext) {
    use gpui::WindowAppearance;
    use stillnote::{settings::ThemeMode, theme::Palette};
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    reveal_settings(cx);
    for (selector, mode) in [
        ("theme-system", ThemeMode::System),
        ("theme-light", ThemeMode::Light),
        ("theme-dark", ThemeMode::Dark),
    ] {
        click(cx, selector);
        for (appearance, system_expected) in [(WindowAppearance::Light, ThemeMode::Light), (WindowAppearance::Dark, ThemeMode::Dark)] {
            cx.update(|_, app| view.update(app, |view, cx| view.appearance_changed(appearance, cx)));
            let expected = if mode == ThemeMode::System { system_expected } else { mode };
            cx.read(|app| {
                let view = view.read(app);
                assert_eq!(view.settings.theme, mode);
                assert_eq!(view.effective_theme, expected);
                for input in [
                    &view.entry_input,
                    &view.search_input,
                    &view.date_input,
                    &view.collection_input,
                    &view.target_input,
                ] {
                    assert_eq!(input.read(app).palette, Palette::for_theme(expected));
                }
            });
        }
    }
}

#[gpui::test]
fn lt02_lt06_settings_failure_localizes_without_blocking_user_data(cx: &mut TestAppContext) {
    use stillnote::settings::Language;
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let invalid = b"{invalid settings";
    fs::write(dir.path().join("settings.json"), invalid).unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    render(cx);
    assert!(cx.read(|app| view.read(app).settings_error.is_some()));
    reveal_settings(cx);
    click(cx, "language-en");
    let error = cx.read(|app| view.read(app).settings_error.clone().unwrap());
    assert!(error.is_ascii(), "English settings error contains untranslated application text: {error}");
    assert!(error.to_lowercase().contains("setting"));
    cx.read(|app| assert_eq!(view.read(app).settings.language, Language::English));
    assert_eq!(fs::read(dir.path().join("settings.json")).unwrap(), invalid);
    add(cx, "설정 오류에도 저널 저장");
    assert_eq!(stillnote::Session::open(path).unwrap().journal.entries[0].text, "설정 오류에도 저널 저장");
    set_date(cx, "2023-02-29");
    let error = snapshot(&view, cx).error.unwrap();
    assert!(error.is_ascii(), "English date error contains Korean: {error}");
}

#[gpui::test]
fn lt07_both_languages_all_required_widths_keep_settings_and_window_controls_reachable(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (_, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    for width in [600., 768., 1024., 1360.] {
        cx.simulate_resize(size(px(width), px(900.)));
        for language in ["language-ko", "language-en"] {
            reveal_settings(cx);
            click(cx, language);
            for selector in [
                "language-ko",
                "language-en",
                "theme-system",
                "theme-light",
                "theme-dark",
                "window-minimize",
                "window-maximize",
                "window-close",
            ] {
                assert_reachable(cx, selector, width, 900.);
            }
            let selectors = ["language-ko", "language-en", "theme-system", "theme-light", "theme-dark"];
            for (i, a) in selectors.iter().enumerate() {
                let a = cx.debug_bounds(a).unwrap();
                for b in &selectors[i + 1..] {
                    let b = cx.debug_bounds(b).unwrap();
                    assert!(
                        a.right() <= b.left() || b.right() <= a.left() || a.bottom() <= b.top() || b.bottom() <= a.top(),
                        "overlapping settings {a:?}/{b:?}"
                    );
                }
            }
            click(cx, "theme-light");
            click(cx, "theme-dark");
        }
    }
}

#[gpui::test]
fn lt08_keyboard_tab_reaches_and_activates_every_setting(cx: &mut TestAppContext) {
    use stillnote::settings::{Language, ThemeMode};
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("settings.json"), r#"{"language":"english","theme":"dark"}"#).unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    cx.simulate_resize(size(px(1600.), px(900.)));
    render(cx);
    for index in 0..5 {
        let mut reached = false;
        for _ in 0..30 {
            cx.simulate_keystrokes("tab");
            render(cx);
            if cx.update(|window, app| view.read(app).setting_focus[index].is_focused(window)) {
                reached = true;
                break;
            }
        }
        assert!(reached, "keyboard could not focus setting {index}");
        cx.simulate_keystrokes(if index % 2 == 0 { "enter" } else { "space" });
        render(cx);
        cx.read(|app| {
            let settings = view.read(app).settings;
            match index {
                | 0 => assert_eq!(settings.language, Language::Korean),
                | 1 => assert_eq!(settings.language, Language::English),
                | 2 => assert_eq!(settings.theme, ThemeMode::System),
                | 3 => assert_eq!(settings.theme, ThemeMode::Light),
                | 4 => assert_eq!(settings.theme, ThemeMode::Dark),
                | _ => unreachable!(),
            }
        });
    }
}

#[gpui::test]
fn lt02_lt03_existing_errors_search_migration_and_ime_survive_presentation_changes(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    add(cx, "검색할 사용자 기록");
    let id = snapshot(&view, cx).journal().entries[0].id;
    click(cx, format!("migrate-{id}"));
    type_in(cx, "target-input", "2027-03-01");
    click(cx, "target-future");
    type_in(cx, "search-input", "사용자");
    let before = snapshot(&view, cx);
    let inputs = cx.read(|app| {
        let view = view.read(app);
        [
            view.entry_input.clone(),
            view.search_input.clone(),
            view.date_input.clone(),
            view.collection_input.clone(),
            view.target_input.clone(),
        ]
    });
    let input = inputs[0].clone();
    cx.update(|window, app| {
        input.update(app, |input, cx| {
            input.replace_and_mark_text_in_range(None, "한🙂", Some(1..1), window, cx);
        })
    });
    reveal_settings(cx);
    click(cx, "language-en");
    click(cx, "theme-light");
    cx.read(|app| {
        let view = view.read(app);
        assert_eq!(view.migrating, Some(id));
        assert_eq!(view.target_log, Log::Future);
        assert_eq!(view.target_input.read(app).content.as_ref(), "2027-03-01");
        assert_eq!(view.search_input.read(app).content.as_ref(), "사용자");
        assert_eq!(view.date, before.date);
        assert_eq!(view.log, before.log);
        assert_eq!(view.filter, before.filter);
        assert_eq!(view.entry_input, inputs[0]);
        assert_eq!(view.search_input, inputs[1]);
        assert_eq!(view.date_input, inputs[2]);
        assert_eq!(view.collection_input, inputs[3]);
        assert_eq!(view.target_input, inputs[4]);
    });
    cx.update(|window, app| {
        input.update(app, |input, cx| {
            assert_eq!(input.content.as_ref(), "한🙂");
            assert_eq!(input.marked_text_range(window, cx), Some(0..3));
            assert_eq!(input.selected_text_range(false, window, cx).unwrap().range, 1..1);
            input.unmark_text(window, cx);
        })
    });
    type_in(cx, "search-input", "");
    // A persisted journal write failure must use the current language, and
    // translating the already-visible error must not clear the user's draft.
    fs::create_dir(path.with_extension("json.bak")).unwrap();
    add(cx, "failed draft");
    let english = snapshot(&view, cx).error.unwrap();
    assert!(english.contains("Unable to save"), "{english}");
    assert!(english.is_ascii(), "application-owned failure text was not translated: {english}");
    reveal_settings(cx);
    click(cx, "language-ko");
    assert!(snapshot(&view, cx).error.unwrap().contains("저장하지 못했습니다"));
    assert_eq!(snapshot(&view, cx).entry_text, "failed draft");
    assert_eq!(snapshot(&view, cx).journal(), before.journal());
}

fn assert_reachable(cx: &mut VisualTestContext, selector: &str, width: f32, height: f32) {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let bounds = cx.debug_bounds(selector).unwrap_or_else(|| panic!("missing {selector} at {width}px"));
    assert!(
        bounds.left() >= px(0.) && bounds.right() <= px(width),
        "horizontal clipping: {selector} {bounds:?} at {width}"
    );
    assert!(
        bounds.top() >= px(0.) && bounds.bottom() <= px(height),
        "vertical clipping: {selector} {bounds:?} at {width}"
    );
    assert!(
        bounds.size.width >= px(40.) && bounds.size.height >= px(40.),
        "small target: {selector} {bounds:?}"
    );
}

#[test]
fn font_ac01_ac02_production_constructor_registers_pretendard_at_every_ui_weight() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let observed = std::rc::Rc::new(std::cell::Cell::new(false));
    let callback_observed = observed.clone();
    // TestAppContext uses NoopTextSystem: it discards font bytes and synthesizes
    // glyph advances. This no-window native app uses the production platform
    // text system so registration and glyph assertions below inspect real fonts.
    gpui::Application::new().run(move |app| {
        let _view = app.new(|cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
        assert!(app.windows().is_empty(), "font verification must not open application windows");
        let text = app.text_system();
        assert!(
            text.all_font_names().iter().any(|name| name == "Pretendard"),
            "registered family must be available without relying on a fallback"
        );
        for weight in [400., 500., 600., 700.] {
            let mut requested = gpui::font("Pretendard");
            requested.weight = gpui::FontWeight(weight);
            let id = text.resolve_font(&requested);
            let resolved = text.get_font_for_id(id).unwrap();
            assert_eq!(resolved.family.as_ref(), "Pretendard");
            assert_eq!(resolved.weight, gpui::FontWeight(weight));
            for ch in ['A', '한', '글', '힣'] {
                assert!(text.advance(id, px(16.), ch).is_ok(), "registered face missing {ch}, weight {weight}");
            }
        }
        let mut heading_font = gpui::font("Pretendard");
        heading_font.weight = gpui::FontWeight(700.);
        let native = app
            .open_window(
                {
                    let mut options = stillnote::ui::journal_window_options(gpui::Bounds::new(point(px(0.), px(0.)), size(px(1360.), px(900.))));
                    assert!(options.titlebar.as_ref().unwrap().appears_transparent);
                    options.show = false;
                    options.focus = false;
                    options
                },
                |_, app| app.new(|cx| JournalView::new(path, parse_date("2026-10-02").unwrap(), cx)),
            )
            .unwrap();
        app.update_window(native.into(), |_, window, app| {
            window.draw(app).clear();
            let wordmark = window.text_system().shape_line(
                "stillnote".into(),
                px(24.),
                &[gpui::TextRun {
                    len: 9,
                    font: heading_font,
                    color: gpui::rgb(0xffffff).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            assert_eq!(wordmark.len(), 9);
            assert!(
                wordmark.width > px(50.) && wordmark.width < px(120.),
                "whole native wordmark must fit nav without per-character wrapping: {:?}",
                wordmark.width
            );
            let mut control_font = gpui::font("Pretendard");
            control_font.weight = gpui::FontWeight(600.);
            let creation_label = "+ 컬렉션 만들기";
            let shaped = window.text_system().shape_line(
                creation_label.into(),
                px(14.),
                &[gpui::TextRun {
                    len: creation_label.len(),
                    font: control_font,
                    color: gpui::rgb(0x0a0a0a).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            assert_eq!(shaped.len(), creation_label.len());
            // 208px sidebar, 24px card padding each side, 20px button padding
            // each side and two 1px borders leave 118px for the whole label.
            assert!(
                shaped.width > px(0.) && shaped.width <= px(118.),
                "full native creation label exceeds sidebar inner width: {:?}",
                shaped.width
            );
            eprintln!("Native Pretendard creation label14/600 width {:?}; available content118px", shaped.width);
            window.remove_window();
        })
        .unwrap();
        callback_observed.set(true);
        app.quit();
    });
    assert!(observed.get(), "native registration assertions must execute");
}

fn reveal_in_journal(cx: &mut VisualTestContext, selector: &str, width: f32, height: f32) {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    for _ in 0..80 {
        render(cx);
        let area = cx.debug_bounds("journal-scroll").unwrap();
        let target = cx.debug_bounds(selector).unwrap_or_else(|| panic!("missing {selector}"));
        if target.top() >= area.top() && target.bottom() <= area.bottom() {
            assert_reachable(cx, selector, width, height);
            return;
        }
        let delta = if target.top() < area.top() { 120. } else { -120. };
        cx.simulate_event(ScrollWheelEvent {
            position: area.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        });
    }
    panic!("cannot reveal {selector} at {width}x{height}");
}

fn assert_chrome(cx: &mut VisualTestContext, width: f32, height: f32) {
    let nav = cx.debug_bounds("design-nav").unwrap();
    let drag = cx.debug_bounds("window-drag-region").unwrap();
    assert_eq!(nav.size.height, px(64.));
    assert!(drag.left() >= nav.left() && drag.right() <= nav.right() && drag.top() >= nav.top() && drag.bottom() <= nav.bottom());
    let mut previous_right = None;
    for selector in ["window-minimize", "window-maximize", "window-close"] {
        assert_reachable(cx, selector, width, height);
        let control = cx.debug_bounds(selector).unwrap();
        assert_eq!(control.size, size(px(40.), px(40.)));
        assert!(control.top() >= nav.top() && control.bottom() <= nav.bottom());
        assert!(drag.right() <= control.left(), "drag blank must end before window controls");
        if let Some(right) = previous_right {
            assert!(control.left() >= right);
        }
        previous_right = Some(control.right());
    }
    assert_eq!(previous_right.unwrap(), px(width - 24.), "controls must align to right24px gutter");
    assert!(cx.debug_bounds("window-maximize-glyph").is_some());
    assert!(cx.debug_bounds("window-restore-glyph").is_none(), "fresh headless windows are not maximized");
    for selector in ["nav-wordmark", if width < 768. { "nav-menu-toggle" } else { "nav-index" }] {
        let interactive = cx.debug_bounds(selector).unwrap();
        assert!(
            interactive.right() <= drag.left(),
            "title drag blank must exclude logo/navigation/menu: {selector}"
        );
        assert!(interactive.right() <= cx.debug_bounds("window-minimize").unwrap().left());
    }
}

#[gpui::test]
fn layout_ac01_ac02_ac03_ac04_long_text_short_windows_scroll_and_resize_preserve_state(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let long_text = format!(
        "{} {} END끝",
        "한국어와 English 긴 기록을 끝까지 읽습니다. ".repeat(8),
        "UnbrokenToken".repeat(30)
    );
    let collection_name = format!("{} END컬렉션", "긴 컬렉션 Collection ".repeat(16));
    for (width, height) in [(600., 400.), (767., 500.), (768., 650.), (1024., 400.), (1360., 500.), (1600., 900.)] {
        let path = dir.path().join(format!("layout-{width}-{height}.json"));
        let mut fixture = stillnote::Session::open(&path).unwrap();
        let collection = fixture.transact(|journal| journal.add_collection(&collection_name)).unwrap();
        let entry = fixture
            .transact(|journal| journal.add_entry(parse_date("2026-10-02")?, Log::Daily, Kind::Task, &long_text))
            .unwrap();
        for index in 0..8 {
            fixture
                .transact(|journal| journal.add_entry(parse_date("2026-10-02")?, Log::Daily, Kind::Note, &format!("스크롤 기록 {index}")))
                .unwrap();
        }
        drop(fixture);
        let window = cx.update(|app| {
            app.open_window(
                gpui::WindowOptions {
                    window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(width), px(height)),
                    ))),
                    ..Default::default()
                },
                |_, app| app.new(|cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx)),
            )
            .unwrap()
        });
        let view = window.root(cx).unwrap();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let cx = &mut visual;
        cx.simulate_resize(size(px(width), px(height)));
        render(cx);
        assert_chrome(cx, width, height);
        let nav = cx.debug_bounds("design-nav").unwrap();
        let logo = cx.debug_bounds("nav-wordmark").unwrap();
        assert!(logo.size.width >= px(86.) && logo.size.height <= px(40.));
        assert!(logo.left() >= nav.left() && logo.right() <= nav.right() && logo.top() >= nav.top() && logo.bottom() <= nav.bottom());
        assert_eq!(nav.size.height, px(64.));
        let text = cx.debug_bounds(Box::leak(format!("text-{entry}").into_boxed_str())).unwrap();
        assert!(
            text.left() >= px(0.) && text.right() <= px(width),
            "long token overflows horizontally: {text:?}"
        );
        assert!(text.size.height > px(50.), "long content must wrap rather than truncate");
        let label = cx.debug_bounds(Box::leak(format!("label-collection-{collection}").into_boxed_str())).unwrap();
        assert!(label.left() >= px(0.) && label.right() <= px(width));
        // DESIGN nav labels use 14px * 1.4 line height; two lines may round
        // down from 39.2 to 39px. The button target still has a 40px minimum.
        assert!(
            label.size.height >= px(2. * 19.6 - 1.),
            "long collection name must have at least two lines at {width}x{height}: {label:?}"
        );
        let collection_button = cx.debug_bounds(Box::leak(format!("collection-{collection}").into_boxed_str())).unwrap();
        assert!(collection_button.size.height >= px(40.));
        assert!(label.left() >= collection_button.left() && label.right() <= collection_button.right());
        assert!(label.top() >= collection_button.top() && label.bottom() <= collection_button.bottom());
        assert_eq!(snapshot(&view, cx).journal().entry(entry).unwrap().text, long_text);
        assert_eq!(snapshot(&view, cx).journal().collections[0].name, collection_name);
        if height < 650. {
            assert!(cx.debug_bounds("design-sidebar").is_none());
            assert!(cx.debug_bounds("design-aside").is_none());
            reveal_in_journal(cx, "collection-input", width, height);
            reveal_in_journal(cx, "create-collection", width, height);
        }
        reveal_in_journal(cx, "collection-input", width, height);
        type_in(cx, "collection-input", &format!("생성 {width}x{height} café🙂"));
        reveal_in_journal(cx, "create-collection", width, height);
        let create = cx.debug_bounds("create-collection").unwrap();
        let fixed_label = cx.debug_bounds("label-create-collection").unwrap();
        assert_eq!(create.size.height, px(40.));
        assert!(
            fixed_label.size.height > px(0.) && fixed_label.size.height <= px(15.),
            "fixed creation label must remain one14px line: {fixed_label:?}"
        );
        assert!(fixed_label.left() >= create.left() + px(21.) && fixed_label.right() <= create.right() - px(21.));
        assert!(fixed_label.top() >= create.top() && fixed_label.bottom() <= create.bottom());
        assert!((fixed_label.center().y - create.center().y).abs() <= px(1.), "creation label must be centered");
        click(cx, "create-collection");
        let state = snapshot(&view, cx);
        assert_eq!(state.journal().collections.len(), 2);
        let created = &state.journal().collections[1];
        assert_eq!(created.name, format!("생성 {width}x{height} café🙂"));
        assert_eq!(state.log, Log::Collection(created.id));
        assert_eq!(stillnote::Session::open(&path).unwrap().journal, state.journal);
        if width < 768. {
            click(cx, "nav-menu-toggle");
        }
        click(cx, "nav-daily");
        reveal_in_journal(cx, &format!("edit-{entry}"), width, height);
        click(cx, format!("edit-{entry}"));
        reveal_in_journal(cx, "entry-input", width, height);
        type_in(cx, "entry-input", "수정 중 한글🙂 draft");
        let before = snapshot(&view, cx).journal().clone();
        cx.simulate_resize(size(px(600.), px(400.)));
        render(cx);
        assert_eq!(snapshot(&view, cx).journal(), &before);
        assert_eq!(snapshot(&view, cx).entry_text, "수정 중 한글🙂 draft");
        if !snapshot(&view, cx).menu_open {
            click(cx, "nav-menu-toggle");
        }
        assert!(snapshot(&view, cx).menu_open);
        click(cx, "nav-menu-toggle");
        assert!(!snapshot(&view, cx).menu_open);
        reveal_in_journal(cx, "save-entry", 600., 400.);
        let save = cx.debug_bounds("save-entry").unwrap();
        let save_label = cx.debug_bounds("label-save-entry").unwrap();
        assert!(save_label.size.height <= px(15.));
        assert!(
            save_label.left() >= save.left() && save_label.right() <= save.right() && save_label.top() >= save.top() && save_label.bottom() <= save.bottom()
        );
        click(cx, "save-entry");
        assert_eq!(snapshot(&view, cx).journal().entry(entry).unwrap().text, "수정 중 한글🙂 draft");
        assert_eq!(stillnote::Session::open(&path).unwrap().journal, snapshot(&view, cx).journal);
        // Render a long diagnostic fixture without mutating journal state.
        let diagnostic = format!("{} END오류", "오류 설명 diagnostic ".repeat(18));
        cx.update(|_, app| {
            view.update(app, |view, cx| {
                view.error = Some(diagnostic.clone());
                cx.notify();
            })
        });
        render(cx);
        let error = cx.debug_bounds("error-message").unwrap();
        assert!(error.left() >= px(0.) && error.right() <= px(600.));
        assert!(error.size.height > px(40.), "diagnostic must wrap without losing its tail");
        assert_eq!(snapshot(&view, cx).error.as_deref(), Some(diagnostic.as_str()));
        reveal_in_journal(cx, "design-footer", 600., 400.);
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }
}

#[gpui::test]
fn design_ac02_ac04_all_breakpoints_keep_controls_and_actions_reachable(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let mut previous_wide = None;
    for width in [600., 767., 768., 1024., 1360., 1600., 1920., 2560., 2880.] {
        let path = dir.path().join(format!("journal-{width}.json"));
        // GPUI 0.2.2 retains removed debug selectors in Frame::clear. Start at
        // each target width so absence assertions cannot read stale wide frames.
        let window = cx.update(|app| {
            app.open_window(
                gpui::WindowOptions {
                    window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(width), px(900.)),
                    ))),
                    ..Default::default()
                },
                |_, app| app.new(|cx| JournalView::new(path, parse_date("2026-10-02").unwrap(), cx)),
            )
            .unwrap()
        });
        let view = window.root(cx).unwrap();
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        let cx = &mut visual;
        cx.simulate_resize(size(px(width), px(900.)));
        render(cx);
        let nav = cx.debug_bounds("design-nav").unwrap();
        assert_chrome(cx, width, 900.);
        assert_eq!(nav.top(), px(0.), "top navigation must be pinned to the top");
        assert_eq!(nav.size.height, px(64.));
        assert_eq!(nav.size.width, px(width));
        let nav_content = cx.debug_bounds("design-nav-content").unwrap();
        assert_eq!(nav_content.left(), px(0.));
        assert_eq!(nav_content.right(), px(width));
        assert_eq!(cx.debug_bounds("nav-wordmark").unwrap().left(), px(24.));
        let content = cx.debug_bounds("design-container").unwrap();
        assert_eq!(content.left(), px(0.));
        assert_eq!(content.right(), px(width));
        let main = cx.debug_bounds("design-main").unwrap();
        let first = cx.debug_bounds("design-sidebar").unwrap_or(main);
        let last = cx.debug_bounds("design-aside").unwrap_or(main);
        assert_eq!(first.left(), px(24.), "inner content must begin at24px");
        assert_eq!(last.right(), px(width - 24.), "inner content must fill through right24px gutter");
        let footer = cx.debug_bounds("design-footer").unwrap();
        assert_eq!(footer.left(), px(24.));
        assert_eq!(footer.right(), px(width - 24.));
        for panel in ["design-sidebar", "design-aside"] {
            if let Some(bounds) = cx.debug_bounds(panel) {
                assert_eq!(bounds.size.width, px(208.), "side panels retain their width");
            }
        }
        if width >= 1360. {
            if let Some((previous_width, previous_main)) = previous_wide {
                assert_eq!(
                    main.size.width - previous_main,
                    px(width - previous_width),
                    "journal must absorb added viewport width"
                );
            }
            previous_wide = Some((width, main.size.width));
        }
        assert_eq!(cx.debug_bounds("nav-menu-toggle").is_some(), width < 768.);
        if width < 768. {
            assert!(!snapshot(&view, cx).menu_open);
            assert!(cx.debug_bounds("design-mobile-menu").is_none());
            assert!(cx.debug_bounds("nav-daily").is_none(), "closed mobile menu must collapse navigation");
            assert_reachable(cx, "nav-menu-toggle", width, 900.);
            click(cx, "nav-menu-toggle");
            assert!(snapshot(&view, cx).menu_open);
            assert!(cx.debug_bounds("design-mobile-menu").is_some());
        }
        for selector in [
            "nav-daily",
            "nav-monthly",
            "nav-future",
            "nav-index",
            "search-input",
            "collection-input",
            "create-collection",
        ] {
            assert_reachable(cx, selector, width, 900.);
        }
        if width < 768. {
            click(cx, "nav-daily");
            assert!(!snapshot(&view, cx).menu_open, "selecting a log must close the mobile menu");
            assert_eq!(snapshot(&view, cx).log, Log::Daily);
        }
        assert_eq!(cx.debug_bounds("design-sidebar").is_some(), width >= 1024.);
        assert_eq!(cx.debug_bounds("design-aside").is_some(), width >= 1180.);
        assert_reachable(cx, "entry-input", width, 900.);
        assert_reachable(cx, "add-entry", width, 900.);
        for selector in ["entry-input", "add-entry", "search-input", "collection-input"] {
            assert_eq!(cx.debug_bounds(selector).unwrap().size.height, px(40.), "control height: {selector}");
        }
        if width >= 1180. {
            let stat = cx.debug_bounds("design-stat").unwrap();
            assert!(stat.size.height >= px(56.), "stat number must retain display hierarchy");
            assert!(stat.left() >= px(0.) && stat.right() <= px(width));
        }
        add(cx, &format!("폭 {width}에서 한글 작성 🙂"));
        let id = snapshot(&view, cx).journal().entries.last().unwrap().id;
        for action in ["complete", "important", "edit", "cancel", "migrate"] {
            assert_reachable(cx, &format!("{action}-{id}"), width, 900.);
        }
        click(cx, format!("complete-{id}"));
        assert_eq!(snapshot(&view, cx).journal().entry(id).unwrap().status, Status::Complete);
        for log in ["nav-monthly", "nav-future"] {
            if width < 768. {
                click(cx, "nav-menu-toggle");
                assert!(snapshot(&view, cx).menu_open);
                assert_reachable(cx, log, width, 900.);
            }
            click(cx, log);
            assert!(!snapshot(&view, cx).menu_open);
            for selector in ["date-input", "entry-input", "add-entry", "previous-date", "next-date"] {
                assert_reachable(cx, selector, width, 900.);
            }
        }
        if width < 768. {
            click(cx, "nav-menu-toggle");
        }
        click(cx, "nav-index");
        assert!(!snapshot(&view, cx).menu_open);
        assert!(snapshot(&view, cx).index);
        type_in(cx, "search-input", &format!("폭 {width}"));
        assert_eq!(snapshot(&view, cx).visible_entries().len(), 1);
        assert_reachable(cx, &format!("jump-{id}"), width, 900.);
        click(cx, format!("jump-{id}"));
        assert_eq!(snapshot(&view, cx).log, Log::Daily);
        if width >= 1360. {
            type_in(cx, "entry-input", "넓은 창에서 작성 중 café🙂");
            let before_resize = snapshot(&view, cx).journal().clone();
            cx.simulate_resize(size(px(600.), px(400.)));
            render(cx);
            assert_chrome(cx, 600., 400.);
            assert_eq!(snapshot(&view, cx).journal(), &before_resize);
            assert_eq!(snapshot(&view, cx).entry_text, "넓은 창에서 작성 중 café🙂");
            assert_reachable(cx, "nav-menu-toggle", 600., 400.);
            click(cx, "nav-menu-toggle");
            assert!(snapshot(&view, cx).menu_open);
            click(cx, "nav-menu-toggle");
            assert!(!snapshot(&view, cx).menu_open);
            reveal_in_journal(cx, "entry-input", 600., 400.);
            reveal_in_journal(cx, "add-entry", 600., 400.);
            reveal_in_journal(cx, "create-collection", 600., 400.);
        }
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
    }
}

#[gpui::test]
fn design_ac02_focus_moves_between_production_inputs_without_geometry_shift(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    cx.simulate_resize(size(px(600.), px(900.)));
    render(cx);
    let resting = cx.debug_bounds("entry-input").unwrap();
    let search_resting = cx.debug_bounds("search-input").unwrap();
    assert_eq!(resting.size.height, px(40.));
    assert_eq!(search_resting.size.height, px(40.));
    click(cx, "entry-input");
    let input = snapshot(&view, cx).entry_input;
    assert!(cx.update(|window, app| input.read(app).focus_handle(app).is_focused(window)));
    assert_eq!(cx.debug_bounds("entry-input").unwrap(), resting);
    type_in(cx, "entry-input", "한글🙂");
    click(cx, "search-input");
    assert!(!cx.update(|window, app| input.read(app).focus_handle(app).is_focused(window)));
    let search = cx.read(|app| view.read(app).search_input.clone());
    assert!(cx.update(|window, app| search.read(app).focus_handle(app).is_focused(window)));
    assert_eq!(cx.debug_bounds("search-input").unwrap(), search_resting);
    assert_eq!(cx.debug_bounds("entry-input").unwrap(), resting);
    assert_eq!(snapshot(&view, cx).entry_text, "한글🙂");
}

#[gpui::test]
fn ac01_ac02_ac03_real_view_quick_capture_and_row_actions(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    render(cx);
    for selector in [
        "nav-daily",
        "nav-monthly",
        "nav-future",
        "nav-index",
        "entry-input",
        "empty-state",
        "journal-scroll",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "missing initial control {selector}");
    }
    assert!(snapshot(&view, cx).journal().entries.is_empty());
    add(cx, " ");
    assert!(snapshot(&view, cx).journal().entries.is_empty());
    assert!(snapshot(&view, cx).error.is_some());
    add(cx, "한글 할 일 📚");
    let task = snapshot(&view, cx).journal().entries[0].id;
    click(cx, "kind-event");
    add(cx, "친구와 점심 ○");
    click(cx, "kind-note");
    add(cx, "오늘의 생각 café");
    assert_eq!(
        snapshot(&view, cx).journal().entries.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [Kind::Task, Kind::Event, Kind::Note]
    );
    assert_eq!(snapshot(&view, cx).journal().entry(task).unwrap().text, "한글 할 일 📚");
    click(cx, format!("complete-{task}"));
    assert_eq!(snapshot(&view, cx).journal().entry(task).unwrap().symbol(), "×");
    click(cx, format!("complete-{task}"));
    assert!(snapshot(&view, cx).journal().entry(task).unwrap().is_open_task());
    click(cx, format!("important-{task}"));
    assert!(snapshot(&view, cx).journal().entry(task).unwrap().important);
    click(cx, format!("edit-{task}"));
    type_in(cx, "entry-input", "고친 할 일 📝");
    click(cx, "save-entry");
    assert_eq!(snapshot(&view, cx).journal().entry(task).unwrap().text, "고친 할 일 📝");
    click(cx, format!("cancel-{task}"));
    assert_eq!(snapshot(&view, cx).journal().entry(task).unwrap().status, Status::Cancelled);
    assert!(snapshot(&view, cx).error.is_none());
}

#[gpui::test]
fn ac04_real_view_date_calendar_month_future_navigation(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-12-31").unwrap(), cx));
    add(cx, "연말 일간");
    click(cx, "next-date");
    assert_eq!(snapshot(&view, cx).date, parse_date("2027-01-01").unwrap());
    assert!(snapshot(&view, cx).visible_entries().is_empty());
    click(cx, "previous-date");
    assert_eq!(snapshot(&view, cx).visible_entries()[0].text, "연말 일간");
    set_date(cx, "2023-02-29");
    assert!(snapshot(&view, cx).error.is_some());
    assert_eq!(snapshot(&view, cx).date, parse_date("2026-12-31").unwrap());
    set_date(cx, "2024-02-29");
    assert!(snapshot(&view, cx).error.is_none());
    click(cx, "nav-monthly");
    add(cx, "윤년 월간 작업");
    assert_eq!(snapshot(&view, cx).log, Log::Monthly);
    assert!(cx.debug_bounds("calendar-29").is_some());
    assert!(cx.debug_bounds("calendar-30").is_none());
    click(cx, "calendar-29");
    assert_eq!(snapshot(&view, cx).log, Log::Daily);
    assert_eq!(snapshot(&view, cx).date, parse_date("2024-02-29").unwrap());
    assert!(snapshot(&view, cx).visible_entries().is_empty());
    click(cx, "nav-future");
    set_date(cx, "2026-12-01");
    add(cx, "미래 계획");
    assert_eq!(snapshot(&view, cx).journal().entries.last().unwrap().log, Log::Future);
    click(cx, "next-date");
    assert_eq!(snapshot(&view, cx).date, parse_date("2027-01-01").unwrap());
    assert!(snapshot(&view, cx).visible_entries().is_empty());
}

#[gpui::test]
fn ac05_ac09_real_view_collection_index_global_search_filter_and_jump(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    add(cx, "책 읽기");
    let task = snapshot(&view, cx).journal().entries[0].id;
    click(cx, format!("complete-{task}"));
    type_in(cx, "collection-input", "독서 📚");
    click(cx, "create-collection");
    let collection = snapshot(&view, cx).journal().collections[0].id;
    assert_eq!(snapshot(&view, cx).log, Log::Collection(collection));
    click(cx, "kind-note");
    add(cx, "책 감상 한글");
    let note = snapshot(&view, cx).journal().entries[1].id;
    click(cx, "nav-index");
    assert!(snapshot(&view, cx).index);
    click(cx, format!("index-{task}"));
    assert_eq!(snapshot(&view, cx).log, Log::Daily);
    assert!(!snapshot(&view, cx).index);
    click(cx, "nav-index");
    click(cx, format!("index-collection-{collection}"));
    assert_eq!(snapshot(&view, cx).visible_entries()[0].id, note);
    type_in(cx, "search-input", "책");
    assert_eq!(snapshot(&view, cx).visible_entries().iter().map(|e| e.id).collect::<Vec<_>>(), [task, note]);
    click(cx, "status-filter");
    assert_eq!(snapshot(&view, cx).filter, Filter::Open);
    assert_eq!(snapshot(&view, cx).visible_entries()[0].id, note);
    click(cx, "status-filter");
    assert_eq!(snapshot(&view, cx).filter, Filter::Complete);
    assert_eq!(snapshot(&view, cx).visible_entries()[0].id, task);
    click(cx, format!("jump-{task}"));
    assert_eq!(snapshot(&view, cx).log, Log::Daily);
    assert_eq!(snapshot(&view, cx).date, parse_date("2026-10-02").unwrap());
    type_in(cx, "search-input", "없는 검색");
    assert!(snapshot(&view, cx).visible_entries().is_empty());
    assert!(cx.debug_bounds("search-empty").is_some());
}

#[gpui::test]
fn ac03_ac09_search_result_edit_opens_visible_composer_at_original_location(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    add(cx, "일간 기록");
    click(cx, "nav-monthly");
    set_date(cx, "2026-11-15");
    click(cx, "kind-note");
    add(cx, "월간 원본 메모");
    let id = snapshot(&view, cx).journal().entries[1].id;
    click(cx, "nav-daily");
    set_date(cx, "2026-10-02");
    type_in(cx, "search-input", "월간 원본");
    assert_eq!(snapshot(&view, cx).visible_entries()[0].id, id);
    click(cx, format!("edit-{id}"));
    assert_eq!(snapshot(&view, cx).date, parse_date("2026-11-15").unwrap());
    assert_eq!(snapshot(&view, cx).log, Log::Monthly);
    assert!(cx.debug_bounds("entry-input").is_some(), "search editing must expose the actual input");
    assert!(cx.debug_bounds("save-entry").is_some());
    type_in(cx, "entry-input", "검색에서 수정한 한글 메모 📝");
    cx.simulate_keystrokes("enter");
    render(cx);
    let state = snapshot(&view, cx);
    assert_eq!(state.journal().entries.len(), 2);
    let entry = state.journal().entry(id).unwrap();
    assert_eq!(entry.text, "검색에서 수정한 한글 메모 📝");
    assert_eq!(entry.kind, Kind::Note);
    assert_eq!(entry.log, Log::Monthly);
    assert_eq!(entry.date, parse_date("2026-11-15").unwrap());
    assert_eq!(stillnote::Session::open(path).unwrap().journal.entry(id).unwrap(), entry);
}

#[gpui::test]
fn ac06_ac07_real_view_migration_and_restart_retains_user_data(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    cx.update(bind_input_keys);
    let expected;
    {
        let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
        add(cx, "다음 달에 읽을 책 📚");
        let source = snapshot(&view, cx).journal().entries[0].id;
        click(cx, format!("important-{source}"));
        click(cx, format!("migrate-{source}"));
        type_in(cx, "target-input", "2026-11-01");
        click(cx, "target-future");
        click(cx, "confirm-migrate");
        let original = snapshot(&view, cx).journal().entry(source).unwrap().clone();
        assert_eq!(original.symbol(), "<");
        let target = original.migrated_to.unwrap();
        assert_eq!(snapshot(&view, cx).journal().entry(target).unwrap().migrated_from, Some(source));
        click(cx, format!("trace-{source}"));
        assert_eq!(snapshot(&view, cx).log, Log::Future);
        assert_eq!(snapshot(&view, cx).date, parse_date("2026-11-01").unwrap());
        assert_eq!(snapshot(&view, cx).visible_entries()[0].id, target);
        assert!(snapshot(&view, cx).visible_entries()[0].important);
        expected = snapshot(&view, cx).journal().clone();
        let before_close = cx.read(|app| app.windows().len());
        assert_eq!(before_close, 1);
        // Test windows have no native titlebar or should_close callback. Use
        // GPUI's window close operation and assert actual removal before restart.
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
        assert_eq!(cx.read(|app| app.windows().len()), 0);
    }
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-11-01").unwrap(), cx));
    render(cx);
    assert_eq!(snapshot(&view, cx).journal(), &expected);
    click(cx, "nav-future");
    assert_eq!(snapshot(&view, cx).visible_entries()[0].text, "다음 달에 읽을 책 📚");
    assert!(snapshot(&view, cx).error.is_none());
}

#[gpui::test]
fn ac08_real_view_corrupt_load_error_preserves_file_and_blocks_mutation(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let corrupt = b"{ broken personal journal";
    fs::write(&path, corrupt).unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    render(cx);
    assert!(snapshot(&view, cx).error.is_some());
    assert!(cx.debug_bounds("error-message").is_some());
    add(cx, "덮어쓰면 안 되는 기록");
    assert!(snapshot(&view, cx).journal().entries.is_empty());
    assert!(snapshot(&view, cx).error.as_ref().unwrap().contains("저장하지 못했습니다"));
    assert_eq!(fs::read(&path).unwrap(), corrupt);
}

#[gpui::test]
fn ac08_real_view_failed_save_error_retains_disk_model_and_input(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(path.clone(), parse_date("2026-10-02").unwrap(), cx));
    add(cx, "기존 기록");
    let expected = snapshot(&view, cx).journal().clone();
    let bytes = fs::read(&path).unwrap();
    fs::create_dir(path.with_extension("json.bak")).unwrap();
    add(cx, "저장 실패 유지");
    assert_eq!(snapshot(&view, cx).journal(), &expected);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(snapshot(&view, cx).error.as_ref().unwrap().contains("저장하지 못했습니다"));
    assert!(cx.debug_bounds("error-message").is_some());
    assert_eq!(snapshot(&view, cx).entry_text.as_str(), "저장 실패 유지");
}

#[gpui::test]
fn ac10_real_input_keyboard_clipboard_composition_utf16_and_resize(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    let (view, cx) = cx.add_window_view(|_, cx| JournalView::new(dir.path().join("journal.json"), parse_date("2026-10-02").unwrap(), cx));
    type_in(cx, "entry-input", "한글🙂");
    cx.simulate_keystrokes("end backspace");
    assert_eq!(snapshot(&view, cx).entry_text.as_str(), "한글");
    cx.update(|_, app| app.write_to_clipboard(ClipboardItem::new_string(" 붙여넣기 📚".into())));
    cx.simulate_keystrokes("ctrl-v enter");
    render(cx);
    assert_eq!(snapshot(&view, cx).journal().entries[0].text, "한글 붙여넣기 📚");
    // Exercise the production input entity's platform IME contract. Relative
    // composition selections must map within the newly composed substring.
    let input = snapshot(&view, cx).entry_input.clone();
    for (prefix, composing, expected_text, expected_caret) in [("한", "a", "한a", 2), ("a", "한", "a한", 2), ("🙂", "한", "🙂한", 3)] {
        type_in(cx, "entry-input", prefix);
        cx.update(|window, app| {
            input.update(app, |input, cx| {
                input.replace_and_mark_text_in_range(None, composing, Some(1..1), window, cx);
                assert_eq!(input.selected_text_range(false, window, cx).unwrap().range, expected_caret..expected_caret);
                assert_eq!(input.content.as_ref(), expected_text);
            })
        });
        render(cx);
        cx.simulate_keystrokes("enter");
        assert_eq!(
            snapshot(&view, cx).journal().entries.len(),
            1,
            "Enter during composition must not prematurely submit"
        );
        cx.update(|window, app| input.update(app, |input, cx| input.unmark_text(window, cx)));
        cx.simulate_keystrokes("end backspace");
        assert_eq!(snapshot(&view, cx).entry_text.as_str(), prefix);
    }
    cx.simulate_resize(size(px(900.), px(680.)));
    render(cx);
    for selector in ["nav-daily", "entry-input", "journal-scroll", "add-entry"] {
        let bounds = cx.debug_bounds(selector).unwrap();
        assert!(
            bounds.left() >= px(0.) && bounds.right() <= px(900.),
            "control off-screen: {selector} {bounds:?}"
        );
    }
    add(cx, "축소한 창에서 기록");
    assert_eq!(snapshot(&view, cx).journal().entries.last().unwrap().text, "축소한 창에서 기록");
    for index in 1..=12 {
        add(cx, &format!("스크롤 기록 {index}"));
    }
    let last = snapshot(&view, cx).journal().entries.last().unwrap().id;
    let last_selector: &'static str = Box::leak(format!("complete-{last}").into_boxed_str());
    let area = cx.debug_bounds("journal-scroll").unwrap();
    let before = cx.debug_bounds(last_selector).unwrap();
    assert!(before.bottom() > area.bottom(), "fixture must overflow its viewport");
    cx.simulate_mouse_move(area.center(), None, Modifiers::none());
    cx.simulate_event(ScrollWheelEvent {
        position: area.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-5000.))),
        ..Default::default()
    });
    render(cx);
    let after = cx.debug_bounds(last_selector).unwrap();
    assert!(after.top() < before.top(), "scroll did not move records");
    assert!(
        after.top() >= area.top() && after.bottom() <= area.bottom(),
        "last record remains inaccessible after scroll: {after:?} within {area:?}"
    );
    click(cx, last_selector);
    assert_eq!(snapshot(&view, cx).journal().entry(last).unwrap().status, Status::Complete);
}
