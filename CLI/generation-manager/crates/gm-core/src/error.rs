pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("no generation is currently active")]
    NoCurrentGeneration,

    #[error("there is no older generation to roll back to")]
    NoPreviousGeneration,
}
