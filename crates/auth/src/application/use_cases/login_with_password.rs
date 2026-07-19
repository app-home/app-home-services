use shared::domain::events::Event;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::user_repository::UserRepository;
use crate::config::auth_settings::AuthSettings;
use crate::domain::entities::user::User;
use crate::domain::errors::AuthError;
use crate::domain::services::password_verification::verify_password_timing_safe;

pub struct LoginResult {
    pub user: User,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub events: Vec<Event>,
}

pub async fn login_with_password(
    user_repo: &impl UserRepository,
    jwt_service: &impl JwtService,
    settings: &AuthSettings,
    username: &str,
    password: &str,
) -> Result<LoginResult, AuthError> {
    let mut aggregate = user_repo
        .find_aggregate_by_username(username)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    let password_ok = verify_password_timing_safe(Some(aggregate.user()), password);
    if !password_ok {
        return Err(AuthError::InvalidCredentials);
    }

    create_session_tokens(user_repo, jwt_service, settings, &mut aggregate).await
}

async fn create_session_tokens(
    user_repo: &impl UserRepository,
    jwt_service: &impl JwtService,
    settings: &AuthSettings,
    aggregate: &mut crate::domain::aggregate::UserAggregate,
) -> Result<LoginResult, AuthError> {
    let session_id = Uuid::now_v7();
    let token_pair = jwt_service.generate_token_pair(aggregate.user().id(), session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let refresh_hash = HashedPassword::new(
        bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
            .map_err(|_| AuthError::TokenGenerationFailed)?,
    )
    .map_err(|_| AuthError::TokenGenerationFailed)?;

    let new_session =
        aggregate.add_session(session_id, refresh_hash, expires_at, AuthMethod::Password)?;

    user_repo.save_aggregate(aggregate, &[new_session]).await?;

    let events = aggregate.take_events();

    Ok(LoginResult {
        user: aggregate.user().clone(),
        session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        events,
    })
}
