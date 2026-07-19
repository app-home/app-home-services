use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use shared::domain::events::Event;
use utoipa::ToSchema;

use crate::AppState;
use crate::adapters::inbound::login_routes::resolve_client_ip;
use crate::adapters::inbound::responses::{ErrorResponse, GoogleAuthResponse};
use crate::application::use_cases::login_with_google;
use crate::domain::errors::AuthError;

const MAX_ID_TOKEN_LEN: usize = 16384;

#[derive(Deserialize, ToSchema)]
pub struct GoogleLoginRequest {
    #[schema(min_length = 1, max_length = 16384, example = "eyJhbGciOiJSUzI1NiIs...placeholder")]
    id_token: String,
}

#[utoipa::path(
    post,
    path = "/api/auth/login/google",
    request_body = GoogleLoginRequest,
    responses(
        (status = 200, description = "Google login successful", body = GoogleAuthResponse),
        (status = 401, description = "Token verification failed", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn login_google_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(req): Json<GoogleLoginRequest>,
) -> Response {
    let ip = resolve_client_ip(peer.ip(), &headers, &state.settings.trusted_proxy_ips);

    if req.id_token.is_empty() || req.id_token.len() > MAX_ID_TOKEN_LEN {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: "ID token is required and must not exceed 16384 characters".into(),
            }),
        )
            .into_response();
    }

    if !state.rate_limiter.try_check_and_record(ip).await {
        tracing::warn!(%ip, "Google login rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "Too many login attempts. Please try again later.".into(),
            }),
        )
            .into_response();
    }

    match login_with_google::login_with_google(
        &state.user_repo,
        &state.session_repo,
        &state.auth_provider,
        &state.jwt_service,
        &state.settings,
        &req.id_token,
    )
    .await
    {
        Ok(result) => {
            state.rate_limiter.reset(ip).await;

            tracing::info!(
                user_id = %result.user.id(),
                is_new_user = result.is_new_user,
                method = "google_oauth",
                "Login successful"
            );

            state.event_bus.publish(Event::UserLoggedIn(result.login_event));
            if let Some(created) = result.created_event {
                state.event_bus.publish(Event::UserCreated(created));
            }

            (
                StatusCode::OK,
                Json(GoogleAuthResponse {
                    status: "authenticated".into(),
                    user_id: result.user.id().to_string(),
                    access_token: result.access_token,
                    refresh_token: result.refresh_token,
                    is_new_user: result.is_new_user,
                }),
            )
                .into_response()
        }
        Err(AuthError::TokenVerificationFailed) => {
            tracing::warn!("Google token verification failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Authentication failed".into(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Google login error");
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
