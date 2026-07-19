use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::adapters::inbound::responses::{ErrorResponse, UpdateRoleRequest, UserResponse};
use crate::application::ports::admin_repository::AdminRepository;
use crate::application::use_cases::{get_user, list_users, update_user_role};
use crate::domain::errors::AdminError;
use shared::auth::AuthenticatedUser;
use uuid::Uuid;

struct AdminGuard;

impl IntoResponse for AdminGuard {
    fn into_response(self) -> Response {
        (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Forbidden: admin access required".into(),
            }),
        )
            .into_response()
    }
}

fn user_to_response(user: crate::domain::entities::admin_user::AdminUser) -> UserResponse {
    UserResponse {
        id: user.id().to_string(),
        username: user.username().map(|s| s.to_string()),
        email: user.email().to_string(),
        display_name: user.display_name().to_string(),
        role: user.role().as_str().to_string(),
        auth_provider: user.auth_provider().to_string(),
        created_at: user.created_at().to_rfc3339(),
        updated_at: user.updated_at().to_rfc3339(),
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/users",
    tag = "Admin",
    security(("bearer_jwt" = [])),
    responses(
        (status = 200, description = "List of users", body = Vec<UserResponse>),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn list_users_handler(
    Extension(repo): Extension<Arc<dyn AdminRepository>>,
    auth_user: AuthenticatedUser,
) -> Response {
    match repo.is_admin(auth_user.user_id).await {
        Ok(true) => {}
        Ok(false) => return AdminGuard.into_response(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".into(),
                }),
            )
                .into_response();
        }
    }

    match list_users::list_users(&*repo).await {
        Ok(users) => {
            let responses: Vec<UserResponse> = users.into_iter().map(user_to_response).collect();
            (StatusCode::OK, Json(responses)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list users");
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
    get,
    path = "/api/admin/users/{id}",
    tag = "Admin",
    security(("bearer_jwt" = [])),
    params(
        ("id" = String, Path, description = "User UUID"),
    ),
    responses(
        (status = 200, description = "User details", body = UserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn get_user_handler(
    Extension(repo): Extension<Arc<dyn AdminRepository>>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Response {
    match repo.is_admin(auth_user.user_id).await {
        Ok(true) => {}
        Ok(false) => return AdminGuard.into_response(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".into(),
                }),
            )
                .into_response();
        }
    }

    match get_user::get_user(&*repo, user_id).await {
        Ok(user) => (StatusCode::OK, Json(user_to_response(user))).into_response(),
        Err(AdminError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "User not found".into(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get user");
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
    path = "/api/admin/users/{id}/role",
    tag = "Admin",
    security(("bearer_jwt" = [])),
    request_body = UpdateRoleRequest,
    params(
        ("id" = String, Path, description = "User UUID"),
    ),
    responses(
        (status = 200, description = "Role updated", body = UserResponse),
        (status = 400, description = "Invalid role value", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn update_user_role_handler(
    Extension(repo): Extension<Arc<dyn AdminRepository>>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> Response {
    match repo.is_admin(auth_user.user_id).await {
        Ok(true) => {}
        Ok(false) => return AdminGuard.into_response(),
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".into(),
                }),
            )
                .into_response();
        }
    }

    match update_user_role::update_user_role(&*repo, user_id, &req.role).await {
        Ok(user) => (StatusCode::OK, Json(user_to_response(user))).into_response(),
        Err(AdminError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "User not found".into(),
            }),
        )
            .into_response(),
        Err(AdminError::InvalidValue(msg)) => {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg })).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update user role");
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
