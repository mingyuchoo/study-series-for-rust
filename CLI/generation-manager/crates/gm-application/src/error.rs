pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type PortResult<T> = std::result::Result<T, BoxError>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Domain(#[from] gm_core::Error),

    #[error("{operation}: {source}")]
    Port {
        operation: &'static str,
        #[source]
        source: BoxError,
    },
}

impl Error {
    pub(crate) fn port<T>(operation: &'static str, result: PortResult<T>) -> Result<T> {
        result.map_err(|source| Error::Port {
            operation,
            source,
        })
    }
}
