//! Workspace port and its local-filesystem adapter.

use amap_domain::FileChange;
use amap_orchestrator::OrchestrationError;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

pub trait Workspace: Send + Sync {
    fn read_files(&self) -> Result<Vec<FileChange>, OrchestrationError>;
    fn apply(&self, files: &[FileChange]) -> Result<Vec<String>, OrchestrationError>;
    fn stage(&self, files: &[FileChange]) -> Result<Vec<String>, OrchestrationError>;
}

pub trait WorkspaceFactory: Send + Sync {
    fn open(&self, root: &Path) -> Box<dyn Workspace>;
}

#[derive(Default)]
pub struct FileSystemWorkspaceFactory;

impl WorkspaceFactory for FileSystemWorkspaceFactory {
    fn open(&self, root: &Path) -> Box<dyn Workspace> {
        Box::new(FileSystemWorkspace::new(root))
    }
}

pub struct FileSystemWorkspace {
    root: PathBuf,
}

impl FileSystemWorkspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn staging_dir(&self) -> PathBuf {
        self.root.join(".amap-staging")
    }
}

impl Workspace for FileSystemWorkspace {
    fn read_files(&self) -> Result<Vec<FileChange>, OrchestrationError> {
        let mut files = Vec::new();
        for entry in WalkDir::new(&self.root) {
            let entry = entry.map_err(|error| OrchestrationError::Other(error.to_string()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(&self.root)
                .map_err(|error| OrchestrationError::Other(error.to_string()))?;
            if relative.starts_with(".amap") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            files.push(FileChange {
                path: relative.display().to_string(),
                content,
            });
        }
        Ok(files)
    }

    fn apply(&self, files: &[FileChange]) -> Result<Vec<String>, OrchestrationError> {
        std::fs::create_dir_all(&self.root)
            .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        write_files(&self.root, files)
    }

    fn stage(&self, files: &[FileChange]) -> Result<Vec<String>, OrchestrationError> {
        let staging = self.staging_dir();
        if staging.exists() {
            std::fs::remove_dir_all(&staging)
                .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        }
        std::fs::create_dir_all(&staging)
            .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        write_files(&staging, files)
    }
}

pub fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return None;
    }
    Some(root.join(path))
}

fn write_files(root: &Path, files: &[FileChange]) -> Result<Vec<String>, OrchestrationError> {
    let mut written = Vec::new();
    for file in files {
        let path = safe_join(root, &file.path)
            .ok_or_else(|| OrchestrationError::Other(format!("unsafe path {}", file.path)))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        }
        std::fs::write(path, &file.content)
            .map_err(|error| OrchestrationError::Other(error.to_string()))?;
        written.push(file.path.clone());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_rejects_parent_and_absolute_paths() {
        let root = Path::new("/tmp/workspace");
        assert!(safe_join(root, "src/main.rs").is_some());
        assert!(safe_join(root, "../secret").is_none());
        assert!(safe_join(root, "/etc/passwd").is_none());
    }
}
