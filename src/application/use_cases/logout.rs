use uuid::Uuid;

use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::errors::AuthError;

pub async fn logout(
    session_repo: &impl SessionRepository,
    _user_repo: &impl UserRepository,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<(), AuthError> {
    let session = session_repo
        .find_by_id(session_id)
        .await?
        .ok_or(AuthError::SessionNotFound)?;

    if session.user_id != user_id {
        return Err(AuthError::SessionNotFound);
    }

    if !session.is_active {
        return Err(AuthError::SessionInvalidated);
    }

    if session.is_expired() {
        return Err(AuthError::SessionExpired);
    }

    session_repo.invalidate(session_id).await
}
