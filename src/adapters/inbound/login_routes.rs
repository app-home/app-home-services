use std::net::IpAddr;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;

use crate::AppState;
use crate::application::ports::rate_limiter::RateLimiter;
use crate::application::use_cases::login_with_password;
use crate::application::use_cases::record_audit_entry;
use crate::domain::errors::AuthError;

#[derive(Deserialize)]
pub struct PasswordLoginRequest {
    username: String,
    password: String,
}

fn client_ip(headers: &axum::http::HeaderMap) -> IpAddr {
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
    IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
}

pub async fn login_password_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PasswordLoginRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers);

    {
        let mut limiter = state.rate_limiter.lock().unwrap();
        if !limiter.check(ip) {
            tracing::warn!(%ip, "Rate limit exceeded");
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(
                    serde_json::json!({"error": "Too many login attempts. Please try again later."}),
                ),
            );
        }
        limiter.record_attempt(ip);
    }

    if req.username.is_empty() || req.password.is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "Username and password are required"})),
        );
    }

    match login_with_password::login_with_password(
        &state.user_repo,
        &state.session_repo,
        &state.jwt_service,
        &state.settings,
        &req.username,
        &req.password,
    )
    .await
    {
        Ok(result) => {
            tracing::info!(user_id = %result.user.id, method = "password", "Login successful");

            {
                let mut limiter = state.rate_limiter.lock().unwrap();
                limiter.reset(ip);
            }

            if let Err(e) = record_audit_entry::record_audit_entry(
                &state.user_repo,
                result.user.id,
                Some(result.session_id),
                "login",
                "password".to_string(),
            )
            .await
            {
                tracing::error!(error = %e, "Failed to record audit entry");
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "authenticated",
                    "user_id": result.user.id.to_string(),
                    "access_token": result.access_token,
                    "refresh_token": result.refresh_token
                })),
            )
        }
        Err(AuthError::InvalidCredentials) => {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            tracing::warn!(username = %req.username, "Invalid login attempt");
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid username or password"})),
            )
        }
        Err(e) => {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            tracing::error!(error = %e, "Login error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        }
    }
}
