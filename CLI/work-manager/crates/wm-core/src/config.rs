use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};

/// Name of the per-project manifest.
pub const MANIFEST: &str = "work-manager.toml";
/// Directory holding all tool-managed state, relative to the project root.
pub const STATE_DIR: &str = ".wm";
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
    pub fn is_empty(&self) -> bool {
        self.cmd.trim().is_empty()
    }
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

fn default_stop_timeout() -> u64 {
    10
}

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

fn default_health_timeout() -> u64 {
    30
}

fn default_health_interval() -> u64 {
    1
}

impl HealthCheck {
    /// A manifest with no probe configured activates without verification.
    pub fn is_configured(&self) -> bool {
        self.http.is_some() || self.tcp.is_some() || self.cmd.is_some()
    }
}

/// The outcome of locating a project: which manifest applies, where its root
/// is, and whether the command was invoked from inside a managed worktree.
#[derive(Debug, Clone)]
pub struct Discovery {
    pub config: Config,
    pub root: PathBuf,
    /// `Some(name)` when the current directory lies inside
    /// `<root>/.wm/worktrees/<name>`.
    pub worktree: Option<String>,
}

/// Recognise `<root>/.wm/worktrees/<name>/...`, innermost match first.
fn worktree_context(start: &Path) -> Option<(PathBuf, String)> {
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
    /// Locate the project from `start`, resolving worktree context.
    pub fn discover(start: &Path) -> Result<Discovery> {
        let start = start
            .canonicalize()
            .ctx(format!("resolving {}", start.display()))?;

        // A worktree is a full checkout, so it carries its own copy of the
        // manifest. Walking up naively would take the worktree for the project
        // root and open a second, empty generation store inside it — the real
        // project's generations would simply vanish from view.
        if let Some((root, name)) = worktree_context(&start) {
            let manifest = root.join(MANIFEST);
            if manifest.is_file() {
                return Ok(Discovery {
                    config: Config::load(&manifest)?,
                    root,
                    worktree: Some(name),
                });
            }
        }

        for dir in start.ancestors() {
            let candidate = dir.join(MANIFEST);
            if candidate.is_file() {
                return Ok(Discovery {
                    config: Config::load(&candidate)?,
                    root: dir.to_path_buf(),
                    worktree: None,
                });
            }
        }
        Err(Error::ProjectNotFound(start))
    }

    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).ctx(format!("reading {}", path.display()))?;
        let config: Config = toml::from_str(&text)?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text).ctx(format!("writing {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
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

/// Starting points for `wm init`, so the multi-language case is a one-liner.
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
            "rust" | "cargo" => Ok(Preset::Rust),
            "node" | "npm" | "js" | "ts" => Ok(Preset::Node),
            "python" | "py" => Ok(Preset::Python),
            "go" | "golang" => Ok(Preset::Go),
            "generic" | "none" => Ok(Preset::Generic),
            other => Err(format!("unknown preset `{other}`")),
        }
    }
}

impl Preset {
    /// Guess from the files present in `dir`.
    pub fn detect(dir: &Path) -> Preset {
        if dir.join("Cargo.toml").is_file() {
            Preset::Rust
        } else if dir.join("package.json").is_file() {
            Preset::Node
        } else if dir.join("go.mod").is_file() {
            Preset::Go
        } else if dir.join("pyproject.toml").is_file() || dir.join("requirements.txt").is_file() {
            Preset::Python
        } else {
            Preset::Generic
        }
    }

    pub fn template(self, name: &str) -> Config {
        let (build, test, include, run) = match self {
            Preset::Rust => (
                "cargo build --release".to_string(),
                "cargo test".to_string(),
                vec![PathBuf::from(format!("target/release/{name}"))],
                format!("./{name}"),
            ),
            Preset::Node => (
                "npm ci && npm run build".to_string(),
                "npm test".to_string(),
                vec![
                    PathBuf::from("dist"),
                    PathBuf::from("package.json"),
                    PathBuf::from("package-lock.json"),
                ],
                "node dist/main.js".to_string(),
            ),
            Preset::Python => (
                "python -m pip install -r requirements.txt --target vendor".to_string(),
                "python -m pytest".to_string(),
                vec![PathBuf::from("src"), PathBuf::from("vendor")],
                "python -m src.main".to_string(),
            ),
            Preset::Go => (
                format!("go build -o bin/{name} ./cmd/{name}"),
                "go test ./...".to_string(),
                vec![PathBuf::from("bin")],
                format!("./bin/{name}"),
            ),
            Preset::Generic => (
                "make build".to_string(),
                "make test".to_string(),
                vec![PathBuf::from("build")],
                "./build/start".to_string(),
            ),
        };

        Config {
            project: Project { name: name.to_string() },
            build: BuildStage { cmd: build, env: BTreeMap::new() },
            test: BuildStage { cmd: test, env: BTreeMap::new() },
            artifacts: Artifacts { include },
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::worktree_context;

    fn context(path: &str) -> Option<(PathBuf, String)> {
        worktree_context(Path::new(path))
    }

    #[test]
    fn recognises_a_managed_worktree() {
        let (root, name) = context("/srv/demo/.wm/worktrees/add-cache").unwrap();
        assert_eq!(root, Path::new("/srv/demo"));
        assert_eq!(name, "add-cache");
    }

    #[test]
    fn recognises_a_subdirectory_of_a_worktree() {
        let (root, name) = context("/srv/demo/.wm/worktrees/add-cache/src/api").unwrap();
        assert_eq!(root, Path::new("/srv/demo"));
        assert_eq!(name, "add-cache");
    }

    #[test]
    fn ignores_the_project_root_and_the_state_dir() {
        assert!(context("/srv/demo").is_none());
        assert!(context("/srv/demo/.wm").is_none());
        assert!(context("/srv/demo/.wm/store/0001-abc").is_none());
        // The worktrees directory itself is not a worktree.
        assert!(context("/srv/demo/.wm/worktrees").is_none());
    }

    #[test]
    fn ignores_a_lookalike_path_outside_the_state_dir() {
        assert!(context("/srv/demo/worktrees/add-cache").is_none());
        assert!(context("/srv/demo/other/worktrees/add-cache").is_none());
    }

    #[test]
    fn picks_the_innermost_worktree_when_nested() {
        let (root, name) =
            context("/srv/demo/.wm/worktrees/outer/.wm/worktrees/inner/src").unwrap();
        assert_eq!(root, Path::new("/srv/demo/.wm/worktrees/outer"));
        assert_eq!(name, "inner");
    }
}
