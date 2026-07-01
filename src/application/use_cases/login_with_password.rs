use crate::domain::entities::user::User;
use crate::domain::errors::AuthError;
use crate::application::ports::user_repository::UserRepository;

pub async fn login_with_password(
    repo: &impl UserRepository,
    username: &str,
    password: &str,
) -> Result<User, AuthError> {
    let user = repo.find_by_username(username).await?;

    match user {
        Some(user) if user.verify_password(password) => Ok(user),
        _ => Err(AuthError::InvalidCredentials),
    }
}
