use std::sync::Arc;

use crate::adapters::outbound::memory_rate_limiter::MemoryRateLimiter;
use crate::adapters::outbound::redis_rate_limiter::RedisRateLimiter;
use crate::application::ports::rate_limiter::RateLimiter;
use crate::infrastructure::config::settings::Settings;

/// Chooses and constructs the rate limiter backends (one for login, one for refresh)
/// based on `settings.redis_url`: Redis-backed when set (required for correct rate
/// limiting when running more than one instance of this service), otherwise
/// in-memory (safe for a single instance only -- see `MemoryRateLimiter`'s docs).
///
/// Login and refresh each get their own limiter instance (and, on the Redis backend,
/// their own key namespace) so attempts against one endpoint never consume the
/// other's attempt budget for the same IP.
///
/// Pulled out of `main` so this selection behavior -- in particular, that an unset
/// `REDIS_URL` falls back to `MemoryRateLimiter` rather than silently doing something
/// else, and that a *set* `REDIS_URL` that's unreachable surfaces as an `Err` rather
/// than falling back silently -- can be asserted directly in a unit test, without
/// spawning the whole service.
pub async fn build_rate_limiters(
    settings: &Settings,
) -> Result<(Arc<dyn RateLimiter>, Arc<dyn RateLimiter>), redis::RedisError> {
    match &settings.redis_url {
        Some(redis_url) => {
            let login_limiter = RedisRateLimiter::connect(
                redis_url,
                settings.rate_limit_max_attempts,
                settings.rate_limit_window_seconds,
                "login",
            )
            .await?;
            let refresh_limiter = RedisRateLimiter::connect(
                redis_url,
                settings.rate_limit_max_attempts,
                settings.rate_limit_window_seconds,
                "refresh",
            )
            .await?;
            tracing::info!("Rate limiting backend: Redis (shared across instances)");
            Ok((Arc::new(login_limiter), Arc::new(refresh_limiter)))
        }
        None => {
            tracing::info!(
                "Rate limiting backend: in-memory (REDIS_URL not set -- only safe for a single instance)"
            );
            Ok((
                Arc::new(MemoryRateLimiter::new(
                    settings.rate_limit_max_attempts,
                    settings.rate_limit_window_seconds,
                )),
                Arc::new(MemoryRateLimiter::new(
                    settings.rate_limit_max_attempts,
                    settings.rate_limit_window_seconds,
                )),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_redis_url(redis_url: Option<String>) -> Settings {
        Settings {
            database_url: String::new(),
            server_host: "0.0.0.0".to_string(),
            server_port: 3000,
            default_user_username: "admin".to_string(),
            default_user_password: "irrelevant".to_string(),
            default_user_email: "admin@example.com".to_string(),
            google_client_id: String::new(),
            jwt_secret: "irrelevant-but-must-be-present".to_string(),
            rate_limit_max_attempts: 10,
            rate_limit_window_seconds: 300,
            access_token_expiry_minutes: 15,
            refresh_token_expiry_days: 7,
            cors_allowed_origins: String::new(),
            trusted_proxy_ips: vec![],
            redis_url,
        }
    }

    /// When REDIS_URL is unset, this must fall back to MemoryRateLimiter without ever
    /// attempting a network connection. There's no way to downcast the returned
    /// `Arc<dyn RateLimiter>` back to a concrete type to assert on it directly (the
    /// port intentionally doesn't require `Any`), so this asserts on the behavior
    /// that distinguishes the two paths instead: this call succeeds immediately with
    /// no Redis reachable in the test environment. If the `None` branch ever
    /// regressed into attempting a Redis connection, this test would fail (or hang)
    /// rather than silently passing.
    #[tokio::test]
    async fn falls_back_to_memory_backend_when_redis_url_is_unset() {
        let settings = settings_with_redis_url(None);

        build_rate_limiters(&settings)
            .await
            .expect("expected the memory-backed fallback to succeed");
    }

    /// The login and refresh limiters returned for the memory backend must be
    /// independent instances, not the same one reused twice -- otherwise a burst of
    /// login attempts would incorrectly eat into the refresh endpoint's budget.
    #[tokio::test]
    async fn memory_backend_login_and_refresh_limiters_are_independent() {
        use std::net::{IpAddr, Ipv4Addr};

        let settings = settings_with_redis_url(None);
        let (login_limiter, refresh_limiter) = build_rate_limiters(&settings)
            .await
            .expect("memory backend should always succeed");

        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        for _ in 0..settings.rate_limit_max_attempts {
            login_limiter.record_attempt(ip).await;
        }

        assert!(
            !login_limiter.check(ip).await,
            "login limiter should be exhausted after max_attempts"
        );
        assert!(
            refresh_limiter.check(ip).await,
            "refresh limiter must be unaffected by login attempts for the same IP"
        );
    }

    /// A configured-but-unreachable REDIS_URL must surface as an error rather than
    /// silently falling back to the memory backend -- losing shared rate limiting
    /// across instances without anyone noticing would be worse than failing loudly.
    #[tokio::test]
    async fn returns_an_error_when_redis_url_is_set_but_unreachable() {
        // Port 1 is a privileged port very unlikely to have anything listening on it
        // in a test environment, and connection refusal there is typically immediate
        // rather than requiring a timeout to elapse.
        let settings = settings_with_redis_url(Some("redis://127.0.0.1:1".to_string()));

        let result = build_rate_limiters(&settings).await;

        assert!(
            result.is_err(),
            "an unreachable REDIS_URL must be reported as an error, not silently ignored"
        );
    }
}
