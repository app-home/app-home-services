use std::sync::Arc;

use axum::extract::Extension;
use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::ErrorResponse;
use crate::ports::{AccessTokenBlacklist, BlacklistError};

/// Explicit issuer/audience for the app's own JWTs, so tokens are rejected
/// unless they were issued by this service instance for this audience. Both are
/// env-configurable (`JWT_ISSUER`/`JWT_AUDIENCE`) so each environment can use a
/// distinct value; without this, a token minted in one environment (e.g.
/// staging) would also validate in any other that shares the same `JWT_SECRET`.
/// See #87.
#[derive(Clone)]
pub struct JwtVerification {
    decoding_key: DecodingKey,
    pub issuer: String,
    pub audience: String,
}

impl JwtVerification {
    pub fn new(secret: &str, issuer: String, audience: String) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            audience,
        }
    }

    pub fn validation(&self) -> Validation {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation
    }

    pub fn decode<T: serde::de::DeserializeOwned>(&self, token: &str) -> Option<T> {
        jsonwebtoken::decode::<T>(token, &self.decoding_key, &self.validation())
            .ok()
            .map(|data| data.claims)
    }
}

pub struct AuthenticatedUser {
    pub user_id: Uuid,
    /// Unique id of the presented access token (`jti` claim), so routes like
    /// logout can revoke exactly that token.
    pub jti: Uuid,
    /// `exp` claim of the presented access token (unix seconds), so the
    /// remaining lifetime (and thus the blacklist TTL) can be computed.
    pub exp: usize,
}

#[derive(Debug)]
pub struct AuthRejection;

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".into(),
            }),
        )
            .into_response()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = {
            let auth_header = parts
                .headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .ok_or(AuthRejection)?;
            auth_header
                .strip_prefix("Bearer ")
                .ok_or(AuthRejection)?
                .to_string()
        };

        let Extension(verification) =
            Extension::<Arc<JwtVerification>>::from_request_parts(parts, _state)
                .await
                .map_err(|_| AuthRejection)?;

        let Extension(blacklist) =
            Extension::<Arc<dyn AccessTokenBlacklist>>::from_request_parts(parts, _state)
                .await
                .map_err(|_| AuthRejection)?;

        #[derive(Deserialize)]
        struct Claims {
            sub: Uuid,
            jti: Uuid,
            exp: usize,
        }

        let claims = verification.decode::<Claims>(&token).ok_or(AuthRejection)?;

        // Revoked access tokens (e.g. after logout, see #88) are rejected before
        // the request reaches the handler. If the blacklist backend is unavailable
        // the check fails open -- the token is allowed -- matching the rate
        // limiter's posture, so a Redis outage degrades availability, not every
        // authenticated request.
        match blacklist.is_revoked(claims.jti).await {
            Ok(true) => return Err(AuthRejection),
            Ok(false) => {}
            Err(BlacklistError) => {
                tracing::warn!(
                    user_id = %claims.sub,
                    jti = %claims.jti,
                    "Access token blacklist check failed, failing open"
                );
            }
        }

        Ok(AuthenticatedUser {
            user_id: claims.sub,
            jti: claims.jti,
            exp: claims.exp,
        })
    }
}
