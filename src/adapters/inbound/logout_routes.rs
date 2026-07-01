use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::adapters::inbound::auth_middleware::AuthenticatedUser;
use crate::application::use_cases::logout;
use crate::application::use_cases::record_audit_entry;
use crate::domain::errors::AuthError;

#[derive(Deserialize)]
pub struct LogoutRequest {
    session_id: Uuid,
}

pub async fn logout_handler(
    auth_user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> impl IntoResponse {
    match logout::logout(
        &state.session_repo,
        &state.user_repo,
        auth_user.user_id,
        req.session_id,
    )
    .await
    {
        Ok(()) => {
            if let Err(e) = record_audit_entry::record_audit_entry(
                &state.user_repo,
                auth_user.user_id,
                Some(req.session_id),
                "logout",
                "password".to_string(),
            )
            .await
            {
                tracing::error!(error = %e, "Failed to record logout audit entry");
            }

            tracing::info!(user_id = %auth_user.user_id, session_id = %req.session_id, "Logout successful");

            (
                StatusCode::OK,
                Json(serde_json::json!({"status": "logged_out"})),
            )
        }
        Err(
            AuthError::SessionNotFound | AuthError::SessionInvalidated | AuthError::SessionExpired,
        ) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid session"})),
        ),
        Err(e) => {
            tracing::error!(error = %e, "Logout error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        }
    }
}
