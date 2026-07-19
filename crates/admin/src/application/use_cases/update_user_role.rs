use uuid::Uuid;

use crate::application::ports::admin_repository::AdminRepository;
use crate::domain::entities::admin_user::AdminUser;
use crate::domain::errors::AdminError;
use crate::domain::value_objects::role::Role;

pub async fn update_user_role(
    repo: &dyn AdminRepository,
    user_id: Uuid,
    new_role: &str,
) -> Result<AdminUser, AdminError> {
    let role = Role::try_from(new_role)
        .map_err(|_| AdminError::InvalidValue("role must be 'user' or 'admin'".into()))?;

    repo.get_user(user_id).await?;

    repo.update_role(user_id, &role).await
}
