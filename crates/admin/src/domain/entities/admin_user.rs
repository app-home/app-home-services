use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_objects::role::Role;

#[derive(Debug, Clone)]
pub struct AdminUser {
    id: Uuid,
    username: Option<String>,
    email: String,
    display_name: String,
    role: Role,
    auth_provider: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl AdminUser {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        username: Option<String>,
        email: String,
        display_name: String,
        role: Role,
        auth_provider: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            username,
            email,
            display_name,
            role,
            auth_provider,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn role(&self) -> &Role {
        &self.role
    }

    pub fn auth_provider(&self) -> &str {
        &self.auth_provider
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn is_admin(&self) -> bool {
        self.role.is_admin()
    }
}
