use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_objects::avatar_url::AvatarUrl;
use crate::domain::value_objects::bio::Bio;

#[derive(Debug, Clone)]
pub struct UserProfile {
    user_id: Uuid,
    avatar_url: Option<AvatarUrl>,
    bio: Option<Bio>,
    updated_at: DateTime<Utc>,
}

impl UserProfile {
    pub fn new(
        user_id: Uuid,
        avatar_url: Option<AvatarUrl>,
        bio: Option<Bio>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            user_id,
            avatar_url,
            bio,
            updated_at,
        }
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn avatar_url(&self) -> Option<&AvatarUrl> {
        self.avatar_url.as_ref()
    }

    pub fn bio(&self) -> Option<&Bio> {
        self.bio.as_ref()
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}
