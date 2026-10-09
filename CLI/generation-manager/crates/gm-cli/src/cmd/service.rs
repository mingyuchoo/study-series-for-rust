use crate::{cmd::Project,
            ui};
use anyhow::{Result,
             bail};
use gm_application::start_service;
use std::{process::ExitCode,
          time::Duration};

pub fn status() -> Result<ExitCode> {
    let project = Project::open()?;
    let supervisor = project.supervisor();

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
    let lock = project.store.lock()?;

    let supervisor = project.supervisor();
    if let Some(state) = supervisor.running() {
        bail!(
            "already running: {} (pid {}) — use `gm service restart`, or `gm service stop` first",
            state.source,
            state.pid
        );
    }

    let repository = project.store.repository(&lock)?;
    let started = start_service(&project.config, &repository, &supervisor, false)?;
    println!("{} started {} (pid {})", ui::OK, started.state.source, started.state.pid);
    Ok(ExitCode::SUCCESS)
}

pub fn stop() -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let supervisor = project.supervisor();
    let timeout = Duration::from_secs(project.config.run.stop_timeout_secs);
    let state = supervisor.stop(timeout)?;

    println!("{} stopped {}", ui::OK, state.source);
    Ok(ExitCode::SUCCESS)
}

pub fn restart() -> Result<ExitCode> {
    let project = Project::open()?;
    let lock = project.store.lock()?;
    let repository = project.store.repository(&lock)?;
    let started = start_service(&project.config, &repository, &project.supervisor(), true)?;
    if let Some(stopped) = started.displaced
        && stopped.source.is_dev()
    {
        println!("  {} stopping {} to take the service slot", ui::ARROW, stopped.source);
    }
    println!("{} restarted {} (pid {})", ui::OK, started.state.source, started.state.pid);
    Ok(ExitCode::SUCCESS)
}

pub fn logs(lines: usize) -> Result<ExitCode> {
    let project = Project::open()?;
    let supervisor = project.supervisor();
    let tail = supervisor.tail(lines)?;

    if tail.trim().is_empty() {
        println!("(log at {} is empty)", supervisor.log_file().display());
    } else {
        println!("{tail}");
    }
    Ok(ExitCode::SUCCESS)
}
