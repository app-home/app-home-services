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
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token_hash: String,
    pub expires_at: DateTime<Utc>,
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
    pub fn new(id: Uuid, user_id: Uuid, refresh_token_hash: String, expires_at: DateTime<Utc>) -> Self {
        Self {
            id,
            user_id,
            refresh_token_hash,
            expires_at,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.refresh_token_hash.is_empty() {
            return Err("refresh_token_hash must not be empty");
        }
        if self.expires_at <= Utc::now() {
            return Err("expires_at must be in the future");
        }
        Ok(())
    }
}
