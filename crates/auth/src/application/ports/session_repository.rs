use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::session::{NewSession, Session};
use crate::domain::errors::AuthError;

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, session: NewSession) -> Result<Session, AuthError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Session>, AuthError>;
    async fn find_active_by_user_id(&self, user_id: Uuid) -> Result<Vec<Session>, AuthError>;
    async fn invalidate(&self, id: Uuid) -> Result<(), AuthError>;
    async fn invalidate_all_for_user(&self, user_id: Uuid) -> Result<(), AuthError>;

    async fn sessions_for_user(&self, user_id: Uuid) -> Result<Vec<Session>, AuthError>;
}
