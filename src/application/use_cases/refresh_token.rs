use uuid::Uuid;

use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::session::NewSession;
use crate::domain::errors::AuthError;

pub struct RefreshResult {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
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

    if !session.is_active {
        return Err(AuthError::SessionInvalidated);
    }

    if session.is_expired() {
        return Err(AuthError::SessionExpired);
    }

    let is_valid_refresh =
        bcrypt::verify(refresh_token, &session.refresh_token_hash).unwrap_or(false);
    if !is_valid_refresh {
        return Err(AuthError::InvalidRefreshToken);
    }

    session_repo.invalidate(claims.session_id).await?;

    let new_session_id = Uuid::now_v7();
    let token_pair = jwt_service.generate_token_pair(claims.sub, new_session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let refresh_hash = bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
        .map_err(|_| AuthError::TokenGenerationFailed)?;

    let new_session = NewSession::new(claims.sub, refresh_hash, expires_at);
    let _session = session_repo.create(new_session).await?;

    Ok(RefreshResult {
        user_id: claims.sub,
        session_id: new_session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
    })
}
