use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;
use crate::domain::value_objects::role::Role;

#[async_trait]
pub trait AdminRepository: Send + Sync {
    async fn list_users(&self) -> Result<Vec<AdminUser>, AdminError>;
    async fn get_user(&self, user_id: Uuid) -> Result<AdminUser, AdminError>;
    async fn is_admin(&self, user_id: Uuid) -> Result<bool, AdminError>;
    async fn update_role(&self, user_id: Uuid, role: &Role) -> Result<AdminUser, AdminError>;
}
