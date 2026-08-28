mod dev;
mod init;
mod lifecycle;
mod project;
mod service;

use crate::{Command,
            GenerationCommand,
            ProjectCommand,
            ServiceCommand,
            WorktreeCommand};
use anyhow::{Result,
             bail};
use gm_core::config::Config;
use gm_store::{Layout,
               Store};
use std::{path::PathBuf,
          process::ExitCode};

/// A resolved project: manifest, opened store, and the worktree the command was
/// invoked from, if any.
pub struct Project {
    pub config: Config,
    pub store: Store,
    pub root: PathBuf,
    /// Set when the current directory is inside `.gm/worktrees/<name>`.
    pub worktree: Option<String>,
}

impl Project {
    /// Find `generation-manager.toml` from the current directory upwards.
    /// Standing inside a worktree resolves to the real project root, not
    /// the worktree.
    pub fn open() -> Result<Project> {
        let cwd = std::env::current_dir()?;
        let found = gm_store::discover_project(&cwd)?;
        let store = Store::open(Layout::new(&found.root))?;
        Ok(Project {
            config: found.config,
            store,
            root: found.root,
            worktree: found.worktree,
        })
    }

    /// Which worktree a command should act on: an explicit positional target
    /// wins, otherwise the one the developer is standing in.
    pub fn select_worktree(&self, from: Option<&str>) -> Option<String> { gm_core::select_worktree(from, self.worktree.as_deref()) }

    /// Resolve a worktree name to its path, failing if it does not exist.
    pub fn worktree_path(&self, name: &str) -> Result<PathBuf> {
        let path = self.store.layout().worktree(name);
        if !path.is_dir() {
            bail!("worktree `{name}` does not exist (see `gm worktree list`)");
        }
        Ok(path)
    }
}

pub fn dispatch(command: Command) -> Result<ExitCode> {
    match command {
        | Command::Project(ProjectCommand::Init {
            preset,
            name,
            force,
        }) => init::run(preset, name, force),
        | Command::Project(ProjectCommand::Status) => project::status(),

        | Command::Worktree(WorktreeCommand::Create {
            name,
            base,
        }) => dev::new(&name, base.as_deref()),
        | Command::Worktree(WorktreeCommand::List) => dev::list(),
        | Command::Worktree(WorktreeCommand::Run {
            worktree,
            no_build,
            detach,
        }) => dev::run(worktree.as_deref(), no_build, detach),
        | Command::Worktree(WorktreeCommand::Remove {
            name,
            force,
        }) => dev::remove(&name, force),

        | Command::Generation(GenerationCommand::Build {
            worktree,
            note,
            activate,
        }) => lifecycle::build(worktree.as_deref(), note, activate),
        | Command::Generation(GenerationCommand::List) => lifecycle::generations(),
        | Command::Generation(GenerationCommand::Activate {
            generation,
        }) => lifecycle::activate_generation(generation),
        | Command::Generation(GenerationCommand::Rollback {
            generation,
        }) => lifecycle::rollback(generation),
        | Command::Generation(GenerationCommand::History) => lifecycle::history(),
        | Command::Generation(GenerationCommand::Prune {
            keep,
        }) => lifecycle::prune(keep),

        | Command::Service(ServiceCommand::Status) => service::status(),
        | Command::Service(ServiceCommand::Start) => service::start(),
        | Command::Service(ServiceCommand::Stop) => service::stop(),
        | Command::Service(ServiceCommand::Restart) => service::restart(),
        | Command::Service(ServiceCommand::Logs {
            lines,
        }) => service::logs(lines),
    }
}
