use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserAction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub auth_method: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUserAction {
    pub user_id: Uuid,
    pub auth_method: String,
}
