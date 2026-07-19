use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateRoleRequest {
    pub role: String,
}
