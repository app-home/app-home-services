use std::fmt;
use std::net::IpAddr;

use url::Url;

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
    /// When `true`, the connection to Postgres is forced to use
    /// `sslmode=verify-full` regardless of what `DATABASE_URL` specifies: any
    /// existing `sslmode` query parameter is replaced. This is the "production
    /// demands an encrypted, certificate-verified connection" switch -- it
    /// refuses to be undermined by an accidentally plaintext `sslmode=disable`
    /// in the connection string. See `force_sslmode_verify_full`. See #85.
    pub db_require_ssl: bool,
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
    /// When `true`, Swagger UI and the combined OpenAPI spec are served at
    /// `/swagger-ui` and `/api-docs/openapi.json`. When `false` (the default),
    /// neither route is registered at all, so an attacker cannot enumerate the
    /// full API surface from a publicly reachable instance. Enable this only
    /// where interactive API documentation is actually needed (e.g. local
    /// development). See #86.
    pub enable_swagger: bool,
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
            .field("db_require_ssl", &self.db_require_ssl)
            .field("metrics_allowed_ips", &self.metrics_allowed_ips)
            .field("enable_swagger", &self.enable_swagger)
            .finish()
    }
}

/// `sslmode` query parameter from a `postgres://`-style `DATABASE_URL`, or
/// `None` when the URL omits it (sqlx then defaults to `prefer`, which tries
/// TLS but silently falls back to plaintext).
fn extract_sslmode(url: &Url) -> Option<String> {
    url.query_pairs()
        .find_map(|(key, value)| (key == "sslmode").then(|| value.into_owned()))
}

/// Whether the database host is on the same machine as this process
/// (`127.0.0.1`, `::1`, or the `localhost` hostname). Plaintext connections to
/// such hosts never leave the machine, so `sslmode=disable` is acceptable there
/// for local development.
fn db_host_is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        // The `url` crate surfaces IPv4 literals like `127.0.0.1` as a Domain,
        // so parse the domain as an IP before falling back to hostname matches.
        Some(url::Host::Domain(domain)) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain
                    .parse::<IpAddr>()
                    .map(|ip| ip.is_loopback())
                    .unwrap_or(false)
        }
        None => false,
    }
}

/// Rejects a `DATABASE_URL` that would send credentials and data in plaintext
/// over the network: `sslmode=disable` against a non-loopback database host is a
/// fatal startup error, by design. The alternative -- starting anyway and
/// streaming password hashes, tokens and user data unencrypted to a remote
/// Postgres -- is the exact vulnerability this exists to close. See #85.
pub fn validate_database_ssl(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|e| format!("DATABASE_URL is not a valid URL: {e}"))?;
    if extract_sslmode(&parsed).as_deref() == Some("disable") && !db_host_is_loopback(&parsed) {
        return Err(format!(
            "DATABASE_URL uses sslmode=disable against a non-loopback database host ({:?}); this sends credentials and data in plaintext over the network. Use `?sslmode=verify-full`, or set DB_REQUIRE_SSL=true, for any non-local database. See .env.example.",
            parsed.host_str()
        ));
    }
    Ok(())
}

/// Non-fatal warning for a connection that can silently fall back to plaintext
/// against a non-loopback database host -- i.e. no `sslmode` (the sqlx default,
/// `prefer`, tries TLS but falls back to plaintext if the server doesn't support
/// it) or an explicit `prefer`. `sslmode=disable` is not reported here because
/// the remote-host case is already rejected by `validate_database_ssl`. See #85.
pub fn database_ssl_warning(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if db_host_is_loopback(&parsed) {
        return None;
    }
    let sslmode = extract_sslmode(&parsed);
    if sslmode.is_none() || sslmode.as_deref() == Some("prefer") {
        return Some(format!(
            "DATABASE_URL targets a non-loopback database host ({:?}) without `sslmode=verify-full`; the default (`prefer`) silently falls back to plaintext if the server doesn't support TLS. Use `?sslmode=verify-full` or set DB_REQUIRE_SSL=true.",
            parsed.host_str()
        ));
    }
    None
}

/// Rewrites a `DATABASE_URL` so it always connects with `sslmode=verify-full`,
/// replacing any existing `sslmode` value (including `disable`). Used when
/// `DB_REQUIRE_SSL=true` to make the "always encrypt and verify" requirement
/// effective regardless of what the connection string itself says. See #85.
pub fn force_sslmode_verify_full(url: &str) -> Result<String, String> {
    let mut parsed =
        Url::parse(url).map_err(|e| format!("DATABASE_URL is not a valid URL: {e}"))?;

    let remaining: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| key != "sslmode")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();

    parsed
        .query_pairs_mut()
        .clear()
        .extend_pairs(remaining.into_iter().chain(std::iter::once((
            "sslmode".to_string(),
            "verify-full".to_string(),
        ))));

    Ok(parsed.to_string())
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        let db_require_ssl = std::env::var("DB_REQUIRE_SSL")
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        let enable_swagger = std::env::var("ENABLE_SWAGGER")
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

        let database_url = {
            let raw = std::env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL must be set".to_string())?;
            if db_require_ssl {
                force_sslmode_verify_full(&raw)?
            } else {
                raw
            }
        };

        validate_database_ssl(&database_url)?;
        if let Some(warning) = database_ssl_warning(&database_url) {
            eprintln!("WARN: {warning}");
        }

        Ok(Self {
            database_url,
            db_require_ssl,
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
            enable_swagger,
        })
    }
}
