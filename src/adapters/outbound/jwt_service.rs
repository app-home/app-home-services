use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use uuid::Uuid;

use crate::application::ports::jwt_service::{
    AccessTokenClaims, JwtService, RefreshTokenClaims, TokenPair,
};
use crate::domain::errors::AuthError;

#[derive(Clone)]
pub struct JwtServiceImpl {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    access_expiry_minutes: i64,
    refresh_expiry_days: i64,
}

impl JwtServiceImpl {
    pub fn new(secret: &str, access_expiry_minutes: i64, refresh_expiry_days: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
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
            exp: now + (self.access_expiry_minutes as usize * 60),
            iat: now,
        };

        let refresh_claims = RefreshTokenClaims {
            sub: user_id,
            session_id,
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
        let token_data = jsonwebtoken::decode::<AccessTokenClaims>(
            token,
            &self.decoding_key,
            &Validation::default(),
        )
        .map_err(|_| AuthError::TokenVerificationFailed)?;

        Ok(token_data.claims)
    }

    fn validate_refresh_token(&self, token: &str) -> Result<RefreshTokenClaims, AuthError> {
        let token_data = jsonwebtoken::decode::<RefreshTokenClaims>(
            token,
            &self.decoding_key,
            &Validation::default(),
        )
        .map_err(|_| AuthError::TokenVerificationFailed)?;

        Ok(token_data.claims)
    }
}
