use crate::domain::entities::user::{NewUser, User};
use crate::domain::errors::AuthError;
use crate::application::ports::user_repository::UserRepository;
use crate::application::ports::auth_provider::AuthProvider;

pub struct LoginWithGoogleResult {
    pub user: User,
    pub is_new_user: bool,
}

pub async fn login_with_google(
    repo: &impl UserRepository,
    auth_provider: &impl AuthProvider,
    id_token: &str,
) -> Result<LoginWithGoogleResult, AuthError> {
    let user_info = auth_provider.verify_id_token(id_token).await?;

    let existing_user = repo.find_by_email(&user_info.email).await?;

    match existing_user {
        Some(user) => Ok(LoginWithGoogleResult { user, is_new_user: false }),
        None => {
            let new_user = NewUser {
                username: None,
                email: user_info.email,
                display_name: user_info.name,
                password_hash: None,
                auth_provider: "google".to_string(),
            };
            let user = repo.create(new_user).await?;
            Ok(LoginWithGoogleResult { user, is_new_user: true })
        }
    }
}
