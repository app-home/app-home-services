use shared::domain::events::Event;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::user_repository::UserRepository;
use crate::config::auth_settings::AuthSettings;
use crate::domain::errors::AuthError;

#[derive(Debug)]
pub struct RefreshResult {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub auth_method: AuthMethod,
    pub events: Vec<Event>,
}

pub async fn refresh_token(
    user_repo: &impl UserRepository,
    jwt_service: &impl JwtService,
    refresh_token: &str,
    settings: &AuthSettings,
) -> Result<RefreshResult, AuthError> {
    let claims = jwt_service.validate_refresh_token(refresh_token)?;

    let mut aggregate = user_repo
        .find_aggregate_by_id(claims.sub)
        .await?
        .ok_or(AuthError::UserNotFound)?;

    let new_session_id = Uuid::now_v7();
    let token_pair = jwt_service.generate_token_pair(claims.sub, new_session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let refresh_hash = HashedPassword::new(
        bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
            .map_err(|_| AuthError::TokenGenerationFailed)?,
    )
    .map_err(|_| AuthError::TokenGenerationFailed)?;

    let new_session = match aggregate.rotate_session(
        claims.session_id,
        refresh_token,
        new_session_id,
        refresh_hash,
        expires_at,
    ) {
        Ok(s) => s,
        Err(AuthError::SessionInvalidated) => {
            tracing::warn!(
                user_id = %claims.sub,
                session_id = %claims.session_id,
                "Refresh token reuse detected — revoking all sessions for user"
            );
            aggregate.invalidate_all_active_sessions();
            user_repo.save_aggregate(&mut aggregate, &[]).await?;
            return Err(AuthError::SessionInvalidated);
        }
        Err(e) => return Err(e),
    };

    user_repo
        .save_aggregate(&mut aggregate, &[new_session])
        .await?;

    let events = aggregate.take_events();
    let auth_method = events
        .first()
        .and_then(|e| e.auth_method())
        .unwrap_or(AuthMethod::Password);

    Ok(RefreshResult {
        user_id: claims.sub,
        session_id: new_session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        auth_method,
        events,
    })
}
