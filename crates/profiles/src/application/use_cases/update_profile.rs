use chrono::Utc;
use uuid::Uuid;

use crate::application::ports::profile_repository::ProfileRepository;
use crate::domain::entities::profile::UserProfile;
use crate::domain::errors::ProfileError;
use crate::domain::value_objects::avatar_url::AvatarUrl;
use crate::domain::value_objects::bio::Bio;

pub async fn update_profile(
    repo: &dyn ProfileRepository,
    user_id: Uuid,
    avatar_url: Option<String>,
    bio: Option<String>,
) -> Result<UserProfile, ProfileError> {
    if avatar_url.is_none() && bio.is_none() {
        return Err(ProfileError::InvalidValue(
            "At least one field must be provided".into(),
        ));
    }

    let existing = repo.find_by_user_id(user_id).await?;

    let avatar_url = match avatar_url {
        Some(val) if val.is_empty() => None,
        Some(val) => Some(
            AvatarUrl::new(val)
                .map_err(|e| ProfileError::InvalidValue(format!("Invalid avatar_url: {e}")))?,
        ),
        None => existing.as_ref().and_then(|p| p.avatar_url().cloned()),
    };

    let bio = match bio {
        Some(val) if val.is_empty() => None,
        Some(val) => Some(
            Bio::new(val).map_err(|e| ProfileError::InvalidValue(format!("Invalid bio: {e}")))?,
        ),
        None => existing.as_ref().and_then(|p| p.bio().cloned()),
    };

    let profile = UserProfile::new(user_id, avatar_url, bio, Utc::now());

    repo.upsert(&profile).await?;

    Ok(profile)
}
