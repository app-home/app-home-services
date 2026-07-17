use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;

/// Minimum acceptable length (in bytes) for `JWT_SECRET`. HS256 signing strength is
/// bounded by the entropy of the secret, so a short/guessable value makes access and
/// refresh tokens forgeable via brute force. 32 bytes is the floor; `.env.example`
/// recommends 64 (`openssl rand -hex 64`).
pub const MIN_JWT_SECRET_LEN: usize = 32;

/// Minimum number of distinct characters required in a JWT secret to reject
/// trivially low-entropy values (e.g. a repeated single character).
const MIN_UNIQUE_CHARS: usize = 8;

/// Validates that a JWT secret meets the minimum strength requirement.
///
/// Pulled out as its own function (rather than inlined in `from_env`) so it can be
/// unit-tested directly without going through process environment variables.
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

/// Returns `true` if the secret has low unique-character diversity (informational
/// warning, not a hard reject). Used at startup to log a warning without aborting.
pub fn jwt_secret_low_entropy_warning(secret: &str) -> bool {
    let total = secret.len();
    let unique: HashSet<char> = secret.chars().collect();
    // Ratio below 10% unique chars suggests low entropy
    unique.len() < (total / 10).max(MIN_UNIQUE_CHARS)
}

#[derive(Clone)]
pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub default_user_username: String,
    pub default_user_password: String,
    pub default_user_email: String,
    pub google_client_id: String,
    pub jwt_secret: String,
    pub rate_limit_max_attempts: u32,
    pub rate_limit_window_seconds: u64,
    pub access_token_expiry_minutes: i64,
    pub refresh_token_expiry_days: i64,
    pub cors_allowed_origins: String,
    /// IP addresses of reverse proxies/load balancers that are trusted to set
    /// `X-Forwarded-For` / `X-Real-IP`. Requests whose direct TCP peer is not in this
    /// list will have those headers ignored entirely, and the real peer address will
    /// be used instead (see `adapters::inbound::login_routes::resolve_client_ip`).
    pub trusted_proxy_ips: Vec<IpAddr>,
    /// Optional Redis connection URL (e.g. `redis://127.0.0.1:6379`). When set, login
    /// rate limiting uses `RedisRateLimiter` so counters are shared across every
    /// instance of the service. When unset, rate limiting falls back to
    /// `MemoryRateLimiter`, which is only effective for a single-instance deployment.
    pub redis_url: Option<String>,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Extract host from database_url for display without the password
        let db_sanitized = self
            .database_url
            .split('@')
            .next_back()
            .unwrap_or(&self.database_url);

        let jwt_preview = if self.jwt_secret.len() > 8 {
            format!("{}...", &self.jwt_secret[..8])
        } else {
            "(too short)".to_string()
        };

        f.debug_struct("Settings")
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("database_url", &format!("<redacted>@{db_sanitized}"))
            .field("default_user_username", &self.default_user_username)
            .field("default_user_email", &self.default_user_email)
            .field("jwt_secret", &jwt_preview)
            .field("default_user_password", &"<redacted>")
            .field("google_client_id", &"<redacted>")
            .field("rate_limit_max_attempts", &self.rate_limit_max_attempts)
            .field("rate_limit_window_seconds", &self.rate_limit_window_seconds)
            .field("access_token_expiry_minutes", &self.access_token_expiry_minutes)
            .field("refresh_token_expiry_days", &self.refresh_token_expiry_days)
            .field("cors_allowed_origins", &self.cors_allowed_origins)
            .field("trusted_proxy_ips", &self.trusted_proxy_ips)
            .field("redis_url", &self.redis_url)
            .finish()
    }
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set".to_string())?,
            server_host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .map_err(|_| "SERVER_PORT must be a valid number".to_string())?,
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
                    tracing::warn!(
                        "JWT_SECRET has low character diversity; consider using `openssl rand -hex 64`"
                    );
                }
                secret
            },
            rate_limit_max_attempts: std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| "RATE_LIMIT_MAX_ATTEMPTS must be a valid number".to_string())?,
            rate_limit_window_seconds: std::env::var("RATE_LIMIT_WINDOW_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .map_err(|_| "RATE_LIMIT_WINDOW_SECONDS must be a valid number".to_string())?,
            access_token_expiry_minutes: std::env::var("ACCESS_TOKEN_EXPIRY_MINUTES")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .map_err(|_| "ACCESS_TOKEN_EXPIRY_MINUTES must be a valid number".to_string())?,
            refresh_token_expiry_days: std::env::var("REFRESH_TOKEN_EXPIRY_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .map_err(|_| "REFRESH_TOKEN_EXPIRY_DAYS must be a valid number".to_string())?,
            cors_allowed_origins: std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| String::new()),
            trusted_proxy_ips: std::env::var("TRUSTED_PROXY_IPS")
                .unwrap_or_else(|_| String::new())
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<IpAddr>().ok())
                .collect(),
            redis_url: std::env::var("REDIS_URL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
        })
    }
}
