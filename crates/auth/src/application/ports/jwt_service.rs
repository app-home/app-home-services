use uuid::Uuid;

use crate::domain::errors::AuthError;

#[derive(Debug, Clone)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccessTokenClaims {
    pub sub: Uuid,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: Uuid,
    pub session_id: Uuid,
    pub exp: usize,
    pub iat: usize,
}

pub trait JwtService: Send + Sync {
    fn generate_token_pair(&self, user_id: Uuid, session_id: Uuid) -> Result<TokenPair, AuthError>;
    fn validate_access_token(&self, token: &str) -> Result<AccessTokenClaims, AuthError>;
    fn validate_refresh_token(&self, token: &str) -> Result<RefreshTokenClaims, AuthError>;
}
