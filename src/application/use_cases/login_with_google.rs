use shared::domain::events::user_created::UserCreated;
use shared::domain::events::user_logged_in::UserLoggedIn;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use crate::application::ports::auth_provider::AuthProvider;
use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::session::NewSession;
use crate::domain::entities::user::{NewUser, User};
use crate::domain::errors::AuthError;

pub struct LoginWithGoogleResult {
    pub user: User,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub is_new_user: bool,
    pub login_event: UserLoggedIn,
    pub created_event: Option<UserCreated>,
}

pub async fn login_with_google(
    user_repo: &impl UserRepository,
    session_repo: &impl SessionRepository,
    auth_provider: &impl AuthProvider,
    jwt_service: &impl JwtService,
    settings: &crate::infrastructure::config::settings::Settings,
    id_token: &str,
) -> Result<LoginWithGoogleResult, AuthError> {
    let user_info = auth_provider.verify_id_token(id_token).await?;

    let existing_user = user_repo.find_by_email(&user_info.email).await?;

    let (user, is_new_user, created_event) = match existing_user {
        Some(user) => (user, false, None),
        None => {
            let email = Email::new(user_info.email)
                .map_err(|e| AuthError::InternalError(e.to_string()))?;
            let new_user = NewUser::new_google(email, user_info.name);
            let user = user_repo.create(new_user).await?;
            let event = UserCreated::new(user.id(), shared::domain::value_objects::auth_provider::AuthProvider::Google);
            (user, true, Some(event))
        }
    };

    let session_id = Uuid::now_v7();

    let token_pair = jwt_service.generate_token_pair(user.id(), session_id)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::days(settings.refresh_token_expiry_days);

    let refresh_hash = HashedPassword::new(
        bcrypt::hash(&token_pair.refresh_token, bcrypt::DEFAULT_COST)
            .map_err(|_| AuthError::TokenGenerationFailed)?,
    )
    .map_err(|_| AuthError::TokenGenerationFailed)?;

    let new_session = NewSession::new(
        session_id,
        user.id(),
        refresh_hash,
        expires_at,
        AuthMethod::GoogleOAuth,
    );
    session_repo.create(new_session).await?;

    let login_event = UserLoggedIn::new(user.id(), session_id, AuthMethod::GoogleOAuth);

    Ok(LoginWithGoogleResult {
        user,
        session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        is_new_user,
        login_event,
        created_event,
    })
}
