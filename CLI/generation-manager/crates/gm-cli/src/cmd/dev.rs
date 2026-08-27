use crate::{cmd::Project,
            ui};
use anyhow::{Result,
             bail};
use gm_runner::{DevOutcome,
                Pipeline,
                git};
use std::{process::ExitCode,
          time::Duration};

/// Stage 1: an isolated checkout so development never disturbs the tree the
/// active generation was built from.
pub fn new(name: &str, base: Option<&str>) -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    if !git::is_repo(&project.root) {
        bail!("{} is not a git repository", project.root.display());
    }

    let path = project.store.layout().worktree(name);
    if path.exists() {
        bail!("worktree `{name}` already exists at {}", path.display());
    }

    git::worktree_add(&project.root, &path, name, base)?;

    println!("{} worktree `{name}` at {}", ui::OK, path.display());
    println!("  branch       {name}");
    println!();
    println!("  cd {}", path.display());
    println!("  # …develop, commit…");
    println!("  gm build --from {name} --switch");
    Ok(ExitCode::SUCCESS)
}

/// Run a worktree in place. This is the fast inner loop: no test stage, no
/// generation, no health check, no rollback — just the code as it stands.
pub fn run(from: Option<&str>, no_build: bool, detach: bool) -> Result<ExitCode> {
    let project = Project::open()?;

    let Some(name) = project.select_worktree(from) else {
        bail!(
            "not inside a worktree — pass `--from <name>`, or use `gm start` to run the \
             active generation"
        );
    };
    let path = project.worktree_path(&name)?;

    let lock = project.store.lock()?;
    let pipeline = Pipeline::new(&project.store, &project.config);
    displace(&pipeline, Duration::from_secs(project.config.run.stop_timeout_secs));

    println!("running worktree `{name}` from {}", path.display());
    if detach {
        println!("  {} unverified: no health check, no rollback", ui::ARROW);
    }
    println!();

    match pipeline.dev_run(&name, &path, !no_build, detach, lock)? {
        | DevOutcome::Detached(state) => {
            println!();
            println!("{} worktree `{name}` running in the background (pid {})", ui::OK, state.pid);
            println!("  logs with `gm logs`, stop with `gm stop`");
            Ok(ExitCode::SUCCESS)
        },
        | DevOutcome::Exited {
            code,
        } => {
            println!();
            if code != 0 {
                eprintln!("{} worktree `{name}` exited with code {code}", ui::ERR);
            }
            // The dev run took the slot; leaving it empty silently would look
            // like the active generation is still serving.
            if let Ok(Some(id)) = project.store.current_id() {
                println!("  service slot is free — `gm start` runs generation {id} again");
            }
            if code == 0 { Ok(ExitCode::SUCCESS) } else { Ok(ExitCode::FAILURE) }
        },
    }
}

/// Report taking the service slot from whatever held it.
fn displace(pipeline: &Pipeline<'_>, timeout: Duration) {
    let supervisor = pipeline.supervisor();
    if let Some(state) = supervisor.running() {
        println!("  {} stopping {} to take the service slot", ui::ARROW, state.source);
    }
    let _ = supervisor.stop_if_running(timeout);
}

pub fn list() -> Result<ExitCode> {
    let project = Project::open()?;
    let dir = project.store.layout().worktrees();

    let mut rows = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let branch = git::current_branch(&path).unwrap_or_else(|| "?".into());
        let commit: String = git::head_commit(&path).unwrap_or_default().chars().take(8).collect();
        let dirty = if git::is_dirty(&path) { "dirty" } else { "clean" };
        let here = project.worktree.as_deref() == Some(name.as_str());
        rows.push((name, branch, commit, dirty, here));
    }

    if rows.is_empty() {
        println!("no worktrees (create one with `gm dev new <name>`)");
        return Ok(ExitCode::SUCCESS);
    }

    rows.sort();
    println!("{:<3} {:<20} {:<20} {:<10} STATE", "", "NAME", "BRANCH", "COMMIT");
    for (name, branch, commit, dirty, here) in rows {
        // `*` marks the worktree the current directory is in — the one a bare
        // `gm dev run` or `gm build` would act on.
        let marker = if here { "*" } else { " " };
        println!("{marker:<3} {name:<20} {branch:<20} {commit:<10} {dirty}");
    }
    Ok(ExitCode::SUCCESS)
}

pub fn remove(name: &str, force: bool) -> Result<ExitCode> {
    let project = Project::open()?;
    let _lock = project.store.lock()?;

    let path = project.store.layout().worktree(name);
    if !path.exists() {
        bail!("worktree `{name}` does not exist");
    }

    git::worktree_remove(&project.root, &path, force)?;
    git::worktree_prune(&project.root);

    println!("{} removed worktree `{name}`", ui::OK);
    println!("  the branch `{name}` was kept; delete it with `git branch -D {name}`");
    Ok(ExitCode::SUCCESS)
}
