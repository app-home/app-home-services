use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub username: Option<String>,
    pub email: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub auth_provider: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn verify_password(&self, password: &str) -> bool {
        match &self.password_hash {
            Some(hash) => bcrypt::verify(password, hash).unwrap_or(false),
            None => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: Option<String>,
    pub email: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub auth_provider: String,
}
