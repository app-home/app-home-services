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

#[derive(Clone)]
pub struct AuthSettings {
    pub default_user_username: String,
    pub default_user_password: String,
    pub default_user_email: String,
    pub google_client_id: String,
    pub jwt_secret: String,
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
            default_user_password: std::env::var("DEFAULT_USER_PASSWORD")
                .map_err(|_| "DEFAULT_USER_PASSWORD must be set".to_string())?,
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
