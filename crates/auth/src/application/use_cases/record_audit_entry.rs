use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::event_type::EventType;
use uuid::Uuid;

use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::user_action::{NewUserAction, UserAction};
use crate::domain::errors::AuthError;

pub async fn record_audit_entry(
    repo: &impl UserRepository,
    user_id: Uuid,
    session_id: Option<Uuid>,
    event_type: EventType,
    auth_method: AuthMethod,
) -> Result<UserAction, AuthError> {
    let action = NewUserAction {
        user_id,
        session_id,
        event_type,
        auth_method,
    };
    repo.create_user_action(action).await
}
