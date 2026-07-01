use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;

use crate::AppState;
use crate::application::use_cases::login_with_google;
use crate::application::use_cases::record_audit_entry;
use crate::domain::errors::AuthError;

#[derive(Deserialize)]
pub struct GoogleLoginRequest {
    id_token: String,
}

pub async fn login_google_handler(
    State(state): State<AppState>,
    Json(req): Json<GoogleLoginRequest>,
) -> impl IntoResponse {
    if req.id_token.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "ID token is required"})),
        );
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
            tracing::info!(
                user_id = %result.user.id,
                is_new_user = result.is_new_user,
                method = "google_oauth",
                "Login successful"
            );

            if let Err(e) = record_audit_entry::record_audit_entry(
                &state.user_repo,
                result.user.id,
                Some(result.session_id),
                "login",
                "google_oauth".to_string(),
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
                    "refresh_token": result.refresh_token,
                    "is_new_user": result.is_new_user
                })),
            )
        }
        Err(AuthError::TokenVerificationFailed) => {
            tracing::warn!("Google token verification failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Authentication failed"})),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "Google login error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        }
    }
}
