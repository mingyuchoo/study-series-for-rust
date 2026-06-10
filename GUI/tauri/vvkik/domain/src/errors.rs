use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("VVKIK item not found")]
    ItemNotFound,

    #[error("Invalid VVKIK data: {0}")]
    InvalidVvkikData(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
