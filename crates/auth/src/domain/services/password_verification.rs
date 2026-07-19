use std::sync::LazyLock;

use tracing::error;

use crate::domain::entities::user::User;

static DUMMY_PASSWORD_HASH: LazyLock<Option<String>> = LazyLock::new(|| {
    match bcrypt::hash("dummy-password-for-timing-safety", bcrypt::DEFAULT_COST) {
        Ok(hash) => Some(hash),
        Err(e) => {
            error!(error = %e, "Failed to precompute dummy bcrypt hash; timing-safe fallback will use a 50ms delay");
            None
        }
    }
});

pub struct PasswordVerificationService;

impl PasswordVerificationService {
    pub fn hash_password(password: &str) -> Result<String, String> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| e.to_string())
    }
}

pub fn verify_password_timing_safe(user: Option<&User>, password: &str) -> bool {
    match user.and_then(|u| u.password_hash().map(|h| h.as_ref())) {
        Some(hash) => match bcrypt::verify(password, hash) {
            Ok(valid) => valid,
            Err(e) => {
                error!(error = %e, "bcrypt::verify failed during login");
                false
            }
        },
        None => match DUMMY_PASSWORD_HASH.as_deref() {
            Some(hash) => {
                let _ = bcrypt::verify(password, hash);
                false
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                false
            }
        },
    }
}
