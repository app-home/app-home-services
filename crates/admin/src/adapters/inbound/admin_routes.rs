use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::adapters::inbound::responses::{ErrorResponse, UpdateRoleRequest, UserResponse, UsersResponse};
use crate::application::ports::admin_repository::AdminRepository;
use crate::application::use_cases::{get_user, list_users, update_user_role};
use crate::application::use_cases::list_users::{DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE};
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

#[derive(Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

fn normalize_per_page(raw: Option<u32>) -> u32 {
    raw.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE)
}

fn normalize_page(raw: Option<u32>) -> Result<u32, ()> {
    match raw.unwrap_or(1) {
        0 => Err(()),
        p => Ok(p),
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/users",
    tag = "Admin",
    security(("bearer_jwt" = [])),
    params(
        ("page" = Option<u32>, Query, description = "1-based page number (default 1)"),
        ("per_page" = Option<u32>, Query, description = "Page size, 1..500 (default 100)"),
    ),
    responses(
        (status = 200, description = "Paginated list of users", body = UsersResponse),
        (status = 400, description = "Invalid page value", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
)]
pub async fn list_users_handler(
    Extension(repo): Extension<Arc<dyn AdminRepository>>,
    auth_user: AuthenticatedUser,
    Query(query): Query<ListUsersQuery>,
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

    let Ok(page) = normalize_page(query.page) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "page must be >= 1".into(),
            }),
        )
            .into_response();
    };
    let per_page = normalize_per_page(query.per_page);

    match list_users::list_users(&*repo, page, per_page).await {
        Ok(result) => {
            let responses: Vec<UserResponse> =
                result.users.into_iter().map(user_to_response).collect();
            (
                StatusCode::OK,
                Json(UsersResponse {
                    items: responses,
                    page,
                    per_page,
                    total: result.total,
                }),
            )
                .into_response()
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

    match update_user_role::update_user_role(&*repo, auth_user.user_id, user_id, &req.role).await {
        Ok(user) => (StatusCode::OK, Json(user_to_response(user))).into_response(),
        Err(AdminError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "User not found".into(),
            }),
        )
            .into_response(),
        Err(AdminError::CannotChangeOwnRole) => (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Cannot change your own role".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_page_defaults_to_1() {
        assert_eq!(normalize_page(None).unwrap(), 1);
    }

    #[test]
    fn normalize_page_rejects_zero() {
        assert!(normalize_page(Some(0)).is_err());
    }

    #[test]
    fn normalize_page_accepts_positive() {
        assert_eq!(normalize_page(Some(5)).unwrap(), 5);
    }

    #[test]
    fn normalize_per_page_defaults_to_default_page_size() {
        assert_eq!(normalize_per_page(None), DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn normalize_per_page_clamps_to_one() {
        assert_eq!(normalize_per_page(Some(0)), 1);
    }

    #[test]
    fn normalize_per_page_clamps_to_max() {
        assert_eq!(normalize_per_page(Some(9999)), MAX_PAGE_SIZE);
    }

    #[test]
    fn normalize_per_page_passes_through_within_range() {
        assert_eq!(normalize_per_page(Some(50)), 50);
    }
}