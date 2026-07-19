use chrono::{DateTime, Utc};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Session {
    id: Uuid,
    user_id: Uuid,
    refresh_token_hash: HashedPassword,
    expires_at: DateTime<Utc>,
    is_active: bool,
    created_at: DateTime<Utc>,
    auth_method: AuthMethod,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: HashedPassword,
    pub expires_at: DateTime<Utc>,
    pub auth_method: AuthMethod,
}

impl Session {
    pub fn new(
        id: Uuid,
        user_id: Uuid,
        refresh_token_hash: HashedPassword,
        expires_at: DateTime<Utc>,
        is_active: bool,
        created_at: DateTime<Utc>,
        auth_method: AuthMethod,
    ) -> Self {
        Self {
            id,
            user_id,
            refresh_token_hash,
            expires_at,
            is_active,
            created_at,
            auth_method,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn refresh_token_hash(&self) -> &HashedPassword {
        &self.refresh_token_hash
    }

    pub fn expires_at(&self) -> &DateTime<Utc> {
        &self.expires_at
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn auth_method(&self) -> &AuthMethod {
        &self.auth_method
    }

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
        refresh_token_hash: HashedPassword,
        expires_at: DateTime<Utc>,
        auth_method: AuthMethod,
    ) -> Self {
        Self {
            id,
            user_id,
            refresh_token_hash,
            expires_at,
            auth_method,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.refresh_token_hash.as_ref().is_empty() {
            return Err("refresh_token_hash must not be empty");
        }
        if self.expires_at <= Utc::now() {
            return Err("expires_at must be in the future");
        }
        Ok(())
    }
}
