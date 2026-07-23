use async_trait::async_trait;
use shared::domain::errors::DomainError;
use shared::user_directory::{UserDirectory, UserSummary};
use sqlx::PgPool;
use uuid::Uuid;

/// `auth`'s implementation of the `UserDirectory` port (`shared::user_directory`).
///
/// Deliberately a separate adapter from `PostgresUserRepo`, not an additional method
/// on it: `PostgresUserRepo` returns `auth`'s internal `User` entity (password hash
/// included) and is only ever used inside this crate. `PostgresUserDirectory`
/// returns the cross-context-safe `UserSummary` DTO and is the only part of `auth`
/// that other bounded contexts (currently `admin`) are meant to depend on. Keeping
/// them separate means a future change to `auth`'s internal `User` entity can't
/// accidentally change what other contexts see.
#[derive(Debug, Clone)]
pub struct PostgresUserDirectory {
    pool: PgPool,
}

impl PostgresUserDirectory {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserDirectory for PostgresUserDirectory {
    async fn get_user_summary(&self, user_id: Uuid) -> Result<Option<UserSummary>, DomainError> {
        let row = sqlx::query_as::<_, UserSummaryRow>(
            "SELECT id, username, email, display_name, auth_provider, created_at, updated_at
             FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::InternalError(e.to_string()))?;

        Ok(row.map(UserSummaryRow::into_summary))
    }

    async fn list_user_summaries(&self) -> Result<Vec<UserSummary>, DomainError> {
        let rows = sqlx::query_as::<_, UserSummaryRow>(
            "SELECT id, username, email, display_name, auth_provider, created_at, updated_at
             FROM users ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::InternalError(e.to_string()))?;

        Ok(rows.into_iter().map(UserSummaryRow::into_summary).collect())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct UserSummaryRow {
    id: Uuid,
    username: Option<String>,
    email: String,
    display_name: String,
    auth_provider: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl UserSummaryRow {
    fn into_summary(self) -> UserSummary {
        UserSummary {
            id: self.id,
            username: self.username,
            email: self.email,
            display_name: self.display_name,
            auth_provider: self.auth_provider,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
