use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use wm_core::error::{Error, IoContext, Result};

/// Advisory exclusive lock over one project.
///
/// Two concurrent `wm switch` runs would race on the `current` symlink and on
/// the pid file, so every mutating command takes this first.
#[derive(Debug)]
pub struct ProjectLock {
    _file: File,
    path: PathBuf,
}

impl ProjectLock {
    /// Fail immediately if another process holds the lock.
    pub fn acquire(path: &Path) -> Result<ProjectLock> {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .ctx(format!("opening lock file {}", path.display()))?;
        file.try_lock_exclusive()
            .map_err(|_| Error::Locked(path.to_path_buf()))?;
        Ok(ProjectLock { _file: file, path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
