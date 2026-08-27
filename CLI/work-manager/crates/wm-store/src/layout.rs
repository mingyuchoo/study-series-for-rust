use std::path::{Path, PathBuf};

use wm_core::config::{STATE_DIR, WORKTREES};
use wm_core::generation::GenerationId;

/// Every path the tool owns, derived from the project root.
#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(project_root: impl Into<PathBuf>) -> Layout {
        Layout { root: project_root.into() }
    }

    /// The directory containing `work-manager.toml`.
    pub fn project_root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> PathBuf {
        self.root.join(wm_core::config::MANIFEST)
    }

    /// `.wm` — all tool-managed state.
    pub fn state(&self) -> PathBuf {
        self.root.join(STATE_DIR)
    }

    pub fn lock_file(&self) -> PathBuf {
        self.state().join("lock")
    }

    pub fn worktrees(&self) -> PathBuf {
        self.state().join(WORKTREES)
    }

    pub fn worktree(&self, name: &str) -> PathBuf {
        self.worktrees().join(name)
    }

    pub fn store(&self) -> PathBuf {
        self.state().join("store")
    }

    pub fn generations(&self) -> PathBuf {
        self.state().join("generations")
    }

    /// `generations/0007` — the stable, numbered handle for a generation.
    pub fn generation_link(&self, id: GenerationId) -> PathBuf {
        self.generations().join(id.dir_prefix())
    }

    /// `current` — the activation pointer replaced atomically on switch.
    pub fn current_link(&self) -> PathBuf {
        self.state().join("current")
    }

    pub fn history(&self) -> PathBuf {
        self.state().join("history.jsonl")
    }

    pub fn run_dir(&self) -> PathBuf {
        self.state().join("run")
    }

    /// Records both the pid and what it is running. A bare pid file cannot
    /// distinguish an activated generation from a development worktree.
    pub fn run_state(&self) -> PathBuf {
        self.run_dir().join("state.json")
    }

    pub fn log_file(&self) -> PathBuf {
        self.run_dir().join("service.log")
    }

    /// Where a generation's artifacts live inside its store directory.
    pub fn generation_payload(dir: &Path) -> PathBuf {
        dir.join("root")
    }

    pub fn generation_meta(dir: &Path) -> PathBuf {
        dir.join("meta.json")
    }
}
