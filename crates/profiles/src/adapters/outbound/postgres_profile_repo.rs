use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::ports::profile_repository::ProfileRepository;
use crate::domain::entities::profile::UserProfile;
use crate::domain::errors::ProfileError;
use crate::domain::value_objects::avatar_url::AvatarUrl;
use crate::domain::value_objects::bio::Bio;

#[derive(Debug, Clone)]
pub struct PostgresProfileRepo {
    pool: PgPool,
}

impl PostgresProfileRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProfileRepository for PostgresProfileRepo {
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<UserProfile>, ProfileError> {
        let row = sqlx::query_as::<_, ProfileRow>(
            r#"
            SELECT user_id, avatar_url, bio, updated_at
            FROM user_profiles
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ProfileError::InternalError(e.to_string()))?;

        Ok(row.map(|r| {
            UserProfile::new(
                r.user_id,
                r.avatar_url
                    .filter(|s| !s.is_empty())
                    .and_then(|s| AvatarUrl::new(s).ok()),
                r.bio
                    .filter(|s| !s.is_empty())
                    .and_then(|s| Bio::new(s).ok()),
                r.updated_at,
            )
        }))
    }

    async fn upsert(&self, profile: &UserProfile) -> Result<(), ProfileError> {
        let avatar_url = profile.avatar_url().map(|a| a.as_str().to_string());
        let bio = profile.bio().map(|b| b.as_str().to_string());

        sqlx::query(
            r#"
            INSERT INTO user_profiles (user_id, avatar_url, bio, updated_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id) DO UPDATE SET
                avatar_url = EXCLUDED.avatar_url,
                bio = EXCLUDED.bio,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(profile.user_id())
        .bind(avatar_url)
        .bind(bio)
        .bind(profile.updated_at())
        .execute(&self.pool)
        .await
        .map_err(|e| ProfileError::InternalError(e.to_string()))?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    user_id: Uuid,
    avatar_url: Option<String>,
    bio: Option<String>,
    updated_at: DateTime<Utc>,
}
