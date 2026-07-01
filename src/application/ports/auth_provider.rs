use async_trait::async_trait;

use crate::domain::errors::AuthError;

#[derive(Debug, Clone)]
pub struct GoogleUserInfo {
    pub email: String,
    pub name: String,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn verify_id_token(&self, token: &str) -> Result<GoogleUserInfo, AuthError>;
}
