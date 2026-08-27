use std::process::ExitCode;

use anyhow::Result;
use wm_core::generation::{GenerationId, GenerationStatus};
use wm_runner::artifacts::dir_size;
use wm_runner::{Activation, Pipeline};

use crate::cmd::Project;
use crate::ui;

/// Stage 2: build, test, freeze.
pub fn build(from: Option<&str>, note: Option<String>, switch_after: bool) -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    // Standing inside a worktree makes it the default build source, so the
    // common case needs no flag.
    let selected = project.select_worktree(from);
    let source = match &selected {
        Some(name) => project.worktree_path(name)?,
        None => project.root.clone(),
    };

    match &selected {
        Some(name) => println!("building from worktree `{name}` ({})", source.display()),
        None => println!("building from project root ({})", source.display()),
    }
    let pipeline = Pipeline::new(&project.store, &project.config);
    let entry = pipeline.build(&source, selected.as_deref(), note)?;

    println!();
    println!(
        "{} generation {} built  [{}]  {}",
        ui::OK,
        entry.meta.id,
        entry.meta.describe_source(),
        ui::human_size(dir_size(&entry.payload()))
    );

    if !switch_after {
        println!("  activate it with `wm switch`");
        return Ok(ExitCode::SUCCESS);
    }

    println!();
    activate(&project, entry.meta.id, "wm build --switch")
}

/// Activate a generation, honouring the health check.
pub fn switch(target: Option<u64>) -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let id = match target {
        Some(n) => GenerationId(n),
        None => project
            .store
            .list()?
            .last()
            .map(|e| e.meta.id)
            .ok_or_else(|| anyhow::anyhow!("no generations yet — run `wm build` first"))?,
    };
    activate(&project, id, "wm switch")
}

/// Stage 3: back out to an older generation.
pub fn rollback(target: Option<u64>) -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let id = match target {
        Some(n) => GenerationId(n),
        None => project.store.rollback_target()?,
    };
    activate(&project, id, "wm rollback")
}

fn activate(project: &Project, id: GenerationId, reason: &str) -> Result<ExitCode> {
    let pipeline = Pipeline::new(&project.store, &project.config);

    // Taking the slot back from a development run is legitimate, but silently
    // killing someone's `wm dev run` is not.
    if let Some(state) = pipeline.supervisor().running()
        && state.source.is_dev()
    {
        println!("  {} stopping {} to take the service slot", ui::ARROW, state.source);
    }

    println!("activating generation {id}…");

    match pipeline.activate(id, reason)? {
        Activation::Healthy { id, pid } => {
            println!("{} generation {id} is live (pid {pid})", ui::OK);
            Ok(ExitCode::SUCCESS)
        }
        Activation::RolledBack { failed, restored, restored_healthy, reason } => {
            eprintln!("{} generation {failed} failed verification: {reason}", ui::ERR);
            match restored {
                Some(prev) if restored_healthy => {
                    eprintln!("  {} rolled back to generation {prev}, which is live again", ui::ARROW);
                }
                Some(prev) => {
                    eprintln!(
                        "  {} rolled back to generation {prev}, but it did not pass its own \
                         health check either",
                        ui::ARROW
                    );
                }
                None => {
                    eprintln!("  {} no earlier generation to restore; nothing is running", ui::ARROW);
                }
            }
            eprintln!("  inspect the failure with `wm logs`");
            Ok(ExitCode::FAILURE)
        }
    }
}

pub fn generations() -> Result<ExitCode> {
    let project = Project::open()?;
    let entries = project.store.list()?;
    if entries.is_empty() {
        println!("no generations yet (run `wm build`)");
        return Ok(ExitCode::SUCCESS);
    }
    let current = project.store.current_id()?;

    println!(
        "{:<3} {:<4} {:<20} {:<24} {:<9} SIZE",
        "", "GEN", "BUILT", "SOURCE", "STATUS"
    );
    for entry in &entries {
        let marker = if Some(entry.meta.id) == current { "*" } else { " " };
        let status = match entry.meta.status {
            GenerationStatus::Built => "built",
            GenerationStatus::Healthy => "healthy",
            GenerationStatus::Rejected => "rejected",
        };
        println!(
            "{:<3} {:<4} {:<20} {:<24} {:<9} {}",
            marker,
            entry.meta.id,
            entry.meta.built_at.format("%Y-%m-%d %H:%M:%S"),
            entry.meta.describe_source(),
            status,
            ui::human_size(dir_size(&entry.payload()))
        );
    }
    if let Some(note) = entries.iter().rev().find_map(|e| e.meta.note.clone()) {
        println!();
        println!("latest note: {note}");
    }
    Ok(ExitCode::SUCCESS)
}

pub fn history() -> Result<ExitCode> {
    let project = Project::open()?;
    let events = project.store.history()?;
    if events.is_empty() {
        println!("no switches recorded yet");
        return Ok(ExitCode::SUCCESS);
    }
    for event in events {
        let from = event.from.map(|f| f.to_string()).unwrap_or_else(|| "-".into());
        println!(
            "{}  {from} {} {}  ({})",
            event.at.format("%Y-%m-%d %H:%M:%S"),
            ui::ARROW,
            event.to,
            event.reason
        );
    }
    Ok(ExitCode::SUCCESS)
}

pub fn gc(keep: usize) -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let removed = project.store.gc(keep)?;
    if removed.is_empty() {
        println!("nothing to collect (keeping {keep})");
    } else {
        let list: Vec<String> = removed.iter().map(|id| id.to_string()).collect();
        println!("{} removed generation(s) {}", ui::OK, list.join(", "));
    }
    Ok(ExitCode::SUCCESS)
}
