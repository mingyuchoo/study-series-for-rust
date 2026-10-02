use crate::{
    input::{Submitted, TextInput},
    *,
};
use chrono::{Datelike, Days, Local, NaiveDate};
use gpui::{prelude::*, *};
use std::path::PathBuf;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::theme::*;

/// GPUI 0.2.2 has no letter-spacing property. Space whole graphemes with
/// negative margins so Hangul and combining marks remain intact and headings
/// can still wrap on narrow windows.
fn tracked_text(text: impl AsRef<str>, tracking: f32) -> Div {
    let graphemes: Vec<_> = text.as_ref().graphemes(true).map(str::to_owned).collect();
    let count = graphemes.len();
    div().flex().flex_wrap().children(
        graphemes
            .into_iter()
            .enumerate()
            .map(|(index, glyph)| div().when(index + 1 < count, |d| d.mr(px(tracking))).child(glyph)),
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
}

pub struct JournalView {
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
        let (session, error) = match Session::open(path.clone()) {
            | Ok(s) => (Some(s), None),
            | Err(e) => (None, Some(format!("{e:#}"))),
        };
        let entry_input = cx.new(|cx| TextInput::new("머릿속의 생각을 한 줄로 기록해 보세요", cx));
        let search_input = cx.new(|cx| TextInput::new("전체 기록 검색…", cx));
        let date_input = cx.new(|cx| {
            let mut input = TextInput::new("YYYY-MM-DD", cx);
            input.set_text(&date.to_string(), cx);
            input
        });
        let collection_input = cx.new(|cx| TextInput::new("새 컬렉션 이름", cx));
        let target_input = cx.new(|cx| TextInput::new("대상 날짜 YYYY-MM-DD", cx));
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
        Self {
            session,
            date,
            log: Log::Daily,
            load_error: error.clone(),
            error,
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
    pub fn journal(&self) -> &Journal {
        self.session.as_ref().map(|s| &s.journal).unwrap_or(&self.empty)
    }
    fn transact<T>(&mut self, op: impl FnOnce(&mut Journal) -> anyhow::Result<T>, cx: &mut Context<Self>) -> Option<T> {
        let result = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("원본 파일 보호를 위해 읽기 전용입니다. README의 복구 안내를 확인해 주세요"))
            .and_then(|s| s.transact(op));
        match result {
            | Ok(v) => {
                self.error = None;
                self.notice = format!("로컬 저장 완료 · {}", Local::now().format("%H:%M"));
                cx.notify();
                Some(v)
            },
            | Err(e) => {
                self.error = Some(format!("저장하지 못했습니다 · {e:#}"));
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
                self.set_date(date, cx);
            },
            | Err(_) => {
                self.error = Some("올바른 날짜를 입력해 주세요 (YYYY-MM-DD)".into());
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
                self.error = Some("이월 대상 날짜를 확인해 주세요".into());
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
                | Err(e) => self.error = Some(e.to_string()),
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
        let segmented = id.starts_with("kind-") || id.starts_with("target-");
        let tab = segmented || id.starts_with("nav-") || id.starts_with("collection-");
        let primary = matches!(id.as_str(), "add-entry" | "save-entry" | "confirm-migrate" | "create-collection");
        let label = label.into();
        let label = if segmented && active { format!("✓ {label}") } else { label };
        div()
            .id(SharedString::from(id.clone()))
            .debug_selector(move || id)
            .px(px(if tab { 14. } else { 20. }))
            .min_w(px(CONTROL_HEIGHT))
            .max_w_full()
            .h(px(CONTROL_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(if tab && !active {
                rgba(0x00000000)
            } else {
                rgb(if primary { PRIMARY } else { CARD })
            })
            .cursor_pointer()
            .text_size(px(14.))
            .line_height(px(if tab { 19.6 } else { 14. }))
            .font_weight(FontWeight(if tab { NAV_WEIGHT } else { CONTROL_WEIGHT }))
            .text_color(rgb(if primary {
                ON_PRIMARY
            } else if tab && !active {
                MUTED
            } else {
                INK
            }))
            .bg(if tab && !active {
                rgba(0x00000000)
            } else {
                rgb(if primary { PRIMARY } else { CARD })
            })
            .when(primary, |d| d.active(|s| s.bg(rgb(PRIMARY_ACTIVE)).border_color(rgb(PRIMARY_ACTIVE))))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.command(command.clone(), cx)))
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
            return "인덱스".into();
        }
        match self.log {
            | Log::Daily => "일간 로그".into(),
            | Log::Monthly => "월간 로그".into(),
            | Log::Future => "미래 로그".into(),
            | Log::Collection(id) => self
                .journal()
                .collections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.name.clone())
                .unwrap_or("컬렉션".into()),
        }
    }
    fn location(&self, e: &Entry) -> String {
        format!(
            "{} · {}",
            e.date,
            match &e.log {
                | Log::Daily => "일간".into(),
                | Log::Monthly => "월간".into(),
                | Log::Future => "미래".into(),
                | Log::Collection(id) => self
                    .journal()
                    .collections
                    .iter()
                    .find(|c| c.id == *id)
                    .map(|c| c.name.clone())
                    .unwrap_or("컬렉션".into()),
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
                    if e.status == Status::Complete { "재개" } else { "완료" },
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
                .child(self.button(format!("edit-{id}"), "수정", Command::Edit(id), false, cx))
                .child(self.button(format!("cancel-{id}"), "취소", Command::Cancel(id), false, cx));
            if e.is_open_task() {
                actions = actions.child(self.button(format!("migrate-{id}"), "이월 →", Command::Migrate(id), false, cx));
            }
        }
        if search {
            actions = actions.child(self.button(format!("jump-{id}"), "로그 열기", Command::Jump(id), false, cx));
        }
        let mut body = div().flex_1().min_w(px(100.)).flex().flex_col().gap_1().child(
            div()
                .text_size(px(16.))
                .text_color(rgb(if e.status == Status::Complete || e.status == Status::Cancelled {
                    MUTED
                } else {
                    BODY
                }))
                .child(format!("{}{}", if e.important { "★  " } else { "" }, e.text)),
        );
        if search {
            body = body.child(div().text_size(px(12.)).line_height(px(18.2)).text_color(rgb(MUTED)).child(self.location(e)));
        }
        if let Some(target) = e.migrated_to.and_then(|id| self.journal().entry(id).ok()) {
            body = body.child(self.button(
                format!("trace-{id}"),
                format!("이월됨 · {}  ↗", self.location(target)),
                Command::Jump(target.id),
                false,
                cx,
            ));
        }
        if let Some(source) = e.migrated_from.and_then(|id| self.journal().entry(id).ok()) {
            body = body.child(self.button(
                format!("source-{id}"),
                format!("원본 · {}  ↗", self.location(source)),
                Command::Jump(source.id),
                false,
                cx,
            ));
        }
        div()
            .id(SharedString::from(format!("row-{id}")))
            .flex_shrink_0()
            .flex()
            .gap_3()
            .py_4()
            .border_b_1()
            .border_color(rgb(HAIRLINE))
            .flex_col()
            .rounded(px(CARD_RADIUS))
            .border_1()
            .border_color(rgb(HAIRLINE_SOFT))
            .bg(rgb(CARD))
            .p_6()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(div().w(px(24.)).text_size(px(24.)).text_color(rgb(INK)).child(e.symbol()))
                    .child(body),
            )
            .child(actions)
    }
}
impl Render for JournalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search = self.search_input.read(cx).content.to_string();
        let searching = !search.trim().is_empty();
        let records: Vec<Entry> = self.visible_entries(cx).into_iter().cloned().collect();
        let open = records.iter().filter(|e| e.is_open_task()).count();
        let width = window.viewport_size().width;
        let compact = width < px(COMPACT_BREAKPOINT);
        let show_sidebar = width >= px(SIDEBAR_BREAKPOINT);
        let mut nav_links = div()
            .id(if compact { "design-mobile-menu" } else { "design-nav-links" })
            .debug_selector(move || if compact { "design-mobile-menu".into() } else { "design-nav-links".into() })
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .bg(rgb(CANVAS));
        for (id, label, short, log) in [
            ("nav-daily", "일간 로그", "일간", Log::Daily),
            ("nav-monthly", "월간 로그", "월간", Log::Monthly),
            ("nav-future", "미래 로그", "미래", Log::Future),
        ] {
            nav_links = nav_links.child(self.button(
                id,
                if compact { short } else { label },
                Command::Nav(log.clone()),
                !self.index && self.log == log,
                cx,
            ));
        }
        nav_links = nav_links.child(self.button("nav-index", "인덱스", Command::Index, self.index, cx));
        let nav_content = div()
            .id("design-nav-content")
            .debug_selector(|| "design-nav-content".into())
            .w_full()
            .max_w(px(MAX_CONTENT_WIDTH))
            .h_full()
            .mx_auto()
            .px_6()
            .flex()
            .items_center()
            .gap_6()
            .child(
                div()
                    .font_weight(FontWeight(HEADING_WEIGHT))
                    .text_size(px(24.))
                    .child(tracked_text("stillnote", -0.3)),
            )
            .when(compact, |d| {
                d.justify_between().child(self.button(
                    "nav-menu-toggle",
                    if self.menu_open { "✕ 메뉴 닫기" } else { "☰ 메뉴" },
                    Command::ToggleMenu,
                    false,
                    cx,
                ))
            });
        let (nav_content, mobile_menu) = if compact {
            (nav_content, if self.menu_open { Some(nav_links) } else { None })
        } else {
            (nav_content.child(nav_links), None)
        };
        let nav = div()
            .id("design-nav")
            .debug_selector(|| "design-nav".into())
            .w_full()
            .h(px(NAV_HEIGHT))
            .flex_shrink_0()
            .bg(rgb(CANVAS))
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
            .flex()
            .when(show_sidebar, |d| d.flex_col())
            .gap_2()
            .child(self.input("collection-input", &self.collection_input))
            .child(self.button(
                "create-collection",
                if compact { "+ 만들기" } else { "+ 컬렉션 만들기" },
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
            .bg(rgb(CARD))
            .when(show_sidebar, |d| d.w(px(208.)).flex_shrink_0().h_full())
            .child(div().text_size(px(14.)).text_color(rgb(MUTED)).child("컬렉션"))
            .child(collections)
            .child(collection_form);
        let mut main = div()
            .id("journal-scroll")
            .debug_selector(|| "journal-scroll".into())
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_4()
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("나의 기록"))
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
                                .text_size(px(32.))
                                .line_height(px(38.4))
                                .font_weight(FontWeight(HEADING_WEIGHT))
                                .child(tracked_text(
                                    if searching { "검색 결과.".into() } else { format!("{}.", self.title()) },
                                    HEADING_TRACKING,
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(16.))
                                .line_height(px(24.8))
                                .font_weight(FontWeight(LEAD_WEIGHT))
                                .text_color(rgb(BODY))
                                .child(if self.index {
                                    "기록이 쌓이면 나만의 지도가 됩니다".into()
                                } else {
                                    format!("{}년 {}월 {}일 · 열린 할 일 {}개", self.date.year(), self.date.month(), self.date.day(), open)
                                }),
                        ),
                )
                .child(self.button(
                    "status-filter",
                    match self.filter {
                        | Filter::All => "모든 기록",
                        | Filter::Open => "미완료",
                        | Filter::Complete => "완료",
                    },
                    Command::Filter,
                    false,
                    cx,
                )),
        );
        if let Some(error) = self.load_error.as_ref().or(self.error.as_ref()) {
            main = main.child(
                div()
                    .id("error-message")
                    .debug_selector(|| "error-message".into())
                    .p_3()
                    .rounded(px(CARD_RADIUS))
                    .bg(rgb(SOFT))
                    .text_color(rgb(INK))
                    .text_size(px(14.))
                    .line_height(px(21.7))
                    .child(if self.load_error.is_some() {
                        "읽기 전용 · 원본 파일을 확인해 주세요"
                    } else {
                        "오류 · 입력을 유지했습니다"
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
                    .child(self.button("go-date", "이동", Command::Date, false, cx))
                    .child(self.button(
                        "next-date",
                        "›",
                        if self.log == Log::Daily { Command::Day(1) } else { Command::Month(1) },
                        false,
                        cx,
                    ))
                    .child(self.button("today", "오늘", Command::Today, false, cx)),
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
                                .bg(rgb(if offset == 0 { CARD } else { CANVAS }))
                                .text_color(rgb(if offset == 0 { INK } else { MUTED }))
                                .cursor_pointer()
                                .child(format!("{}년 {}월", date.year(), date.month()))
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
                main = main.child(div().flex().gap_2().child(div().text_color(rgb(INK)).child("기록 수정 중")).child(self.button(
                    "cancel-edit",
                    "수정 취소",
                    Command::StopEditing,
                    false,
                    cx,
                )));
            }
            main = main.child(
                div()
                    .flex()
                    .gap_2()
                    .p_2()
                    .bg(rgb(SOFT))
                    .rounded(px(CONTROL_RADIUS))
                    .flex_wrap()
                    .child(self.button("kind-task", "• 할 일", Command::Kind(Kind::Task), self.kind == Kind::Task, cx))
                    .child(self.button("kind-event", "○ 이벤트", Command::Kind(Kind::Event), self.kind == Kind::Event, cx))
                    .child(self.button("kind-note", "– 메모", Command::Kind(Kind::Note), self.kind == Kind::Note, cx)),
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
                        if self.editing.is_some() { "수정 저장" } else { "기록 +" },
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
                    .bg(rgb(CARD))
                    .rounded(px(CARD_RADIUS))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("이 할 일은 언제 다시 할까요?")
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .p_2()
                            .bg(rgb(SOFT))
                            .rounded(px(CONTROL_RADIUS))
                            .flex_wrap()
                            .child(self.button("target-daily", "일간", Command::Target(Log::Daily), self.target_log == Log::Daily, cx))
                            .child(self.button("target-monthly", "월간", Command::Target(Log::Monthly), self.target_log == Log::Monthly, cx))
                            .child(self.button("target-future", "미래", Command::Target(Log::Future), self.target_log == Log::Future, cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(self.input("target-input", &self.target_input))
                            .child(self.button("confirm-migrate", "이월 저장", Command::ConfirmMigration, true, cx))
                            .child(self.button("cancel-migrate", "닫기", Command::StopEditing, false, cx)),
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
                        .text_color(rgb(MUTED))
                        .child("아직 인덱스가 비어 있어요. 오늘의 첫 기록을 남겨 보세요."),
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
                        .text_color(rgb(INK))
                        .child("이번 달의 달력 · 날짜를 누르면 일간 로그가 열립니다"),
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
                            .text_color(rgb(INK))
                            .bg(rgb(if date == Local::now().date_naive() { ELEVATED } else { CARD }))
                            .cursor_pointer()
                            .child(format!(
                                "{} {}",
                                day,
                                ["월", "화", "수", "목", "금", "토", "일"][date.weekday().num_days_from_monday() as usize]
                            ))
                            .child(div().text_size(px(12.)).line_height(px(18.2)).text_color(rgb(MUTED)).child(if count > 0 {
                                format!("{count} 기록")
                            } else {
                                String::new()
                            }))
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
                        .text_color(rgb(INK))
                        .child("이번 달의 할 일과 기록"),
                );
            }
            if records.is_empty() {
                content = content.child(
                    div()
                        .id("empty-state")
                        .debug_selector(move || if searching { "search-empty".into() } else { "empty-state".into() })
                        .p_8()
                        .rounded(px(CARD_RADIUS))
                        .bg(rgb(CARD))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(24.))
                                .line_height(px(30.))
                                .font_weight(FontWeight(HEADING_WEIGHT))
                                .text_color(rgb(INK))
                                .child(tracked_text(
                                    if searching {
                                        "찾은 기록이 없습니다."
                                    } else {
                                        "작은 기록으로 시작하세요."
                                    },
                                    -0.3,
                                )),
                        )
                        .child(div().text_size(px(14.)).line_height(px(21.7)).text_color(rgb(MUTED)).child(if searching {
                            "검색어 또는 상태 필터를 바꿔 보세요."
                        } else {
                            "해야 할 일, 있었던 일, 기억하고 싶은 생각.\n한 줄이면 충분해요. 입력 후 Enter로 저장합니다."
                        })),
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
            .bg(rgb(CANVAS))
            .border_t_1()
            .border_color(rgb(HAIRLINE))
            .px_6()
            .py_4()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .text_size(px(12.))
            .line_height(px(18.2))
            .text_color(rgb(MUTED))
            .child(
                div()
                    .text_size(px(24.))
                    .font_weight(FontWeight(HEADING_WEIGHT))
                    .text_color(rgb(INK))
                    .child(tracked_text("stillnote", -0.3)),
            )
            .child(self.notice.clone())
            .when(!compact, |d| d.child("Enter 기록 · Ctrl+A 선택 · Ctrl+V 붙여넣기"));
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
                    .child(main),
            );
        if width >= px(ASIDE_BREAKPOINT) {
            body = body.child(
                div()
                    .id("design-aside")
                    .debug_selector(|| "design-aside".into())
                    .w(px(208.))
                    .flex_shrink_0()
                    .h_full()
                    .rounded(px(CARD_RADIUS))
                    .bg(rgb(CARD))
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
                                    .text_color(rgb(PRIMARY))
                                    .child(tracked_text(open.to_string(), STAT_TRACKING)),
                            )
                            .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child("열린 할 일")),
                    )
                    .child(
                        div()
                            .text_size(px(24.))
                            .line_height(px(30.))
                            .font_weight(FontWeight(HEADING_WEIGHT))
                            .child(tracked_text("천천히 돌아보기.", -0.3)),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(21.7))
                            .text_color(rgb(BODY))
                            .child("오늘의 기록이 내일의 방향이 됩니다. 필요한 일만 다음으로 가져가세요."),
                    )
                    .child(div().h(px(1.)).bg(rgb(HAIRLINE)))
                    .child("빠른 기록 범례")
                    .children(
                        [
                            "•   해야 할 일",
                            "×   완료한 일",
                            "○   이벤트",
                            "–   생각과 메모",
                            ">   다른 로그로 이월",
                            "<   미래 로그에 예약",
                            "★   중요한 기록",
                            "⊘   취소한 기록",
                        ]
                        .into_iter()
                        .map(|label| div().text_size(px(14.)).line_height(px(21.7)).child(label)),
                    )
                    .child(
                        div()
                            .mt_6()
                            .text_size(px(12.))
                            .line_height(px(18.2))
                            .text_color(rgb(MUTED))
                            .child("Ryder Carroll의 불렛저널 방법에서 영감을 받았습니다."),
                    ),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(CANVAS))
            .text_color(rgb(INK))
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
                    .max_w(px(MAX_CONTENT_WIDTH))
                    .mx_auto()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .p_6()
                    .when_some(mobile_menu, |d, menu| d.child(menu))
                    .child(body)
                    .child(footer),
            )
    }
}
