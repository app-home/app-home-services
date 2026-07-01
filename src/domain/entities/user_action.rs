use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserAction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub event_type: String,
    pub auth_method: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUserAction {
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub event_type: String,
    pub auth_method: String,
}

impl NewUserAction {
    pub fn new_login(user_id: Uuid, auth_method: String) -> Self {
        Self {
            user_id,
            session_id: None,
            event_type: "login".to_string(),
            auth_method,
        }
    }

    pub fn new_logout(user_id: Uuid, session_id: Uuid, auth_method: String) -> Self {
        Self {
            user_id,
            session_id: Some(session_id),
            event_type: "logout".to_string(),
            auth_method,
        }
    }

    pub fn new_refresh(user_id: Uuid, session_id: Uuid, auth_method: String) -> Self {
        Self {
            user_id,
            session_id: Some(session_id),
            event_type: "refresh".to_string(),
            auth_method,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        match self.event_type.as_str() {
            "login" | "logout" | "refresh" => Ok(()),
            _ => Err("event_type must be one of: login, logout, refresh"),
        }
    }
}
