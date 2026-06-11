use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("IVKIK item not found")]
    ItemNotFound,

    #[error("Invalid IVKIK data: {0}")]
    InvalidIvkikData(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
