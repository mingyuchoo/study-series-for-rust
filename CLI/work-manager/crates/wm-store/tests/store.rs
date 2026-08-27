//! Behaviour tests for the activation pointer: these cover the invariants the
//! rest of the tool relies on — numbering, atomic switching, rollback target
//! selection, and GC never eating something you still need.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::Utc;
use wm_core::generation::{Generation, GenerationId, GenerationStatus};
use wm_store::{Layout, Store};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> TempProject {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("wm-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempProject(path)
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn add_generation(store: &Store, commit: &str) -> GenerationId {
    let staged = store.stage(Some(commit)).unwrap();
    std::fs::write(staged.payload.join("marker"), commit).unwrap();
    let id = staged.id;
    let meta = Generation {
        id,
        commit: Some(commit.to_string()),
        worktree: None,
        dirty: false,
        built_at: Utc::now(),
        status: GenerationStatus::Built,
        artifacts: vec![PathBuf::from("marker")],
        note: None,
    };
    store.commit(staged, meta).unwrap();
    id
}

#[test]
fn generations_are_numbered_from_one() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();

    assert_eq!(add_generation(&store, "aaaaaaa"), GenerationId(1));
    assert_eq!(add_generation(&store, "bbbbbbb"), GenerationId(2));
    assert_eq!(store.list().unwrap().len(), 2);
}

#[test]
fn nothing_is_active_before_the_first_switch() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();
    add_generation(&store, "aaaaaaa");

    assert_eq!(store.current_id().unwrap(), None);
    assert!(store.current().is_err());
}

#[test]
fn switching_moves_the_current_pointer_and_records_history() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();
    let first = add_generation(&store, "aaaaaaa");
    let second = add_generation(&store, "bbbbbbb");

    store.switch(first, "test").unwrap();
    assert_eq!(store.current_id().unwrap(), Some(first));

    store.switch(second, "test").unwrap();
    assert_eq!(store.current_id().unwrap(), Some(second));
    // The payload really follows the pointer.
    assert_eq!(
        std::fs::read_to_string(store.current().unwrap().payload().join("marker")).unwrap(),
        "bbbbbbb"
    );

    let history = store.history().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].from, Some(first));
    assert_eq!(history[1].to, second);
}

#[test]
fn rollback_target_is_the_highest_generation_below_current() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();
    let first = add_generation(&store, "aaaaaaa");
    let second = add_generation(&store, "bbbbbbb");
    let third = add_generation(&store, "ccccccc");

    store.switch(third, "test").unwrap();
    assert_eq!(store.rollback_target().unwrap(), second);

    store.switch(second, "test").unwrap();
    assert_eq!(store.rollback_target().unwrap(), first);

    store.switch(first, "test").unwrap();
    assert!(store.rollback_target().is_err());
}

#[test]
fn switching_to_a_missing_generation_leaves_current_untouched() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();
    let first = add_generation(&store, "aaaaaaa");
    store.switch(first, "test").unwrap();

    assert!(store.switch(GenerationId(99), "test").is_err());
    assert_eq!(store.current_id().unwrap(), Some(first));
}

#[test]
fn gc_keeps_the_active_generation_and_its_rollback_target() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();
    let first = add_generation(&store, "aaaaaaa");
    let second = add_generation(&store, "bbbbbbb");
    let third = add_generation(&store, "ccccccc");
    let fourth = add_generation(&store, "ddddddd");

    store.switch(fourth, "test").unwrap();
    let removed = store.gc(2).unwrap();

    assert_eq!(removed, vec![first, second]);
    let remaining: Vec<_> = store.list().unwrap().into_iter().map(|e| e.meta.id).collect();
    assert_eq!(remaining, vec![third, fourth]);
    assert_eq!(store.current_id().unwrap(), Some(fourth));
}

#[test]
fn a_second_lock_is_refused_while_the_first_is_held() {
    let temp = TempProject::new();
    let store = Store::open(Layout::new(&temp.0)).unwrap();

    let held = store.lock().unwrap();
    assert!(store.lock().is_err());
    drop(held);
    assert!(store.lock().is_ok());
}
