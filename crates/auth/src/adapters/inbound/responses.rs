use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AuthTokensResponse {
    pub status: String,
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct GoogleAuthResponse {
    pub status: String,
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub is_new_user: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct StatusResponse {
    pub status: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    #[schema(example = "0.1.0")]
    pub version: String,
}

pub use shared::api::ErrorResponse;
