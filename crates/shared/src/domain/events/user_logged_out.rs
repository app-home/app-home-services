use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_objects::auth_method::AuthMethod;

#[derive(Debug, Clone)]
pub struct UserLoggedOut {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub method: AuthMethod,
    pub occurred_at: DateTime<Utc>,
}

impl UserLoggedOut {
    pub fn new(user_id: Uuid, session_id: Uuid, method: AuthMethod) -> Self {
        Self {
            user_id,
            session_id,
            method,
            occurred_at: Utc::now(),
        }
    }
}
