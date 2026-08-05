use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use shared::api::ErrorResponse;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    #[schema(nullable)]
    pub username: Option<String>,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub auth_provider: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UsersResponse {
    pub items: Vec<UserResponse>,
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateRoleRequest {
    pub role: String,
}
