use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("IKIK item not found")]
    ItemNotFound,

    #[error("Invalid IKIK data: {0}")]
    InvalidIkikData(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
