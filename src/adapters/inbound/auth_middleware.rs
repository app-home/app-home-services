use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::AppState;
use crate::adapters::inbound::responses::ErrorResponse;
use crate::application::ports::jwt_service::JwtService;

pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

pub struct AuthErrorResponse;

impl IntoResponse for AuthErrorResponse {
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

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = AuthErrorResponse;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthErrorResponse)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthErrorResponse)?;

        let claims = state
            .jwt_service
            .validate_access_token(token)
            .map_err(|_| AuthErrorResponse)?;

        Ok(AuthenticatedUser {
            user_id: claims.sub,
        })
    }
}
