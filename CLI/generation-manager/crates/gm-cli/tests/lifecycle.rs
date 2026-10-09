//! End-to-end tests for `gm generation` commands.

mod common;

use common::{Fixture,
             Manifest,
             stderr,
             stdout};
use gm_core::{generation::GenerationId,
              run::RunSource};

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

    let output = fixture.gm_ok(&["generation", "build"]);
    assert!(stdout(&output).contains("generation 1 built"), "stdout: {}", stdout(&output));

    let listed = stdout(&fixture.gm_ok(&["generation", "list"]));
    assert!(listed.contains("built"), "generations: {listed}");
    // Nothing is marked active, and nothing is running.
    assert!(!listed.contains('*'), "generations: {listed}");
    assert!(fixture.run_state().is_none());
}

#[test]
fn a_failing_test_stage_consumes_no_generation_number() {
    let fixture = Fixture::with(Manifest::new("true").test("exit 1"));

    let failed = fixture.gm(&["generation", "build"]);
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("`test` failed"), "stderr: {}", stderr(&failed));

    let listed = stdout(&fixture.gm_ok(&["generation", "list"]));
    assert!(listed.contains("no generations yet"), "generations: {listed}");

    // The next successful build still gets number 1.
    let ok = Fixture::with(Manifest::new("true"));
    assert!(stdout(&ok.gm_ok(&["generation", "build"])).contains("generation 1 built"));
}

#[test]
fn a_missing_artifact_creates_no_generation() {
    let fixture = Fixture::with(Manifest::new("true").artifacts(&["never-produced"]));

    let failed = fixture.gm(&["generation", "build"]);
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("does not exist after the build"), "stderr: {}", stderr(&failed));
    assert!(stdout(&fixture.gm_ok(&["generation", "list"])).contains("no generations yet"));
}

#[test]
fn activate_uses_the_newest_generation_and_marks_it_healthy() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build"]);

    let switched = fixture.gm_ok(&["generation", "activate"]);
    assert!(stdout(&switched).contains("generation 1 is live"), "stdout: {}", stdout(&switched));

    let listed = stdout(&fixture.gm_ok(&["generation", "list"]));
    assert!(listed.contains("healthy"), "generations: {listed}");
    assert!(listed.contains('*'), "generations: {listed}");
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        }
    );
}

#[test]
fn activating_an_unknown_generation_fails() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build"]);

    let output = fixture.gm(&["generation", "activate", "99"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("generation 99 does not exist"), "stderr: {}", stderr(&output));
}

#[test]
fn an_older_generation_keeps_the_artifacts_it_was_built_with() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build"]);

    // Later development must not reach back into a frozen generation.
    fixture.write("source.txt", "good, but changed\n");
    fixture.gm_ok(&["generation", "build"]);

    assert_eq!(std::fs::read_to_string(fixture.generation_payload(1).join("payload.txt")).unwrap(), "good\n");
    assert_eq!(
        std::fs::read_to_string(fixture.generation_payload(2).join("payload.txt")).unwrap(),
        "good, but changed\n"
    );
}

#[test]
fn rollback_returns_to_the_previous_generation() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build", "--activate"]);
    fixture.write("source.txt", "good, second\n");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    let rolled = fixture.gm_ok(&["generation", "rollback"]);
    assert!(stdout(&rolled).contains("generation 1 is live"), "stdout: {}", stdout(&rolled));
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        }
    );

    // Rolling back again from generation 1 has nowhere to go.
    let exhausted = fixture.gm(&["generation", "rollback"]);
    assert!(!exhausted.status.success());
    assert!(stderr(&exhausted).contains("no older generation"), "stderr: {}", stderr(&exhausted));
}

#[test]
fn rollback_can_target_a_specific_generation() {
    let fixture = project();
    for _ in 0 .. 3 {
        fixture.gm_ok(&["generation", "build"]);
    }
    fixture.gm_ok(&["generation", "activate", "3"]);

    fixture.gm_ok(&["generation", "rollback", "1"]);

    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        }
    );
}

#[test]
fn a_failed_health_check_rolls_back_automatically() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build", "--activate"]);

    // The next generation builds and tests fine but fails verification.
    fixture.write("source.txt", "bad\n");
    let failed = fixture.gm(&["generation", "build", "--activate"]);

    assert!(!failed.status.success(), "a rejected generation must not report success");
    let err = stderr(&failed);
    assert!(err.contains("generation 2 failed verification"), "stderr: {err}");
    assert!(err.contains("rolled back to generation 1, which is live again"), "stderr: {err}");

    let listed = stdout(&fixture.gm_ok(&["generation", "list"]));
    assert!(listed.contains("rejected"), "generations: {listed}");
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        },
        "the previous generation must be serving again"
    );
}

#[test]
fn history_records_every_activation_with_its_reason() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build", "--activate"]);
    fixture.write("source.txt", "good, second\n");
    fixture.gm_ok(&["generation", "build", "--activate"]);
    fixture.gm_ok(&["generation", "rollback"]);

    let history = stdout(&fixture.gm_ok(&["generation", "history"]));
    let lines: Vec<&str> = history.lines().collect();

    assert_eq!(lines.len(), 3, "history: {history}");
    assert!(
        lines[0].contains("- → 1") && lines[0].contains("gm generation build --activate"),
        "{}",
        lines[0]
    );
    assert!(lines[1].contains("1 → 2"), "{}", lines[1]);
    assert!(lines[2].contains("2 → 1") && lines[2].contains("gm generation rollback"), "{}", lines[2]);
}

#[test]
fn prune_keeps_the_active_generation_and_its_rollback_target() {
    let fixture = project();
    for _ in 0 .. 4 {
        fixture.gm_ok(&["generation", "build"]);
    }
    fixture.gm_ok(&["generation", "activate", "4"]);

    let collected = fixture.gm_ok(&["generation", "prune", "--keep", "2"]);
    assert!(stdout(&collected).contains("removed generation(s) 1, 2"), "stdout: {}", stdout(&collected));

    let listed = stdout(&fixture.gm_ok(&["generation", "list"]));
    assert!(!listed.contains(" 1 ") && !listed.contains(" 2 "), "generations: {listed}");
    assert!(listed.contains(" 3 ") && listed.contains(" 4 "), "generations: {listed}");
    // The store directories are gone too, not just the numbered links.
    assert!(!fixture.path(".gm/generations/0001").exists());
    assert_eq!(std::fs::read_dir(fixture.path(".gm/store")).unwrap().count(), 2);
}

#[test]
fn prune_never_removes_the_last_generations_when_asked_to_keep_none() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build", "--activate"]);

    fixture.gm_ok(&["generation", "prune", "--keep", "0"]);

    assert!(fixture.path(".gm/generations/0001").exists());
}

#[test]
fn a_note_is_stored_with_the_generation() {
    let fixture = project();

    fixture.gm_ok(&["generation", "build", "--note", "tuned the cache"]);

    let listed = stdout(&fixture.gm_ok(&["generation", "list"]));
    assert!(listed.contains("tuned the cache"), "generations: {listed}");
}

#[test]
fn a_history_write_failure_restores_the_previous_service() {
    let fixture = project();
    fixture.gm_ok(&["generation", "build", "--activate"]);
    fixture.gm_ok(&["generation", "build"]);
    std::fs::remove_file(fixture.path(".gm/history.jsonl")).unwrap();
    std::fs::create_dir(fixture.path(".gm/history.jsonl")).unwrap();

    let failed = fixture.gm(&["generation", "activate", "2"]);
    assert!(!failed.status.success());
    assert_eq!(
        std::fs::read_link(fixture.path(".gm/current")).unwrap(),
        std::path::Path::new("generations/0001")
    );
    assert_eq!(
        fixture.run_state().unwrap().source,
        RunSource::Generation {
            id: GenerationId(1)
        }
    );
    assert!(stderr(&failed).contains("which is live again"));
}

#[test]
fn artifact_symlinks_are_frozen_as_independent_files() {
    let fixture = Fixture::with(Manifest::new("true").artifacts(&["linked"]));
    fixture.write("original", "old");
    std::os::unix::fs::symlink(fixture.path("original"), fixture.path("linked")).unwrap();
    fixture.gm_ok(&["generation", "build"]);
    fixture.write("original", "new");
    let frozen = fixture.generation_payload(1).join("linked");
    assert!(!frozen.is_symlink());
    assert_eq!(std::fs::read_to_string(frozen).unwrap(), "old");
}

#[test]
fn artifact_symlinks_cannot_escape_or_form_directory_cycles() {
    let outside = Fixture::bare();
    outside.write("secret", "secret");
    for cycle in [false, true] {
        let fixture = Fixture::with(Manifest::new("true").artifacts(&["linked"]));
        fixture.write("tree/file", "content");
        if cycle {
            std::os::unix::fs::symlink(fixture.path("tree"), fixture.path("tree/back")).unwrap();
        }
        let target = if cycle { fixture.path("tree") } else { outside.path("secret") };
        std::os::unix::fs::symlink(target, fixture.path("linked")).unwrap();
        let failed = fixture.gm(&["generation", "build"]);
        assert!(!failed.status.success());
        assert!(stderr(&failed).contains(if cycle { "directory cycle" } else { "escapes the build directory" }));
        assert!(!fixture.path(".gm/generations/0001").exists());
    }
}

#[test]
fn copying_the_project_root_cannot_copy_the_store_into_itself() {
    let fixture = Fixture::with(Manifest::new("true").artifacts(&["."]));
    let failed = fixture.gm(&["generation", "build"]);
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("overlaps the generation destination"));
    assert_eq!(std::fs::read_dir(fixture.path(".gm/store")).unwrap().count(), 0);
}
