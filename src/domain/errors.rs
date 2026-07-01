use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid username or password")]
    InvalidCredentials,

    #[error("User not found")]
    UserNotFound,

    #[error("Token verification failed")]
    TokenVerificationFailed,

    #[error("Internal error: {0}")]
    InternalError(String),
}
