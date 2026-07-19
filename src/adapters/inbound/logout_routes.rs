use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use shared::domain::events::Event;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::adapters::inbound::auth_middleware::AuthenticatedUser;
use crate::adapters::inbound::responses::{ErrorResponse, StatusResponse};
use crate::application::use_cases::logout;
use crate::domain::errors::AuthError;

#[derive(Deserialize, ToSchema)]
pub struct LogoutRequest {
    #[schema(example = "018f9a8b-7c3d-4e5f-8a1b-2c3d4e5f6a7b")]
    session_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/auth/logout",
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
    Json(req): Json<LogoutRequest>,
) -> Response {
    match logout::logout(
        &state.session_repo,
        &state.user_repo,
        auth_user.user_id,
        req.session_id,
    )
    .await
    {
        Ok((_auth_method, event)) => {
            state.event_bus.publish(Event::UserLoggedOut(event));

            tracing::info!(user_id = %auth_user.user_id, session_id = %req.session_id, "Logout successful");

            (
                StatusCode::OK,
                Json(StatusResponse {
                    status: "logged_out".into(),
                }),
            )
                .into_response()
        }
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
