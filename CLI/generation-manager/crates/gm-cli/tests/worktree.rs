//! End-to-end tests for `gm init` and the worktree commands, which are the only
//! ones that need a real git repository behind them.

mod common;

use common::{Fixture,
             Manifest,
             stderr,
             stdout};

/// A project backed by a git repository with one commit, which
/// `git worktree add` requires.
fn repo() -> Fixture {
    let fixture = Fixture::with(Manifest::new("true"));
    fixture.git_init();
    fixture
}

#[test]
fn init_detects_the_project_type() {
    let fixture = Fixture::bare();
    fixture.write("Cargo.toml", "[package]\nname = \"svc\"\n");

    let output = fixture.gm_ok(&["init"]);
    assert!(stdout(&output).contains("wrote"), "stdout: {}", stdout(&output));

    let manifest = fixture.read("generation-manager.toml");
    assert!(manifest.contains("cargo build --release"), "manifest: {manifest}");
    assert!(manifest.contains("cargo test"), "manifest: {manifest}");
}

#[test]
fn init_accepts_an_explicit_preset_and_name() {
    let fixture = Fixture::bare();

    fixture.gm_ok(&["init", "--preset", "node", "--name", "checkout"]);

    let manifest = fixture.read("generation-manager.toml");
    assert!(manifest.contains("npm ci"), "manifest: {manifest}");
    assert!(manifest.contains("name = \"checkout\""), "manifest: {manifest}");
}

#[test]
fn init_rejects_an_unknown_preset() {
    let fixture = Fixture::bare();

    let output = fixture.gm(&["init", "--preset", "cobol"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown preset"), "stderr: {}", stderr(&output));
}

#[test]
fn init_will_not_overwrite_an_existing_manifest_by_accident() {
    let fixture = Fixture::with(Manifest::new("true"));

    let refused = fixture.gm(&["init"]);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("--force"), "stderr: {}", stderr(&refused));
    // The original manifest survived.
    assert!(fixture.read("generation-manager.toml").contains("stop_timeout_secs"));

    fixture.gm_ok(&["init", "--force", "--preset", "go", "--name", "svc"]);
    assert!(fixture.read("generation-manager.toml").contains("go build"));
}

#[test]
fn dev_new_creates_a_worktree_on_its_own_branch() {
    let fixture = repo();

    let output = fixture.gm_ok(&["dev", "new", "add-cache"]);
    assert!(stdout(&output).contains("worktree `add-cache`"), "stdout: {}", stdout(&output));

    let worktree = fixture.worktree_dir("add-cache");
    assert!(worktree.join("generation-manager.toml").is_file(), "the checkout is populated");

    let branch = fixture.git(&worktree, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(String::from_utf8_lossy(&branch.stdout).trim(), "add-cache");
}

#[test]
fn dev_new_rejects_a_name_already_in_use() {
    let fixture = repo();
    fixture.gm_ok(&["dev", "new", "add-cache"]);

    let output = fixture.gm(&["dev", "new", "add-cache"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("worktree `add-cache` already exists"), "stderr: {}", stderr(&output));
}

#[test]
fn dev_new_requires_a_git_repository() {
    let fixture = Fixture::with(Manifest::new("true"));

    let output = fixture.gm(&["dev", "new", "add-cache"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("is not a git repository"), "stderr: {}", stderr(&output));
}

#[test]
fn dev_list_marks_the_worktree_you_are_standing_in() {
    let fixture = repo();
    fixture.gm_ok(&["dev", "new", "add-cache"]);
    fixture.gm_ok(&["dev", "new", "fix-login"]);

    let from_root = stdout(&fixture.gm_ok(&["dev", "list"]));
    assert!(from_root.contains("add-cache"), "list: {from_root}");
    assert!(from_root.contains("fix-login"), "list: {from_root}");
    assert!(!from_root.contains('*'), "nothing is current from the root: {from_root}");

    let worktree = fixture.worktree_dir("fix-login");
    let from_worktree = stdout(&fixture.gm_in(&worktree, &["dev", "list"]));
    let marked: Vec<&str> = from_worktree.lines().filter(|line| line.starts_with('*')).collect();
    assert_eq!(marked.len(), 1, "list: {from_worktree}");
    assert!(marked[0].contains("fix-login"), "list: {from_worktree}");
}

#[test]
fn dev_rm_removes_the_worktree() {
    let fixture = repo();
    fixture.gm_ok(&["dev", "new", "add-cache"]);

    let output = fixture.gm_ok(&["dev", "rm", "add-cache"]);
    assert!(stdout(&output).contains("removed worktree"), "stdout: {}", stdout(&output));
    assert!(!fixture.worktree_dir("add-cache").exists());

    let missing = fixture.gm(&["dev", "rm", "add-cache"]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("worktree `add-cache` does not exist"), "stderr: {}", stderr(&missing));
}

#[test]
fn a_worktree_never_gets_a_state_directory_of_its_own() {
    // The manifest is committed, so the worktree carries a copy of it. Commands
    // run from inside must still resolve to the real project root.
    let fixture = repo();
    fixture.gm_ok(&["dev", "new", "add-cache"]);
    fixture.gm_ok(&["build"]);
    let worktree = fixture.worktree_dir("add-cache");

    let status = stdout(&fixture.gm_in(&worktree, &["status"]));

    assert!(status.contains("add-cache (you are here)"), "status: {status}");
    assert!(status.contains("1 generation(s)"), "the root's generations are visible: {status}");
    assert!(status.contains(fixture.root.canonicalize().unwrap().to_str().unwrap()), "status: {status}");
    assert!(!worktree.join(".gm").exists(), "a second state directory appeared");
}

#[test]
fn building_inside_a_worktree_needs_no_target_flag() {
    let fixture = repo();
    fixture.gm_ok(&["dev", "new", "add-cache"]);
    let worktree = fixture.worktree_dir("add-cache");

    let output = fixture.gm_in(&worktree, &["build"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("building from worktree `add-cache`"), "stdout: {out}");
    // The generation records where it came from.
    assert!(out.contains("(add-cache)"), "stdout: {out}");
    // And it was built there, not in the project root.
    assert!(worktree.join("built.marker").exists());
    assert!(!fixture.path("built.marker").exists());
}

#[test]
fn building_from_the_root_targets_the_root() {
    let fixture = repo();
    fixture.gm_ok(&["dev", "new", "add-cache"]);

    let out = stdout(&fixture.gm_ok(&["build"]));

    assert!(out.contains("building from project root"), "stdout: {out}");
    assert!(fixture.path("built.marker").exists());
}
