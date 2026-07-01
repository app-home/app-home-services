use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::user::{NewUser, User};
use crate::domain::entities::user_action::{NewUserAction, UserAction};
use crate::domain::errors::AuthError;

#[derive(Debug, Clone)]
pub struct PostgresUserRepo {
    pool: PgPool,
}

impl PostgresUserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepo {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, display_name, password_hash, auth_provider, created_at, updated_at FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        row.map(|r| r.into_user()).transpose()
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, display_name, password_hash, auth_provider, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        row.map(|r| r.into_user()).transpose()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, email, display_name, password_hash, auth_provider, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        row.map(|r| r.into_user()).transpose()
    }

    async fn create(&self, user: NewUser) -> Result<User, AuthError> {
        let now = Utc::now();
        let id = Uuid::now_v7();

        let row = sqlx::query_as::<_, UserRow>(
            r#"INSERT INTO users (id, username, email, display_name, password_hash, auth_provider, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, username, email, display_name, password_hash, auth_provider, created_at, updated_at"#,
        )
        .bind(id)
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.display_name)
        .bind(&user.password_hash)
        .bind(&user.auth_provider)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        row.into_user()
    }

    async fn create_user_action(&self, action: NewUserAction) -> Result<UserAction, AuthError> {
        let id = Uuid::now_v7();
        let now = Utc::now();

        let row = sqlx::query_as::<_, UserActionRow>(
            r#"INSERT INTO user_actions (id, user_id, session_id, event_type, auth_method, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, user_id, session_id, event_type, auth_method, created_at"#,
        )
        .bind(id)
        .bind(action.user_id)
        .bind(action.session_id)
        .bind(&action.event_type)
        .bind(&action.auth_method)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(UserAction {
            id: row.id,
            user_id: row.user_id,
            session_id: row.session_id,
            event_type: row.event_type,
            auth_method: row.auth_method,
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: Option<String>,
    email: String,
    display_name: String,
    password_hash: Option<String>,
    auth_provider: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

impl UserRow {
    fn into_user(self) -> Result<User, AuthError> {
        Ok(User {
            id: self.id,
            username: self.username,
            email: self.email,
            display_name: self.display_name,
            password_hash: self.password_hash,
            auth_provider: self.auth_provider,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct UserActionRow {
    id: Uuid,
    user_id: Uuid,
    session_id: Option<Uuid>,
    event_type: String,
    auth_method: String,
    created_at: chrono::DateTime<Utc>,
}
