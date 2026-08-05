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
    /// Builds the verification config from the HMAC `secret` and the expected
    /// `issuer`/`audience` claim values.
    pub fn new(secret: &str, issuer: String, audience: String) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            audience,
        }
    }

    /// Validation requiring HS256 and the configured `iss`/`aud` claims.
    /// `exp`, `iss`, and `aud` are all explicitly listed in
    /// `set_required_spec_claims`, so a token that omits any of them is
    /// rejected as a missing-claim error rather than relying on
    /// `jsonwebtoken`'s own default (`exp` only) to keep catching the other
    /// two -- this is defense-in-depth against that default ever changing
    /// out from under `set_issuer`/`set_audience`'s value checks, not a
    /// behavior change: a token without `iss`/`aud` was already rejected
    /// (see `crates/auth/tests/jwt_test.rs`).
    pub fn validation(&self) -> Validation {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation
    }

    /// Decodes and validates `token`, returning its claims, or `None` if the
    /// signature, algorithm, `iss`, `aud`, or `exp` check fails.
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
    /// `i64` rather than `usize` for the same platform/overflow reasons as the
    /// claim itself (see #95).
    pub exp: i64,
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
            let Some(auth_header) = parts
                .headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
            else {
                // Routine and expected for any anonymous/unauthenticated request
                // hitting a protected route, so this stays at debug -- logging it
                // at warn/error would just be noise on every logged-out client.
                tracing::debug!("Auth rejected: missing or non-UTF-8 Authorization header");
                return Err(AuthRejection);
            };
            let Some(token) = auth_header.strip_prefix("Bearer ") else {
                tracing::debug!("Auth rejected: Authorization header is not a Bearer token");
                return Err(AuthRejection);
            };
            token.to_string()
        };

        let Extension(verification) =
            Extension::<Arc<JwtVerification>>::from_request_parts(parts, _state)
                .await
                .map_err(|_| {
                    // Unlike the other rejections in this function, this one
                    // isn't caused by anything the client sent -- it means this
                    // route wasn't wired up with the JwtVerification Extension
                    // layer at all, which is a deployment/config bug, not a
                    // client error. Every request to an affected route would
                    // 401 regardless of how valid its token is, so this is
                    // worth its own error-level line to stand out from normal
                    // auth-rejection noise.
                    tracing::error!(
                        "Auth rejected: JwtVerification extension missing -- this route is misconfigured (missing Extension layer), not a client error"
                    );
                    AuthRejection
                })?;

        let Extension(blacklist) =
            Extension::<Arc<dyn AccessTokenBlacklist>>::from_request_parts(parts, _state)
                .await
                .map_err(|_| {
                    tracing::error!(
                        "Auth rejected: AccessTokenBlacklist extension missing -- this route is misconfigured (missing Extension layer), not a client error"
                    );
                    AuthRejection
                })?;

        #[derive(Deserialize)]
        struct Claims {
            sub: Uuid,
            jti: Uuid,
            exp: i64,
        }

        let Some(claims) = verification.decode::<Claims>(&token) else {
            // Expected background noise (expired tokens, tokens from another
            // environment after an iss/aud mismatch, tampered/garbage tokens) --
            // debug level so it's available when actively investigating a "why
            // am I getting 401s" report, without polluting normal logs.
            tracing::debug!(
                "Auth rejected: token failed signature/algorithm/iss/aud/exp validation"
            );
            return Err(AuthRejection);
        };

        // Revoked access tokens (e.g. after logout, see #88) are rejected before
        // the request reaches the handler. If the blacklist backend is unavailable
        // the check fails open -- the token is allowed -- so a Redis outage degrades
        // revocation, not every authenticated request.
        match blacklist.is_revoked(claims.jti).await {
            Ok(true) => {
                tracing::debug!(
                    user_id = %claims.sub,
                    jti = %claims.jti,
                    "Auth rejected: access token is revoked"
                );
                return Err(AuthRejection);
            }
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
