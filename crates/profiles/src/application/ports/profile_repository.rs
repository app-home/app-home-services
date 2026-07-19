use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::profile::UserProfile;
use crate::domain::errors::ProfileError;

#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<UserProfile>, ProfileError>;
    async fn upsert(&self, profile: &UserProfile) -> Result<(), ProfileError>;
}
