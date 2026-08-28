//! End-to-end tests for `gm project status` and `gm service` commands.

mod common;

use common::{Fixture,
             Manifest,
             stderr,
             stdout};

fn project(run_cmd: &str) -> Fixture { Fixture::with(Manifest::new(run_cmd)) }

#[test]
fn status_describes_a_project_with_nothing_built_yet() {
    let fixture = project("true");

    let status = stdout(&fixture.gm_ok(&["project", "status"]));

    assert!(status.contains("project demo"), "status: {status}");
    assert!(status.contains("none active"), "status: {status}");
    assert!(status.contains("stopped"), "status: {status}");
    assert!(status.contains("0 generation(s)"), "status: {status}");
}

#[test]
fn commands_outside_a_project_say_so() {
    let fixture = Fixture::bare();

    let output = fixture.gm(&["project", "status"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("no generation-manager.toml found"), "stderr: {}", stderr(&output));
}

#[test]
fn start_requires_an_active_generation() {
    let fixture = project("exec sleep 300");
    fixture.gm_ok(&["generation", "build"]);

    let output = fixture.gm(&["service", "start"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("no generation is currently active"), "stderr: {}", stderr(&output));
}

#[test]
fn start_runs_the_active_generation_again_after_a_stop() {
    let fixture = project("exec sleep 300");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    let stopped = fixture.gm_ok(&["service", "stop"]);
    assert!(stdout(&stopped).contains("stopped generation 1"), "stdout: {}", stdout(&stopped));
    assert!(fixture.run_state().is_none());

    let started = fixture.gm_ok(&["service", "start"]);
    assert!(stdout(&started).contains("started generation 1"), "stdout: {}", stdout(&started));
    assert!(fixture.run_state().is_some());
}

#[test]
fn start_refuses_while_something_already_holds_the_slot() {
    let fixture = project("exec sleep 300");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    let output = fixture.gm(&["service", "start"]);

    assert!(!output.status.success());
    let err = stderr(&output);
    // The refusal names what is running, not just that something is.
    assert!(err.contains("already running: generation 1"), "stderr: {err}");
    assert!(err.contains("gm service restart"), "stderr: {err}");
}

#[test]
fn stop_without_a_running_service_fails() {
    let fixture = project("true");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    // The run command exited on its own; the slot is already free.
    let output = fixture.gm(&["service", "stop"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("not running"), "stderr: {}", stderr(&output));
}

#[test]
fn restart_replaces_the_running_process() {
    let fixture = project("exec sleep 300");
    fixture.gm_ok(&["generation", "build", "--activate"]);
    let first = fixture.run_state().unwrap().pid;

    let restarted = fixture.gm_ok(&["service", "restart"]);
    assert!(stdout(&restarted).contains("restarted generation 1"), "stdout: {}", stdout(&restarted));

    let second = fixture.run_state().unwrap().pid;
    assert_ne!(first, second, "restart must start a new process");
}

#[test]
fn logs_show_what_the_service_wrote() {
    let fixture = project("echo hello from the service; exec sleep 300");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    let logs = stdout(&fixture.gm_ok(&["service", "logs"]));

    assert!(logs.contains("hello from the service"), "logs: {logs}");
}

#[test]
fn logs_are_empty_before_anything_runs() {
    let fixture = project("true");

    let logs = stdout(&fixture.gm_ok(&["service", "logs"]));

    assert!(logs.contains("is empty"), "logs: {logs}");
}

#[test]
fn status_names_the_active_generation_and_its_payload() {
    let fixture = project("exec sleep 300");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    let status = stdout(&fixture.gm_ok(&["project", "status"]));

    assert!(status.contains("running      generation 1"), "status: {status}");
    assert!(status.contains("background"), "status: {status}");
    assert!(status.contains("1 generation(s)"), "status: {status}");
    assert!(status.contains(".gm/store/0001-"), "status: {status}");
}

#[test]
fn the_directory_flag_runs_against_another_project() {
    let fixture = project("true");
    fixture.gm_ok(&["generation", "build"]);

    // `-C` is what makes the tool usable from outside the project tree.
    let elsewhere = std::env::temp_dir();
    let output = Fixture::bare().gm_in(&elsewhere, &["-C", fixture.root.to_str().unwrap(), "generation", "list"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("built"), "stdout: {}", stdout(&output));
}

#[test]
fn service_status_reports_only_the_runtime_slot() {
    let fixture = project("exec sleep 300");
    fixture.gm_ok(&["generation", "build", "--activate"]);

    let status = stdout(&fixture.gm_ok(&["service", "status"]));

    assert!(status.contains("service for project demo"), "status: {status}");
    assert!(status.contains("state        running"), "status: {status}");
    assert!(status.contains("source       generation 1"), "status: {status}");
    assert!(status.contains("service.log"), "status: {status}");
    assert!(!status.contains("stored"), "service status should not show project inventory: {status}");
}
