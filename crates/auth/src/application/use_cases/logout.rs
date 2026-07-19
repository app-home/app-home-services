use shared::domain::events::Event;
use uuid::Uuid;

use crate::application::ports::user_repository::UserRepository;
use crate::domain::errors::AuthError;

pub async fn logout(
    user_repo: &impl UserRepository,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<Vec<Event>, AuthError> {
    let mut aggregate = user_repo
        .find_aggregate_by_id(user_id)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let _auth_method = aggregate.invalidate_session(session_id)?;

    user_repo.save_aggregate(&mut aggregate, &[]).await?;

    Ok(aggregate.take_events())
}
