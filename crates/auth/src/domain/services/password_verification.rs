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

/// Extracts the cost embedded in a bcrypt hash like `$2b$12$...`.
fn stored_hash_cost(hash: &str) -> Option<u32> {
    let mut parts = hash.split('$');
    if !parts.next()?.is_empty() {
        return None;
    }
    let _version = parts.next()?;
    parts.next()?.parse().ok()
}

/// Equalizes the verification time of a stored hash whose cost is lower than
/// the configured cost (a pre-#94 legacy hash), so its login takes ~the same
/// time as a not-found login (which spends `configured_cost` work).
///
/// bcrypt work doubles per +1 cost, so `2^(D-C)` verifications at cost `C`
/// take the same time as a single verification at cost `D`. A found user's
/// real verification already spends `C`; padding runs the remaining
/// `2^(D-C) - 1` verifications against the cached dummy hash at cost `C`.
fn pad_timing_to(stored_hash: &str, configured_cost: u32) {
    let Some(stored_cost) = stored_hash_cost(stored_hash) else {
        return;
    };
    if stored_cost >= configured_cost {
        return;
    }
    let Some(dummy) = dummy_password_hash(stored_cost) else {
        std::thread::sleep(std::time::Duration::from_millis(50));
        return;
    };
    let extra_verifies = (1u32 << (configured_cost - stored_cost)) - 1;
    for _ in 0..extra_verifies {
        let _ = bcrypt::verify("dummy-password-for-timing-safety", &dummy);
    }
}

/// Stateless bcrypt password-hashing service.
///
/// All operations take an explicit `cost` (the configured
/// `AuthSettings::bcrypt_cost`); callers must pass the same cost they use
/// elsewhere so verify timing and hashing stay consistent. See #94.
pub struct PasswordVerificationService;

impl PasswordVerificationService {
    /// Hashes `password` with the given bcrypt `cost`.
    ///
    /// Returns the `$2b$...` hash string, or a human-readable `Err(String)` if
    /// bcrypt rejects the cost (e.g. > 31) or the password is empty.
    pub fn hash_password(password: &str, cost: u32) -> Result<String, String> {
        bcrypt::hash(password, cost).map_err(|e| e.to_string())
    }
}

/// Verifies `password` against `user`'s stored hash in a timing-safe way.
///
/// When `user` is `None` (or has no stored hash -- e.g. a Google-only account)
/// the password is verified against a precomputed dummy hash generated at
/// `cost`, so a username-not-found login takes roughly the same time as a
/// wrong-password login and cannot be distinguished by response time.
///
/// Returns `true` only when the stored hash exists and verifies; every other
/// path (no user, no hash, unverifiable/corrupt stored hash) returns `false`
/// and never leaks which one it was. `cost` must match the cost used to hash
/// the stored password; it only affects the dummy-hash path for missing users.
pub fn verify_password_timing_safe(user: Option<&User>, password: &str, cost: u32) -> bool {
    match user.and_then(|u| u.password_hash().map(|h| h.as_ref())) {
        Some(hash) => {
            pad_timing_to(hash, cost);
            match bcrypt::verify(password, hash) {
                Ok(valid) => valid,
                Err(e) => {
                    error!(error = %e, "bcrypt::verify failed during login");
                    false
                }
            }
        }
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
