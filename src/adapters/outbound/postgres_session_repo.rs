use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::ports::session_repository::SessionRepository;
use crate::domain::entities::session::{NewSession, Session};
use crate::domain::errors::AuthError;

#[derive(Debug, Clone)]
pub struct PostgresSessionRepo {
    pool: PgPool,
}

impl PostgresSessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepo {
    async fn create(&self, session: NewSession) -> Result<Session, AuthError> {
        let now = Utc::now();

        let row = sqlx::query_as::<_, SessionRow>(
            r#"INSERT INTO sessions (id, user_id, refresh_token_hash, expires_at, is_active, created_at, auth_method)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, user_id, refresh_token_hash, expires_at, is_active, created_at, auth_method"#,
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.refresh_token_hash)
        .bind(session.expires_at)
        .bind(true)
        .bind(now)
        .bind(&session.auth_method)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(row.into_session())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Session>, AuthError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"SELECT id, user_id, refresh_token_hash, expires_at, is_active, created_at, auth_method
            FROM sessions WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(row.map(|r| r.into_session()))
    }

    async fn find_active_by_user_id(&self, user_id: Uuid) -> Result<Vec<Session>, AuthError> {
        let rows = sqlx::query_as::<_, SessionRow>(
            r#"SELECT id, user_id, refresh_token_hash, expires_at, is_active, created_at, auth_method
            FROM sessions
            WHERE user_id = $1 AND is_active = TRUE AND expires_at > NOW()
            ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.into_session()).collect())
    }

    async fn invalidate(&self, id: Uuid) -> Result<(), AuthError> {
        sqlx::query("UPDATE sessions SET is_active = FALSE WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(())
    }

    async fn invalidate_all_for_user(&self, user_id: Uuid) -> Result<(), AuthError> {
        sqlx::query(
            "UPDATE sessions SET is_active = FALSE WHERE user_id = $1 AND is_active = TRUE",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    refresh_token_hash: String,
    expires_at: chrono::DateTime<Utc>,
    is_active: bool,
    created_at: chrono::DateTime<Utc>,
    auth_method: String,
}

impl SessionRow {
    fn into_session(self) -> Session {
        Session {
            id: self.id,
            user_id: self.user_id,
            refresh_token_hash: self.refresh_token_hash,
            expires_at: self.expires_at,
            is_active: self.is_active,
            created_at: self.created_at,
            auth_method: self.auth_method,
        }
    }
}
