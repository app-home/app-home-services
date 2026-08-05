use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::adapters::inbound::responses::{ErrorResponse, StatusResponse};
use crate::application::use_cases::logout;
use crate::domain::errors::AuthError;
use shared::auth::AuthenticatedUser;
use shared::ports::{AccessTokenBlacklist, BlacklistError};

#[derive(Deserialize, ToSchema)]
pub struct LogoutRequest {
    #[schema(example = "018f9a8b-7c3d-4e5f-8a1b-2c3d4e5f6a7b")]
    session_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "Authentication",
    request_body = LogoutRequest,
    security(("bearer_jwt" = [])),
    responses(
        (status = 200, description = "Logout successful", body = StatusResponse),
        (status = 400, description = "Invalid session", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn logout_handler(
    auth_user: AuthenticatedUser,
    State(state): State<AppState>,
    Extension(blacklist): Extension<Arc<dyn AccessTokenBlacklist>>,
    Json(req): Json<LogoutRequest>,
) -> Response {
    match logout::logout(&state.user_repo, auth_user.user_id, req.session_id).await {
        Ok(events) => {
            for event in &events {
                state.event_bus.publish(event.clone());
            }

            tracing::info!(user_id = %auth_user.user_id, session_id = %req.session_id, "Logout successful");

            // Revoke the presented access token so it stops validating until its
            // natural expiry (see #88). TTL is the token's remaining lifetime.
            // Best-effort and fail-open: if the revocation list backend is
            // unavailable the session is still closed and logout still succeeds.
            // Since #140, the Redis-backed revocation is also durable: a revoke()
            // that Redis rejects is journaled in Postgres and retried by a
            // background flush worker, so a Redis outage delays -- but never
            // silently drops -- the revocation. revoke() only surfaces an error
            // when Redis AND Postgres are both down at once, in which case the
            // token stays valid until its natural expiry (logged at error level).
            let ttl_secs = revocation_ttl_secs(auth_user.exp, chrono::Utc::now().timestamp());
            match blacklist.revoke(auth_user.jti, ttl_secs).await {
                Ok(()) => {
                    tracing::info!(
                        user_id = %auth_user.user_id,
                        jti = %auth_user.jti,
                        ttl_secs,
                        "Access token revoked on logout"
                    );
                }
                Err(BlacklistError) => {
                    tracing::error!(
                        user_id = %auth_user.user_id,
                        jti = %auth_user.jti,
                        "Failed to revoke access token on logout"
                    );
                }
            }

            (
                StatusCode::OK,
                Json(StatusResponse {
                    status: "logged_out".into(),
                }),
            )
                .into_response()
        }
        Err(AuthError::UserNotFound) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Authentication required".into(),
            }),
        )
            .into_response(),
        Err(
            AuthError::SessionNotFound | AuthError::SessionInvalidated | AuthError::SessionExpired,
        ) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid session".into(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Logout error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".into(),
                }),
            )
                .into_response()
        }
    }
}

/// `jsonwebtoken`'s default `Validation` leeway, applied around `exp` when
/// validating tokens (see `shared::auth::JwtVerification::validation`). A token
/// expired less than this many seconds ago is still accepted, so the revocation
/// entry must outlive it by the same window or the revoked token would keep
/// validating after logout (CWE-613). Keep in sync with `jsonwebtoken`'s
/// default.
const JWT_EXP_LEEWAY_SECS: i64 = 60;

/// Remaining acceptance window of an access token, in whole seconds, used as
/// the revocation list TTL so the entry lives exactly as long as the token
/// could still validate (its nominal lifetime plus the JWT `exp` leeway).
/// `now` is taken as a parameter for testability. Timestamps are `i64`
/// (see #95).
fn revocation_ttl_secs(exp: i64, now: i64) -> u64 {
    (exp + JWT_EXP_LEEWAY_SECS - now).max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::revocation_ttl_secs;

    #[test]
    fn ttl_is_remaining_acceptance_window() {
        assert_eq!(revocation_ttl_secs(1_800, 1_000), 860);
    }

    #[test]
    fn ttl_covers_the_exp_leeway_window() {
        assert_eq!(revocation_ttl_secs(1_000, 1_030), 30);
    }

    #[test]
    fn ttl_clamps_at_zero_beyond_the_leeway_window() {
        assert_eq!(revocation_ttl_secs(1_000, 1_800), 0);
        assert_eq!(revocation_ttl_secs(1_000, 1_060), 0);
    }
}
