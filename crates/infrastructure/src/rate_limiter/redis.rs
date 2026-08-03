use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;

use shared::ports::RateLimiter;

use crate::rate_limiter::memory::MemoryRateLimiter;

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

/// Upper bound for a single Redis round-trip. `redis` 1.x's `ConnectionManagerConfig`
/// defaults `response_timeout`/`connection_timeout` to `None` -- i.e. unbounded -- so
/// without this, a Redis that is slow or partitioned (as opposed to one that cleanly
/// returns a connection error) would simply hang every `await` below indefinitely.
/// That's strictly worse than the fail-open behavior this type replaced: the shadow
/// fallback (see the struct docs) only ever engages once a call resolves to `Err`, so
/// an unbounded hang would mean neither Redis nor the shadow enforce anything for as
/// long as the hang lasts. See #89 review (CodeRabbit).
const REDIS_TIMEOUT: Duration = Duration::from_millis(250);

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
/// On Redis errors or timeouts (see `REDIS_TIMEOUT`), this implementation does not
/// stall the request and does not silently drop rate limiting: every operation is
/// replayed against an internal in-memory `MemoryRateLimiter` *shadow*, so a per-IP
/// budget is still enforced on this instance while Redis is unreachable (see #89).
/// The shadow only covers a single instance (each replica enforces its own budget
/// during an outage) and its memory is bounded (see `MemoryRateLimiter`'s
/// `MAX_ENTRIES` and `clean_expired`). Each failure is logged at `error` level so the
/// outage is visible in logs, and also counted in `redis_error_count` (see its docs)
/// so it can additionally be surfaced as a metric. `ConnectionManager` also
/// reconnects automatically in the background; once Redis is reachable again,
/// operations go back to Redis and the shadow is ignored.
#[derive(Clone)]
pub struct RedisRateLimiter {
    conn: ConnectionManager,
    max_attempts: u32,
    window_seconds: u64,
    key_prefix: String,
    redis_error_count: Arc<AtomicU64>,
    shadow: Arc<MemoryRateLimiter>,
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
            shadow: Arc::new(MemoryRateLimiter::new(max_attempts, window_seconds)),
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
        let outcome =
            tokio::time::timeout(REDIS_TIMEOUT, conn.get::<_, Option<u32>>(self.key(ip))).await;

        match outcome {
            Ok(Ok(Some(count))) => count < self.max_attempts,
            Ok(Ok(None)) => true,
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: check failed, enforcing the in-memory shadow budget"
                );
                self.shadow.check(ip).await
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    scope = %self.key_prefix,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis rate limiter: check timed out, enforcing the in-memory shadow budget"
                );
                self.shadow.check(ip).await
            }
        }
    }

    async fn record_attempt(&self, ip: IpAddr) {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(INCR_WITH_EXPIRE_SCRIPT);
        let outcome = tokio::time::timeout(
            REDIS_TIMEOUT,
            script
                .key(self.key(ip))
                .arg(self.window_seconds)
                .invoke_async::<i64>(&mut conn),
        )
        .await;

        match outcome {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: failed to record attempt, recording in the in-memory shadow"
                );
                self.shadow.record_attempt(ip).await;
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    scope = %self.key_prefix,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis rate limiter: record_attempt timed out, recording in the in-memory shadow"
                );
                self.shadow.record_attempt(ip).await;
            }
        }
    }

    async fn try_check_and_record(&self, ip: IpAddr) -> bool {
        let mut conn = self.conn.clone();
        let script = redis::Script::new(TRY_CHECK_AND_RECORD_SCRIPT);
        let outcome = tokio::time::timeout(
            REDIS_TIMEOUT,
            script
                .key(self.key(ip))
                .arg(self.window_seconds)
                .arg(self.max_attempts)
                .invoke_async::<i64>(&mut conn),
        )
        .await;

        match outcome {
            Ok(Ok(allowed)) => allowed != 0,
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: try_check_and_record failed, enforcing the in-memory shadow budget"
                );
                self.shadow.try_check_and_record(ip).await
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    scope = %self.key_prefix,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis rate limiter: try_check_and_record timed out, enforcing the in-memory shadow budget"
                );
                self.shadow.try_check_and_record(ip).await
            }
        }
    }

    async fn remaining_attempts(&self, ip: IpAddr) -> u32 {
        let mut conn = self.conn.clone();
        let outcome =
            tokio::time::timeout(REDIS_TIMEOUT, conn.get::<_, Option<u32>>(self.key(ip))).await;

        match outcome {
            Ok(Ok(Some(count))) => self.max_attempts.saturating_sub(count),
            Ok(Ok(None)) => self.max_attempts,
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: remaining_attempts failed, reading the in-memory shadow"
                );
                self.shadow.remaining_attempts(ip).await
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    scope = %self.key_prefix,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis rate limiter: remaining_attempts timed out, reading the in-memory shadow"
                );
                self.shadow.remaining_attempts(ip).await
            }
        }
    }

    async fn reset(&self, ip: IpAddr) {
        let mut conn = self.conn.clone();
        let outcome = tokio::time::timeout(REDIS_TIMEOUT, conn.del::<_, i64>(self.key(ip))).await;

        // The shadow is cleared unconditionally, regardless of the Redis outcome
        // below: it's what THIS instance enforces right now, and a reset (e.g.
        // after a successful login) should take effect on this instance
        // immediately even if Redis is unreachable.
        //
        // Attempted the Redis DEL first (not after, as an earlier version of this
        // did) so a successful DEL and the shadow clear both reflect the same
        // underlying fact -- the reset actually landed everywhere -- rather than
        // clearing local state first and then finding out Redis was never told.
        //
        // If DEL fails or times out, the counter in Redis is NOT cleared: once
        // Redis recovers, a request on *another* instance (or this one, after the
        // shadow's own TTL for this jti elapses) reads that stale, un-reset
        // count. This is bounded, not permanent -- the key already carries a TTL
        // of `window_seconds` from `INCR_WITH_EXPIRE_SCRIPT`, so the stale count
        // self-expires within one window at worst. Retrying the DEL durably
        // (mirroring the outbox pattern `DurableRevocationBlacklist` uses for
        // access-token revocation, see #140) would close that window entirely,
        // but is more machinery than a bounded, self-healing reset gap justifies
        // here -- revisit if `window_seconds` is ever configured large enough
        // that the gap becomes operationally meaningful.
        match outcome {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    scope = %self.key_prefix,
                    "Redis rate limiter: failed to reset counter (shadow was still cleared; the stale Redis counter self-expires within one window)"
                );
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    scope = %self.key_prefix,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis rate limiter: reset timed out (shadow was still cleared; the stale Redis counter self-expires within one window)"
                );
            }
        }

        self.shadow.reset(ip).await;
    }
}
