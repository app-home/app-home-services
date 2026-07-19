use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value_objects::auth_provider::AuthProvider;

#[derive(Debug, Clone)]
pub struct UserCreated {
    pub user_id: Uuid,
    pub provider: AuthProvider,
    pub occurred_at: DateTime<Utc>,
}

impl UserCreated {
    pub fn new(user_id: Uuid, provider: AuthProvider) -> Self {
        Self {
            user_id,
            provider,
            occurred_at: Utc::now(),
        }
    }
}
