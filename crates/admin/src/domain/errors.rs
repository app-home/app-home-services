use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdminError {
    #[error("User not found: {0}")]
    NotFound(uuid::Uuid),
    #[error("Not authorized")]
    Unauthorized,
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}
