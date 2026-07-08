use std::net::IpAddr;

/// Minimum acceptable length (in bytes) for `JWT_SECRET`. HS256 signing strength is
/// bounded by the entropy of the secret, so a short/guessable value makes access and
/// refresh tokens forgeable via brute force. 32 bytes is the floor; `.env.example`
/// recommends 64 (`openssl rand -hex 64`).
pub const MIN_JWT_SECRET_LEN: usize = 32;

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
    Ok(())
}

#[derive(Debug, Clone)]
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

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set".to_string())?,
            server_host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
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
