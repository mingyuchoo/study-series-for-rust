use crate::{cmd::Project,
            ui};
use anyhow::Result;
use gm_runner::supervisor::Supervisor;
use std::process::ExitCode;

/// Show a project-wide overview. Service-only inspection lives under
/// `gm service status`.
pub fn status() -> Result<ExitCode> {
    let project = Project::open()?;
    let supervisor = Supervisor::new(project.store.layout());

    ui::heading(&format!("project {}", project.config.project.name));
    ui::field("root", project.root.display());

    if let Some(name) = &project.worktree {
        ui::field("worktree", format!("{name} (you are here)"));
    }

    match project.store.current_id()? {
        | Some(id) => {
            let entry = project.store.get(id)?;
            ui::field("generation", format!("{id}  [{}]", entry.meta.describe_source()));
            ui::field("built", entry.meta.built_at.format("%Y-%m-%d %H:%M:%S"));
            ui::field("path", entry.payload().display());
        },
        | None => ui::field("generation", "none active (run `gm generation build --activate`)"),
    }

    match supervisor.running() {
        | Some(state) => {
            let mode = if state.detached { "background" } else { "foreground" };
            ui::field("service", format!("running (pid {}, {mode})", state.pid));
            ui::field("running", state.source.to_string());
        },
        | None => ui::field("service", "stopped"),
    }

    let count = project.store.list()?.len();
    ui::field("stored", format!("{count} generation(s)"));
    Ok(ExitCode::SUCCESS)
}
