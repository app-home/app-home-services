use uuid::Uuid;

use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::user_action::{NewUserAction, UserAction};
use crate::domain::errors::AuthError;

pub async fn record_audit_entry(
    repo: &impl UserRepository,
    user_id: Uuid,
    session_id: Option<Uuid>,
    event_type: &str,
    auth_method: String,
) -> Result<UserAction, AuthError> {
    let action = NewUserAction {
        user_id,
        session_id,
        event_type: event_type.to_string(),
        auth_method,
    };
    repo.create_user_action(action).await
}
