use uuid::Uuid;

use crate::application::ports::profile_repository::ProfileRepository;
use crate::domain::entities::profile::UserProfile;
use crate::domain::errors::ProfileError;

pub async fn get_profile(
    repo: &dyn ProfileRepository,
    user_id: Uuid,
) -> Result<UserProfile, ProfileError> {
    repo.find_by_user_id(user_id)
        .await?
        .ok_or(ProfileError::NotFound(user_id))
}
