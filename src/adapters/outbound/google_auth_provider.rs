use async_trait::async_trait;

use crate::application::ports::auth_provider::{AuthProvider, GoogleUserInfo};
use crate::domain::errors::AuthError;

#[derive(Debug, Clone)]
pub struct GoogleAuthProvider {
    client_id: String,
}

impl GoogleAuthProvider {
    pub fn new(client_id: String) -> Self {
        Self { client_id }
    }
}

#[async_trait]
impl AuthProvider for GoogleAuthProvider {
    async fn verify_id_token(&self, token: &str) -> Result<GoogleUserInfo, AuthError> {
        let client = reqwest::Client::new();

        let jwks = client
            .get("https://www.googleapis.com/oauth2/v3/certs")
            .send()
            .await
            .map_err(|_e| AuthError::TokenVerificationFailed)?;

        let jwks: jsonwebtoken::jwk::JwkSet = jwks
            .json()
            .await
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let header = jsonwebtoken::decode_header(token)
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let kid = header.kid.as_ref()
            .ok_or(AuthError::TokenVerificationFailed)?;

        let jwk = jwks.find(kid)
            .ok_or(AuthError::TokenVerificationFailed)?;

        let decoding_key = jsonwebtoken::DecodingKey::from_jwk(jwk)
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
        validation.set_audience(&[&self.client_id]);
        validation.set_required_spec_claims(&["sub", "email", "iss", "aud"]);

        let token_data = jsonwebtoken::decode::<GoogleClaims>(
            token,
            &decoding_key,
            &validation,
        )
        .map_err(|_| AuthError::TokenVerificationFailed)?;

        let claims = token_data.claims;

        let email = claims.email.ok_or(AuthError::TokenVerificationFailed)?;
        let name = claims.name.unwrap_or_else(|| email.clone());

        Ok(GoogleUserInfo { email, name })
    }
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct GoogleClaims {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    iss: String,
    aud: String,
    exp: usize,
    iat: usize,
}
