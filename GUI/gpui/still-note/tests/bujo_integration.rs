use std::fs;
use stillnote::{Journal, JournalStore, Kind, Log, Session, Status, parse_date};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn ac07_empty_first_run_does_not_seed_or_write_until_user_mutation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let session = Session::open(&path).unwrap();
    assert!(session.journal.entries.is_empty() && session.journal.collections.is_empty());
    assert!(!path.exists());
}

#[test]
fn ac05_ac06_ac07_real_json_roundtrip_collections_status_unicode_and_links() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested/journal.json");
    let mut session = Session::open(&path).unwrap();
    let collection = session.transact(|j| j.add_collection("독서 📚")).unwrap();
    let original = session
        .transact(|j| j.add_entry(parse_date("2024-02-29")?, Log::Daily, Kind::Task, "한글 café 📝"))
        .unwrap();
    session.transact(|j| j.toggle_important(original)).unwrap();
    let target = session.transact(|j| j.migrate(original, parse_date("2025-01-01")?, Log::Future)).unwrap();
    session.transact(|j| j.set_status(target, Status::Complete)).unwrap();
    session
        .transact(|j| j.add_entry(parse_date("2026-10-02")?, Log::Collection(collection), Kind::Note, "읽은 책"))
        .unwrap();
    session
        .transact(|j| j.add_entry(parse_date("2026-10-03")?, Log::Monthly, Kind::Event, "생일 ○"))
        .unwrap();
    let expected = session.journal.clone();
    drop(session);
    let reopened = Session::open(&path).unwrap();
    assert_eq!(reopened.journal, expected);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["version"], 1);
    assert!(reopened.store.backup_path().exists());
    assert_eq!(reopened.journal.entry(original).unwrap().migrated_to, Some(target));
    assert_eq!(reopened.journal.entry(target).unwrap().migrated_from, Some(original));
}

#[test]
fn ac07_backup_is_exact_previous_durable_snapshot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let mut session = Session::open(&path).unwrap();
    session
        .transact(|j| j.add_entry(parse_date("2026-10-02")?, Log::Daily, Kind::Note, "first"))
        .unwrap();
    let previous = fs::read(&path).unwrap();
    session.transact(|j| j.add_collection("next")).unwrap();
    assert_eq!(fs::read(session.store.backup_path()).unwrap(), previous);
    assert_ne!(fs::read(&path).unwrap(), previous);
}

#[test]
fn ac08_corrupt_json_load_blocks_save_and_preserves_original_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let corrupt = b"{\"version\":1,\"entries\":[broken";
    fs::write(&path, corrupt).unwrap();
    let mut store = JournalStore::new(&path);
    assert!(store.load().is_err());
    assert!(store.save(&Journal::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), corrupt);
    assert!(!store.backup_path().exists());
}

fn assert_invalid_preserved(value: serde_json::Value) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let bytes = serde_json::to_vec_pretty(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    let mut store = JournalStore::new(&path);
    assert!(store.load().is_err(), "invalid journal was accepted: {value}");
    assert!(store.save(&Journal::default()).is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn ac08_semantic_json_validation_rejects_version_ids_date_and_kind_status() {
    let mut journal = Journal::default();
    journal.add_entry(parse_date("2026-10-02").unwrap(), Log::Daily, Kind::Note, "note").unwrap();
    let baseline = serde_json::to_value(&journal).unwrap();
    let mut value = baseline.clone();
    value["version"] = 999.into();
    assert_invalid_preserved(value);
    let mut value = baseline.clone();
    value["entries"][0]["id"] = Uuid::nil().to_string().into();
    assert_invalid_preserved(value);
    let mut value = baseline.clone();
    let duplicate = value["entries"][0].clone();
    value["entries"].as_array_mut().unwrap().push(duplicate);
    assert_invalid_preserved(value);
    let mut value = baseline.clone();
    value["entries"][0]["date"] = "2023-02-29".into();
    assert_invalid_preserved(value);
    let mut value = baseline.clone();
    value["entries"][0]["status"] = "Complete".into();
    assert_invalid_preserved(value);
    let mut value = baseline;
    value["entries"][0]["text"] = " \t".into();
    assert_invalid_preserved(value);
}

#[test]
fn ac08_semantic_json_validation_rejects_missing_collection_and_duplicate_names() {
    let mut journal = Journal::default();
    let id = journal.add_collection("독서").unwrap();
    journal
        .add_entry(parse_date("2026-10-02").unwrap(), Log::Collection(id), Kind::Note, "책")
        .unwrap();
    let mut value = serde_json::to_value(&journal).unwrap();
    value["collections"] = serde_json::json!([]);
    assert_invalid_preserved(value);
    let mut value = serde_json::to_value(journal).unwrap();
    value["collections"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":Uuid::new_v4(),"name":"독서"}));
    assert_invalid_preserved(value);
}

#[test]
fn ac06_ac08_semantic_migration_links_must_be_reciprocal_and_status_consistent() {
    let mut journal = Journal::default();
    let id = journal.add_entry(parse_date("2026-10-02").unwrap(), Log::Daily, Kind::Task, "task").unwrap();
    journal.migrate(id, parse_date("2026-11-01").unwrap(), Log::Future).unwrap();
    let baseline = serde_json::to_value(journal).unwrap();
    let mut value = baseline.clone();
    value["entries"][1]["migrated_from"] = serde_json::Value::Null;
    assert_invalid_preserved(value);
    let mut value = baseline.clone();
    value["entries"][0]["migrated_to"] = Uuid::new_v4().to_string().into();
    assert_invalid_preserved(value);
    let mut value = baseline.clone();
    value["entries"][0]["status"] = "Open".into();
    assert_invalid_preserved(value);
    let mut value = baseline;
    value["entries"][0]["status"] = "Migrated".into();
    assert_invalid_preserved(value);
}

#[test]
fn ac06_ac08_serialized_migration_into_collection_is_rejected_and_preserved() {
    let mut journal = Journal::default();
    let collection = journal.add_collection("개인 프로젝트").unwrap();
    let source = journal
        .add_entry(parse_date("2026-10-02").unwrap(), Log::Daily, Kind::Task, "다음으로 가져갈 일")
        .unwrap();
    let target = journal.migrate(source, parse_date("2026-11-01").unwrap(), Log::Monthly).unwrap();
    // Start with a valid reciprocal edge, then corrupt only its effective target
    // location. Matching references alone do not make a legal migration.
    journal.entries.iter_mut().find(|entry| entry.id == target).unwrap().log = Log::Collection(collection);
    assert!(
        journal.validate().is_err(),
        "load validation must match the constructor's prohibited target policy"
    );
    assert_invalid_preserved(serde_json::to_value(journal).unwrap());
}

#[test]
fn ac08_failed_backup_save_retains_file_model_and_cleans_temporary_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let mut session = Session::open(&path).unwrap();
    session
        .transact(|j| j.add_entry(parse_date("2026-10-02")?, Log::Daily, Kind::Task, "original"))
        .unwrap();
    let model = session.journal.clone();
    let bytes = fs::read(&path).unwrap();
    fs::create_dir(session.store.backup_path()).unwrap();
    assert!(session.transact(|j| j.add_collection("should rollback")).is_err());
    assert_eq!(session.journal, model);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(
        !fs::read_dir(dir.path())
            .unwrap()
            .any(|entry| { entry.unwrap().file_name().to_string_lossy().ends_with(".tmp") })
    );
}

#[test]
fn ac08_external_edits_cannot_be_overwritten_by_stale_session() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let mut session = Session::open(&path).unwrap();
    session.transact(|j| j.add_collection("original")).unwrap();
    let model = session.journal.clone();
    let external = b"external bytes preserved";
    fs::write(&path, external).unwrap();
    assert!(session.transact(|j| j.add_collection("new")).is_err());
    assert_eq!(session.journal, model);
    assert_eq!(fs::read(path).unwrap(), external);
}

#[test]
fn ac08_operation_error_does_not_partially_commit_model_or_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("journal.json");
    let mut session = Session::open(&path).unwrap();
    session.transact(|j| j.add_collection("before")).unwrap();
    let model = session.journal.clone();
    let bytes = fs::read(&path).unwrap();
    let failed: anyhow::Result<()> = session.transact(|j| {
        j.add_collection("partial")?;
        anyhow::bail!("operation failed")
    });
    assert!(failed.is_err());
    assert_eq!(session.journal, model);
    assert_eq!(fs::read(path).unwrap(), bytes);
}
