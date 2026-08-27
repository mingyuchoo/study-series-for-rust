mod dev;
mod init;
mod lifecycle;
mod service;

use crate::{Command,
            DevCommand};
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
        let found = Config::discover(&cwd)?;
        let store = Store::open(Layout::new(&found.root))?;
        Ok(Project {
            config: found.config,
            store,
            root: found.root,
            worktree: found.worktree,
        })
    }

    /// Which worktree a command should act on: an explicit `--from` wins,
    /// otherwise the one the developer is standing in.
    pub fn select_worktree(&self, from: Option<&str>) -> Option<String> { from.map(str::to_string).or_else(|| self.worktree.clone()) }

    /// Resolve a worktree name to its path, failing if it does not exist.
    pub fn worktree_path(&self, name: &str) -> Result<PathBuf> {
        let path = self.store.layout().worktree(name);
        if !path.is_dir() {
            bail!("worktree `{name}` does not exist (see `gm dev list`)");
        }
        Ok(path)
    }
}

pub fn dispatch(command: Command) -> Result<ExitCode> {
    match command {
        | Command::Init {
            preset,
            name,
            force,
        } => init::run(preset, name, force),

        | Command::Status => service::status(),

        | Command::Dev(DevCommand::New {
            name,
            base,
        }) => dev::new(&name, base.as_deref()),
        | Command::Dev(DevCommand::List) => dev::list(),
        | Command::Dev(DevCommand::Run {
            from,
            no_build,
            detach,
        }) => dev::run(from.as_deref(), no_build, detach),
        | Command::Dev(DevCommand::Rm {
            name,
            force,
        }) => dev::remove(&name, force),

        | Command::Build {
            from,
            note,
            switch,
        } => lifecycle::build(from.as_deref(), note, switch),
        | Command::Switch {
            generation,
        } => lifecycle::switch(generation),
        | Command::Rollback {
            to,
        } => lifecycle::rollback(to),
        | Command::Generations => lifecycle::generations(),
        | Command::History => lifecycle::history(),
        | Command::Gc {
            keep,
        } => lifecycle::gc(keep),

        | Command::Start => service::start(),
        | Command::Stop => service::stop(),
        | Command::Restart => service::restart(),
        | Command::Logs {
            lines,
        } => service::logs(lines),
    }
}
