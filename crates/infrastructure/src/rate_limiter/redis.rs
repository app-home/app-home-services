use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use shared::ports::RateLimiter;

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

/// Atomically increments AND checks whether the result is within budget,
/// all in a single Lua call. Returns 1 (true) if the request is allowed,
/// 0 (false) if rate-limited.
const TRY_CHECK_AND_RECORD_SCRIPT: &str = r#"
local current = redis.call('INCR', KEYS[1])
if tonumber(current) == 1 then
    redis.call('EXPIRE', KEYS[1], ARGV[1])
end
if tonumber(current) <= tonumber(ARGV[2]) then
    return 1
else
    return 0
end
"#;

/// Redis-backed implementation of the `RateLimiter` port.
///
/// Unlike `MemoryRateLimiter`, counters here live in Redis and are shared by every
/// instance of the service connected to the same Redis deployment, so the rate limit
/// stays effective when the service is scaled horizontally or restarted.
///
/// `key_prefix` scopes the counters to a specific protected action (e.g. `"login"` or
/// `"refresh"`), so two different endpoints rate-limited independently never share a
/// counter -- a burst of refresh attempts from an IP must not eat into that same IP's
/// login attempt budget, and vice versa.
///
/// On Redis errors (e.g. connection dropped), this implementation fails open --
/// `check` returns `true` and `remaining_attempts` returns the max -- rather than
/// blocking every request because Redis is briefly unavailable. Each failure is
/// logged at `error` level so the outage is visible in logs, and also counted in
/// `redis_error_count` (see its docs) so it can additionally be surfaced as a metric.
/// `ConnectionManager` also reconnects automatically in the background.
#[derive(Clone)]
pub struct RedisRateLimiter {
    conn: ConnectionManager,
    max_attempts: u32,
    window_seconds: u64,
    key_prefix: String,
    redis_error_count: Arc<AtomicU64>,
}

impl RedisRateLimiter {
    pub async fn connect(
        redis_url: &str,
        max_attempts: u32,
        window_seconds: u64,
        key_prefix: impl Into<String>,
    ) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_connection_manager().await?;
        redis::cmd("PING")
            .query_async::<()>(&mut conn.clone())
            .await?;
        Ok(Self {
            conn,
            max_attempts,
            window_seconds,
            key_prefix: key_prefix.into(),
            redis_error_count: Arc::new(AtomicU64::new(0)),
        })
    }

    fn key(&self, ip: IpAddr) -> String {
        format!("ratelimit:{}:{ip}", self.key_prefix)
    }

    pub fn redis_error_count(&self) -> u64 {
        self.redis_error_count.load(Ordering::Relaxed)
    }

    pub fn error_counter_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.redis_error_count)
    }

    fn record_redis_error(&self) {
        self.redis_error_count.fetch_add(1, Ordering::Relaxed);
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
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: check failed, failing open"
                );
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
            self.record_redis_error();
            tracing::error!(
                error = %e,
                scope = %self.key_prefix,
                "Redis rate limiter: failed to record attempt"
            );
        }
    }

    async fn try_check_and_record(&self, ip: IpAddr) -> bool {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(TRY_CHECK_AND_RECORD_SCRIPT);
        let result: redis::RedisResult<i64> = script
            .key(self.key(ip))
            .arg(self.window_seconds)
            .arg(self.max_attempts)
            .invoke_async(&mut conn)
            .await;

        match result {
            Ok(allowed) => allowed != 0,
            Err(e) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: try_check_and_record failed, failing open"
                );
                true
            }
        }
    }

    async fn remaining_attempts(&self, ip: IpAddr) -> u32 {
        let mut conn = self.conn.clone();
        let result: redis::RedisResult<Option<u32>> = conn.get(self.key(ip)).await;

        match result {
            Ok(Some(count)) => self.max_attempts.saturating_sub(count),
            Ok(None) => self.max_attempts,
            Err(e) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
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
            self.record_redis_error();
            tracing::error!(
                error = %e,
                scope = %self.key_prefix,
                "Redis rate limiter: failed to reset counter"
            );
        }
    }
}
