use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, FromRequestParts},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::adapters::inbound::responses::{ErrorResponse, ProfileResponse, UpdateProfileRequest};
use crate::application::ports::profile_repository::ProfileRepository;
use crate::application::use_cases::{get_profile, update_profile};
use crate::domain::errors::ProfileError;

pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

pub struct AuthRejection;

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".into(),
            }),
        )
            .into_response()
    }
}

/// Extract user_id from JWT in Authorization header using a simple decode
/// (base64url → JSON payload → read "sub" claim).
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthRejection)?;

        let token = auth_header.strip_prefix("Bearer ").ok_or(AuthRejection)?;

        let payload = token
            .split('.')
            .nth(1)
            .and_then(|s| decode_base64url(s).ok())
            .ok_or(AuthRejection)?;

        let claims: serde_json::Value =
            serde_json::from_slice(&payload).map_err(|_| AuthRejection)?;

        let sub = claims
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or(AuthRejection)?;

        let user_id = Uuid::parse_str(sub).map_err(|_| AuthRejection)?;

        Ok(AuthenticatedUser { user_id })
    }
}

fn decode_base64url(input: &str) -> Result<Vec<u8>, ()> {
    let mut s = input.as_bytes().to_vec();
    for b in &mut s {
        match *b {
            b'-' => *b = b'+',
            b'_' => *b = b'/',
            _ => {}
        }
    }
    let remainder = s.len() % 4;
    if remainder != 0 {
        let pad_len = 4 - remainder;
        s.resize(s.len() + pad_len, b'=');
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut i = 0;
    while i < s.len() {
        let a = base64_val(s[i])?;
        let b = base64_val(s[i + 1])?;
        let c = base64_val(s[i + 2])?;
        let d = base64_val(s[i + 3])?;
        out.push((a << 2) | (b >> 4));
        if c != 64 {
            out.push(((b & 0x0f) << 4) | (c >> 2));
        }
        if d != 64 {
            out.push(((c & 0x03) << 6) | d);
        }
        i += 4;
    }
    Ok(out)
}

fn base64_val(b: u8) -> Result<u8, ()> {
    match b {
        b'A'..=b'Z' => Ok(b - b'A'),
        b'a'..=b'z' => Ok(b - b'a' + 26),
        b'0'..=b'9' => Ok(b - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        b'=' => Ok(64),
        _ => Err(()),
    }
}

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
