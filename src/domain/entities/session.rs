use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    /// How the user authenticated when this session was created ("password" or
    /// "google_oauth"). Stored per-session (rather than derived from the user's
    /// account) so logout/refresh audit entries can report the method actually used
    /// for *this* session, and so it survives token rotation -- see
    /// `refresh_token.rs`, which carries this value forward from the old session to
    /// the new one instead of losing it.
    pub auth_method: String,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub auth_method: String,
}

impl Session {
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    pub fn invalidate(&mut self) {
        self.is_active = false;
    }
}

impl NewSession {
    pub fn new(
        id: Uuid,
        user_id: Uuid,
        refresh_token_hash: String,
        expires_at: DateTime<Utc>,
        auth_method: impl Into<String>,
    ) -> Self {
        Self {
            id,
            user_id,
            refresh_token_hash,
            expires_at,
            auth_method: auth_method.into(),
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.refresh_token_hash.is_empty() {
            return Err("refresh_token_hash must not be empty");
        }
        if self.expires_at <= Utc::now() {
            return Err("expires_at must be in the future");
        }
        if self.auth_method.is_empty() {
            return Err("auth_method must not be empty");
        }
        Ok(())
    }
}
