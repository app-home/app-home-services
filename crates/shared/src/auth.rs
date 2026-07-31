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
}

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

        #[derive(Deserialize)]
        struct Claims {
            sub: Uuid,
        }

        let claims = verification.decode::<Claims>(&token).ok_or(AuthRejection)?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
        })
    }
}
