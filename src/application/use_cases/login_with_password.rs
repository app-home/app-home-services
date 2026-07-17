use std::sync::LazyLock;

use uuid::Uuid;

use crate::application::ports::jwt_service::JwtService;
use crate::application::ports::session_repository::SessionRepository;
use crate::application::ports::user_repository::UserRepository;
use crate::domain::entities::session::NewSession;
use crate::domain::entities::user::User;
use crate::domain::errors::AuthError;
use tracing::error;

pub struct LoginResult {
    pub user: User,
    pub session_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
}

/// A precomputed bcrypt hash used to perform a "dummy" password verification whenever
/// there's no real hash to check the supplied password against -- either because the
/// username doesn't exist, or because it exists but has no password set (e.g. a
/// Google-only account). Without this, those paths return almost instantly (no
/// `bcrypt::verify` call), while a real "wrong password for an existing user" always
/// costs one `bcrypt::verify` (tens of milliseconds) -- a timing difference an
/// attacker can use to enumerate valid usernames even though every case returns the
/// same `AuthError::InvalidCredentials` and HTTP response.
///
/// Computed once, lazily, at the same cost factor (`bcrypt::DEFAULT_COST`) real user
/// password hashes use, so the dummy check costs the same as a real one.
///
/// Uses `Option` so a bcrypt failure at lazy-init time does not crash the process on
/// the first login request -- the error is logged and a fixed 50ms sleep is used as a
/// timing-safe fallback instead.
static DUMMY_PASSWORD_HASH: LazyLock<Option<String>> = LazyLock::new(|| {
    match bcrypt::hash("dummy-password-for-timing-safety", bcrypt::DEFAULT_COST) {
        Ok(hash) => Some(hash),
        Err(e) => {
            error!(error = %e, "Failed to precompute dummy bcrypt hash; timing-safe fallback will use a 50ms delay");
            None
        }
    }
});

/// Verifies `password` against `user`'s stored hash, always performing exactly one
/// `bcrypt::verify` call regardless of whether `user` is `None` or has no password
/// set -- see `DUMMY_PASSWORD_HASH` for why this matters. `pub` so its timing
/// behavior can be directly unit-tested.
pub fn verify_password_timing_safe(user: Option<&User>, password: &str) -> bool {
    match user.and_then(|u| u.password_hash.as_deref()) {
        Some(hash) => match bcrypt::verify(password, hash) {
            Ok(valid) => valid,
            Err(e) => {
                error!(error = %e, "bcrypt::verify failed during login");
                false
            }
        },
        None => {
            match DUMMY_PASSWORD_HASH.as_deref() {
                Some(hash) => {
                    let _ = bcrypt::verify(password, hash);
                }
                None => {
                    // Fallback timing-safe delay if bcrypt precomputation failed.
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
            false
        }
    }
}

pub async fn login_with_password(
    user_repo: &impl UserRepository,
    session_repo: &impl SessionRepository,
    jwt_service: &impl JwtService,
    settings: &crate::infrastructure::config::settings::Settings,
    username: &str,
    password: &str,
) -> Result<LoginResult, AuthError> {
    let user_opt = user_repo.find_by_username(username).await?;
    let password_ok = verify_password_timing_safe(user_opt.as_ref(), password);

    let user = match (password_ok, user_opt) {
        (true, Some(user)) => user,
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

    let new_session = NewSession::new(session_id, user.id, password_hash, expires_at, "password");
    session_repo.create(new_session).await?;

    Ok(LoginResult {
        user,
        session_id,
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
    })
}
