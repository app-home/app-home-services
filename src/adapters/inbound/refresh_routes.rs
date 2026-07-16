use std::net::{IpAddr, SocketAddr};

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::adapters::inbound::login_routes::resolve_client_ip;
use crate::adapters::inbound::responses::{ErrorResponse, RefreshResponse};
use crate::application::use_cases::record_audit_entry;
use crate::application::use_cases::refresh_token;
use crate::domain::errors::AuthError;

#[derive(Deserialize, ToSchema)]
pub struct RefreshTokenRequest {
    #[schema(min_length = 1, example = "eyJhbGciOiJIUzI1NiIs...placeholder")]
    refresh_token: String,
}

#[utoipa::path(
    post,
    path = "/api/auth/refresh",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token refreshed", body = RefreshResponse),
        (status = 401, description = "Invalid or expired refresh token", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn refresh_token_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<RefreshTokenRequest>,
) -> Response {
    let ip: IpAddr = resolve_client_ip(peer.ip(), &headers, &state.settings.trusted_proxy_ips);

    if !state.refresh_rate_limiter.check(ip).await {
        tracing::warn!(%ip, "Refresh rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "Too many refresh attempts. Please try again later.".into(),
            }),
        )
            .into_response();
    }
    state.refresh_rate_limiter.record_attempt(ip).await;

    if req.refresh_token.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "refresh_token is required".into(),
            }),
        )
            .into_response();
    }

    match refresh_token::refresh_token(
        &state.session_repo,
        &state.user_repo,
        &state.jwt_service,
        &req.refresh_token,
        &state.settings,
    )
    .await
    {
        Ok(result) => {
            state.refresh_rate_limiter.reset(ip).await;

            if let Err(e) = record_audit_entry::record_audit_entry(
                &state.user_repo,
                result.user_id,
                Some(result.session_id),
                "refresh",
                result.auth_method.clone(),
            )
            .await
            {
                tracing::error!(error = %e, "Failed to record refresh audit entry");
            }

            tracing::info!(user_id = %result.user_id, "Token refresh successful");

            (
                StatusCode::OK,
                Json(RefreshResponse {
                    access_token: result.access_token,
                    refresh_token: result.refresh_token,
                }),
            )
                .into_response()
        }
        Err(AuthError::InvalidRefreshToken) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid or expired refresh token".into(),
            }),
        )
            .into_response(),
        Err(AuthError::TokenVerificationFailed) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid refresh token".into(),
            }),
        )
            .into_response(),
        Err(AuthError::SessionExpired | AuthError::SessionInvalidated) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Session has expired".into(),
            }),
        )
            .into_response(),
        // A missing session is an expected client-side outcome (e.g. the session was
        // deleted, or a token references a session id that no longer exists) -- not
        // an internal failure. Treat it the same as an invalid refresh token rather
        // than falling through to the 500 catch-all below, which both misreports it
        // to clients and pollutes error-rate monitoring with tracing::error! noise.
        Err(AuthError::SessionNotFound) => (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid or expired refresh token".into(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Token refresh error");
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
