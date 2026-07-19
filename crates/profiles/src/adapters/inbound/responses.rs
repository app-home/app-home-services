use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use shared::api::ErrorResponse;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileResponse {
    pub user_id: String,
    #[schema(nullable)]
    pub avatar_url: Option<String>,
    #[schema(nullable)]
    pub bio: Option<String>,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    #[schema(max_length = 500, nullable)]
    pub avatar_url: Option<String>,
    #[schema(max_length = 2000, nullable)]
    pub bio: Option<String>,
}


