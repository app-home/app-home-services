use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;

use crate::application::use_cases::login_with_password;
use crate::application::use_cases::record_audit_entry;
use crate::domain::errors::AuthError;
use crate::AppState;

#[derive(Deserialize)]
pub struct PasswordLoginRequest {
    username: String,
    password: String,
}

#[derive(serde::Serialize)]
pub struct LoginResponse {
    status: String,
    user_id: String,
}

#[derive(serde::Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub async fn login_password_handler(
    State(state): State<AppState>,
    Json(req): Json<PasswordLoginRequest>,
) -> impl IntoResponse {
    if req.username.is_empty() || req.password.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "Username and password are required"})),
        );
    }

    match login_with_password::login_with_password(
        &state.user_repo,
        &req.username,
        &req.password,
    )
    .await
    {
        Ok(user) => {
            tracing::info!(user_id = %user.id, method = "password", "Login successful");

            if let Err(e) = record_audit_entry::record_audit_entry(
                &state.user_repo,
                user.id,
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
                    "user_id": user.id.to_string()
                })),
            )
        }
        Err(AuthError::InvalidCredentials) => {
            tracing::warn!(username = %req.username, "Invalid login attempt");
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid username or password"})),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "Login error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        }
    }
}
