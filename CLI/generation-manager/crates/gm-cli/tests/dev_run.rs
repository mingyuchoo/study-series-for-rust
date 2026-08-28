//! End-to-end tests for `gm worktree run`.
//!
//! These cover the contract the command promises: it runs the worktree's own
//! code, in the foreground, with the build stage but no verification, and it
//! takes the single service slot from whoever held it — recording itself as the
//! source so `gm project status` can still say what is running.

mod common;

use common::{Fixture,
             Manifest,
             stderr,
             stdout};
use gm_core::{generation::GenerationId,
              run::RunSource};

const WORKTREE: &str = "add-cache";

/// A project with one worktree that needs no git behind it: `gm worktree run`
/// only requires the directory to exist, so the fixtures stay fast and
/// hermetic.
fn project(run_cmd: &str) -> Fixture {
    let fixture = Fixture::new(run_cmd);
    fixture.fake_worktree(WORKTREE);
    fixture
}

#[test]
fn runs_the_worktree_code_in_the_foreground() {
    let fixture = project("cat worktree.marker");
    std::fs::write(fixture.worktree_dir(WORKTREE).join("worktree.marker"), "hello from the worktree").unwrap();

    let output = fixture.gm(&["worktree", "run", WORKTREE]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    // The child inherits stdio, so its output really is the command's output.
    assert!(stdout(&output).contains("hello from the worktree"), "stdout: {}", stdout(&output));
}

#[test]
fn reports_the_exit_code_of_the_code_it_ran() {
    let fixture = project("exit 7");

    let output = fixture.gm(&["worktree", "run", WORKTREE]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("exited with code 7"), "stderr: {}", stderr(&output));
}

#[test]
fn runs_the_build_stage_first() {
    let fixture = project("cat built.marker");

    let output = fixture.gm(&["worktree", "run", WORKTREE]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("built"), "stdout: {}", stdout(&output));
    // The build ran in the worktree, not in the project root.
    assert!(fixture.worktree_dir(WORKTREE).join("built.marker").exists());
    assert!(!fixture.path("built.marker").exists());
}

#[test]
fn no_build_skips_the_build_stage() {
    let fixture = project("test -f built.marker");

    let output = fixture.gm(&["worktree", "run", WORKTREE, "--no-build"]);

    assert!(!output.status.success(), "the artifact should not exist yet");
    assert!(!fixture.worktree_dir(WORKTREE).join("built.marker").exists());
}

#[test]
fn defaults_to_the_worktree_you_are_standing_in() {
    let fixture = project("pwd");
    let worktree = fixture.worktree_dir(WORKTREE);

    let output = fixture.gm_in(&worktree, &["worktree", "run"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains(&format!("running worktree `{WORKTREE}`")), "stdout: {out}");
    // The process really ran with the worktree as its working directory.
    let printed = worktree.canonicalize().unwrap();
    assert!(out.contains(printed.to_str().unwrap()), "stdout: {out}");
}

#[test]
fn asks_for_a_target_when_run_outside_a_worktree() {
    let fixture = project("true");

    let output = fixture.gm(&["worktree", "run"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("not inside a worktree"), "stderr: {}", stderr(&output));
}

#[test]
fn rejects_an_unknown_worktree() {
    let fixture = project("true");

    let output = fixture.gm(&["worktree", "run", "no-such-worktree"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("worktree `no-such-worktree` does not exist"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn a_detached_run_records_the_worktree_as_the_running_source() {
    let fixture = project("exec sleep 300");

    let output = fixture.gm(&["worktree", "run", WORKTREE, "--detach"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let state = fixture.run_state().expect("a detached run must record its state");
    assert_eq!(
        state.source,
        RunSource::Worktree {
            name: WORKTREE.to_string()
        }
    );
    assert!(state.detached);
    assert!(state.source.is_dev());

    // `gm project status` names the source rather than only reporting that a pid
    // exists.
    let status = stdout(&fixture.gm(&["project", "status"]));
    assert!(status.contains("worktree `add-cache` (dev, unverified)"), "status: {status}");

    let stopped = fixture.gm(&["service", "stop"]);
    assert!(stopped.status.success(), "stderr: {}", stderr(&stopped));
    assert!(stdout(&stopped).contains("worktree `add-cache`"));
}

#[test]
fn a_foreground_run_frees_the_slot_when_it_exits() {
    let fixture = project("true");

    fixture.gm(&["worktree", "run", WORKTREE]);

    // The state file is cleared, so nothing claims to be running afterwards.
    assert!(fixture.run_state().is_none());
    assert!(stdout(&fixture.gm(&["project", "status"])).contains("stopped"));
}

#[test]
fn skips_the_test_stage() {
    // A worktree run is for code that may not work yet; failing tests must not
    // stand in the way.
    let fixture = Fixture::with(Manifest::new("true").test("exit 1"));
    fixture.fake_worktree(WORKTREE);

    let output = fixture.gm(&["worktree", "run", WORKTREE]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
}

#[test]
fn a_dev_run_and_a_generation_hand_the_slot_back_and_forth() {
    let fixture = project("exec sleep 300");

    let built = fixture.gm_ok(&["generation", "build", "--activate"]);
    assert!(built.status.success());
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        }
    );

    // Taking the slot for development is announced, not silent.
    let taken = fixture.gm_ok(&["worktree", "run", WORKTREE, "--detach"]);
    assert!(
        stdout(&taken).contains("stopping generation 1 to take the service slot"),
        "stdout: {}",
        stdout(&taken)
    );
    assert!(fixture.run_state().unwrap().source.is_dev());

    // And so is taking it back.
    let restored = fixture.gm_ok(&["generation", "activate", "1"]);
    assert!(stdout(&restored).contains("stopping worktree `add-cache`"), "stdout: {}", stdout(&restored));
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        }
    );
}
