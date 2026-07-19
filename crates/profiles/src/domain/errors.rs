use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("Profile not found for user {0}")]
    NotFound(uuid::Uuid),
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}
