use crate::{error::{Error,
                    IoContext,
                    Result},
            layout::{MANIFEST,
                     worktree_context}};
use gm_core::{Config,
              DetectedFiles,
              Preset};
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
    parse_config(&text)
}

pub fn save_config(config: &Config, path: &Path) -> Result<()> {
    let text = render_config(config)?;
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

/// TOML belongs to this adapter; the domain validates the decoded value.
pub fn parse_config(text: &str) -> Result<Config> {
    let config: Config = toml::from_str(text).map_err(|error| gm_core::Error::Config(error.to_string()))?;
    config.validate()?;
    Ok(config)
}

pub fn render_config(config: &Config) -> Result<String> {
    config.validate()?;
    toml::to_string_pretty(config).map_err(|error| gm_core::Error::Config(error.to_string()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_roundtrips_and_is_validated_at_the_boundary() {
        let rendered = render_config(&Preset::Rust.template("demo")).unwrap();
        let parsed = parse_config(&rendered).unwrap();
        assert_eq!(parsed.project.name, "demo");
        assert_eq!(parsed.run.cmd, "./target/release/demo");
        assert!(parse_config(&rendered.replace("./target/release/demo", "")).is_err());
        assert!(parse_config("invalid toml").is_err());
    }
}
