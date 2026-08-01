use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;

use shared::ports::{AccessTokenBlacklist, BlacklistError};

/// One journaled-but-not-yet-flushed revocation, tracked in process memory so
/// this instance rejects the token immediately even while Redis is down (it
/// lives in the `access_token_revocation_outbox` table for durability across
/// restarts and instances; the in-memory copy is only what makes the very next
/// `is_revoked` check fast enough for the request path).
#[derive(Debug, Clone, Copy)]
struct PendingEntry {
    added_at: Instant,
    ttl: Duration,
}

/// Wraps a revocation list backend (in practice `RedisAccessTokenBlacklist`) with
/// Postgres journaling, so a revocation that the backend rejects at logout time is
/// not silently lost (see #140).
///
/// The plain `RedisAccessTokenBlacklist` fails open on a Redis error: a failed
/// `revoke` simply means the token is never recorded, and stays valid until its
/// natural expiry even though the client believes it logged out. This decorator
/// changes that: when the inner backend rejects a `revoke`, the revocation is
/// journaled in the `access_token_revocation_outbox` table (migration 009) and the
/// logout succeeds -- the revocation is now *durable*, and a background flush
/// worker (see `flush_pending`, spawned from `main`) retries it against the inner
/// backend until it lands.
///
/// The one corner that still cannot be made durable is when the inner backend AND
/// Postgres are both down at once: then there is nowhere to journal, `revoke`
/// surfaces `BlacklistError`, and the caller's existing fail-open behavior applies
/// (the logout still succeeds, logged at error).
///
/// `is_revoked` checks the in-process pending set first, so a token whose
/// revocation is journaled-but-unflushed is rejected by *this* instance
/// immediately, without waiting for the flush. Other instances reject it once the
/// flush worker lands it in Redis (within one flush interval) -- an inherent
/// eventual-consistency window, strictly better than the pre-#140 behavior where a
/// Redis outage dropped the revocation entirely.
pub struct DurableRevocationBlacklist {
    inner: Arc<dyn AccessTokenBlacklist>,
    pool: PgPool,
    pending: Mutex<HashMap<Uuid, PendingEntry>>,
}

impl DurableRevocationBlacklist {
    /// Wraps `inner` (normally the Redis-backed blacklist) with durable journaling
    /// against `pool`. `pool` is only touched when a revocation is journaled or
    /// flushed, never on the hot `is_revoked` path.
    pub fn new(inner: Arc<dyn AccessTokenBlacklist>, pool: PgPool) -> Self {
        Self {
            inner,
            pool,
            pending: Mutex::new(HashMap::new()),
        }
    }

    fn prune_pending(entries: &mut HashMap<Uuid, PendingEntry>, now: Instant) {
        entries.retain(|_, entry| now.duration_since(entry.added_at) < entry.ttl);
    }

    /// Journals `jti` in the outbox. `ON CONFLICT DO NOTHING` keeps the write
    /// idempotent: a token is revoked at most once, so re-journaling the same
    /// `jti` (e.g. a second logout presenting the same token while Redis is
    /// still down) must not replace or duplicate the original row.
    async fn journal(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError> {
        match sqlx::query(
            "INSERT INTO access_token_revocation_outbox (jti, ttl_secs) VALUES ($1, $2)
             ON CONFLICT (jti) DO NOTHING",
        )
        .bind(jti)
        .bind(ttl_secs as i64)
        .execute(&self.pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    jti = %jti,
                    "Access token revocation: Redis failed AND the outbox journal write failed -- revocation is not durable and the token stays valid until its natural expiry"
                );
                Err(BlacklistError)
            }
        }
    }

    /// Retries every journaled revocation against the inner backend and returns
    /// how many still could not be flushed (the current outbox backlog).
    ///
    /// Called on an interval by the flush worker spawned from `main` (first tick
    /// fires immediately, so a backlog that accumulated while the process was
    /// down is retried right at startup). The inner backend's `revoke` is
    /// idempotent, so multiple instances flushing the same rows concurrently is
    /// harmless: a row may be revoked twice, but the entry (and Redis TTL) is the
    /// same either way.
    pub async fn flush_pending(&self) -> Result<usize, sqlx::Error> {
        // Rows whose token lifetime has already elapsed have nothing left to
        // revoke -- the token can't validate anymore -- so drop them without a
        // Redis round-trip. Doing it in SQL avoids decoding the timestamp in
        // Rust (see migration 009).
        sqlx::query(
            "DELETE FROM access_token_revocation_outbox
             WHERE created_at + (ttl_secs * INTERVAL '1 second') <= NOW()",
        )
        .execute(&self.pool)
        .await?;

        let rows: Vec<(Uuid, i64)> = sqlx::query_as(
            "SELECT jti, ttl_secs FROM access_token_revocation_outbox ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;

        // The loop below awaits the inner backend for each row, so the pending
        // set lock is NOT held across it -- an `is_revoked` check during a long
        // sweep must not stall behind a slow Redis.
        let mut flushed: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let mut remaining = 0usize;

        for (jti, ttl_secs) in rows {
            match self.inner.revoke(jti, ttl_secs as u64).await {
                Ok(()) => {
                    flushed.insert(jti);
                }
                Err(BlacklistError) => {
                    // Redis is still down; keep the row so the next sweep retries
                    // it, and keep the in-memory pending entry rejecting it.
                    remaining += 1;
                }
            }
        }

        for jti in &flushed {
            sqlx::query("DELETE FROM access_token_revocation_outbox WHERE jti = $1")
                .bind(jti)
                .execute(&self.pool)
                .await?;
        }

        if !flushed.is_empty() {
            let now = Instant::now();
            let mut pending = self.pending.lock().await;
            pending.retain(|jti, entry| {
                !flushed.contains(jti) && now.duration_since(entry.added_at) < entry.ttl
            });
        }

        Ok(remaining)
    }

    /// Number of journaled revocations this instance still rejects via its
    /// in-memory pending set. Exists so tests can assert on the pending set
    /// without exposing it; the durable backlog for metrics comes from
    /// `flush_pending`'s return value.
    pub async fn pending_count(&self) -> usize {
        let mut pending = self.pending.lock().await;
        Self::prune_pending(&mut pending, Instant::now());
        pending.len()
    }
}

#[async_trait]
impl AccessTokenBlacklist for DurableRevocationBlacklist {
    async fn revoke(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError> {
        if self.inner.revoke(jti, ttl_secs).await.is_ok() {
            return Ok(());
        }

        // The inner backend rejected the revocation (e.g. Redis down). Make it
        // durable instead of dropping it, so the flush worker can finish the job
        // later.
        self.journal(jti, ttl_secs).await?;

        tracing::warn!(
            jti = %jti,
            ttl_secs,
            "Access token revocation: Redis unavailable, journaled in Postgres for durable retry (see #140)"
        );
        self.pending.lock().await.insert(
            jti,
            PendingEntry {
                added_at: Instant::now(),
                ttl: Duration::from_secs(ttl_secs),
            },
        );
        Ok(())
    }

    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
        let now = Instant::now();
        let mut pending = self.pending.lock().await;
        match pending.get(&jti) {
            Some(entry) if now.duration_since(entry.added_at) < entry.ttl => return Ok(true),
            Some(_) => {
                // Expired pending entry: nothing durable to back it anymore,
                // fall through to the inner backend like any other token.
                pending.remove(&jti);
            }
            None => {}
        }
        self.inner.is_revoked(jti).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use shared::ports::{AccessTokenBlacklist, BlacklistError};

    /// Backend that can be told to fail `revoke` (and optionally `is_revoked`),
    /// recording every jti it was asked to revoke so tests can assert on what
    /// the decorator delegated.
    #[derive(Default)]
    struct MockBackend {
        fail_revoke: bool,
        revoked: tokio::sync::Mutex<HashSet<Uuid>>,
        revoke_calls: tokio::sync::Mutex<Vec<Uuid>>,
    }

    #[async_trait::async_trait]
    impl AccessTokenBlacklist for MockBackend {
        async fn revoke(&self, jti: Uuid, _ttl_secs: u64) -> Result<(), BlacklistError> {
            self.revoke_calls.lock().await.push(jti);
            if self.fail_revoke {
                return Err(BlacklistError);
            }
            self.revoked.lock().await.insert(jti);
            Ok(())
        }

        async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
            Ok(self.revoked.lock().await.contains(&jti))
        }
    }

    /// A pool that is never actually connected: every test below only exercises
    /// behavior that does not touch the database (the journal path is covered by
    /// the ignored integration tests, which need a real Postgres). Points at
    /// port 1 -- nothing listens there -- rather than the default `localhost`
    /// Postgres, so an accidental journal attempt fails fast with a connection
    /// error instead of silently writing to whatever dev database happens to be
    /// running (same technique as `rate_limiter_setup.rs`'s unreachable-URL test).
    fn dead_pool() -> PgPool {
        PgPool::connect_lazy("postgres://user:pass@127.0.0.1:1/app_home")
            .expect("dead pool should not need a live connection to be created")
    }

    fn durable(mock: MockBackend) -> DurableRevocationBlacklist {
        DurableRevocationBlacklist::new(Arc::new(mock), dead_pool())
    }

    #[tokio::test]
    async fn revoke_delegates_to_inner_and_does_not_journal_on_success() {
        let mock = MockBackend::default();
        let blacklist = durable(mock);

        blacklist.revoke(Uuid::now_v7(), 900).await.unwrap();
        blacklist.revoke(Uuid::now_v7(), 900).await.unwrap();

        // Journaling only happens when the inner backend fails; if a successful
        // revoke ever tried to journal, the lazy (never-connected) pool would
        // make `revoke` fail instead of returning Ok.
        assert_eq!(
            blacklist.pending_count().await,
            0,
            "successful revokes must not be journaled"
        );
    }

    #[tokio::test]
    async fn is_revoked_delegates_to_inner_when_not_pending() {
        let mock = MockBackend::default();
        let blacklist = durable(mock);
        let jti = Uuid::now_v7();

        assert!(
            !blacklist.is_revoked(jti).await.unwrap(),
            "a token that was never revoked must not be reported as revoked"
        );
    }

    #[tokio::test]
    async fn revoke_returns_err_when_journal_also_fails() {
        // Inner (Redis) fails AND the journal write fails (the pool points at
        // nothing): the revocation has nowhere to be made durable, so `revoke`
        // must surface the error and not pretend otherwise.
        let mock = MockBackend {
            fail_revoke: true,
            ..Default::default()
        };
        let blacklist = DurableRevocationBlacklist::new(Arc::new(mock), dead_pool());
        let jti = Uuid::now_v7();

        assert!(
            blacklist.revoke(jti, 900).await.is_err(),
            "a revocation that is neither in Redis nor journaled must be reported as failed"
        );
        assert_eq!(
            blacklist.pending_count().await,
            0,
            "a failed journal must not add the jti to the pending set"
        );
    }
}
