use chrono::{DateTime, Utc};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::event_type::EventType;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UserAction {
    id: Uuid,
    user_id: Uuid,
    session_id: Option<Uuid>,
    event_type: EventType,
    auth_method: AuthMethod,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUserAction {
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub event_type: EventType,
    pub auth_method: AuthMethod,
}

impl UserAction {
    pub fn new(
        id: Uuid,
        user_id: Uuid,
        session_id: Option<Uuid>,
        event_type: EventType,
        auth_method: AuthMethod,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            session_id,
            event_type,
            auth_method,
            created_at,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn session_id(&self) -> Option<Uuid> {
        self.session_id
    }

    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }

    pub fn auth_method(&self) -> &AuthMethod {
        &self.auth_method
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }
}

impl NewUserAction {
    pub fn new_login(user_id: Uuid, auth_method: AuthMethod) -> Self {
        Self {
            user_id,
            session_id: None,
            event_type: EventType::Login,
            auth_method,
        }
    }

    pub fn new_logout(user_id: Uuid, session_id: Uuid, auth_method: AuthMethod) -> Self {
        Self {
            user_id,
            session_id: Some(session_id),
            event_type: EventType::Logout,
            auth_method,
        }
    }

    pub fn new_refresh(user_id: Uuid, session_id: Uuid, auth_method: AuthMethod) -> Self {
        Self {
            user_id,
            session_id: Some(session_id),
            event_type: EventType::Refresh,
            auth_method,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        Ok(())
    }
}
