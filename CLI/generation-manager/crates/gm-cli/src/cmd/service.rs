use crate::{cmd::Project,
            ui};
use anyhow::{Result,
             bail};
use gm_core::run::RunSource;
use gm_runner::supervisor::Supervisor;
use std::{process::ExitCode,
          time::Duration};

pub fn status() -> Result<ExitCode> {
    let project = Project::open()?;
    let supervisor = Supervisor::new(project.store.layout());

    ui::heading(&format!("service for project {}", project.config.project.name));
    match supervisor.running() {
        | Some(state) => {
            let mode = if state.detached { "background" } else { "foreground" };
            ui::field("state", format!("running (pid {}, {mode})", state.pid));
            ui::field("source", state.source.to_string());
        },
        | None => ui::field("state", "stopped"),
    }
    ui::field("log", supervisor.log_file().display());
    Ok(ExitCode::SUCCESS)
}

pub fn start() -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let entry = project.store.current()?;
    let supervisor = Supervisor::new(project.store.layout());
    if let Some(state) = supervisor.running() {
        bail!(
            "already running: {} (pid {}) — use `gm service restart`, or `gm service stop` first",
            state.source,
            state.pid
        );
    }

    let state = supervisor.start_detached(
        &project.config.run,
        &entry.payload(),
        RunSource::Generation {
            id: entry.meta.id,
        },
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
        RunSource::Generation {
            id: entry.meta.id,
        },
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
