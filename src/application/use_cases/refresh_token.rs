use shared::domain::events::session_refreshed::SessionRefreshed;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use tracing::error;
use uuid::Uuid;

use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::session::NewSession;
use crate::domain::errors::AuthError;

#[derive(Debug)]
pub struct RefreshResult {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub auth_method: AuthMethod,
    pub event: SessionRefreshed,
}

pub async fn refresh_token(
    session_repo: &impl SessionRepository,
    _user_repo: &impl UserRepository,
    jwt_service: &impl JwtService,
    refresh_token: &str,
    settings: &crate::infrastructure::config::settings::Settings,
) -> Result<RefreshResult, AuthError> {
    let claims = jwt_service.validate_refresh_token(refresh_token)?;

    let session = session_repo
        .find_by_id(claims.session_id)
        .await?
        .ok_or(AuthError::SessionNotFound)?;

    if !session.is_active() {
        tracing::warn!(
            user_id = %claims.sub,
            session_id = %claims.session_id,
            "Refresh token reuse detected -- revoking all sessions for user"
        );
        session_repo.invalidate_all_for_user(claims.sub).await?;
        return Err(AuthError::SessionInvalidated);
    }

    if session.is_expired() {
        return Err(AuthError::SessionExpired);
    }

    let is_valid_refresh =
        match bcrypt::verify(refresh_token, session.refresh_token_hash().as_ref()) {
            Ok(valid) => valid,
            Err(e) => {
                error!(error = %e, "bcrypt::verify failed for refresh token hash");
                false
            }
        };
    if !is_valid_refresh {
        return Err(AuthError::InvalidRefreshToken);
    }

    session_repo.invalidate(claims.session_id).await?;

    let new_session_id = Uuid::now_v7();
    let token_pair = jwt_service.generate_token_pair(claims.sub, new_session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let refresh_hash = HashedPassword::new(
        bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
            .map_err(|_| AuthError::TokenGenerationFailed)?,
    )
    .map_err(|_| AuthError::TokenGenerationFailed)?;

    let auth_method = *session.auth_method();

    let new_session = NewSession::new(
        new_session_id,
        claims.sub,
        refresh_hash,
        expires_at,
        auth_method,
    );
    session_repo.create(new_session).await?;

    let event = SessionRefreshed::new(claims.sub, new_session_id, auth_method);

    Ok(RefreshResult {
        user_id: claims.sub,
        session_id: new_session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        auth_method,
        event,
    })
}
