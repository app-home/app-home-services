use std::sync::Arc;

use axum::{
    Json,
    extract::Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::adapters::inbound::responses::{ErrorResponse, ProfileResponse, UpdateProfileRequest};
use crate::application::ports::profile_repository::ProfileRepository;
use crate::application::use_cases::{get_profile, update_profile};
use crate::domain::errors::ProfileError;
use shared::auth::AuthenticatedUser;

#[utoipa::path(
    get,
    path = "/api/profile",
    tag = "Profiles",
    security(("bearer_jwt" = [])),
    responses(
        (status = 200, description = "User profile", body = ProfileResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn get_profile_handler(
    Extension(repo): Extension<Arc<dyn ProfileRepository>>,
    auth_user: AuthenticatedUser,
) -> Response {
    match get_profile::get_profile(&*repo, auth_user.user_id).await {
        Ok(profile) => (
            StatusCode::OK,
            Json(ProfileResponse {
                user_id: profile.user_id().to_string(),
                avatar_url: profile.avatar_url().map(|a| a.as_str().to_string()),
                bio: profile.bio().map(|b| b.as_str().to_string()),
                updated_at: profile.updated_at().to_rfc3339(),
            }),
        )
            .into_response(),
        Err(ProfileError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Profile not found".into(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get profile");
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

#[utoipa::path(
    put,
    path = "/api/profile",
    tag = "Profiles",
    security(("bearer_jwt" = [])),
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profile updated", body = ProfileResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn update_profile_handler(
    Extension(repo): Extension<Arc<dyn ProfileRepository>>,
    auth_user: AuthenticatedUser,
    Json(req): Json<UpdateProfileRequest>,
) -> Response {
    match update_profile::update_profile(&*repo, auth_user.user_id, req.avatar_url, req.bio).await {
        Ok(profile) => (
            StatusCode::OK,
            Json(ProfileResponse {
                user_id: profile.user_id().to_string(),
                avatar_url: profile.avatar_url().map(|a| a.as_str().to_string()),
                bio: profile.bio().map(|b| b.as_str().to_string()),
                updated_at: profile.updated_at().to_rfc3339(),
            }),
        )
            .into_response(),
        Err(ProfileError::InvalidValue(msg)) => {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg })).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update profile");
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
