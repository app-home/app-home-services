use async_trait::async_trait;
use chrono::Utc;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::event_type::EventType;
use shared::domain::value_objects::hashed_password::HashedPassword;
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
        .bind(user.email.as_ref())
        .bind(&user.display_name)
        .bind(user.password_hash.as_ref().map(|h| h.as_ref()))
        .bind(user.auth_provider.as_str())
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
        .bind(action.event_type.as_str())
        .bind(action.auth_method.as_str())
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        row.into_user_action()
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
        let email = Email::new(self.email).map_err(|e| AuthError::InternalError(e.to_string()))?;
        let auth_provider = AuthProvider::try_from(self.auth_provider.as_str())
            .map_err(|e| AuthError::InternalError(e))?;
        let password_hash = self
            .password_hash
            .map(|h| HashedPassword::new(h))
            .transpose()
            .map_err(|e| AuthError::InternalError(e))?;

        Ok(User::new(
            self.id,
            self.username,
            email,
            self.display_name,
            password_hash,
            auth_provider,
            self.created_at,
            self.updated_at,
        ))
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

impl UserActionRow {
    fn into_user_action(self) -> Result<UserAction, AuthError> {
        let event_type = EventType::try_from(self.event_type.as_str())
            .map_err(|e| AuthError::InternalError(e))?;
        let auth_method = AuthMethod::try_from(self.auth_method.as_str())
            .map_err(|e| AuthError::InternalError(e))?;

        Ok(UserAction::new(
            self.id,
            self.user_id,
            self.session_id,
            event_type,
            auth_method,
            self.created_at,
        ))
    }
}
