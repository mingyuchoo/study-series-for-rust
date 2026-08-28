use crate::error::{Error,
                   IoContext,
                   Result};
use gm_core::{Config,
              DetectedFiles,
              Preset,
              config::{MANIFEST,
                       worktree_context}};
use std::path::{Path,
                PathBuf};

#[derive(Debug, Clone)]
pub struct Discovery {
    pub config: Config,
    pub root: PathBuf,
    pub worktree: Option<String>,
}

pub fn discover_project(start: &Path) -> Result<Discovery> {
    let start = start.canonicalize().ctx(format!("resolving {}", start.display()))?;
    if let Some((root, name)) = worktree_context(&start) {
        let manifest = root.join(MANIFEST);
        if manifest.is_file() {
            return Ok(Discovery {
                config: load_config(&manifest)?,
                root,
                worktree: Some(name),
            });
        }
    }

    for directory in start.ancestors() {
        let manifest = directory.join(MANIFEST);
        if manifest.is_file() {
            return Ok(Discovery {
                config: load_config(&manifest)?,
                root: directory.to_path_buf(),
                worktree: None,
            });
        }
    }
    Err(Error::ProjectNotFound(start))
}

pub fn load_config(path: &Path) -> Result<Config> {
    let text = std::fs::read_to_string(path).ctx(format!("reading {}", path.display()))?;
    Config::parse(&text).map_err(Error::from)
}

pub fn save_config(config: &Config, path: &Path) -> Result<()> {
    let text = config.to_toml()?;
    std::fs::write(path, text).ctx(format!("writing {}", path.display()))
}

pub fn detect_preset(directory: &Path) -> Preset {
    Preset::detect(DetectedFiles {
        cargo_toml: directory.join("Cargo.toml").is_file(),
        package_json: directory.join("package.json").is_file(),
        go_mod: directory.join("go.mod").is_file(),
        pyproject_toml: directory.join("pyproject.toml").is_file(),
        requirements_txt: directory.join("requirements.txt").is_file(),
    })
}
