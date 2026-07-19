use std::fmt;
use std::net::IpAddr;

#[derive(Clone)]
pub struct Settings {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub rate_limit_max_attempts: u32,
    pub rate_limit_window_seconds: u64,
    pub cors_allowed_origins: String,
    pub trusted_proxy_ips: Vec<IpAddr>,
    pub redis_url: Option<String>,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db_sanitized = self
            .database_url
            .split('@')
            .next_back()
            .unwrap_or(&self.database_url);

        f.debug_struct("Settings")
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("database_url", &format!("<redacted>@{db_sanitized}"))
            .field("rate_limit_max_attempts", &self.rate_limit_max_attempts)
            .field("rate_limit_window_seconds", &self.rate_limit_window_seconds)
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
            rate_limit_max_attempts: std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| "RATE_LIMIT_MAX_ATTEMPTS must be a valid number".to_string())?,
            rate_limit_window_seconds: std::env::var("RATE_LIMIT_WINDOW_SECONDS")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .map_err(|_| "RATE_LIMIT_WINDOW_SECONDS must be a valid number".to_string())?,
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
