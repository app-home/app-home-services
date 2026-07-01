use uuid::Uuid;

use crate::domain::entities::user_action::UserAction;
use crate::domain::errors::AuthError;
use crate::application::ports::user_repository::UserRepository;

pub async fn record_audit_entry(
    repo: &impl UserRepository,
    user_id: Uuid,
    auth_method: String,
) -> Result<UserAction, AuthError> {
    let action = crate::domain::entities::user_action::NewUserAction {
        user_id,
        auth_method,
    };
    repo.create_user_action(action).await
}
