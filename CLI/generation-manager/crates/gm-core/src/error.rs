use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no generation-manager.toml found in {0} or any parent directory (run `gm init`)")]
    ProjectNotFound(PathBuf),

    #[error("generation-manager.toml is already present at {0}")]
    AlreadyInitialized(PathBuf),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("generation {0} does not exist")]
    NoSuchGeneration(u64),

    #[error("no generation is currently active")]
    NoCurrentGeneration,

    #[error("there is no older generation to roll back to")]
    NoPreviousGeneration,

    #[error("another `gm` process holds the project lock ({0})")]
    Locked(PathBuf),

    #[error("worktree `{0}` already exists")]
    WorktreeExists(String),

    #[error("worktree `{0}` does not exist")]
    NoSuchWorktree(String),

    #[error("`{stage}` failed with exit code {code}")]
    StageFailed { stage: String, code: i32 },

    #[error("health check did not pass within {0}s")]
    HealthTimeout(u64),

    #[error("the service is not running")]
    NotRunning,

    #[error("the service is already running (pid {0})")]
    AlreadyRunning(i32),

    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
}

impl Error {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }
}

/// Attach a human-readable context string to an `io::Result`.
pub trait IoContext<T> {
    fn ctx(self, context: impl Into<String>) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn ctx(self, context: impl Into<String>) -> Result<T> { self.map_err(|e| Error::io(context, e)) }
}
