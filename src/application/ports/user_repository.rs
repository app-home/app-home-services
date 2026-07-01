use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::user::{NewUser, User};
use crate::domain::entities::user_action::{NewUserAction, UserAction};
use crate::domain::errors::AuthError;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, AuthError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AuthError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AuthError>;
    async fn create(&self, user: NewUser) -> Result<User, AuthError>;
    async fn create_user_action(&self, action: NewUserAction) -> Result<UserAction, AuthError>;
}
