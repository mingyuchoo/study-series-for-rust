// Import authoritative desktop serialization and validation without building GPUI.
#[path = "../../../../../../desktop-app/src/model.rs"]
mod model;
#[path = "../../../../../../desktop-app/src/i18n.rs"]
mod i18n;
mod settings {
    #[derive(Clone, Copy, PartialEq)]
    pub enum Language { Korean, English }
}
use model::{Collection, Entry, Journal, Kind, Log, Status, parse_date};
use uuid::Uuid;

fn id(n: u128) -> Uuid { Uuid::from_u128(n) }
fn fixture() -> Journal {
    let mut journal = Journal { version: 1, entries: vec![], collections: vec![Collection { id: id(100), name: "독서 📚".into() }] };
    for (n, date, log, kind, status, text) in [
        (1, "2024-02-29", Log::Daily, Kind::Task, Status::Scheduled, "한글 café 📝"),
        (2, "2025-01-01", Log::Future, Kind::Task, Status::Complete, "한글 café 📝"),
        (3, "2026-10-02", Log::Collection(id(100)), Kind::Note, Status::Open, "읽은 책"),
        (4, "2026-10-03", Log::Monthly, Kind::Event, Status::Cancelled, "생일 ○"),
        (5, "0001-01-01", Log::Daily, Kind::Task, Status::Migrated, "경계"),
        (6, "9999-12-31", Log::Monthly, Kind::Task, Status::Open, "경계"),
    ] {
        journal.entries.push(Entry { id: id(n), date: parse_date(date).unwrap(), log, kind, status,
            important: n <= 2, text: text.into(),
            migrated_from: match n { 2 => Some(id(1)), 6 => Some(id(5)), _ => None },
            migrated_to: match n { 1 => Some(id(2)), 5 => Some(id(6)), _ => None },
        });
    }
    journal.validate().unwrap();
    journal
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let path = args.get(2).expect("usage: generate|validate PATH");
    match args.get(1).map(String::as_str) {
        Some("generate") => std::fs::write(path, serde_json::to_vec_pretty(&fixture())?)?,
        Some("validate") => {
            let journal: Journal = serde_json::from_slice(&std::fs::read(path)?)?;
            journal.validate()?;
            anyhow::ensure!(journal == fixture(), "Kotlin output changed authoritative fixture values");
            println!("Desktop model validated all fixture values and migration links");
        },
        _ => anyhow::bail!("unknown command"),
    }
    Ok(())
}
