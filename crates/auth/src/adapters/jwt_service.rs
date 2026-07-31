use chrono::Utc;
use jsonwebtoken::{EncodingKey, Header};
use uuid::Uuid;

use crate::application::ports::jwt_service::{
    AccessTokenClaims, JwtService, RefreshTokenClaims, TokenPair,
};
use crate::domain::errors::AuthError;

#[derive(Clone)]
pub struct JwtServiceImpl {
    encoding_key: EncodingKey,
    verification: shared::auth::JwtVerification,
    access_expiry_minutes: i64,
    refresh_expiry_days: i64,
}

impl JwtServiceImpl {
    pub fn new(
        secret: &str,
        access_expiry_minutes: i64,
        refresh_expiry_days: i64,
        issuer: &str,
        audience: &str,
    ) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            verification: shared::auth::JwtVerification::new(
                secret,
                issuer.to_string(),
                audience.to_string(),
            ),
            access_expiry_minutes,
            refresh_expiry_days,
        }
    }
}

impl JwtService for JwtServiceImpl {
    fn generate_token_pair(&self, user_id: Uuid, session_id: Uuid) -> Result<TokenPair, AuthError> {
        let now = Utc::now().timestamp() as usize;

        let access_claims = AccessTokenClaims {
            sub: user_id,
            iss: self.verification.issuer.clone(),
            aud: self.verification.audience.clone(),
            exp: now + (self.access_expiry_minutes as usize * 60),
            iat: now,
        };

        let refresh_claims = RefreshTokenClaims {
            sub: user_id,
            session_id,
            iss: self.verification.issuer.clone(),
            aud: self.verification.audience.clone(),
            exp: now + (self.refresh_expiry_days as usize * 86400),
            iat: now,
        };

        let access_token =
            jsonwebtoken::encode(&Header::default(), &access_claims, &self.encoding_key)
                .map_err(|_| AuthError::TokenGenerationFailed)?;

        let refresh_token =
            jsonwebtoken::encode(&Header::default(), &refresh_claims, &self.encoding_key)
                .map_err(|_| AuthError::TokenGenerationFailed)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
        })
    }

    fn validate_access_token(&self, token: &str) -> Result<AccessTokenClaims, AuthError> {
        self.verification
            .decode::<AccessTokenClaims>(token)
            .ok_or(AuthError::TokenVerificationFailed)
    }

    fn validate_refresh_token(&self, token: &str) -> Result<RefreshTokenClaims, AuthError> {
        self.verification
            .decode::<RefreshTokenClaims>(token)
            .ok_or(AuthError::TokenVerificationFailed)
    }
}
