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
    /// Maximum number of connections the database pool will open at once. See
    /// `.env.example` for guidance on choosing this relative to Postgres's own
    /// `max_connections` when running more than one instance of this service.
    pub db_max_connections: u32,
    /// Minimum number of idle connections the pool tries to keep open, so a burst
    /// of traffic after a quiet period doesn't have to pay connection-setup latency
    /// on the first requests. `0` (the default) opens connections lazily instead.
    pub db_min_connections: u32,
    /// How long a query/request is allowed to wait for a connection to become
    /// available from the pool before giving up. This is what turns pool
    /// exhaustion into a fast, explicit error instead of a request hanging
    /// indefinitely.
    pub db_acquire_timeout_seconds: u64,
    /// How long a connection may sit idle in the pool before being closed and
    /// removed, recycling connections that would otherwise accumulate past what's
    /// actually needed. `0` disables idle recycling (connections are never closed
    /// for being idle).
    pub db_idle_timeout_seconds: u64,
    /// Maximum lifetime of a single connection before it's closed and replaced,
    /// regardless of activity -- protects against a connection going silently
    /// stale behind an intermediary (e.g. a proxy or load balancer in front of
    /// Postgres) that can drop long-lived connections without either side
    /// noticing immediately. `0` disables lifetime-based recycling.
    pub db_max_lifetime_seconds: u64,
    /// IP addresses allowed to reach `GET /metrics` (e.g. the Prometheus server's
    /// IP). Resolved the same way as rate-limiting IPs -- honoring
    /// `X-Forwarded-For`/`X-Real-IP` only from `trusted_proxy_ips` -- so this works
    /// correctly behind a reverse proxy too. Loopback addresses are always allowed
    /// regardless of this list, so local scraping/testing never gets locked out.
    ///
    /// Empty (the default) means no IP restriction is applied -- `/metrics` is
    /// reachable by anything that can reach the port, same as before this setting
    /// existed. This is a deliberately backward-compatible default: the actual fix
    /// for `/metrics` being unauthenticated-by-default was changing `SERVER_HOST`'s
    /// default to `127.0.0.1` (see #80); this allowlist is additional,
    /// defense-in-depth hardening for deployments that explicitly opt into
    /// `SERVER_HOST=0.0.0.0` (e.g. containers) and want `/metrics` locked down
    /// without standing up a full reverse-proxy/auth setup. See #83.
    pub metrics_allowed_ips: Vec<IpAddr>,
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
            .field("db_max_connections", &self.db_max_connections)
            .field("db_min_connections", &self.db_min_connections)
            .field(
                "db_acquire_timeout_seconds",
                &self.db_acquire_timeout_seconds,
            )
            .field("db_idle_timeout_seconds", &self.db_idle_timeout_seconds)
            .field("db_max_lifetime_seconds", &self.db_max_lifetime_seconds)
            .field("metrics_allowed_ips", &self.metrics_allowed_ips)
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
            db_max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| "DB_MAX_CONNECTIONS must be a valid number".to_string())?,
            db_min_connections: std::env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .map_err(|_| "DB_MIN_CONNECTIONS must be a valid number".to_string())?,
            db_acquire_timeout_seconds: std::env::var("DB_ACQUIRE_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .map_err(|_| "DB_ACQUIRE_TIMEOUT_SECONDS must be a valid number".to_string())?,
            db_idle_timeout_seconds: std::env::var("DB_IDLE_TIMEOUT_SECONDS")
                .unwrap_or_else(|_| "600".to_string())
                .parse()
                .map_err(|_| "DB_IDLE_TIMEOUT_SECONDS must be a valid number".to_string())?,
            db_max_lifetime_seconds: std::env::var("DB_MAX_LIFETIME_SECONDS")
                .unwrap_or_else(|_| "1800".to_string())
                .parse()
                .map_err(|_| "DB_MAX_LIFETIME_SECONDS must be a valid number".to_string())?,
            metrics_allowed_ips: std::env::var("METRICS_ALLOWED_IPS")
                .unwrap_or_else(|_| String::new())
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<IpAddr>().ok())
                .collect(),
        })
    }
}
