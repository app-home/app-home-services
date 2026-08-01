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
impl AccessTokenBlacklist for RedisAccessTokenBlacklist {
    async fn revoke(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError> {
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
