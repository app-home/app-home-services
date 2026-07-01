use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid username or password")]
    InvalidCredentials,

    #[error("User not found")]
    UserNotFound,

    #[error("Token verification failed")]
    TokenVerificationFailed,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Session has expired")]
    SessionExpired,

    #[error("Session is no longer active")]
    SessionInvalidated,

    #[error("Refresh token is invalid")]
    InvalidRefreshToken,

    #[error("Rate limit exceeded")]
    RateLimited,

    #[error("Token generation failed")]
    TokenGenerationFailed,

    #[error("Internal error: {0}")]
    InternalError(String),
}
