use async_trait::async_trait;
use chrono::Utc;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::event_type::EventType;
use shared::domain::value_objects::hashed_password::HashedPassword;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::application::ports::user_repository::UserRepository;
use crate::domain::aggregate::UserAggregate;
use crate::domain::entities::session::{NewSession, Session};
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

    async fn load_aggregate_with_tx(&self, user: User) -> Result<UserAggregate, AuthError> {
        let rows = sqlx::query_as::<_, SessionRow>(
            r#"SELECT id, user_id, refresh_token_hash, expires_at, is_active, created_at, auth_method
            FROM sessions
            WHERE user_id = $1
            ORDER BY created_at DESC"#,
        )
        .bind(user.id())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AuthError::InternalError(e.to_string()))?;

        let sessions = rows
            .into_iter()
            .map(|r| r.into_session())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(UserAggregate::new(user, sessions))
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

    async fn find_aggregate_by_id(&self, id: Uuid) -> Result<Option<UserAggregate>, AuthError> {
        let user_opt = self.find_by_id(id).await?;
        match user_opt {
            Some(user) => self.load_aggregate_with_tx(user).await.map(Some),
            None => Ok(None),
        }
    }

    async fn find_aggregate_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserAggregate>, AuthError> {
        let user_opt = self.find_by_username(username).await?;
        match user_opt {
            Some(user) => self.load_aggregate_with_tx(user).await.map(Some),
            None => Ok(None),
        }
    }

    async fn find_aggregate_by_email(
        &self,
        email: &str,
    ) -> Result<Option<UserAggregate>, AuthError> {
        let user_opt = self.find_by_email(email).await?;
        match user_opt {
            Some(user) => self.load_aggregate_with_tx(user).await.map(Some),
            None => Ok(None),
        }
    }

    async fn save_aggregate(
        &self,
        aggregate: &mut UserAggregate,
        new_sessions: &[NewSession],
    ) -> Result<(), AuthError> {
        let invalidated_ids: Vec<Uuid> = aggregate
            .sessions
            .iter()
            .filter(|s| !s.is_active())
            .map(|s| s.id())
            .collect();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;

        for new_session in new_sessions {
            insert_session(&mut tx, new_session).await?;
        }

        for id in &invalidated_ids {
            sqlx::query("UPDATE sessions SET is_active = FALSE WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AuthError::InternalError(e.to_string()))?;

        Ok(())
    }
}

async fn insert_session(
    tx: &mut Transaction<'_, Postgres>,
    session: &NewSession,
) -> Result<(), AuthError> {
    let now = Utc::now();

    sqlx::query(
        r#"INSERT INTO sessions (id, user_id, refresh_token_hash, expires_at, is_active, created_at, auth_method)
        VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(session.id)
    .bind(session.user_id)
    .bind(session.refresh_token_hash.as_ref())
    .bind(session.expires_at)
    .bind(true)
    .bind(now)
    .bind(session.auth_method.as_str())
    .execute(&mut **tx)
    .await
    .map_err(|e| AuthError::InternalError(e.to_string()))?;

    Ok(())
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
            .map_err(AuthError::InternalError)?;
        let password_hash = self
            .password_hash
            .map(HashedPassword::new)
            .transpose()
            .map_err(AuthError::InternalError)?;

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
        let event_type =
            EventType::try_from(self.event_type.as_str()).map_err(AuthError::InternalError)?;
        let auth_method =
            AuthMethod::try_from(self.auth_method.as_str()).map_err(AuthError::InternalError)?;

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
    fn into_session(self) -> Result<Session, AuthError> {
        let refresh_token_hash =
            HashedPassword::new(self.refresh_token_hash).map_err(AuthError::InternalError)?;
        let auth_method =
            AuthMethod::try_from(self.auth_method.as_str()).map_err(AuthError::InternalError)?;

        Ok(Session::new(
            self.id,
            self.user_id,
            refresh_token_hash,
            self.expires_at,
            self.is_active,
            self.created_at,
            auth_method,
        ))
    }
}
