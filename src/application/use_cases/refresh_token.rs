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
    /// The auth method the *original* session was created with ("password" or
    /// "google_oauth"), carried forward from the session being rotated so callers
    /// (e.g. the audit log) can record the real method instead of assuming one.
    pub auth_method: String,
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
        // Reusing an already-invalidated refresh token is a strong signal of theft:
        // the legitimate client always holds the *current* token after rotation, so
        // a request bearing the old one most likely means it was copied by someone
        // else. Revoke every active session for this user rather than only rejecting
        // this one request, and log it distinctly from a routine "session expired"
        // rejection so it's visible to whoever is watching security events.
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

    // Carry the auth_method forward from the session being rotated, rather than
    // assuming/hardcoding one -- a rotated Google-originated session must stay
    // attributed to "google_oauth", not silently become "password".
    let auth_method = session.auth_method.clone();

    let new_session = NewSession::new(
        new_session_id,
        claims.sub,
        refresh_hash,
        expires_at,
        auth_method.clone(),
    );
    session_repo.create(new_session).await?;

    Ok(RefreshResult {
        user_id: claims.sub,
        session_id: new_session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        auth_method,
    })
}
