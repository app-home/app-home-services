use uuid::Uuid;

use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::session::NewSession;
use crate::domain::entities::user::User;
use crate::domain::errors::AuthError;

pub struct LoginResult {
    pub user: User,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
}

pub async fn login_with_password(
    user_repo: &impl UserRepository,
    session_repo: &impl SessionRepository,
    jwt_service: &impl JwtService,
    settings: &crate::infrastructure::config::settings::Settings,
    username: &str,
    password: &str,
) -> Result<LoginResult, AuthError> {
    let user = user_repo.find_by_username(username).await?;

    let user = match user {
        Some(user) if user.verify_password(password) => user,
        _ => return Err(AuthError::InvalidCredentials),
    };

    create_session_tokens(user_repo, session_repo, jwt_service, settings, user).await
}

pub async fn create_session_tokens(
    _user_repo: &impl UserRepository,
    session_repo: &impl SessionRepository,
    jwt_service: &impl JwtService,
    settings: &crate::infrastructure::config::settings::Settings,
    user: User,
) -> Result<LoginResult, AuthError> {
    let session_id = Uuid::now_v7();

    let token_pair = jwt_service.generate_token_pair(user.id, session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let password_hash = bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
        .map_err(|_| AuthError::TokenGenerationFailed)?;

    let new_session = NewSession::new(session_id, user.id, password_hash, expires_at);
    session_repo.create(new_session).await?;

    Ok(LoginResult {
        user,
        session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
    })
}
