use crate::error::{Error,
                   Result};
use serde::{Deserialize,
            Serialize};
use std::{collections::BTreeMap,
          path::{Path,
                 PathBuf}};

/// Name of the per-project manifest.
pub const MANIFEST: &str = "generation-manager.toml";
/// Directory holding all tool-managed state, relative to the project root.
pub const STATE_DIR: &str = ".gm";
/// Directory holding development worktrees, relative to [`STATE_DIR`].
pub const WORKTREES: &str = "worktrees";

/// The declarative project manifest.
///
/// Everything language-specific lives in shell commands, so the tool stays
/// agnostic: a Rust service, a Node service and a Go service differ only in the
/// strings below.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project: Project,
    #[serde(default)]
    pub build: BuildStage,
    #[serde(default)]
    pub test: BuildStage,
    #[serde(default)]
    pub artifacts: Artifacts,
    pub run: RunStage,
    #[serde(default)]
    pub health: HealthCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
}

/// A shell stage. An empty `cmd` means "skip this stage".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildStage {
    #[serde(default)]
    pub cmd: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl BuildStage {
    pub fn is_empty(&self) -> bool { self.cmd.trim().is_empty() }
}

/// Paths (relative to the build directory) copied into the immutable store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Artifacts {
    #[serde(default)]
    pub include: Vec<PathBuf>,
}

/// How the activated generation is executed. One local process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStage {
    pub cmd: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Seconds to wait for a graceful SIGTERM before escalating to SIGKILL.
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout_secs: u64,
}

fn default_stop_timeout() -> u64 { 10 }

/// Post-activation verification. If it fails, the switch is reverted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthCheck {
    /// `GET` this URL and accept any 2xx/3xx response. Plain HTTP only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    /// Accept as healthy once a TCP connection to `host:port` succeeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp: Option<String>,
    /// Accept as healthy once this shell command exits 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    #[serde(default = "default_health_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_health_interval")]
    pub interval_secs: u64,
}

fn default_health_timeout() -> u64 { 30 }

fn default_health_interval() -> u64 { 1 }

impl HealthCheck {
    /// A manifest with no probe configured activates without verification.
    pub fn is_configured(&self) -> bool { self.http.is_some() || self.tcp.is_some() || self.cmd.is_some() }
}

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

impl Config {
    /// Parse and validate a manifest without performing any I/O.
    pub fn parse(text: &str) -> Result<Config> {
        let config: Config = toml::from_str(text).map_err(|error| Error::Config(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Render a manifest without performing any I/O.
    pub fn to_toml(&self) -> Result<String> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|error| Error::Config(error.to_string()))
    }

    pub fn validate(&self) -> Result<()> {
        if self.project.name.trim().is_empty() {
            return Err(Error::Config("project.name must not be empty".into()));
        }
        if self.run.cmd.trim().is_empty() {
            return Err(Error::Config("run.cmd must not be empty".into()));
        }
        for path in &self.artifacts.include {
            if path.is_absolute() || path.components().any(|c| c.as_os_str() == "..") {
                return Err(Error::Config(format!(
                    "artifacts.include entries must be relative and stay inside the build \
                     directory, got `{}`",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

/// Starting points for `gm init`, so the multi-language case is a one-liner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Rust,
    Node,
    Python,
    Go,
    Generic,
}

impl std::str::FromStr for Preset {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            | "rust" | "cargo" => Ok(Preset::Rust),
            | "node" | "npm" | "js" | "ts" => Ok(Preset::Node),
            | "python" | "py" => Ok(Preset::Python),
            | "go" | "golang" => Ok(Preset::Go),
            | "generic" | "none" => Ok(Preset::Generic),
            | other => Err(format!("unknown preset `{other}`")),
        }
    }
}

impl Preset {
    /// Choose a preset from a side-effect-free snapshot of marker files.
    pub fn detect(files: DetectedFiles) -> Preset {
        if files.cargo_toml {
            Preset::Rust
        } else if files.package_json {
            Preset::Node
        } else if files.go_mod {
            Preset::Go
        } else if files.pyproject_toml || files.requirements_txt {
            Preset::Python
        } else {
            Preset::Generic
        }
    }

    pub fn template(self, name: &str) -> Config {
        let (build, test, include, run) = match self {
            | Preset::Rust => (
                "cargo build --release".to_string(),
                "cargo test".to_string(),
                vec![PathBuf::from(format!("target/release/{name}"))],
                format!("./{name}"),
            ),
            | Preset::Node => (
                "npm ci && npm run build".to_string(),
                "npm test".to_string(),
                vec![PathBuf::from("dist"), PathBuf::from("package.json"), PathBuf::from("package-lock.json")],
                "node dist/main.js".to_string(),
            ),
            | Preset::Python => (
                "python -m pip install -r requirements.txt --target vendor".to_string(),
                "python -m pytest".to_string(),
                vec![PathBuf::from("src"), PathBuf::from("vendor")],
                "python -m src.main".to_string(),
            ),
            | Preset::Go => (
                format!("go build -o bin/{name} ./cmd/{name}"),
                "go test ./...".to_string(),
                vec![PathBuf::from("bin")],
                format!("./bin/{name}"),
            ),
            | Preset::Generic => (
                "make build".to_string(),
                "make test".to_string(),
                vec![PathBuf::from("build")],
                "./build/start".to_string(),
            ),
        };

        Config {
            project: Project {
                name: name.to_string(),
            },
            build: BuildStage {
                cmd: build,
                env: BTreeMap::new(),
            },
            test: BuildStage {
                cmd: test,
                env: BTreeMap::new(),
            },
            artifacts: Artifacts {
                include,
            },
            run: RunStage {
                cmd: run,
                env: BTreeMap::new(),
                stop_timeout_secs: default_stop_timeout(),
            },
            health: HealthCheck {
                timeout_secs: default_health_timeout(),
                interval_secs: default_health_interval(),
                ..HealthCheck::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DetectedFiles {
    pub cargo_toml: bool,
    pub package_json: bool,
    pub go_mod: bool,
    pub pyproject_toml: bool,
    pub requirements_txt: bool,
}

#[cfg(test)]
mod tests {
    use super::{Config,
                DetectedFiles,
                Preset,
                worktree_context};
    use std::path::{Path,
                    PathBuf};

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

    #[test]
    fn parses_and_validates_configuration_without_io() {
        let rendered = Preset::Rust.template("demo").to_toml().unwrap();
        let parsed = Config::parse(&rendered).unwrap();

        assert_eq!(parsed.project.name, "demo");
        assert_eq!(parsed.run.cmd, "./demo");
    }

    #[test]
    fn rejects_an_artifact_path_that_escapes_the_build_directory() {
        let mut config = Preset::Generic.template("demo");
        config.artifacts.include = vec![PathBuf::from("../secret")];

        assert!(config.validate().is_err());
    }

    #[test]
    fn detects_a_preset_from_a_pure_file_snapshot() {
        let preset = Preset::detect(DetectedFiles {
            package_json: true,
            ..DetectedFiles::default()
        });

        assert_eq!(preset, Preset::Node);
    }
}
