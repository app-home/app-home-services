use std::net::{IpAddr, SocketAddr};

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::AppState;
use crate::adapters::inbound::login_routes::resolve_client_ip;
use crate::application::use_cases::record_audit_entry;
use crate::application::use_cases::refresh_token;
use crate::domain::errors::AuthError;

#[derive(Deserialize)]
pub struct RefreshTokenRequest {
    refresh_token: String,
}

pub async fn refresh_token_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let ip: IpAddr = resolve_client_ip(peer.ip(), &headers, &state.settings.trusted_proxy_ips);

    if !state.refresh_rate_limiter.check(ip).await {
        tracing::warn!(%ip, "Refresh rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(
                serde_json::json!({"error": "Too many refresh attempts. Please try again later."}),
            ),
        );
    }
    state.refresh_rate_limiter.record_attempt(ip).await;

    if req.refresh_token.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "refresh_token is required"})),
        );
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
                "password".to_string(),
            )
            .await
            {
                tracing::error!(error = %e, "Failed to record refresh audit entry");
            }

            tracing::info!(user_id = %result.user_id, "Token refresh successful");

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "access_token": result.access_token,
                    "refresh_token": result.refresh_token
                })),
            )
        }
        Err(AuthError::InvalidRefreshToken) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid or expired refresh token"})),
        ),
        Err(AuthError::TokenVerificationFailed) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid refresh token"})),
        ),
        Err(AuthError::SessionExpired | AuthError::SessionInvalidated) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Session has expired"})),
        ),
        Err(e) => {
            tracing::error!(error = %e, "Token refresh error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        }
    }
}
