use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::errors::DomainError;

/// A read-only view of a user's identity fields, owned by the `auth` context.
///
/// Deliberately excludes anything auth-internal (password hash, auth provider
/// credentials, session data) -- this is only what other bounded contexts need to
/// *display* a user, not to authenticate one.
#[derive(Debug, Clone)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: Option<String>,
    pub email: String,
    pub display_name: String,
    pub auth_provider: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A read-only port for looking up user identity, implemented by the `auth` context
/// (see `auth::adapters::postgres_user_directory::PostgresUserDirectory`) and
/// consumed by any other bounded context that needs to display user identity fields
/// without owning them.
///
/// This exists specifically so contexts like `admin` never need to query `auth`'s
/// `users` table directly, or depend on the `auth` crate at all -- they depend only
/// on this trait, which lives in `shared` alongside them both. See
/// `docs/adr/0001-modular-monolith.md` for the coupling this replaces and why it's
/// shaped this way (an in-process port today, ready to become a network call behind
/// the same interface if `auth` is ever extracted into its own service).
#[async_trait]
pub trait UserDirectory: Send + Sync {
    async fn get_user_summary(&self, user_id: Uuid) -> Result<Option<UserSummary>, DomainError>;
    async fn list_user_summaries(&self) -> Result<Vec<UserSummary>, DomainError>;
}
