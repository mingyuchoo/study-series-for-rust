use crate::error::{Error,
                   IoContext,
                   Result};
use std::{fs,
          path::{Path,
                 PathBuf}};

/// Copy every declared artifact path from the build directory into the store,
/// preserving its relative location so the run command sees the same layout it
/// saw during the build.
///
/// Copying (rather than symlinking back into the worktree) is what makes a
/// generation immutable: later development in the worktree cannot retroactively
/// change what an old generation runs.
pub fn collect(src_root: &Path, includes: &[PathBuf], dest_root: &Path) -> Result<Vec<PathBuf>> {
    let src_root = src_root.canonicalize().ctx("resolving artifact source")?;
    let dest_root = dest_root.canonicalize().ctx("resolving artifact destination")?;
    let mut copied = Vec::new();
    for rel in includes {
        if rel.is_absolute() || rel.components().any(|part| part == std::path::Component::ParentDir) {
            return Err(Error::Config(format!("artifact path must stay relative: {}", rel.display())));
        }
        let src = src_root.join(rel);
        if !src.exists() {
            return Err(Error::Config(format!("declared artifact `{}` does not exist after the build", rel.display())));
        }
        let dest = dest_root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ctx(format!("creating {}", parent.display()))?;
        }
        copy_path(&src, &dest, &src_root, &dest_root, &mut Vec::new())?;
        copied.push(rel.clone());
    }
    Ok(copied)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FileArtifactCollector;

impl gm_application::ArtifactCollector for FileArtifactCollector {
    fn collect(&self, source: &Path, includes: &[PathBuf], destination: &Path) -> gm_application::PortResult<Vec<PathBuf>> {
        Ok(collect(source, includes, destination)?)
    }
}

fn copy_path(src: &Path, dest: &Path, source_root: &Path, destination_root: &Path, ancestors: &mut Vec<PathBuf>) -> Result<()> {
    let src = src.canonicalize().ctx(format!("resolving {}", src.display()))?;
    if !src.starts_with(source_root) {
        return Err(Error::Config(format!("artifact link escapes the build directory: {}", src.display())));
    }
    let meta = fs::metadata(&src).ctx(format!("stat {}", src.display()))?;
    if meta.is_dir() {
        if destination_root.starts_with(&src) || src.starts_with(destination_root) {
            return Err(Error::Config(format!(
                "artifact directory overlaps the generation destination: {}",
                src.display()
            )));
        }
        if ancestors.contains(&src) {
            return Err(Error::Config(format!("artifact directory cycle: {}", src.display())));
        }
        ancestors.push(src.clone());
        fs::create_dir_all(dest).ctx(format!("creating {}", dest.display()))?;
        for entry in fs::read_dir(&src).ctx(format!("reading {}", src.display()))? {
            let entry = entry.ctx(format!("reading {}", src.display()))?;
            copy_path(&entry.path(), &dest.join(entry.file_name()), source_root, destination_root, ancestors)?;
        }
        ancestors.pop();
    } else {
        // fs::copy carries the mode across, so executables stay executable.
        fs::copy(&src, dest).ctx(format!("copying {} to {}", src.display(), dest.display()))?;
    }
    Ok(())
}

/// Total size of a directory tree, for `gm generation list` reporting.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|entry| match entry.file_type() {
            | Ok(t) if t.is_dir() => dir_size(&entry.path()),
            | Ok(t) if t.is_file() => entry.metadata().map(|m| m.len()).unwrap_or(0),
            | _ => 0,
        })
        .sum()
}
