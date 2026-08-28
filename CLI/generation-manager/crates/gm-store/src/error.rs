use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no generation-manager.toml found in {0} or any parent directory (run `gm init`)")]
    ProjectNotFound(PathBuf),

    #[error("generation {0} does not exist")]
    NoSuchGeneration(u64),

    #[error("no generation is currently active")]
    NoCurrentGeneration,

    #[error("there is no older generation to roll back to")]
    NoPreviousGeneration,

    #[error("another `gm` process holds the project lock ({0})")]
    Locked(PathBuf),

    #[error("a store mutation was attempted without its project lock")]
    InvalidLock,

    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Core(#[from] gm_core::Error),
}

pub trait IoContext<T> {
    fn ctx(self, context: impl Into<String>) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn ctx(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|source| Error::Io {
            context: context.into(),
            source,
        })
    }
}
