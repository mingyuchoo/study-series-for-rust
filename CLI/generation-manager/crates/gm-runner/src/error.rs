pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid runner configuration: {0}")]
    Config(String),

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
    Store(#[from] gm_store::Error),

    #[error(transparent)]
    Application(#[from] gm_application::Error),
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
