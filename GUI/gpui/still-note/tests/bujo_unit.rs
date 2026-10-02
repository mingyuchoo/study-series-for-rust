use chrono::{Datelike, NaiveDate};
use stillnote::model::{Filter, Journal, Kind, Log, Status, parse_date, shift_month};
use uuid::Uuid;

#[path = "support/font_metadata.rs"]
mod font_metadata;

fn day(value: &str) -> NaiveDate {
    parse_date(value).unwrap()
}

#[test]
fn design_ac01_ac02_ac03_exact_black_yellow_and_type_contract() {
    use stillnote::theme::*;
    // DESIGN.md tokens; user-requested FONT-01 overrides the original font family.
    assert_eq!(
        [
            CANVAS,
            INK,
            BODY,
            BODY_STRONG,
            MUTED,
            FAINT,
            FIELD,
            SOFT,
            CARD,
            ELEVATED,
            HAIRLINE,
            HAIRLINE_SOFT
        ],
        [
            0x0a0a0a, 0xffffff, 0xcccccc, 0xe6e6e6, 0x888888, 0x5a5a5a, 0x1a1a1a, 0x121212, 0x1a1a1a, 0x242424, 0x2a2a2a, 0x3a3a3a
        ]
    );
    assert_eq!(
        [PRIMARY, PRIMARY_ACTIVE, PRIMARY_DISABLED, ON_PRIMARY],
        [0xfaff69, 0xe6eb52, 0x3a3a1f, 0x0a0a0a]
    );
    assert_eq!(FONT_FAMILY, "Pretendard");
    assert_eq!(CODE_FONT_FAMILY, "Pretendard");
    assert_eq!([HEADING_WEIGHT, BODY_WEIGHT, CONTROL_WEIGHT, NAV_WEIGHT], [700., 400., 600., 500.]);
    assert_eq!([STAT_SIZE, STAT_WEIGHT, HEADING_TRACKING, STAT_TRACKING], [56., 700., -1., -1.5]);
    assert_eq!([CONTROL_HEIGHT, INPUT_RADIUS, CONTROL_RADIUS, CARD_RADIUS], [40., 8., 8., 12.]);
    // WIDTH-01 user override removes the prior maximum-width contract.
    assert_eq!([NAV_HEIGHT, COMPACT_BREAKPOINT], [64., 768.]);
    assert_eq!(MIN_WINDOW_WIDTH, 600.);
}

#[test]
fn ac02_unicode_three_kinds_stable_ids_and_blank_rejection() {
    let mut journal = Journal::default();
    let date = day("2024-02-29");
    let kinds = [(Kind::Task, "책 읽기 📚"), (Kind::Event, "친구와 점심"), (Kind::Note, "오늘의 생각 café")];
    let mut ids = Vec::new();
    for (kind, text) in kinds {
        let id = journal.add_entry(date, Log::Daily, kind, &format!("  {text}  ")).unwrap();
        let entry = journal.entry(id).unwrap();
        assert_eq!((entry.date, entry.kind, entry.status), (date, kind, Status::Open));
        assert_eq!(entry.text, text);
        assert!(!id.is_nil());
        assert!(!ids.contains(&id));
        ids.push(id);
    }
    let before = journal.clone();
    for text in ["", " \t\n", "\u{3000}\u{2003}"] {
        assert!(journal.add_entry(date, Log::Daily, Kind::Task, text).is_err());
        assert_eq!(journal, before);
    }
    assert_eq!(journal.entries.iter().map(|e| e.symbol()).collect::<Vec<_>>(), ["•", "○", "–"]);
    assert!(journal.validate().is_ok());
}

#[test]
fn ac03_task_states_symbols_edit_and_importance() {
    let mut journal = Journal::default();
    let id = journal.add_entry(day("2026-10-02"), Log::Daily, Kind::Task, "초안").unwrap();
    journal.toggle_important(id).unwrap();
    journal.edit_entry(id, "  고친 내용 📝  ").unwrap();
    assert!(journal.entry(id).unwrap().important);
    assert_eq!(journal.entry(id).unwrap().text, "고친 내용 📝");
    journal.set_status(id, Status::Complete).unwrap();
    assert_eq!(journal.entry(id).unwrap().symbol(), "×");
    assert!(!journal.entry(id).unwrap().is_open_task());
    journal.set_status(id, Status::Open).unwrap();
    assert_eq!(journal.entry(id).unwrap().symbol(), "•");
    assert!(journal.entry(id).unwrap().is_open_task());
    journal.set_status(id, Status::Cancelled).unwrap();
    assert!(!journal.entry(id).unwrap().is_open_task());
    let before = journal.clone();
    assert!(journal.edit_entry(id, "  ").is_err());
    assert!(journal.set_status(id, Status::Migrated).is_err());
    assert_eq!(journal, before);
}

#[test]
fn ac03_non_tasks_cannot_complete_or_migrate() {
    let mut journal = Journal::default();
    for kind in [Kind::Event, Kind::Note] {
        let id = journal.add_entry(day("2026-10-02"), Log::Daily, kind, "기록").unwrap();
        let before = journal.clone();
        assert!(journal.set_status(id, Status::Complete).is_err());
        assert!(journal.migrate(id, day("2026-10-03"), Log::Daily).is_err());
        assert_eq!(journal, before);
    }
}

#[test]
fn ac04_calendar_leap_year_and_year_boundaries() {
    assert_eq!(day("2024-02-29").day(), 29);
    for date in [
        "2023-02-29",
        "2024-02-30",
        "2026-13-01",
        "2026-00-01",
        "0000-01-01",
        "10000-01-01",
        "not a date",
    ] {
        assert!(parse_date(date).is_err(), "unexpected valid date: {date}");
    }
    assert_eq!(shift_month(day("2026-12-31"), 1).unwrap(), day("2027-01-01"));
    assert_eq!(shift_month(day("2027-01-31"), -1).unwrap(), day("2026-12-01"));
    assert_eq!(shift_month(day("2024-03-31"), -1).unwrap(), day("2024-02-01"));
    assert!(shift_month(day("0001-01-01"), -1).is_err());
    assert!(shift_month(day("9999-12-31"), 1).is_err());
}

#[test]
fn ac04_log_visibility_respects_day_month_year_and_kind_of_log() {
    let mut journal = Journal::default();
    let daily = journal.add_entry(day("2026-12-31"), Log::Daily, Kind::Task, "daily").unwrap();
    let monthly = journal.add_entry(day("2026-12-03"), Log::Monthly, Kind::Task, "monthly").unwrap();
    let future = journal.add_entry(day("2027-01-01"), Log::Future, Kind::Event, "future").unwrap();
    journal.add_entry(day("2025-12-03"), Log::Monthly, Kind::Note, "old year").unwrap();
    assert_eq!(
        journal.visible(day("2026-12-31"), &Log::Daily).iter().map(|e| e.id).collect::<Vec<_>>(),
        [daily]
    );
    assert!(journal.visible(day("2026-12-30"), &Log::Daily).is_empty());
    assert_eq!(
        journal.visible(day("2026-12-31"), &Log::Monthly).iter().map(|e| e.id).collect::<Vec<_>>(),
        [monthly]
    );
    assert_eq!(
        journal.visible(day("2027-01-31"), &Log::Future).iter().map(|e| e.id).collect::<Vec<_>>(),
        [future]
    );
}

#[test]
fn ac05_collections_trim_names_reject_duplicate_or_missing_reference() {
    let mut journal = Journal::default();
    let collection = journal.add_collection("  독서 📚  ").unwrap();
    assert_eq!(journal.collections[0].name, "독서 📚");
    assert!(journal.add_collection("독서 📚").is_err());
    assert!(journal.add_collection(" \t").is_err());
    let id = journal
        .add_entry(day("2026-10-02"), Log::Collection(collection), Kind::Note, "읽은 책")
        .unwrap();
    assert_eq!(journal.visible(day("2027-12-31"), &Log::Collection(collection))[0].id, id);
    assert!(
        journal
            .add_entry(day("2026-10-02"), Log::Collection(Uuid::new_v4()), Kind::Note, "missing")
            .is_err()
    );
}

#[test]
fn ac06_migration_preserves_original_and_reciprocal_open_target() {
    for log in [Log::Daily, Log::Monthly, Log::Future] {
        let mut journal = Journal::default();
        let id = journal.add_entry(day("2026-10-02"), Log::Daily, Kind::Task, "읽기 📚").unwrap();
        journal.toggle_important(id).unwrap();
        let target = journal.migrate(id, day("2026-11-03"), log.clone()).unwrap();
        assert_ne!(id, target);
        let source = journal.entry(id).unwrap();
        let dest = journal.entry(target).unwrap();
        assert_eq!(source.date, day("2026-10-02"));
        assert_eq!(source.log, Log::Daily);
        assert_eq!(source.text, "읽기 📚");
        assert_eq!(source.migrated_to, Some(target));
        assert_eq!(source.symbol(), if log == Log::Future { "<" } else { ">" });
        assert!(!source.is_open_task());
        assert_eq!((dest.date, &dest.log, dest.status), (day("2026-11-03"), &log, Status::Open));
        assert_eq!(dest.migrated_from, Some(id));
        assert_eq!(dest.text, source.text);
        assert!(dest.important && dest.is_open_task());
        assert!(journal.validate().is_ok());
        let before = journal.clone();
        assert!(journal.migrate(id, day("2026-12-01"), Log::Daily).is_err());
        assert!(journal.set_status(id, Status::Open).is_err());
        assert!(journal.edit_entry(id, "rewrite source").is_err());
        assert_eq!(journal, before);
    }
}

#[test]
fn ac06_migration_rejects_same_effective_location_and_closed_tasks() {
    for log in [Log::Daily, Log::Monthly, Log::Future] {
        let mut journal = Journal::default();
        let id = journal.add_entry(day("2026-10-01"), log.clone(), Kind::Task, "task").unwrap();
        let target_date = if log == Log::Daily { day("2026-10-01") } else { day("2026-10-20") };
        let before = journal.clone();
        assert!(journal.migrate(id, target_date, log).is_err());
        assert_eq!(journal, before);
        journal.set_status(id, Status::Complete).unwrap();
        assert!(journal.migrate(id, day("2026-11-01"), Log::Daily).is_err());
    }
}

#[test]
fn ac06_same_day_different_log_is_valid_and_collection_target_is_refused() {
    let mut journal = Journal::default();
    let id = journal.add_entry(day("2026-10-02"), Log::Daily, Kind::Task, "task").unwrap();
    let collection = journal.add_collection("Projects").unwrap();
    assert!(journal.migrate(id, day("2026-10-03"), Log::Collection(collection)).is_err());
    assert!(journal.migrate(id, day("2026-10-02"), Log::Monthly).is_ok());
}

#[test]
fn ac09_search_unicode_across_logs_collections_and_status_filter() {
    let mut journal = Journal::default();
    let collection = journal.add_collection("독서").unwrap();
    let open = journal.add_entry(day("2026-10-02"), Log::Daily, Kind::Task, "한글 책 📚").unwrap();
    let complete = journal.add_entry(day("2026-11-01"), Log::Future, Kind::Task, "책 읽기").unwrap();
    let note = journal
        .add_entry(day("2026-09-01"), Log::Collection(collection), Kind::Note, "책 REVIEW")
        .unwrap();
    journal.set_status(complete, Status::Complete).unwrap();
    assert_eq!(
        journal.search(" 책 ", Filter::All).iter().map(|e| e.id).collect::<Vec<_>>(),
        [open, complete, note]
    );
    assert_eq!(journal.search("책", Filter::Open).iter().map(|e| e.id).collect::<Vec<_>>(), [open, note]);
    assert_eq!(journal.search("책", Filter::Complete).iter().map(|e| e.id).collect::<Vec<_>>(), [complete]);
    assert_eq!(journal.search("review", Filter::All)[0].id, note);
    assert_eq!(journal.search("📚", Filter::All)[0].id, open);
    assert!(journal.search("없는 기록", Filter::All).is_empty());
}
