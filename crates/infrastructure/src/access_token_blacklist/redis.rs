use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use uuid::Uuid;

use shared::ports::{AccessTokenBlacklist, BlacklistError};

/// Upper bound for a single Redis round-trip (`revoke`/`is_revoked`, and the
/// startup `PING`). The blacklist check runs on every authenticated request,
/// so a Redis that is slow or partitioned (as opposed to one that cleanly
/// returns an error) must still degrade to fail-open instead of stalling the
/// request indefinitely -- `ConnectionManager` retries internally, which
/// would otherwise extend the wait even further. See #144.
const REDIS_TIMEOUT: Duration = Duration::from_millis(250);

/// Redis-backed access token revocation list.
///
/// Revoked tokens live in Redis (keyed by their `jti`) with a TTL equal to their
/// remaining lifetime, so they are shared by every instance of the service
/// connected to the same Redis deployment and expire on their own once the token
/// would have expired anyway.
///
/// On Redis errors (including a timeout, see `REDIS_TIMEOUT`) this
/// implementation surfaces `BlacklistError` and the caller (the
/// `AuthenticatedUser` extractor) fails open -- the token is allowed --
/// matching the rate limiter's posture, so a Redis outage degrades availability
/// rather than locking out every authenticated user. Each failure is logged at
/// `error` level and counted in `redis_error_count` so the outage is visible in
/// logs and metrics. `ConnectionManager` also reconnects automatically, so a
/// transient outage self-heals without restarting the process.
#[derive(Clone)]
pub struct RedisAccessTokenBlacklist {
    conn: ConnectionManager,
    redis_error_count: Arc<AtomicU64>,
}

impl RedisAccessTokenBlacklist {
    /// Builds a lazy connection to `redis_url` and attempts a `PING` (bounded
    /// by `REDIS_TIMEOUT`) to surface an obviously misconfigured URL early.
    ///
    /// Uses `get_connection_manager_lazy`, **not** `get_connection_manager`:
    /// the latter connects eagerly and would make `connect` itself return
    /// `Err` whenever Redis simply happens to be down at this exact moment --
    /// which is precisely the transient-outage case #143 needs to survive.
    /// The lazy variant returns immediately without requiring a live
    /// connection; the actual connection (and `ConnectionManager`'s automatic
    /// reconnection) happens on first use.
    ///
    /// A failed (or timed-out) `PING` does **not** fail `connect` either, for
    /// the same reason: it only logs a warning and returns the Redis-backed
    /// blacklist anyway, so every subsequent call fails open per-request (see
    /// `REDIS_TIMEOUT` and the trait docs) and self-heals automatically once
    /// Redis becomes reachable. `connect` only returns `Err` for a
    /// genuine configuration problem (e.g. `redis_url` fails to parse), in
    /// which case `build_access_token_blacklist` falls back to the in-memory
    /// backend.
    pub async fn connect(redis_url: &str) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_connection_manager_lazy(ConnectionManagerConfig::default())?;

        match tokio::time::timeout(
            REDIS_TIMEOUT,
            redis::cmd("PING").query_async::<()>(&mut conn.clone()),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(
                    error = %e,
                    "Redis access token blacklist: initial PING failed, continuing with the Redis backend -- ConnectionManager will retry and revoke/is_revoked will fail open until it reconnects"
                );
            }
            Err(_elapsed) => {
                tracing::warn!(
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis access token blacklist: initial PING timed out, continuing with the Redis backend -- ConnectionManager will retry and revoke/is_revoked will fail open until it reconnects"
                );
            }
        }

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
        let outcome = tokio::time::timeout(
            REDIS_TIMEOUT,
            conn.set_ex::<_, _, ()>(self.key(jti), "1", ttl_secs),
        )
        .await;

        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    jti = %jti,
                    "Redis access token blacklist: revoke failed"
                );
                Err(BlacklistError)
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    jti = %jti,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis access token blacklist: revoke timed out, failing open"
                );
                Err(BlacklistError)
            }
        }
    }

    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
        let mut conn = self.conn.clone();
        let outcome = tokio::time::timeout(REDIS_TIMEOUT, conn.exists::<_, i64>(self.key(jti))).await;

        match outcome {
            Ok(Ok(count)) => Ok(count > 0),
            Ok(Err(e)) => {
                self.record_redis_error();
                tracing::error!(
                    error = %e,
                    jti = %jti,
                    "Redis access token blacklist: exists check failed, failing open"
                );
                Err(BlacklistError)
            }
            Err(_elapsed) => {
                self.record_redis_error();
                tracing::error!(
                    jti = %jti,
                    timeout_ms = REDIS_TIMEOUT.as_millis(),
                    "Redis access token blacklist: exists check timed out, failing open"
                );
                Err(BlacklistError)
            }
        }
    }
}
