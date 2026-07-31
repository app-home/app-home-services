use std::collections::HashSet;
use std::fmt;

pub const MIN_JWT_SECRET_LEN: usize = 32;

const MIN_UNIQUE_CHARS: usize = 8;

pub fn validate_jwt_secret(secret: &str) -> Result<(), String> {
    if secret.len() < MIN_JWT_SECRET_LEN {
        return Err(format!(
            "JWT_SECRET must be at least {MIN_JWT_SECRET_LEN} bytes long (got {}); generate one with `openssl rand -hex 64`",
            secret.len()
        ));
    }

    let unique_chars: HashSet<char> = secret.chars().collect();
    if unique_chars.len() < MIN_UNIQUE_CHARS {
        return Err(format!(
            "JWT_SECRET has too few unique characters ({}, minimum {MIN_UNIQUE_CHARS}); generate one with `openssl rand -hex 64`",
            unique_chars.len()
        ));
    }

    Ok(())
}

pub fn jwt_secret_low_entropy_warning(secret: &str) -> bool {
    let total = secret.len();
    let unique: HashSet<char> = secret.chars().collect();
    unique.len() < (total / 10).max(MIN_UNIQUE_CHARS)
}

/// Minimum length for `DEFAULT_USER_PASSWORD` -- the password for the admin
/// account this service auto-creates on first startup (`seed_default_user` in
/// `src/main.rs`). Deliberately longer than a typical "8 characters" minimum,
/// since this specific account is a known, predictable target (its username is
/// public knowledge -- it's whatever `DEFAULT_USER_USERNAME` is set to, `admin` by
/// default) and is created automatically without the operator necessarily having
/// thought hard about its password the way they might for a hand-created account.
const MIN_DEFAULT_PASSWORD_LEN: usize = 12;

/// Below this length (but still passing the hard minimum above), the password is
/// accepted but flagged as worth strengthening -- mirrors the JWT_SECRET
/// hard-minimum-vs-recommended-length split above.
const RECOMMENDED_DEFAULT_PASSWORD_LEN: usize = 16;

/// A backstop against the single most common real-world failure mode for this
/// specific field: an operator leaving a well-known placeholder value (or a
/// trivially guessable one) in `DEFAULT_USER_PASSWORD` because they didn't realize
/// it's the actual password for a real, automatically-created admin account.
///
/// This is intentionally short and un-exhaustive -- it is not a general-purpose
/// breached-password check (that would need an external list/service, which is out
/// of scope here). It exists to catch exactly the case that motivated this
/// validation: shipping with a value like `admin123` untouched.
const KNOWN_WEAK_PASSWORDS: &[&str] = &[
    "password",
    "password1",
    "password123",
    "admin",
    "admin123",
    "administrator",
    "12345678",
    "123456789",
    "1234567890",
    "qwerty123",
    "qwertyuiop",
    "letmein123",
    "welcome123",
    "changeme",
    "changeme123",
    "changethis",
    "default123",
    "rootroot",
    "p@ssw0rd",
    "password1!",
];

/// Validates `DEFAULT_USER_PASSWORD` strength. Returning `Err` here is a fatal
/// startup error (see `AuthSettings::from_env`), by design: the alternative --
/// starting anyway with a weak password for an automatically-created,
/// predictable-username admin account -- is the exact vulnerability this exists to
/// close. See #82.
pub fn validate_default_user_password(password: &str) -> Result<(), String> {
    if password.len() < MIN_DEFAULT_PASSWORD_LEN {
        return Err(format!(
            "DEFAULT_USER_PASSWORD must be at least {MIN_DEFAULT_PASSWORD_LEN} characters long (got {}); this is the password for the automatically-created admin account and must not be left weak",
            password.len()
        ));
    }

    let lowered = password.to_lowercase();
    if KNOWN_WEAK_PASSWORDS.iter().any(|weak| lowered == *weak) {
        return Err(
            "DEFAULT_USER_PASSWORD is a known weak/placeholder password and must not be used for the automatically-created admin account -- choose a unique password, e.g. `openssl rand -base64 18`"
                .to_string(),
        );
    }

    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password.chars().any(|c| !c.is_ascii_alphanumeric());
    let class_count = [has_lower, has_upper, has_digit, has_symbol]
        .iter()
        .filter(|&&present| present)
        .count();

    if class_count < 3 {
        return Err(format!(
            "DEFAULT_USER_PASSWORD must contain at least 3 of: lowercase letters, uppercase letters, digits, symbols (found {class_count}); e.g. generate one with `openssl rand -base64 18`"
        ));
    }

    Ok(())
}

/// Non-fatal check for a password that passes `validate_default_user_password` but
/// is still short relative to `RECOMMENDED_DEFAULT_PASSWORD_LEN` -- surfaced as a
/// startup warning, not a rejection, mirroring `jwt_secret_low_entropy_warning`.
pub fn default_user_password_below_recommended_length(password: &str) -> bool {
    password.len() < RECOMMENDED_DEFAULT_PASSWORD_LEN
}

#[derive(Clone)]
pub struct AuthSettings {
    pub default_user_username: String,
    pub default_user_password: String,
    pub default_user_email: String,
    pub google_client_id: String,
    pub jwt_secret: String,
    /// `iss` claim minted and required on this service's JWTs. Env-configurable
    /// (`JWT_ISSUER`) so each environment can reject tokens issued elsewhere.
    /// See #87.
    pub jwt_issuer: String,
    /// `aud` claim minted and required on this service's JWTs. Env-configurable
    /// (`JWT_AUDIENCE`) so a token for staging is never valid in production.
    /// See #87.
    pub jwt_audience: String,
    pub access_token_expiry_minutes: i64,
    pub refresh_token_expiry_days: i64,
}

impl fmt::Debug for AuthSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let jwt_preview = if self.jwt_secret.len() > 8 {
            format!("{}...", &self.jwt_secret[..8])
        } else {
            "(too short)".to_string()
        };

        f.debug_struct("AuthSettings")
            .field("default_user_username", &self.default_user_username)
            .field("default_user_email", &self.default_user_email)
            .field("jwt_secret", &jwt_preview)
            .field("jwt_issuer", &self.jwt_issuer)
            .field("jwt_audience", &self.jwt_audience)
            .field("default_user_password", &"<redacted>")
            .field("google_client_id", &"<redacted>")
            .field(
                "access_token_expiry_minutes",
                &self.access_token_expiry_minutes,
            )
            .field("refresh_token_expiry_days", &self.refresh_token_expiry_days)
            .finish()
    }
}

impl AuthSettings {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            default_user_username: std::env::var("DEFAULT_USER_USERNAME")
                .unwrap_or_else(|_| "admin".to_string()),
            default_user_password: {
                let password = std::env::var("DEFAULT_USER_PASSWORD")
                    .map_err(|_| "DEFAULT_USER_PASSWORD must be set".to_string())?;
                validate_default_user_password(&password)?;
                if default_user_password_below_recommended_length(&password) {
                    eprintln!(
                        "WARN: DEFAULT_USER_PASSWORD is shorter than the recommended {RECOMMENDED_DEFAULT_PASSWORD_LEN} characters; consider a longer, generated password (e.g. `openssl rand -base64 18`)"
                    );
                }
                password
            },
            default_user_email: std::env::var("DEFAULT_USER_EMAIL")
                .unwrap_or_else(|_| "admin@example.com".to_string()),
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| String::new()),
            jwt_secret: {
                let secret = std::env::var("JWT_SECRET")
                    .map_err(|_| "JWT_SECRET must be set".to_string())?;
                validate_jwt_secret(&secret)?;
                if jwt_secret_low_entropy_warning(&secret) {
                    eprintln!(
                        "WARN: JWT_SECRET has low character diversity; consider using `openssl rand -hex 64`"
                    );
                }
                secret
            },
            jwt_issuer: std::env::var("JWT_ISSUER")
                .unwrap_or_else(|_| "app-home-services".to_string()),
            jwt_audience: std::env::var("JWT_AUDIENCE")
                .unwrap_or_else(|_| "app-home-services".to_string()),
            access_token_expiry_minutes: std::env::var("ACCESS_TOKEN_EXPIRY_MINUTES")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .map_err(|_| "ACCESS_TOKEN_EXPIRY_MINUTES must be a valid number".to_string())?,
            refresh_token_expiry_days: std::env::var("REFRESH_TOKEN_EXPIRY_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .map_err(|_| "REFRESH_TOKEN_EXPIRY_DAYS must be a valid number".to_string())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_password_shorter_than_the_minimum() {
        let result = validate_default_user_password("Sh0rt!");
        assert!(result.is_err(), "a 6-character password must be rejected");
    }

    #[test]
    fn rejects_known_weak_passwords_case_insensitively() {
        for weak in [
            "admin123",
            "Admin123",
            "ADMIN123",
            "password123",
            "changeme123",
        ] {
            let result = validate_default_user_password(weak);
            assert!(
                result.is_err(),
                "expected {weak:?} to be rejected as a known weak password"
            );
        }
    }

    #[test]
    fn rejects_a_long_password_with_only_one_character_class() {
        // 16 lowercase letters: long enough, but fails the character-class check.
        let result = validate_default_user_password("aaaaaaaaaaaaaaaa");
        assert!(
            result.is_err(),
            "a password using only one character class must be rejected regardless of length"
        );
    }

    #[test]
    fn accepts_a_reasonably_strong_password() {
        let result = validate_default_user_password("C0rrect-Horse-Battery9");
        assert!(
            result.is_ok(),
            "expected a long password with 3+ character classes to be accepted, got {result:?}"
        );
    }

    #[test]
    fn flags_a_valid_but_short_password_for_the_recommended_length_warning() {
        // Passes validate_default_user_password (12+ chars, 3+ classes) but is
        // still under the recommended length.
        let password = "Sh0rt-Pass!!";
        assert!(validate_default_user_password(password).is_ok());
        assert!(default_user_password_below_recommended_length(password));
    }

    #[test]
    fn does_not_flag_a_password_at_or_above_the_recommended_length() {
        let password = "C0rrect-Horse-Battery9";
        assert!(!default_user_password_below_recommended_length(password));
    }
}
