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
use crate::adapters::inbound::responses::{AuthTokensResponse, ErrorResponse};
use crate::application::use_cases::login_with_password;
use crate::domain::errors::AuthError;

const MAX_USERNAME_LEN: usize = 256;
const MAX_PASSWORD_LEN: usize = 4096;

#[derive(Deserialize, ToSchema)]
pub struct PasswordLoginRequest {
    #[schema(min_length = 1, max_length = 256, example = "jdoe")]
    username: String,
    #[schema(min_length = 1, max_length = 4096, example = "hunter2")]
    password: String,
}

pub fn resolve_client_ip(
    peer_ip: IpAddr,
    headers: &axum::http::HeaderMap,
    trusted_proxies: &[IpAddr],
) -> IpAddr {
    if !trusted_proxies.contains(&peer_ip) {
        return peer_ip;
    }

    if let Some(value) = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse::<IpAddr>().ok())
    {
        return value;
    }

    if let Some(value) = headers
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<IpAddr>().ok())
    {
        return value;
    }

    peer_ip
}

#[utoipa::path(
    post,
    path = "/api/auth/login/password",
    request_body = PasswordLoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthTokensResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn login_password_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PasswordLoginRequest>,
) -> Response {
    let ip = resolve_client_ip(peer.ip(), &headers, &state.trusted_proxy_ips);

    if req.username.is_empty()
        || req.password.is_empty()
        || req.username.len() > MAX_USERNAME_LEN
        || req.password.len() > MAX_PASSWORD_LEN
    {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "Username and password are required and must not exceed 256 and 4096 characters respectively".into(),
            }),
        )
            .into_response();
    }

    if !state.rate_limiter.try_check_and_record(ip).await {
        tracing::warn!(%ip, "Rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "Too many login attempts. Please try again later.".into(),
            }),
        )
            .into_response();
    }

    match login_with_password::login_with_password(
        &state.user_repo,
        &state.jwt_service,
        &state.auth_settings,
        &req.username,
        &req.password,
    )
    .await
    {
        Ok(result) => {
            tracing::info!(user_id = %result.user.id(), method = "password", "Login successful");

            state.rate_limiter.reset(ip).await;
            for event in &result.events {
                state.event_bus.publish(event.clone());
            }

            (
                StatusCode::OK,
                Json(AuthTokensResponse {
                    status: "authenticated".into(),
                    user_id: result.user.id().to_string(),
                    access_token: result.access_token,
                    refresh_token: result.refresh_token,
                }),
            )
                .into_response()
        }
        Err(AuthError::InvalidCredentials) => {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            tracing::warn!(username = %req.username, "Invalid login attempt");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid username or password".into(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            tracing::error!(error = %e, "Login error");
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
