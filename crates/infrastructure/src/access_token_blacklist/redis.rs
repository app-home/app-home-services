use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use uuid::Uuid;

use shared::ports::{AccessTokenBlacklist, BlacklistError};

/// Redis-backed access token revocation list.
///
/// Revoked tokens live in Redis (keyed by their `jti`) with a TTL equal to their
/// remaining lifetime, so they are shared by every instance of the service
/// connected to the same Redis deployment and expire on their own once the token
/// would have expired anyway.
///
/// On Redis errors this implementation surfaces `BlacklistError` and the caller
/// (the `AuthenticatedUser` extractor) fails open -- the token is allowed --
/// matching the rate limiter's posture, so a Redis outage degrades availability
/// rather than locking out every authenticated user. Each failure is logged at
/// `error` level and counted in `redis_error_count` so the outage is visible in
/// logs and metrics. `ConnectionManager` also reconnects automatically.
#[derive(Clone)]
pub struct RedisAccessTokenBlacklist {
    conn: ConnectionManager,
    redis_error_count: Arc<AtomicU64>,
}

impl RedisAccessTokenBlacklist {
    /// Opens a connection to `redis_url` and verifies it with a `PING` before
    /// returning, so a misconfigured/unreachable Redis is caught at startup
    /// (see `build_access_token_blacklist`) rather than surfacing as a
    /// mysterious failure on the first `revoke`/`is_revoked` call.
    pub async fn connect(redis_url: &str) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_connection_manager().await?;
        redis::cmd("PING")
            .query_async::<()>(&mut conn.clone())
            .await?;
        Ok(Self {
            conn,
            redis_error_count: Arc::new(AtomicU64::new(0)),
        })
    }

    fn key(&self, jti: Uuid) -> String {
        format!("acl:revoked:{jti}")
    }

    /// Cumulative count of Redis errors observed by this instance since startup
    /// (polled into the `access_token_blacklist_redis_errors_total` metric --
    /// see `spawn_access_token_blacklist_metrics_poller` in `src/main.rs`).
    pub fn redis_error_count(&self) -> u64 {
        self.redis_error_count.load(Ordering::Relaxed)
    }

    /// Returns a shared handle to the error counter so it can be polled from
    /// outside this struct (e.g. by the metrics poller in `src/main.rs`)
    /// without holding a reference to the whole blacklist.
    pub fn error_counter_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.redis_error_count)
    }

    fn record_redis_error(&self) {
        self.redis_error_count.fetch_add(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl AccessTokenBlacklist for RedisAccessTokenBlacklist {
    async fn revoke(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError> {
        // `SET ... EX 0` / `SETEX` with a zero TTL is rejected by Redis ("ERR
        // invalid expire time"). `ttl_secs == 0` means the token's remaining
        // lifetime is already zero (it's expired or expiring this instant), so
        // there is nothing to revoke -- an already-expired token can't be used
        // regardless. Skip the round-trip instead of recording a spurious Redis
        // error for a token that was never going to validate again.
        if ttl_secs == 0 {
            return Ok(());
        }

        let mut conn = self.conn.clone();
        let result: redis::RedisResult<()> = conn.set_ex(self.key(jti), "1", ttl_secs).await;

        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    jti = %jti,
                    "Redis access token blacklist: revoke failed"
                );
                Err(BlacklistError)
            }
        }
    }

    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
        let mut conn = self.conn.clone();
        let result: redis::RedisResult<i64> = conn.exists(self.key(jti)).await;

        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    jti = %jti,
                    "Redis access token blacklist: exists check failed, failing open"
                );
                Err(BlacklistError)
            }
        }
    }
}
