use gm_core::generation::GenerationId;
use std::path::{Path,
                PathBuf};

/// Every path the tool owns, derived from the project root.
#[derive(Debug, Clone)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(project_root: impl Into<PathBuf>) -> Layout {
        Layout {
            root: project_root.into(),
        }
    }

    /// The directory containing `generation-manager.toml`.
    pub fn project_root(&self) -> &Path { &self.root }

    pub fn manifest(&self) -> PathBuf { self.root.join(MANIFEST) }

    /// `.gm` — all tool-managed state.
    pub fn state(&self) -> PathBuf { self.root.join(STATE_DIR) }

    pub fn lock_file(&self) -> PathBuf { self.state().join("lock") }

    pub fn worktrees(&self) -> PathBuf { self.state().join(WORKTREES) }

    pub fn worktree(&self, name: &str) -> crate::Result<PathBuf> {
        gm_core::config::validate_worktree_name(name)?;
        Ok(self.worktrees().join(name))
    }

    pub fn store(&self) -> PathBuf { self.state().join("store") }

    pub fn generations(&self) -> PathBuf { self.state().join("generations") }

    /// `generations/0007` — the stable, numbered handle for a generation.
    pub fn generation_link(&self, id: GenerationId) -> PathBuf { self.generations().join(id.dir_prefix()) }

    /// `current` — the activation pointer replaced atomically on switch.
    pub fn current_link(&self) -> PathBuf { self.state().join("current") }

    pub fn history(&self) -> PathBuf { self.state().join("history.jsonl") }

    pub fn run_dir(&self) -> PathBuf { self.state().join("run") }

    /// Records both the pid and what it is running. A bare pid file cannot
    /// distinguish an activated generation from a development worktree.
    pub fn run_state(&self) -> PathBuf { self.run_dir().join("state.json") }

    pub fn log_file(&self) -> PathBuf { self.run_dir().join("service.log") }

    /// Where a generation's artifacts live inside its store directory.
    pub fn generation_payload(dir: &Path) -> PathBuf { dir.join("root") }

    pub fn generation_meta(dir: &Path) -> PathBuf { dir.join("meta.json") }
}

/// Name of the per-project manifest.
pub const MANIFEST: &str = "generation-manager.toml";
/// Directory holding all tool-managed state, relative to the project root.
pub const STATE_DIR: &str = ".gm";
/// Directory holding development worktrees, relative to [`STATE_DIR`].
pub const WORKTREES: &str = "worktrees";

/// Recognise `<root>/.gm/worktrees/<name>/...`, innermost match first.
pub fn worktree_context(start: &Path) -> Option<(PathBuf, String)> {
    for dir in start.ancestors() {
        let parent = dir.parent()?;
        if parent.file_name()?.to_str()? != WORKTREES {
            continue;
        }
        let state = parent.parent()?;
        if state.file_name()?.to_str()? != STATE_DIR {
            continue;
        }
        let root = state.parent()?;
        let name = dir.file_name()?.to_str()?.to_string();
        return Some((root.to_path_buf(), name));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(path: &str) -> Option<(PathBuf, String)> { worktree_context(Path::new(path)) }

    #[test]
    fn recognises_a_managed_worktree() {
        let (root, name) = context("/srv/demo/.gm/worktrees/add-cache").unwrap();
        assert_eq!(root, Path::new("/srv/demo"));
        assert_eq!(name, "add-cache");
    }

    #[test]
    fn recognises_a_subdirectory_of_a_worktree() {
        let (root, name) = context("/srv/demo/.gm/worktrees/add-cache/src/api").unwrap();
        assert_eq!(root, Path::new("/srv/demo"));
        assert_eq!(name, "add-cache");
    }

    #[test]
    fn ignores_the_project_root_and_the_state_dir() {
        assert!(context("/srv/demo").is_none());
        assert!(context("/srv/demo/.gm").is_none());
        assert!(context("/srv/demo/.gm/store/0001-abc").is_none());
        // The worktrees directory itself is not a worktree.
        assert!(context("/srv/demo/.gm/worktrees").is_none());
    }

    #[test]
    fn ignores_a_lookalike_path_outside_the_state_dir() {
        assert!(context("/srv/demo/worktrees/add-cache").is_none());
        assert!(context("/srv/demo/other/worktrees/add-cache").is_none());
    }

    #[test]
    fn picks_the_innermost_worktree_when_nested() {
        let (root, name) = context("/srv/demo/.gm/worktrees/outer/.gm/worktrees/inner/src").unwrap();
        assert_eq!(root, Path::new("/srv/demo/.gm/worktrees/outer"));
        assert_eq!(name, "inner");
    }
}
