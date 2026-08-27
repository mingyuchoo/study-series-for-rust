use crate::exec::capture;
use gm_core::error::{Error,
                     IoContext,
                     Result};
use std::{path::Path,
          process::Command};

/// Git is driven through the CLI rather than a binding: `git worktree` support
/// in libgit2 is partial, and shelling out matches what a developer would type.
pub fn is_repo(dir: &Path) -> bool { capture("git", &["rev-parse", "--git-dir"], dir).is_ok() }

pub fn head_commit(dir: &Path) -> Option<String> { capture("git", &["rev-parse", "HEAD"], dir).ok() }

pub fn current_branch(dir: &Path) -> Option<String> { capture("git", &["rev-parse", "--abbrev-ref", "HEAD"], dir).ok() }

/// True when the checkout has uncommitted changes, which makes a generation
/// unreproducible — recorded in its metadata rather than rejected.
pub fn is_dirty(dir: &Path) -> bool {
    match capture("git", &["status", "--porcelain"], dir) {
        | Ok(out) => !out.trim().is_empty(),
        | Err(_) => false,
    }
}

/// `git worktree add -b <branch> <path> <base>`
pub fn worktree_add(repo: &Path, path: &Path, branch: &str, base: Option<&str>) -> Result<()> {
    let mut args: Vec<String> = vec!["worktree".into(), "add".into(), "-b".into(), branch.into(), path.to_string_lossy().into_owned()];
    if let Some(base) = base {
        args.push(base.to_string());
    }

    let status = Command::new("git").args(&args).current_dir(repo).status().ctx("spawning `git worktree add`")?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::StageFailed {
            stage: "git worktree add".into(),
            code: status.code().unwrap_or(-1),
        })
    }
}

pub fn worktree_remove(repo: &Path, path: &Path, force: bool) -> Result<()> {
    let mut args: Vec<String> = vec!["worktree".into(), "remove".into(), path.to_string_lossy().into_owned()];
    if force {
        args.insert(2, "--force".into());
    }

    let status = Command::new("git")
        .args(&args)
        .current_dir(repo)
        .status()
        .ctx("spawning `git worktree remove`")?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::StageFailed {
            stage: "git worktree remove".into(),
            code: status.code().unwrap_or(-1),
        })
    }
}

/// Drop administrative entries for worktrees whose directories are gone.
pub fn worktree_prune(repo: &Path) { let _ = Command::new("git").args(["worktree", "prune"]).current_dir(repo).status(); }
