use std::sync::Arc;

use axum::{
    Json,
    extract::{FromRequestParts},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum::extract::Extension;
use jsonwebtoken::{DecodingKey, Validation};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::ErrorResponse;

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

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedUser {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let token = {
            let auth_header = parts
                .headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .ok_or(AuthRejection)?;
            auth_header.strip_prefix("Bearer ").ok_or(AuthRejection)?.to_string()
        };

        let Extension(decoding_key) =
            Extension::<Arc<DecodingKey>>::from_request_parts(parts, _state)
                .await
                .map_err(|_| AuthRejection)?;

        #[derive(Deserialize)]
        struct Claims {
            sub: Uuid,
        }

        let token_data =
            jsonwebtoken::decode::<Claims>(&token, &decoding_key, &Validation::default())
                .map_err(|_| AuthRejection)?;

        Ok(AuthenticatedUser {
            user_id: token_data.claims.sub,
        })
    }
}
