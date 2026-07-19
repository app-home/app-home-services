use shared::domain::events::Event;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use crate::application::ports::auth_provider::AuthProvider;
use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::user_repository::UserRepository;
use crate::config::auth_settings::AuthSettings;
use crate::domain::entities::user::{NewUser, User};
use crate::domain::errors::AuthError;

pub struct LoginWithGoogleResult {
    pub user: User,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub is_new_user: bool,
    pub events: Vec<Event>,
}

pub async fn login_with_google(
    user_repo: &impl UserRepository,
    auth_provider: &impl AuthProvider,
    jwt_service: &impl JwtService,
    settings: &AuthSettings,
    id_token: &str,
) -> Result<LoginWithGoogleResult, AuthError> {
    let user_info = auth_provider.verify_id_token(id_token).await?;

    let existing = user_repo.find_aggregate_by_email(&user_info.email).await?;

    let (mut aggregate, is_new_user) = match existing {
        Some(agg) => (agg, false),
        None => {
            let email =
                Email::new(user_info.email).map_err(|e| AuthError::InternalError(e.to_string()))?;
            let new_user = NewUser::new_google(email, user_info.name);
            let user = user_repo.create(new_user).await?;
            // Load aggregate (user has no sessions yet)
            let agg = user_repo
                .find_aggregate_by_id(user.id())
                .await?
                .ok_or(AuthError::InternalError("created user not found".into()))?;
            (agg, true)
        }
    };

    let session_id = Uuid::now_v7();
    let token_pair = jwt_service.generate_token_pair(aggregate.user().id(), session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let refresh_hash = HashedPassword::new(
        bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
            .map_err(|_| AuthError::TokenGenerationFailed)?,
    )
    .map_err(|_| AuthError::TokenGenerationFailed)?;

    let new_session = aggregate.add_session(
        session_id,
        refresh_hash,
        expires_at,
        AuthMethod::GoogleOAuth,
    )?;

    user_repo
        .save_aggregate(&mut aggregate, &[new_session])
        .await?;

    let mut events = aggregate.take_events();
    if is_new_user {
        events.insert(
            0,
            Event::UserCreated(shared::domain::events::user_created::UserCreated::new(
                aggregate.user().id(),
                shared::domain::value_objects::auth_provider::AuthProvider::Google,
            )),
        );
    }

    Ok(LoginWithGoogleResult {
        user: aggregate.user().clone(),
        session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        is_new_user,
        events,
    })
}
