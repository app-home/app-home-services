use shared::domain::events::user_logged_out::UserLoggedOut;
use shared::domain::value_objects::auth_method::AuthMethod;
use uuid::Uuid;

use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::errors::AuthError;

pub async fn logout(
    session_repo: &impl SessionRepository,
    _user_repo: &impl UserRepository,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<(AuthMethod, UserLoggedOut), AuthError> {
    let session = session_repo
        .find_by_id(session_id)
        .await?
        .ok_or(AuthError::SessionNotFound)?;

    if session.user_id() != user_id {
        return Err(AuthError::SessionNotFound);
    }

    if !session.is_active() {
        return Err(AuthError::SessionInvalidated);
    }

    if session.is_expired() {
        return Err(AuthError::SessionExpired);
    }

    session_repo.invalidate(session_id).await?;

    let auth_method = *session.auth_method();
    let event = UserLoggedOut::new(user_id, session_id, auth_method);

    Ok((auth_method, event))
}
