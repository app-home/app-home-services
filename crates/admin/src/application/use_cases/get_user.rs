use uuid::Uuid;

use crate::application::ports::admin_repository::AdminRepository;
use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;

pub async fn get_user(repo: &dyn AdminRepository, user_id: Uuid) -> Result<AdminUser, AdminError> {
    repo.get_user(user_id).await
}
