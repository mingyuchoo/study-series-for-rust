//! End-to-end tests for the generation lifecycle: `wm build`, `wm switch`,
//! `wm rollback`, `wm generations`, `wm history` and `wm gc`.

mod common;

use common::{Fixture, Manifest, stderr, stdout};
use wm_core::generation::GenerationId;
use wm_core::run::RunSource;

/// A project whose generation content is decided by `source.txt` at build time,
/// and whose health check reads that content back out of the frozen payload.
/// That lets a test choose, per generation, whether activation should succeed.
fn project() -> Fixture {
    let fixture = Fixture::with(
        Manifest::new("exec sleep 300")
            .build("cp source.txt payload.txt")
            .artifacts(&["payload.txt"])
            .health("grep -q good payload.txt"),
    );
    fixture.write("source.txt", "good\n");
    fixture
}

#[test]
fn build_creates_a_generation_without_activating_it() {
    let fixture = project();

    let output = fixture.wm_ok(&["build"]);
    assert!(stdout(&output).contains("generation 1 built"), "stdout: {}", stdout(&output));

    let listed = stdout(&fixture.wm_ok(&["generations"]));
    assert!(listed.contains("built"), "generations: {listed}");
    // Nothing is marked active, and nothing is running.
    assert!(!listed.contains('*'), "generations: {listed}");
    assert!(fixture.run_state().is_none());
}

#[test]
fn a_failing_test_stage_consumes_no_generation_number() {
    let fixture = Fixture::with(Manifest::new("true").test("exit 1"));

    let failed = fixture.wm(&["build"]);
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("`test` failed"), "stderr: {}", stderr(&failed));

    let listed = stdout(&fixture.wm_ok(&["generations"]));
    assert!(listed.contains("no generations yet"), "generations: {listed}");

    // The next successful build still gets number 1.
    let ok = Fixture::with(Manifest::new("true"));
    assert!(stdout(&ok.wm_ok(&["build"])).contains("generation 1 built"));
}

#[test]
fn a_missing_artifact_creates_no_generation() {
    let fixture = Fixture::with(Manifest::new("true").artifacts(&["never-produced"]));

    let failed = fixture.wm(&["build"]);
    assert!(!failed.status.success());
    assert!(
        stderr(&failed).contains("does not exist after the build"),
        "stderr: {}",
        stderr(&failed)
    );
    assert!(stdout(&fixture.wm_ok(&["generations"])).contains("no generations yet"));
}

#[test]
fn switch_activates_the_newest_generation_and_marks_it_healthy() {
    let fixture = project();
    fixture.wm_ok(&["build"]);

    let switched = fixture.wm_ok(&["switch"]);
    assert!(stdout(&switched).contains("generation 1 is live"), "stdout: {}", stdout(&switched));

    let listed = stdout(&fixture.wm_ok(&["generations"]));
    assert!(listed.contains("healthy"), "generations: {listed}");
    assert!(listed.contains('*'), "generations: {listed}");
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation { id: GenerationId(1) }
    );
}

#[test]
fn switching_to_an_unknown_generation_fails() {
    let fixture = project();
    fixture.wm_ok(&["build"]);

    let output = fixture.wm(&["switch", "--gen", "99"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("generation 99 does not exist"), "stderr: {}", stderr(&output));
}

#[test]
fn an_older_generation_keeps_the_artifacts_it_was_built_with() {
    let fixture = project();
    fixture.wm_ok(&["build"]);

    // Later development must not reach back into a frozen generation.
    fixture.write("source.txt", "good, but changed\n");
    fixture.wm_ok(&["build"]);

    assert_eq!(std::fs::read_to_string(fixture.generation_payload(1).join("payload.txt")).unwrap(), "good\n");
    assert_eq!(
        std::fs::read_to_string(fixture.generation_payload(2).join("payload.txt")).unwrap(),
        "good, but changed\n"
    );
}

#[test]
fn rollback_returns_to_the_previous_generation() {
    let fixture = project();
    fixture.wm_ok(&["build", "--switch"]);
    fixture.write("source.txt", "good, second\n");
    fixture.wm_ok(&["build", "--switch"]);

    let rolled = fixture.wm_ok(&["rollback"]);
    assert!(stdout(&rolled).contains("generation 1 is live"), "stdout: {}", stdout(&rolled));
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation { id: GenerationId(1) }
    );

    // Rolling back again from generation 1 has nowhere to go.
    let exhausted = fixture.wm(&["rollback"]);
    assert!(!exhausted.status.success());
    assert!(
        stderr(&exhausted).contains("no older generation"),
        "stderr: {}",
        stderr(&exhausted)
    );
}

#[test]
fn rollback_can_target_a_specific_generation() {
    let fixture = project();
    for _ in 0..3 {
        fixture.wm_ok(&["build"]);
    }
    fixture.wm_ok(&["switch", "--gen", "3"]);

    fixture.wm_ok(&["rollback", "--to", "1"]);

    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation { id: GenerationId(1) }
    );
}

#[test]
fn a_failed_health_check_rolls_back_automatically() {
    let fixture = project();
    fixture.wm_ok(&["build", "--switch"]);

    // The next generation builds and tests fine but fails verification.
    fixture.write("source.txt", "bad\n");
    let failed = fixture.wm(&["build", "--switch"]);

    assert!(!failed.status.success(), "a rejected generation must not report success");
    let err = stderr(&failed);
    assert!(err.contains("generation 2 failed verification"), "stderr: {err}");
    assert!(err.contains("rolled back to generation 1, which is live again"), "stderr: {err}");

    let listed = stdout(&fixture.wm_ok(&["generations"]));
    assert!(listed.contains("rejected"), "generations: {listed}");
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation { id: GenerationId(1) },
        "the previous generation must be serving again"
    );
}

#[test]
fn history_records_every_switch_with_its_reason() {
    let fixture = project();
    fixture.wm_ok(&["build", "--switch"]);
    fixture.write("source.txt", "good, second\n");
    fixture.wm_ok(&["build", "--switch"]);
    fixture.wm_ok(&["rollback"]);

    let history = stdout(&fixture.wm_ok(&["history"]));
    let lines: Vec<&str> = history.lines().collect();

    assert_eq!(lines.len(), 3, "history: {history}");
    assert!(lines[0].contains("- → 1") && lines[0].contains("wm build --switch"), "{}", lines[0]);
    assert!(lines[1].contains("1 → 2"), "{}", lines[1]);
    assert!(lines[2].contains("2 → 1") && lines[2].contains("wm rollback"), "{}", lines[2]);
}

#[test]
fn gc_keeps_the_active_generation_and_its_rollback_target() {
    let fixture = project();
    for _ in 0..4 {
        fixture.wm_ok(&["build"]);
    }
    fixture.wm_ok(&["switch", "--gen", "4"]);

    let collected = fixture.wm_ok(&["gc", "--keep", "2"]);
    assert!(
        stdout(&collected).contains("removed generation(s) 1, 2"),
        "stdout: {}",
        stdout(&collected)
    );

    let listed = stdout(&fixture.wm_ok(&["generations"]));
    assert!(!listed.contains(" 1 ") && !listed.contains(" 2 "), "generations: {listed}");
    assert!(listed.contains(" 3 ") && listed.contains(" 4 "), "generations: {listed}");
    // The store directories are gone too, not just the numbered links.
    assert!(!fixture.path(".wm/generations/0001").exists());
    assert_eq!(std::fs::read_dir(fixture.path(".wm/store")).unwrap().count(), 2);
}

#[test]
fn gc_never_removes_the_last_generations_when_asked_to_keep_none() {
    let fixture = project();
    fixture.wm_ok(&["build", "--switch"]);

    fixture.wm_ok(&["gc", "--keep", "0"]);

    assert!(fixture.path(".wm/generations/0001").exists());
}

#[test]
fn a_note_is_stored_with_the_generation() {
    let fixture = project();

    fixture.wm_ok(&["build", "--note", "tuned the cache"]);

    let listed = stdout(&fixture.wm_ok(&["generations"]));
    assert!(listed.contains("tuned the cache"), "generations: {listed}");
}
