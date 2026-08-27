use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Result, bail};
use wm_runner::supervisor::Supervisor;

use wm_core::run::RunSource;

use crate::cmd::Project;
use crate::ui;

pub fn status() -> Result<ExitCode> {
    let project = Project::open()?;
    let supervisor = Supervisor::new(project.store.layout());

    ui::heading(&format!("project {}", project.config.project.name));
    ui::field("root", project.root.display());

    if let Some(name) = &project.worktree {
        ui::field("worktree", format!("{name} (you are here)"));
    }

    match project.store.current_id()? {
        Some(id) => {
            let entry = project.store.get(id)?;
            ui::field("generation", format!("{id}  [{}]", entry.meta.describe_source()));
            ui::field("built", entry.meta.built_at.format("%Y-%m-%d %H:%M:%S"));
            ui::field("path", entry.payload().display());
        }
        None => ui::field("generation", "none active (run `wm build --switch`)"),
    }

    // Naming the source matters: the running process is not necessarily the
    // active generation once `wm dev run` exists.
    match supervisor.running() {
        Some(state) => {
            let mode = if state.detached { "background" } else { "foreground" };
            ui::field("service", format!("running (pid {}, {mode})", state.pid));
            ui::field("running", state.source.to_string());
        }
        None => ui::field("service", "stopped"),
    }

    let count = project.store.list()?.len();
    ui::field("stored", format!("{count} generation(s)"));
    Ok(ExitCode::SUCCESS)
}

pub fn start() -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let entry = project.store.current()?;
    let supervisor = Supervisor::new(project.store.layout());
    if let Some(state) = supervisor.running() {
        bail!(
            "already running: {} (pid {}) — use `wm restart`, or `wm stop` first",
            state.source,
            state.pid
        );
    }

    let state = supervisor.start_detached(
        &project.config.run,
        &entry.payload(),
        RunSource::Generation { id: entry.meta.id },
    )?;

    println!("{} started generation {} (pid {})", ui::OK, entry.meta.id, state.pid);
    Ok(ExitCode::SUCCESS)
}

pub fn stop() -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let supervisor = Supervisor::new(project.store.layout());
    let timeout = Duration::from_secs(project.config.run.stop_timeout_secs);
    let state = supervisor.stop(timeout)?;

    println!("{} stopped {}", ui::OK, state.source);
    Ok(ExitCode::SUCCESS)
}

pub fn restart() -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let entry = project.store.current()?;
    let supervisor = Supervisor::new(project.store.layout());
    let timeout = Duration::from_secs(project.config.run.stop_timeout_secs);
    if let Some(stopped) = supervisor.stop_if_running(timeout)?
        && stopped.source.is_dev()
    {
        println!("  {} stopping {} to take the service slot", ui::ARROW, stopped.source);
    }

    let state = supervisor.start_detached(
        &project.config.run,
        &entry.payload(),
        RunSource::Generation { id: entry.meta.id },
    )?;

    println!("{} restarted generation {} (pid {})", ui::OK, entry.meta.id, state.pid);
    Ok(ExitCode::SUCCESS)
}

pub fn logs(lines: usize) -> Result<ExitCode> {
    let project = Project::open()?;
    let supervisor = Supervisor::new(project.store.layout());
    let tail = supervisor.tail(lines)?;

    if tail.trim().is_empty() {
        println!("(log at {} is empty)", supervisor.log_file().display());
    } else {
        println!("{tail}");
    }
    Ok(ExitCode::SUCCESS)
}
