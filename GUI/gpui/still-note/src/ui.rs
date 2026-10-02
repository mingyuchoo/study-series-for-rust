use crate::{
    i18n::{Message, error_text},
    settings::{Language, Settings, SettingsStore, ThemeMode},
};
use crate::{
    input::{Submitted, TextInput},
    *,
};
use chrono::{Datelike, Days, Local, NaiveDate};
use gpui::{prelude::*, *};
use std::path::PathBuf;
use uuid::Uuid;

use crate::theme::*;

/// The production window policy, also used by isolated native render checks.
pub fn journal_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT))),
        titlebar: Some(TitlebarOptions {
            title: Some("Stillnote · 나의 불렛저널".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn window_control(id: &'static str, glyph: &'static str, area: WindowControlArea, palette: Palette) -> impl IntoElement {
    let glyph_selector = if id == "window-maximize" && glyph == "❐" {
        "window-restore-glyph"
    } else if id == "window-maximize" {
        "window-maximize-glyph"
    } else {
        id
    };
    div()
        .id(id)
        .debug_selector(move || id.into())
        .w(px(CONTROL_HEIGHT))
        .h(px(CONTROL_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(rgb(palette.card))
        .text_color(rgb(palette.ink))
        .text_size(px(16.))
        .line_height(px(16.))
        .font_weight(FontWeight(CONTROL_WEIGHT))
        .whitespace_nowrap()
        // Keep the root focus listener from cancelling Windows non-client clicks.
        .occlude()
        .window_control_area(area)
        .child(
            div()
                .when(id == "window-maximize", |d| d.debug_selector(move || glyph_selector.into()))
                .child(glyph),
        )
}

#[derive(Clone)]
enum Command {
    Nav(Log),
    Index,
    Kind(Kind),
    Add,
    Date,
    Day(i32),
    Month(i32),
    Today,
    Filter,
    Collection,
    Complete(Uuid),
    Cancel(Uuid),
    Important(Uuid),
    Edit(Uuid),
    Migrate(Uuid),
    Target(Log),
    ConfirmMigration,
    StopEditing,
    Jump(Uuid),
    ToggleMenu,
    Language(Language),
    Theme(ThemeMode),
}

pub struct JournalView {
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub effective_theme: ThemeMode,
    pub palette: Palette,
    pub window_title: String,
    pub setting_focus: [FocusHandle; 5],
    pub settings_error: Option<String>,
    settings_store: SettingsStore,
    settings_error_source: Option<anyhow::Error>,
    error_source: Option<anyhow::Error>,
    load_error_source: Option<anyhow::Error>,
    appearance: WindowAppearance,
    appearance_observed: bool,
    root_focus: FocusHandle,
    pub session: Option<Session>,
    pub date: NaiveDate,
    pub log: Log,
    pub error: Option<String>,
    pub load_error: Option<String>,
    pub filter: Filter,
    pub kind: Kind,
    pub index: bool,
    pub menu_open: bool,
    pub entry_input: Entity<TextInput>,
    pub search_input: Entity<TextInput>,
    pub date_input: Entity<TextInput>,
    pub collection_input: Entity<TextInput>,
    pub target_input: Entity<TextInput>,
    pub editing: Option<Uuid>,
    pub migrating: Option<Uuid>,
    pub target_log: Log,
    empty: Journal,
    notice: String,
    _subscriptions: Vec<Subscription>,
}
impl JournalView {
    pub fn new(path: PathBuf, date: NaiveDate, cx: &mut Context<Self>) -> Self {
        register_fonts(cx);
        let mut settings_store = SettingsStore::new(&path);
        let (settings, settings_error_source) = match settings_store.load() {
            | Ok(settings) => (settings, None),
            | Err(error) => (Settings::default(), Some(error)),
        };
        let language = settings.language;
        let settings_path = settings_store.path.clone();
        let settings_error = settings_error_source.as_ref().map(|e| error_text(e, language));
        let (session, error) = match Session::open(path.clone()) {
            | Ok(s) => (Some(s), None),
            | Err(e) => (None, Some(e)),
        };
        let entry_input = cx.new(|cx| TextInput::new(language.text("머릿속의 생각을 한 줄로 기록해 보세요"), cx));
        let search_input = cx.new(|cx| TextInput::new(language.text("전체 기록 검색…"), cx));
        let date_input = cx.new(|cx| {
            let mut input = TextInput::new("YYYY-MM-DD", cx);
            input.set_text(&date.to_string(), cx);
            input
        });
        let collection_input = cx.new(|cx| TextInput::new(language.text("새 컬렉션 이름"), cx));
        let target_input = cx.new(|cx| TextInput::new(language.text("대상 날짜 YYYY-MM-DD"), cx));
        let subscriptions = vec![
            cx.subscribe(&entry_input, |this, _, _: &Submitted, cx| {
                this.commit_entry(cx);
            }),
            cx.subscribe(&date_input, |this, _, _: &Submitted, cx| {
                this.change_date(cx);
            }),
            cx.subscribe(&collection_input, |this, _, _: &Submitted, cx| {
                this.create_collection(cx);
            }),
            cx.subscribe(&target_input, |this, _, _: &Submitted, cx| {
                this.confirm_migration(cx);
            }),
            cx.observe(&search_input, |_, _, cx| cx.notify()),
        ];
        let load_error = error.as_ref().map(|e| error_text(e, language));
        Self {
            settings,
            settings_path,
            settings_store,
            settings_error,
            settings_error_source,
            effective_theme: ThemeMode::Dark,
            palette: Palette::DARK,
            window_title: language.text("Stillnote · 나의 불렛저널").to_owned(),
            setting_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            appearance: WindowAppearance::Dark,
            appearance_observed: false,
            root_focus: cx.focus_handle().tab_stop(false),
            error_source: None,
            load_error_source: error,
            session,
            date,
            log: Log::Daily,
            error: load_error.clone(),
            load_error,
            filter: Filter::All,
            kind: Kind::Task,
            index: false,
            menu_open: false,
            entry_input,
            search_input,
            date_input,
            collection_input,
            target_input,
            editing: None,
            migrating: None,
            target_log: Log::Daily,
            empty: Journal::default(),
            notice: "기록은 이 컴퓨터에만 저장됩니다".into(),
            _subscriptions: subscriptions,
        }
    }
    fn tr<'a>(&self, key: &'a str) -> &'a str {
        self.settings.language.text(key)
    }
    fn present_errors(&mut self) {
        self.error = self
            .error_source
            .as_ref()
            .or(self.load_error_source.as_ref())
            .map(|e| error_text(e, self.settings.language));
        self.load_error = self.load_error_source.as_ref().map(|e| error_text(e, self.settings.language));
        self.settings_error = self.settings_error_source.as_ref().map(|e| error_text(e, self.settings.language));
    }
    fn set_error(&mut self, error: anyhow::Error) {
        self.error_source = Some(error);
        self.present_errors();
    }
    fn save_settings(&mut self) {
        self.settings_error_source = self
            .settings_store
            .save(&self.settings)
            .map_err(|e| Message::new("설정을 저장하지 못했습니다. 선택은 현재 세션에 적용됩니다").wrap(e))
            .err();
        self.present_errors();
    }
    pub fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.settings.language = language;
        for (input, key) in [
            (&self.entry_input, "머릿속의 생각을 한 줄로 기록해 보세요"),
            (&self.search_input, "전체 기록 검색…"),
            (&self.collection_input, "새 컬렉션 이름"),
            (&self.target_input, "대상 날짜 YYYY-MM-DD"),
        ] {
            input.update(cx, |input, cx| {
                input.placeholder = language.text(key).to_owned().into();
                cx.notify();
            });
        }
        self.window_title = language.text("Stillnote · 나의 불렛저널").to_owned();
        self.save_settings();
        cx.notify();
    }
    pub fn set_theme(&mut self, theme: ThemeMode, cx: &mut Context<Self>) {
        self.settings.theme = theme;
        self.apply_palette(cx);
        self.save_settings();
        cx.notify();
    }
    pub fn appearance_changed(&mut self, appearance: WindowAppearance, cx: &mut Context<Self>) {
        self.appearance = appearance;
        self.apply_palette(cx);
        cx.notify();
    }
    fn apply_palette(&mut self, cx: &mut Context<Self>) {
        self.effective_theme = self.settings.theme.resolve(self.appearance);
        self.palette = Palette::for_theme(self.effective_theme);
        for input in [
            &self.entry_input,
            &self.search_input,
            &self.date_input,
            &self.collection_input,
            &self.target_input,
        ] {
            input.update(cx, |input, cx| {
                input.palette = self.palette;
                cx.notify();
            });
        }
    }
    pub fn journal(&self) -> &Journal {
        self.session.as_ref().map(|s| &s.journal).unwrap_or(&self.empty)
    }
    fn transact<T>(&mut self, op: impl FnOnce(&mut Journal) -> anyhow::Result<T>, cx: &mut Context<Self>) -> Option<T> {
        let result = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!(Message::new("원본 파일 보호를 위해 읽기 전용입니다. README의 복구 안내를 확인해 주세요")))
            .and_then(|s| s.transact(op));
        match result {
            | Ok(v) => {
                self.error = None;
                self.error_source = None;
                self.notice = format!("로컬 저장 완료 · {}", Local::now().format("%H:%M"));
                cx.notify();
                Some(v)
            },
            | Err(e) => {
                self.set_error(Message::new("저장하지 못했습니다").wrap(e));
                cx.notify();
                None
            },
        }
    }
    fn set_date(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        self.date = date;
        self.date_input.update(cx, |i, cx| i.set_text(&date.to_string(), cx));
        cx.notify();
    }
    fn change_date(&mut self, cx: &mut Context<Self>) {
        match parse_date(&self.date_input.read(cx).content) {
            | Ok(date) => {
                self.error = None;
                self.error_source = None;
                self.set_date(date, cx);
            },
            | Err(_) => {
                self.set_error(Message::new("올바른 날짜를 입력해 주세요 (YYYY-MM-DD)").into());
                cx.notify();
            },
        }
    }
    fn commit_entry(&mut self, cx: &mut Context<Self>) {
        let text = self.entry_input.read(cx).content.to_string();
        let date = self.date;
        let log = self.log.clone();
        let kind = self.kind;
        let editing = self.editing;
        if self
            .transact(
                |j| {
                    if let Some(id) = editing {
                        j.edit_entry(id, &text)
                    } else {
                        j.add_entry(date, log, kind, &text).map(|_| ())
                    }
                },
                cx,
            )
            .is_some()
        {
            self.entry_input.update(cx, |i, cx| {
                i.reset();
                cx.notify();
            });
            self.editing = None;
        }
    }
    fn create_collection(&mut self, cx: &mut Context<Self>) {
        let name = self.collection_input.read(cx).content.to_string();
        if let Some(id) = self.transact(|j| j.add_collection(&name), cx) {
            self.log = Log::Collection(id);
            self.index = false;
            self.collection_input.update(cx, |i, cx| {
                i.reset();
                cx.notify();
            });
        }
    }
    fn confirm_migration(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.migrating else {
            return;
        };
        let raw = self.target_input.read(cx).content.to_string();
        let date = match parse_date(&raw) {
            | Ok(d) => d,
            | Err(_) => {
                self.set_error(Message::new("이월 대상 날짜를 확인해 주세요").into());
                cx.notify();
                return;
            },
        };
        let log = self.target_log.clone();
        if self.transact(|j| j.migrate(id, date, log), cx).is_some() {
            self.migrating = None;
        }
    }
    fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            | Command::ToggleMenu => self.menu_open = !self.menu_open,
            | Command::Language(language) => self.set_language(language, cx),
            | Command::Theme(theme) => self.set_theme(theme, cx),
            | Command::Nav(log) => {
                self.menu_open = false;
                self.log = log;
                self.index = false;
                self.editing = None;
                self.search_input.update(cx, |i, cx| {
                    i.reset();
                    cx.notify();
                });
            },
            | Command::Index => {
                self.menu_open = false;
                self.index = true;
                self.search_input.update(cx, |i, cx| {
                    i.reset();
                    cx.notify();
                });
            },
            | Command::Kind(kind) => self.kind = kind,
            | Command::Add => self.commit_entry(cx),
            | Command::Date => self.change_date(cx),
            | Command::Day(delta) => {
                let next = if delta >= 0 {
                    self.date.checked_add_days(Days::new(delta as u64))
                } else {
                    self.date.checked_sub_days(Days::new((-delta) as u64))
                };
                if let Some(date) = next.filter(|d| (1..=9999).contains(&d.year())) {
                    self.set_date(date, cx)
                }
            },
            | Command::Month(delta) => match shift_month(self.date, delta) {
                | Ok(date) => self.set_date(date, cx),
                | Err(e) => self.set_error(e),
            },
            | Command::Today => self.set_date(Local::now().date_naive(), cx),
            | Command::Filter => {
                self.filter = match self.filter {
                    | Filter::All => Filter::Open,
                    | Filter::Open => Filter::Complete,
                    | Filter::Complete => Filter::All,
                }
            },
            | Command::Collection => self.create_collection(cx),
            | Command::Complete(id) => {
                let status = self
                    .journal()
                    .entry(id)
                    .map(|e| if e.status == Status::Complete { Status::Open } else { Status::Complete })
                    .unwrap_or(Status::Open);
                self.transact(|j| j.set_status(id, status), cx);
            },
            | Command::Cancel(id) => {
                self.transact(|j| j.set_status(id, Status::Cancelled), cx);
            },
            | Command::Important(id) => {
                self.transact(|j| j.toggle_important(id), cx);
            },
            | Command::Edit(id) => {
                if let Ok(e) = self.journal().entry(id) {
                    let text = e.text.clone();
                    let date = e.date;
                    let log = e.log.clone();
                    self.log = log;
                    self.index = false;
                    self.set_date(date, cx);
                    self.search_input.update(cx, |i, cx| {
                        i.reset();
                        cx.notify();
                    });
                    self.entry_input.update(cx, |i, cx| i.set_text(&text, cx));
                    self.editing = Some(id);
                }
            },
            | Command::Migrate(id) => {
                self.migrating = Some(id);
                self.target_log = Log::Daily;
                let date = self.date.checked_add_days(Days::new(1)).unwrap_or(self.date);
                self.target_input.update(cx, |i, cx| i.set_text(&date.to_string(), cx));
            },
            | Command::Target(log) => self.target_log = log,
            | Command::ConfirmMigration => self.confirm_migration(cx),
            | Command::StopEditing => {
                self.editing = None;
                self.migrating = None;
                self.entry_input.update(cx, |i, cx| {
                    i.reset();
                    cx.notify();
                });
            },
            | Command::Jump(id) => {
                if let Ok(e) = self.journal().entry(id) {
                    let date = e.date;
                    let log = e.log.clone();
                    self.log = log;
                    self.index = false;
                    self.set_date(date, cx);
                    self.search_input.update(cx, |i, cx| {
                        i.reset();
                        cx.notify();
                    });
                }
            },
        }
        cx.notify();
    }
    fn button<I: Into<String>, L: Into<String>>(
        &self,
        id: I,
        label: L,
        command: Command,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<I, L> {
        let id = id.into();
        let setting = id.starts_with("language-") || id.starts_with("theme-");
        let titlebar_control = setting || id.starts_with("nav-");
        let focus = match id.as_str() {
            | "language-ko" => Some(self.setting_focus[0].clone()),
            | "language-en" => Some(self.setting_focus[1].clone()),
            | "theme-system" => Some(self.setting_focus[2].clone()),
            | "theme-light" => Some(self.setting_focus[3].clone()),
            | "theme-dark" => Some(self.setting_focus[4].clone()),
            | _ => None,
        };
        let keyboard_command = command.clone();
        let palette = self.palette;
        let segmented = setting || id.starts_with("kind-") || id.starts_with("target-");
        let tab = segmented || id.starts_with("nav-") || id.starts_with("collection-");
        let primary = matches!(id.as_str(), "add-entry" | "save-entry" | "confirm-migrate" | "create-collection");
        let wrapping = id.starts_with("collection-") || id.starts_with("index-") || id.starts_with("trace-") || id.starts_with("source-");
        let label_id = format!("label-{id}");
        let label = label.into();
        let label = if segmented && active { format!("✓ {label}") } else { label };
        div()
            .id(SharedString::from(id.clone()))
            .debug_selector(move || id)
            .px(px(if tab { 14. } else { 20. }))
            .min_w(px(CONTROL_HEIGHT))
            .max_w_full()
            .when(wrapping, |d| d.w_full().min_h(px(CONTROL_HEIGHT)).py_2())
            .when(!wrapping, |d| d.h(px(CONTROL_HEIGHT)))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(if tab && !active {
                rgba(0x00000000)
            } else {
                rgb(if primary { self.palette.primary } else { self.palette.card })
            })
            .cursor_pointer()
            // Interactive titlebar controls override the surrounding caption hitbox.
            .when(titlebar_control, |d| d.occlude())
            .tab_index(0)
            .when_some(focus, |d, focus| d.track_focus(&focus))
            .focus(move |s| s.border_color(rgb(palette.primary)).bg(rgb(palette.elevated)).text_color(rgb(palette.ink)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.command(keyboard_command.clone(), cx);
                    cx.stop_propagation();
                }
            }))
            .text_size(px(14.))
            .line_height(px(if tab { 19.6 } else { 14. }))
            .font_weight(FontWeight(if tab { NAV_WEIGHT } else { CONTROL_WEIGHT }))
            .text_color(rgb(if primary {
                self.palette.on_primary
            } else if tab && !active {
                self.palette.muted
            } else {
                self.palette.ink
            }))
            .bg(if tab && !active {
                rgba(0x00000000)
            } else {
                rgb(if primary { self.palette.primary } else { self.palette.card })
            })
            .when(primary, |d| {
                d.active(|s| s.bg(rgb(self.palette.primary_active)).border_color(rgb(self.palette.primary_active)))
            })
            .child(
                div()
                    .id(SharedString::from(label_id.clone()))
                    .debug_selector(move || label_id)
                    .when(wrapping, |d| d.min_w(px(0.)).max_w_full().w_full().whitespace_normal())
                    .when(!wrapping, |d| d.flex_shrink_0().whitespace_nowrap())
                    .child(label),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.command(command.clone(), cx)))
    }
    fn settings_controls(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("settings-controls")
            .debug_selector(|| "settings-controls".into())
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                div()
                    .id("language-controls")
                    .flex()
                    .gap_1()
                    .rounded(px(CONTROL_RADIUS))
                    .bg(rgb(self.palette.soft))
                    .child(self.button(
                        "language-ko",
                        "한국어",
                        Command::Language(Language::Korean),
                        self.settings.language == Language::Korean,
                        cx,
                    ))
                    .child(self.button(
                        "language-en",
                        "English",
                        Command::Language(Language::English),
                        self.settings.language == Language::English,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("theme-controls")
                    .flex()
                    .gap_1()
                    .rounded(px(CONTROL_RADIUS))
                    .bg(rgb(self.palette.soft))
                    .child(self.button(
                        "theme-system",
                        self.tr("시스템").to_owned(),
                        Command::Theme(ThemeMode::System),
                        self.settings.theme == ThemeMode::System,
                        cx,
                    ))
                    .child(self.button(
                        "theme-light",
                        self.tr("라이트").to_owned(),
                        Command::Theme(ThemeMode::Light),
                        self.settings.theme == ThemeMode::Light,
                        cx,
                    ))
                    .child(self.button(
                        "theme-dark",
                        self.tr("다크").to_owned(),
                        Command::Theme(ThemeMode::Dark),
                        self.settings.theme == ThemeMode::Dark,
                        cx,
                    )),
            )
    }
    fn input(&self, id: &'static str, input: &Entity<TextInput>) -> impl IntoElement + use<> {
        div()
            .id(id)
            .debug_selector(move || id.into())
            .flex_1()
            .min_w(px(80.))
            .min_h(px(CONTROL_HEIGHT))
            .child(input.clone())
    }
    pub fn visible_entries(&self, cx: &App) -> Vec<&Entry> {
        let search = &self.search_input.read(cx).content;
        if !search.trim().is_empty() {
            self.journal().search(search, self.filter)
        } else {
            self.journal()
                .visible(self.date, &self.log)
                .into_iter()
                .filter(|e| match self.filter {
                    | Filter::All => true,
                    | Filter::Open => e.status == Status::Open,
                    | Filter::Complete => e.status == Status::Complete,
                })
                .collect()
        }
    }
    fn title(&self) -> String {
        if self.index {
            return self.tr("인덱스").to_owned();
        }
        match self.log {
            | Log::Daily => self.tr("일간 로그").to_owned(),
            | Log::Monthly => self.tr("월간 로그").to_owned(),
            | Log::Future => self.tr("미래 로그").to_owned(),
            | Log::Collection(id) => self
                .journal()
                .collections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.clone())
                .unwrap_or(self.tr("컬렉션").to_owned()),
        }
    }
    fn location(&self, e: &Entry) -> String {
        format!(
            "{} · {}",
            e.date,
            match &e.log {
                | Log::Daily => self.tr("일간").to_owned(),
                | Log::Monthly => self.tr("월간").to_owned(),
                | Log::Future => self.tr("미래").to_owned(),
                | Log::Collection(id) => self
                    .journal()
                    .collections
                    .iter()
                    .find(|c| c.id == *id)
                    .map(|c| c.name.clone())
                    .unwrap_or(self.tr("컬렉션").to_owned()),
            }
        )
    }
    fn row(&self, e: &Entry, search: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = e.id;
        let frozen = matches!(e.status, Status::Migrated | Status::Scheduled);
        let mut actions = div().flex().flex_wrap().gap_2();
        if !frozen {
            if e.kind == Kind::Task {
                actions = actions.child(self.button(
                    format!("complete-{id}"),
                    if e.status == Status::Complete {
                        self.tr("재개").to_owned()
                    } else {
                        self.tr("완료").to_owned()
                    },
                    Command::Complete(id),
                    false,
                    cx,
                ));
            }
            actions = actions
                .child(self.button(
                    format!("important-{id}"),
                    if e.important { "★" } else { "☆" },
                    Command::Important(id),
                    false,
                    cx,
                ))
                .child(self.button(format!("edit-{id}"), self.tr("수정").to_owned(), Command::Edit(id), false, cx))
                .child(self.button(format!("cancel-{id}"), self.tr("취소").to_owned(), Command::Cancel(id), false, cx));
            if e.is_open_task() {
                actions = actions.child(self.button(format!("migrate-{id}"), self.tr("이월 →").to_owned(), Command::Migrate(id), false, cx));
            }
        }
        if search {
            actions = actions.child(self.button(format!("jump-{id}"), self.tr("로그 열기").to_owned(), Command::Jump(id), false, cx));
        }
        let mut body = div().flex_1().min_w(px(0.)).flex().flex_col().gap_1().child(
            div()
                .id(SharedString::from(format!("text-{id}")))
                .debug_selector(move || format!("text-{id}"))
                .w_full()
                .min_w(px(0.))
                .whitespace_normal()
                .text_size(px(16.))
                .text_color(rgb(if e.status == Status::Complete || e.status == Status::Cancelled {
                    self.palette.muted
                } else {
                    self.palette.body
                }))
                .child(format!("{}{}", if e.important { "★  " } else { "" }, e.text)),
        );
        if search {
            body = body.child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.2))
                    .text_color(rgb(self.palette.muted))
                    .child(self.location(e)),
            );
        }
        if let Some(target) = e.migrated_to.and_then(|id| self.journal().entry(id).ok()) {
            body = body.child(self.button(
                format!("trace-{id}"),
                format!("{} · {}  ↗", self.tr("이월됨"), self.location(target)),
                Command::Jump(target.id),
                false,
                cx,
            ));
        }
        if let Some(source) = e.migrated_from.and_then(|id| self.journal().entry(id).ok()) {
            body = body.child(self.button(
                format!("source-{id}"),
                format!("{} · {}  ↗", self.tr("원본"), self.location(source)),
                Command::Jump(source.id),
                false,
                cx,
            ));
        }
        div()
            .id(SharedString::from(format!("row-{id}")))
            .debug_selector(move || format!("row-{id}"))
            .flex_shrink_0()
            .flex()
            .gap_3()
            .py_4()
            .border_b_1()
            .border_color(rgb(self.palette.hairline))
            .flex_col()
            .rounded(px(CARD_RADIUS))
            .border_1()
            .border_color(rgb(self.palette.hairline_soft))
            .bg(rgb(self.palette.card))
            .p_6()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .w(px(24.))
                            .flex_shrink_0()
                            .text_size(px(24.))
                            .text_color(rgb(self.palette.ink))
                            .child(e.symbol()),
                    )
                    .child(body),
            )
            .child(actions)
    }
}
impl Render for JournalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.appearance_observed {
            // A fresh window needs a dispatch target before its first Tab press.
            // Focus only once; subsequent renders preserve the user's input focus.
            if window.focused(cx).is_none() {
                self.root_focus.focus(window);
            }
            self.appearance_observed = true;
            self.appearance_changed(window.appearance(), cx);
            self._subscriptions
                .push(cx.observe_window_appearance(window, |this, window, cx| this.appearance_changed(window.appearance(), cx)));
        }
        window.set_window_title(&self.window_title);
        let search = self.search_input.read(cx).content.to_string();
        let searching = !search.trim().is_empty();
        let records: Vec<Entry> = self.visible_entries(cx).into_iter().cloned().collect();
        let open = records.iter().filter(|e| e.is_open_task()).count();
        let width = window.viewport_size().width;
        let compact = width < px(COMPACT_BREAKPOINT);
        let settings_in_menu = width < px(1360.);
        let settings_controls = self.settings_controls(cx);
        let short = window.viewport_size().height < px(650.);
        let show_sidebar = width >= px(SIDEBAR_BREAKPOINT) && !short;
        let mut nav_links = div()
            .id(if compact { "design-mobile-menu" } else { "design-nav-links" })
            .debug_selector(move || if compact { "design-mobile-menu".into() } else { "design-nav-links".into() })
            .flex()
            .flex_shrink_0()
            .flex_wrap()
            .items_center()
            .gap_2()
            .bg(rgb(self.palette.canvas));
        for (id, label, short, log) in [
            ("nav-daily", self.tr("일간 로그").to_owned(), self.tr("일간").to_owned(), Log::Daily),
            ("nav-monthly", self.tr("월간 로그").to_owned(), self.tr("월간").to_owned(), Log::Monthly),
            ("nav-future", self.tr("미래 로그").to_owned(), self.tr("미래").to_owned(), Log::Future),
        ] {
            nav_links = nav_links.child(self.button(
                id,
                if width < px(SIDEBAR_BREAKPOINT) { short } else { label },
                Command::Nav(log.clone()),
                !self.index && self.log == log,
                cx,
            ));
        }
        nav_links = nav_links.child(self.button("nav-index", self.tr("인덱스").to_owned(), Command::Index, self.index, cx));
        let nav_content = div()
            .id("design-nav-content")
            .debug_selector(|| "design-nav-content".into())
            .w_full()
            .h_full()
            .px_6()
            .flex()
            .items_center()
            .gap_2()
            .when(!settings_in_menu, |d| d.gap_6())
            .child(
                div()
                    .id("nav-wordmark")
                    .debug_selector(|| "nav-wordmark".into())
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .font_weight(FontWeight(HEADING_WEIGHT))
                    .text_size(px(24.))
                    .child("stillnote"),
            )
            .when(settings_in_menu, |d| {
                d.justify_between().child(self.button(
                    "nav-menu-toggle",
                    if compact {
                        if self.menu_open {
                            self.tr("✕ 메뉴 닫기").to_owned()
                        } else {
                            self.tr("☰ 메뉴").to_owned()
                        }
                    } else {
                        "⚙".to_owned()
                    },
                    Command::ToggleMenu,
                    false,
                    cx,
                ))
            });
        let (nav_content, mobile_menu) = if compact {
            (nav_content, if self.menu_open { Some(nav_links.child(settings_controls)) } else { None })
        } else if settings_in_menu {
            (
                nav_content.child(nav_links),
                if self.menu_open {
                    Some(div().id("settings-menu").flex().flex_wrap().child(settings_controls))
                } else {
                    None
                },
            )
        } else {
            (nav_content.child(nav_links).child(div().flex_1().min_w(px(0.))).child(settings_controls), None)
        };
        // Keep a flexible gap before the native controls; the whole navigation
        // background is the caption, with interactive controls occluding it.
        let nav_content = nav_content
            .child(
                div()
                    .id("window-drag-region")
                    .debug_selector(|| "window-drag-region".into())
                    .flex_1()
                    .min_w(px(0.))
                    .h_full(),
            )
            .child(
                div()
                    .id("window-controls")
                    .debug_selector(|| "window-controls".into())
                    .flex()
                    .flex_shrink_0()
                    .gap_2()
                    .child(window_control("window-minimize", "−", WindowControlArea::Min, self.palette))
                    .child(window_control(
                        "window-maximize",
                        if window.is_maximized() { "❐" } else { "□" },
                        WindowControlArea::Max,
                        self.palette,
                    ))
                    .child(window_control("window-close", "×", WindowControlArea::Close, self.palette)),
            );
        let nav = div()
            .id("design-nav")
            .debug_selector(|| "design-nav".into())
            .w_full()
            .h(px(NAV_HEIGHT))
            .flex_shrink_0()
            .bg(rgb(self.palette.canvas))
            // Caption clicks must reach Windows without the root focus listener.
            .occlude()
            .window_control_area(WindowControlArea::Drag)
            .child(nav_content);
        let mut collections = div().id("collections-scroll").flex().gap_2();
        if show_sidebar {
            collections = collections.flex_col().flex_1().min_h(px(44.)).overflow_y_scroll();
        } else {
            collections = collections.flex_wrap();
        }
        for c in self.journal().collections.clone() {
            collections = collections.child(self.button(
                format!("collection-{}", c.id),
                format!("# {}", c.name),
                Command::Nav(Log::Collection(c.id)),
                self.log == Log::Collection(c.id) && !self.index,
                cx,
            ));
        }
        let collection_form = div()
            .flex_shrink_0()
            .flex()
            .when(show_sidebar, |d| d.flex_col())
            .gap_2()
            .child(self.input("collection-input", &self.collection_input))
            .child(self.button(
                "create-collection",
                if compact {
                    self.tr("+ 만들기").to_owned()
                } else {
                    self.tr("+ 컬렉션 만들기").to_owned()
                },
                Command::Collection,
                false,
                cx,
            ));
        let collection_panel = div()
            .flex_shrink_0()
            .id(if show_sidebar { "design-sidebar" } else { "compact-collections" })
            .debug_selector(move || {
                if show_sidebar {
                    "design-sidebar".into()
                } else {
                    "compact-collections".into()
                }
            })
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .rounded(px(CARD_RADIUS))
            .bg(rgb(self.palette.card))
            .when(show_sidebar, |d| d.w(px(208.)).flex_shrink_0().h_full())
            .child(div().text_size(px(14.)).text_color(rgb(self.palette.muted)).child(self.tr("컬렉션").to_owned()))
            .child(collections)
            .child(collection_form);
        let mut main = div().w_full().min_w(px(0.)).flex().flex_col().gap_4().child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(self.palette.muted))
                        .child(self.tr("나의 기록").to_owned()),
                )
                .child(
                    div()
                        .w(px(if compact { 240. } else { 280. }))
                        .child(self.input("search-input", &self.search_input)),
                ),
        );
        main = main.child(
            div()
                .flex()
                .justify_between()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .id("journal-heading")
                                .debug_selector(|| "journal-heading".into())
                                .w_full()
                                .min_w(px(0.))
                                .whitespace_normal()
                                .text_size(px(32.))
                                .line_height(px(38.4))
                                .font_weight(FontWeight(HEADING_WEIGHT))
                                .child(if searching {
                                    self.tr("검색 결과.").to_owned()
                                } else {
                                    format!("{}.", self.title())
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(16.))
                                .line_height(px(24.8))
                                .font_weight(FontWeight(LEAD_WEIGHT))
                                .text_color(rgb(self.palette.body))
                                .child(if self.index {
                                    self.tr("기록이 쌓이면 나만의 지도가 됩니다").to_owned()
                                } else {
                                    if self.settings.language == Language::Korean {
                                        format!("{}년 {}월 {}일 · 열린 할 일 {}개", self.date.year(), self.date.month(), self.date.day(), open)
                                    } else {
                                        format!("{} · {open} open tasks", self.date)
                                    }
                                }),
                        ),
                )
                .child(self.button(
                    "status-filter",
                    match self.filter {
                        | Filter::All => self.tr("모든 기록").to_owned(),
                        | Filter::Open => self.tr("미완료").to_owned(),
                        | Filter::Complete => self.tr("완료").to_owned(),
                    },
                    Command::Filter,
                    false,
                    cx,
                )),
        );
        if let Some(error) = &self.settings_error {
            main = main.child(
                div()
                    .id("settings-error")
                    .debug_selector(|| "settings-error".into())
                    .p_3()
                    .rounded(px(CARD_RADIUS))
                    .bg(rgb(self.palette.soft))
                    .text_color(rgb(self.palette.ink))
                    .text_size(px(14.))
                    .child(error.clone()),
            );
        }
        if let Some(error) = self.load_error.as_ref().or(self.error.as_ref()) {
            main = main.child(
                div()
                    .id("error-message")
                    .debug_selector(|| "error-message".into())
                    .p_3()
                    .rounded(px(CARD_RADIUS))
                    .bg(rgb(self.palette.soft))
                    .text_color(rgb(self.palette.ink))
                    .text_size(px(14.))
                    .line_height(px(21.7))
                    .child(if self.load_error.is_some() {
                        self.tr("읽기 전용 · 원본 파일을 확인해 주세요").to_owned()
                    } else {
                        self.tr("오류 · 입력을 유지했습니다").to_owned()
                    })
                    .child(div().mt_2().child(error.clone())),
            );
        }
        if !self.index && !searching {
            main = main.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(self.button(
                        "previous-date",
                        "‹",
                        if self.log == Log::Daily { Command::Day(-1) } else { Command::Month(-1) },
                        false,
                        cx,
                    ))
                    .child(div().w(px(155.)).child(self.input("date-input", &self.date_input)))
                    .child(self.button("go-date", self.tr("이동").to_owned(), Command::Date, false, cx))
                    .child(self.button(
                        "next-date",
                        "›",
                        if self.log == Log::Daily { Command::Day(1) } else { Command::Month(1) },
                        false,
                        cx,
                    ))
                    .child(self.button("today", self.tr("오늘").to_owned(), Command::Today, false, cx)),
            );
            if self.log == Log::Future {
                let mut months = div().flex().flex_wrap().gap_2();
                for offset in 0..6 {
                    if let Ok(date) = shift_month(self.date, offset) {
                        let raw = date.to_string();
                        months = months.child(
                            div()
                                .id(SharedString::from(format!("future-month-{offset}")))
                                .px_3()
                                .py_2()
                                .rounded(px(CONTROL_RADIUS))
                                .min_h(px(CONTROL_HEIGHT))
                                .flex()
                                .items_center()
                                .text_size(px(14.))
                                .font_weight(FontWeight(NAV_WEIGHT))
                                .bg(rgb(if offset == 0 { self.palette.card } else { self.palette.canvas }))
                                .text_color(rgb(if offset == 0 { self.palette.ink } else { self.palette.muted }))
                                .cursor_pointer()
                                .child(if self.settings.language == Language::Korean {
                                    format!("{}년 {}월", date.year(), date.month())
                                } else {
                                    format!("{}-{:02}", date.year(), date.month())
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Ok(date) = parse_date(&raw) {
                                        this.set_date(date, cx)
                                    }
                                })),
                        );
                    }
                }
                main = main.child(months);
            }
            if self.editing.is_some() {
                main = main.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().text_color(rgb(self.palette.ink)).child(self.tr("기록 수정 중").to_owned()))
                        .child(self.button("cancel-edit", self.tr("수정 취소").to_owned(), Command::StopEditing, false, cx)),
                );
            }
            main = main.child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .bg(rgb(self.palette.soft))
                    .rounded(px(CONTROL_RADIUS))
                    .flex_wrap()
                    .child(self.button(
                        "kind-task",
                        self.tr("• 할 일").to_owned(),
                        Command::Kind(Kind::Task),
                        self.kind == Kind::Task,
                        cx,
                    ))
                    .child(self.button(
                        "kind-event",
                        self.tr("○ 이벤트").to_owned(),
                        Command::Kind(Kind::Event),
                        self.kind == Kind::Event,
                        cx,
                    ))
                    .child(self.button(
                        "kind-note",
                        self.tr("– 메모").to_owned(),
                        Command::Kind(Kind::Note),
                        self.kind == Kind::Note,
                        cx,
                    )),
            );
            main = main.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(self.input("entry-input", &self.entry_input))
                    .child(self.button(
                        if self.editing.is_some() { "save-entry" } else { "add-entry" },
                        if self.editing.is_some() {
                            self.tr("수정 저장").to_owned()
                        } else {
                            self.tr("기록 +").to_owned()
                        },
                        Command::Add,
                        true,
                        cx,
                    )),
            );
        }
        if self.migrating.is_some() {
            main = main.child(
                div()
                    .p_6()
                    .bg(rgb(self.palette.card))
                    .rounded(px(CARD_RADIUS))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.tr("이 할 일은 언제 다시 할까요?").to_owned())
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .p_2()
                            .bg(rgb(self.palette.soft))
                            .rounded(px(CONTROL_RADIUS))
                            .flex_wrap()
                            .child(self.button(
                                "target-daily",
                                self.tr("일간").to_owned(),
                                Command::Target(Log::Daily),
                                self.target_log == Log::Daily,
                                cx,
                            ))
                            .child(self.button(
                                "target-monthly",
                                self.tr("월간").to_owned(),
                                Command::Target(Log::Monthly),
                                self.target_log == Log::Monthly,
                                cx,
                            ))
                            .child(self.button(
                                "target-future",
                                self.tr("미래").to_owned(),
                                Command::Target(Log::Future),
                                self.target_log == Log::Future,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(self.input("target-input", &self.target_input))
                            .child(self.button("confirm-migrate", self.tr("이월 저장").to_owned(), Command::ConfirmMigration, true, cx))
                            .child(self.button("cancel-migrate", self.tr("닫기").to_owned(), Command::StopEditing, false, cx)),
                    ),
            );
        }
        let sidebar = if show_sidebar {
            Some(collection_panel)
        } else {
            main = main.child(collection_panel);
            None
        };
        let mut content = div().id("records-list").flex_shrink_0().min_h(px(160.)).flex().flex_col().gap_3();
        if self.index && !searching {
            if self.journal().entries.is_empty() && self.journal().collections.is_empty() {
                content = content.child(
                    div()
                        .py_10()
                        .text_color(rgb(self.palette.muted))
                        .child(self.tr("아직 인덱스가 비어 있어요. 오늘의 첫 기록을 남겨 보세요.").to_owned()),
                );
            }
            let mut locations = Vec::new();
            for e in &self.journal().entries {
                let key = (
                    match e.log {
                        | Log::Daily => e.date.to_string(),
                        | Log::Monthly | Log::Future => {
                            format!("{}-{:02}", e.date.year(), e.date.month())
                        },
                        | Log::Collection(_) => String::new(),
                    },
                    e.log.clone(),
                );
                if !locations.contains(&key) {
                    locations.push(key);
                    content = content.child(self.button(format!("index-{}", e.id), self.location(e), Command::Jump(e.id), false, cx));
                }
            }
            for c in &self.journal().collections {
                content = content.child(self.button(
                    format!("index-collection-{}", c.id),
                    format!("# {}", c.name),
                    Command::Nav(Log::Collection(c.id)),
                    false,
                    cx,
                ));
            }
        } else {
            if self.log == Log::Monthly && !searching {
                content = content.child(
                    div()
                        .mb_3()
                        .text_size(px(14.))
                        .line_height(px(21.7))
                        .text_color(rgb(self.palette.ink))
                        .child(self.tr("이번 달의 달력 · 날짜를 누르면 일간 로그가 열립니다").to_owned()),
                );
                let mut calendar = div().flex().flex_wrap().gap_1().mb_5();
                let first = self.date.with_day(1).unwrap();
                let next = shift_month(first, 1).ok();
                let days = next.map(|n| (n - first).num_days() as u32).unwrap_or(31);
                for day in 1..=days {
                    let date = first.with_day(day).unwrap();
                    let count = self.journal().visible(date, &Log::Daily).len();
                    calendar = calendar.child(
                        div()
                            .id(SharedString::from(format!("calendar-{day}")))
                            .debug_selector(move || format!("calendar-{day}"))
                            .w(px(62.))
                            .h(px(57.))
                            .p_2()
                            .rounded(px(CONTROL_RADIUS))
                            .text_size(px(14.))
                            .text_color(rgb(self.palette.ink))
                            .bg(rgb(if date == Local::now().date_naive() {
                                self.palette.elevated
                            } else {
                                self.palette.card
                            }))
                            .cursor_pointer()
                            .child(format!(
                                "{} {}",
                                day,
                                [
                                    self.tr("월").to_owned(),
                                    self.tr("화").to_owned(),
                                    self.tr("수").to_owned(),
                                    self.tr("목").to_owned(),
                                    self.tr("금").to_owned(),
                                    self.tr("토").to_owned(),
                                    self.tr("일").to_owned()
                                ][date.weekday().num_days_from_monday() as usize]
                            ))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .line_height(px(18.2))
                                    .text_color(rgb(self.palette.muted))
                                    .child(if count > 0 {
                                        if self.settings.language == Language::Korean {
                                            format!("{count} 기록")
                                        } else {
                                            format!("{count} entries")
                                        }
                                    } else {
                                        String::new()
                                    }),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.log = Log::Daily;
                                this.set_date(date, cx)
                            })),
                    );
                }
                content = content.child(calendar).child(
                    div()
                        .text_size(px(14.))
                        .line_height(px(21.7))
                        .text_color(rgb(self.palette.ink))
                        .child(self.tr("이번 달의 할 일과 기록").to_owned()),
                );
            }
            if records.is_empty() {
                content = content.child(
                    div()
                        .id("empty-state")
                        .debug_selector(move || if searching { "search-empty".into() } else { "empty-state".into() })
                        .p_8()
                        .rounded(px(CARD_RADIUS))
                        .bg(rgb(self.palette.card))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(24.))
                                .line_height(px(30.))
                                .font_weight(FontWeight(HEADING_WEIGHT))
                                .text_color(rgb(self.palette.ink))
                                .child(if searching {
                                    self.tr("찾은 기록이 없습니다.").to_owned()
                                } else {
                                    self.tr("작은 기록으로 시작하세요.").to_owned()
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .line_height(px(21.7))
                                .text_color(rgb(self.palette.muted))
                                .child(if searching {
                                    self.tr("검색어 또는 상태 필터를 바꿔 보세요.").to_owned()
                                } else {
                                    self.tr("해야 할 일, 있었던 일, 기억하고 싶은 생각.\n한 줄이면 충분해요. 입력 후 Enter로 저장합니다.")
                                        .to_owned()
                                }),
                        ),
                );
            }
            for e in &records {
                content = content.child(self.row(e, searching, cx));
            }
        }
        main = main.child(content);
        let footer = div()
            .id("design-footer")
            .debug_selector(|| "design-footer".into())
            .flex_shrink_0()
            .bg(rgb(self.palette.canvas))
            .border_t_1()
            .border_color(rgb(self.palette.hairline))
            .px_6()
            .py_4()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_4()
            .text_size(px(12.))
            .line_height(px(18.2))
            .text_color(rgb(self.palette.muted))
            .child(
                div()
                    .text_size(px(24.))
                    .font_weight(FontWeight(HEADING_WEIGHT))
                    .text_color(rgb(self.palette.ink))
                    .whitespace_nowrap()
                    .flex_shrink_0()
                    .child("stillnote"),
            )
            .child(
                self.notice
                    .split_once(" · ")
                    .map(|(key, detail)| format!("{} · {detail}", self.tr(key)))
                    .unwrap_or_else(|| self.tr(&self.notice).to_owned()),
            )
            .when(!compact, |d| d.child(self.tr("Enter 기록 · Ctrl+A 선택 · Ctrl+V 붙여넣기").to_owned()));
        let footer = if short {
            main = main.child(footer);
            None
        } else {
            Some(footer)
        };
        let mut body = div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .gap_6()
            .when_some(sidebar, |d, sidebar| d.child(sidebar))
            .child(
                div()
                    .id("design-main")
                    .debug_selector(|| "design-main".into())
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .child(
                        div()
                            .id("journal-scroll")
                            .debug_selector(|| "journal-scroll".into())
                            .size_full()
                            .overflow_y_scroll()
                            .child(main),
                    ),
            );
        if width >= px(ASIDE_BREAKPOINT) && !short {
            body = body.child(
                div()
                    .id("design-aside")
                    .debug_selector(|| "design-aside".into())
                    .w(px(208.))
                    .flex_shrink_0()
                    .h_full()
                    .overflow_y_scroll()
                    .rounded(px(CARD_RADIUS))
                    .bg(rgb(self.palette.card))
                    .child(
                        div()
                            .px_6()
                            .py_8()
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("design-stat")
                                            .debug_selector(|| "design-stat".into())
                                            .text_size(px(STAT_SIZE))
                                            .line_height(px(STAT_SIZE))
                                            .font_weight(FontWeight(STAT_WEIGHT))
                                            .text_color(rgb(self.palette.primary))
                                            .child(open.to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(self.palette.muted))
                                            .child(self.tr("열린 할 일").to_owned()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .line_height(px(30.))
                                    .font_weight(FontWeight(HEADING_WEIGHT))
                                    .child(self.tr("천천히 돌아보기.").to_owned()),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .line_height(px(21.7))
                                    .text_color(rgb(self.palette.body))
                                    .child(self.tr("오늘의 기록이 내일의 방향이 됩니다. 필요한 일만 다음으로 가져가세요.").to_owned()),
                            )
                            .child(div().h(px(1.)).bg(rgb(self.palette.hairline)))
                            .child(self.tr("빠른 기록 범례").to_owned())
                            .children(
                                [
                                    self.tr("•   해야 할 일").to_owned(),
                                    self.tr("×   완료한 일").to_owned(),
                                    self.tr("○   이벤트").to_owned(),
                                    self.tr("–   생각과 메모").to_owned(),
                                    self.tr(">   다른 로그로 이월").to_owned(),
                                    self.tr("<   미래 로그에 예약").to_owned(),
                                    self.tr("★   중요한 기록").to_owned(),
                                    self.tr("⊘   취소한 기록").to_owned(),
                                ]
                                .into_iter()
                                .map(|label| div().text_size(px(14.)).line_height(px(21.7)).child(label)),
                            )
                            .child(
                                div()
                                    .mt_6()
                                    .text_size(px(12.))
                                    .line_height(px(18.2))
                                    .text_color(rgb(self.palette.muted))
                                    .child(self.tr("Ryder Carroll의 불렛저널 방법에서 영감을 받았습니다.").to_owned()),
                            ),
                    ),
            );
        }
        div()
            .size_full()
            .track_focus(&self.root_focus)
            .on_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    cx.stop_propagation();
                }
            })
            .flex()
            .flex_col()
            .bg(rgb(self.palette.canvas))
            .text_color(rgb(self.palette.ink))
            .font_family(FONT_FAMILY)
            .font_weight(FontWeight(BODY_WEIGHT))
            .text_size(px(16.))
            .line_height(px(24.8))
            .child(nav)
            .child(
                div()
                    .id("design-container")
                    .debug_selector(|| "design-container".into())
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .p_6()
                    .when_some(mobile_menu, |d, menu| d.child(menu))
                    .child(body)
                    .when_some(footer, |d, footer| d.child(footer)),
            )
    }
}
