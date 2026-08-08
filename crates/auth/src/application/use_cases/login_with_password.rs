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
    let aggregate = user_repo.find_aggregate_by_username(username).await?;

    // Verify against the existing user when found, otherwise against the
    // precomputed dummy hash for the same cost, so a username-not-found login
    // takes ~the same time as a wrong-password login (timing safety).
    //
    // Off the async runtime's worker threads and bounded process-wide (see
    // #175) -- `verify_password_timing_safe` is synchronous, CPU-bound bcrypt
    // work, so calling it inline here would block whichever Tokio worker
    // thread is running this task for the duration. `user` is cloned into an
    // owned value (rather than borrowed) because the closure below must be
    // `'static` to run on `spawn_blocking`'s thread pool.
    let user_owned = aggregate.as_ref().map(|a| a.user().clone());
    let password_owned = password.to_string();
    let cost = settings.bcrypt_cost;
    let password_ok = settings
        .bcrypt_limiter
        .run_bounded(move || {
            verify_password_timing_safe(user_owned.as_ref(), &password_owned, cost)
        })
        .await?;
    if !password_ok {
        return Err(AuthError::InvalidCredentials);
    }

    let mut aggregate = aggregate.ok_or(AuthError::InvalidCredentials)?;

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

    // See the bounded-hash comment in `login_with_password` above -- same
    // reasoning applies to hashing the new refresh token.
    let refresh_token_owned = token_pair.refresh_token.clone();
    let cost = settings.bcrypt_cost;
    let refresh_hash = HashedPassword::new(
        settings
            .bcrypt_limiter
            .run_bounded(move || bcrypt::hash(refresh_token_owned, cost))
            .await?
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
