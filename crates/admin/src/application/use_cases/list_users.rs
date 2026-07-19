use crate::application::ports::admin_repository::AdminRepository;
use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;

pub async fn list_users(repo: &dyn AdminRepository) -> Result<Vec<AdminUser>, AdminError> {
    repo.list_users().await
}
