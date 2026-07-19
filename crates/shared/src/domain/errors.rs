use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Invalid email: {0}")]
    InvalidEmail(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
