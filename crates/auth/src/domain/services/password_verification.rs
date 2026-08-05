use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

use tracing::error;

use crate::domain::entities::user::User;

/// Precomputed dummy hashes (one per bcrypt cost) used to equalize the "user
/// not found" timing path. Computing a hash is ~4x slower per +1 cost, so the
/// result is cached per cost rather than recomputed on every such login.
/// See `verify_password_timing_safe`.
static DUMMY_PASSWORD_HASHES: LazyLock<Mutex<HashMap<u32, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn dummy_password_hash(cost: u32) -> Option<String> {
    {
        let cache = DUMMY_PASSWORD_HASHES
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(hash) = cache.get(&cost) {
            return hash.clone();
        }
    }

    let hash = match bcrypt::hash("dummy-password-for-timing-safety", cost) {
        Ok(hash) => Some(hash),
        Err(e) => {
            error!(error = %e, cost, "Failed to precompute dummy bcrypt hash; timing-safe fallback will use a 50ms delay");
            None
        }
    };

    let mut cache = DUMMY_PASSWORD_HASHES
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache.entry(cost).or_insert_with(|| hash.clone());
    hash
}

pub struct PasswordVerificationService;

impl PasswordVerificationService {
    pub fn hash_password(password: &str, cost: u32) -> Result<String, String> {
        bcrypt::hash(password, cost).map_err(|e| e.to_string())
    }
}

pub fn verify_password_timing_safe(user: Option<&User>, password: &str, cost: u32) -> bool {
    match user.and_then(|u| u.password_hash().map(|h| h.as_ref())) {
        Some(hash) => match bcrypt::verify(password, hash) {
            Ok(valid) => valid,
            Err(e) => {
                error!(error = %e, "bcrypt::verify failed during login");
                false
            }
        },
        None => match dummy_password_hash(cost) {
            Some(hash) => {
                let _ = bcrypt::verify(password, &hash);
                false
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                false
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::auth_settings::DEFAULT_BCRYPT_COST;

    #[test]
    fn dummy_hash_is_cached_per_cost() {
        let a = dummy_password_hash(DEFAULT_BCRYPT_COST);
        let b = dummy_password_hash(DEFAULT_BCRYPT_COST);
        assert_eq!(a, b);
        assert!(a.is_some());
    }
}
