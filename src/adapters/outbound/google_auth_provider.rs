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

        let header =
            jsonwebtoken::decode_header(token).map_err(|_| AuthError::TokenVerificationFailed)?;

        let kid = header
            .kid
            .as_ref()
            .ok_or(AuthError::TokenVerificationFailed)?;

        let jwk = jwks.find(kid).ok_or(AuthError::TokenVerificationFailed)?;

        let decoding_key = jsonwebtoken::DecodingKey::from_jwk(jwk)
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
        validation.set_audience(&[&self.client_id]);
        validation.set_required_spec_claims(&["sub", "email", "iss", "aud"]);

        let token_data = jsonwebtoken::decode::<GoogleClaims>(token, &decoding_key, &validation)
            .map_err(|_| AuthError::TokenVerificationFailed)?;

        let claims = token_data.claims;

        validate_google_claims(claims)
    }
}

/// Validates decoded Google ID token claims and turns them into `GoogleUserInfo`,
/// separately from the JWKS-fetching/signature-verification mechanics above so this
/// business logic (in particular, the `email_verified` check) can be unit-tested
/// directly against hand-built claims, without needing a real signed token or network
/// access to Google's JWKS endpoint.
pub fn validate_google_claims(claims: GoogleClaims) -> Result<GoogleUserInfo, AuthError> {
    let email = claims.email.ok_or(AuthError::TokenVerificationFailed)?;

    // Google's guidance for "Sign in with Google" is to check email_verified before
    // trusting the email claim as an identifier. Without this, an account with an
    // unverified email could be used to create or match a user record for an email
    // address its holder doesn't actually control.
    if claims.email_verified != Some(true) {
        tracing::warn!("Google login rejected: email_verified is not true");
        return Err(AuthError::TokenVerificationFailed);
    }

    let name = claims.name.unwrap_or_else(|| email.clone());

    Ok(GoogleUserInfo { email, name })
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GoogleClaims {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}
