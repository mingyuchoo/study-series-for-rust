use crate::{DevelopmentRuntime,
            Error,
            Result,
            StageExecutor};
use gm_core::{Config,
              RunSource,
              RunState};
use std::path::Path;

pub fn create_worktree(
    name: &str,
    base: Option<&str>,
    repository: &impl crate::WorktreeRepository,
    git: &impl crate::WorktreeControl,
) -> Result<std::path::PathBuf> {
    gm_core::config::validate_worktree_name(name)?;
    let root = repository.project_root();
    if !git.is_repo(root) {
        return Err(Error::Precondition(format!("{} is not a git repository", root.display())));
    }
    let location = Error::port("locating worktree", repository.locate(name))?;
    if location.exists {
        return Err(Error::Precondition(format!("worktree `{name}` already exists at {}", location.path.display())));
    }
    Error::port("creating worktree", git.add(root, &location.path, name, base))?;
    Ok(location.path)
}

pub fn remove_worktree(name: &str, force: bool, repository: &impl crate::WorktreeRepository, git: &impl crate::WorktreeControl) -> Result<()> {
    gm_core::config::validate_worktree_name(name)?;
    let location = Error::port("locating worktree", repository.locate(name))?;
    if !location.exists {
        return Err(Error::Precondition(format!("worktree `{name}` does not exist")));
    }
    Error::port("removing worktree", git.remove(repository.project_root(), &location.path, force))?;
    git.prune(repository.project_root());
    Ok(())
}

pub struct RunWorktree<'a> {
    pub name: &'a str,
    pub path: &'a Path,
    pub build: bool,
    pub detach: bool,
}

pub enum WorktreeRun<F> {
    Foreground(F),
    Detached(RunState),
}

/// Take the slot without publishing a generation or verifying development code.
/// The caller releases its project lock before waiting for a foreground run.
pub fn run_worktree<R: DevelopmentRuntime>(
    request: RunWorktree<'_>,
    config: &Config,
    stages: &impl StageExecutor,
    runtime: &R,
) -> Result<WorktreeRun<R::Foreground>> {
    config.validate()?;
    gm_core::config::validate_worktree_name(request.name)?;
    Error::port(
        "stopping running service",
        runtime.stop_if_running(std::time::Duration::from_secs(config.run.stop_timeout_secs)),
    )?;
    if request.build && !config.build.is_empty() {
        Error::port("running build stage", stages.run_configured("build", &config.build, request.path))?;
    }
    let source = RunSource::Worktree {
        name: request.name.to_string(),
    };
    if request.detach {
        Ok(WorktreeRun::Detached(Error::port(
            "starting development service",
            runtime.start_detached(&config.run, request.path, source),
        )?))
    } else {
        Ok(WorktreeRun::Foreground(Error::port(
            "starting foreground service",
            runtime.spawn_foreground(&config.run, request.path, source),
        )?))
    }
}
