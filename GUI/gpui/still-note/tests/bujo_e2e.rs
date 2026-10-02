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
        let _view = app.new(|cx| JournalView::new(path, parse_date("2026-10-02").unwrap(), cx));
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
        callback_observed.set(true);
        app.quit();
    });
    assert!(observed.get(), "native registration assertions must execute");
}

#[gpui::test]
fn design_ac02_ac04_all_breakpoints_keep_controls_and_actions_reachable(cx: &mut TestAppContext) {
    let dir = tempdir().unwrap();
    cx.update(bind_input_keys);
    for width in [600., 767., 768., 1024., 1360., 1600.] {
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
        assert_eq!(nav.top(), px(0.), "top navigation must be pinned to the top");
        assert_eq!(nav.size.height, px(64.));
        assert_eq!(nav.size.width, px(width));
        let content = cx.debug_bounds("design-container").unwrap();
        assert!(content.size.width <= px(1280.));
        assert!(
            (content.left() - (px(width) - content.right())).abs() <= px(1.),
            "content not centered: {content:?}"
        );
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
