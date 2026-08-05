use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;
use crate::domain::value_objects::role::Role;

/// Paginated page of users plus the total user count, so clients can render
/// pagination controls without a second request (see #101).
#[derive(Debug, Clone)]
pub struct ListUsersResult {
    pub users: Vec<AdminUser>,
    pub total: u64,
}

#[async_trait]
pub trait AdminRepository: Send + Sync {
    /// Returns `per_page` users for 1-based `page`, ordered by `created_at`
    /// descending, together with the total user count. `page`/`per_page` are
    /// validated by the caller before they reach the repository (see #101).
    async fn list_users(&self, page: u32, per_page: u32) -> Result<ListUsersResult, AdminError>;
    async fn get_user(&self, user_id: Uuid) -> Result<AdminUser, AdminError>;
    async fn is_admin(&self, user_id: Uuid) -> Result<bool, AdminError>;
    async fn update_role(&self, user_id: Uuid, role: &Role) -> Result<AdminUser, AdminError>;
}
