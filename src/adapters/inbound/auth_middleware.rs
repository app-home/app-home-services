use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::AppState;
use crate::application::ports::jwt_service::JwtService;

pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

pub struct AuthErrorResponse;

impl IntoResponse for AuthErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    }
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = AuthErrorResponse;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 AppState,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
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
        })
    }
}
