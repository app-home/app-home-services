use std::net::IpAddr;

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use crate::application::ports::rate_limiter::RateLimiter;

/// Atomically increments the per-IP attempt counter and sets its expiry on the first
/// increment within a window (fixed-window counter). Doing this in a single Lua
/// script keeps INCR and EXPIRE atomic, so a crash or race between the two can never
/// leave a counter key without a TTL (which would otherwise grow unbounded in Redis).
const INCR_WITH_EXPIRE_SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if tonumber(current) == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
return current
"#;

/// Redis-backed implementation of the `RateLimiter` port.
///
/// Unlike `MemoryRateLimiter`, counters here live in Redis and are shared by every
/// instance of the service connected to the same Redis deployment, so the rate limit
/// stays effective when the service is scaled horizontally or restarted.
///
/// On Redis errors (e.g. connection dropped), this implementation fails open --
/// `check` returns `true` and `remaining_attempts` returns the max -- rather than
/// blocking every login because Redis is briefly unavailable. Each failure is logged
/// at `error` level so the outage is visible; `ConnectionManager` also reconnects
/// automatically in the background.
#[derive(Clone)]
pub struct RedisRateLimiter {
    conn: ConnectionManager,
    max_attempts: u32,
    window_seconds: u64,
}

impl RedisRateLimiter {
    /// Connects to Redis and returns a limiter with the given attempt budget and
    /// window. Fails fast (returning an error) if the initial connection cannot be
    /// established, so a misconfigured `REDIS_URL` is caught at startup rather than
    /// silently degrading rate limiting later.
    pub async fn connect(
        redis_url: &str,
        max_attempts: u32,
        window_seconds: u64,
    ) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_connection_manager().await?;
        Ok(Self {
            conn,
            max_attempts,
            window_seconds,
        })
    }

    fn key(&self, ip: IpAddr) -> String {
        format!("ratelimit:login:{ip}")
    }
}

#[async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn check(&self, ip: IpAddr) -> bool {
        let mut conn = self.conn.clone();
        let result: redis::RedisResult<Option<u32>> = conn.get(self.key(ip)).await;

        match result {
            Ok(Some(count)) => count < self.max_attempts,
            Ok(None) => true,
            Err(e) => {
                tracing::error!(error = %e, "Redis rate limiter: check failed, failing open");
                true
            }
        }
    }

    async fn record_attempt(&self, ip: IpAddr) {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(INCR_WITH_EXPIRE_SCRIPT);
        let result: redis::RedisResult<i64> = script
            .key(self.key(ip))
            .arg(self.window_seconds)
            .invoke_async(&mut conn)
            .await;

        if let Err(e) = result {
            tracing::error!(error = %e, "Redis rate limiter: failed to record attempt");
        }
    }

    async fn remaining_attempts(&self, ip: IpAddr) -> u32 {
        let mut conn = self.conn.clone();
        let result: redis::RedisResult<Option<u32>> = conn.get(self.key(ip)).await;

        match result {
            Ok(Some(count)) => self.max_attempts.saturating_sub(count),
            Ok(None) => self.max_attempts,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Redis rate limiter: remaining_attempts failed, defaulting to max"
                );
                self.max_attempts
            }
        }
    }

    async fn reset(&self, ip: IpAddr) {
        let mut conn = self.conn.clone();
        let result: redis::RedisResult<i64> = conn.del(self.key(ip)).await;

        if let Err(e) = result {
            tracing::error!(error = %e, "Redis rate limiter: failed to reset counter");
        }
    }
}
