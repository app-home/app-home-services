// Integration tests for the durable access token revocation outbox (issue #140):
// exercises the journaling/flush behavior of `DurableRevocationBlacklist` against
// a real Postgres. The inner "Redis" backend is a mock that can be switched
// between failing and succeeding, so these tests don't need a real Redis -- the
// full loop with a real Redis is covered by the podman-based tests in
// `redis_access_token_blacklist_test.rs`.
//
// To run: cargo test --test integration -- --ignored access_token_revocation_outbox
//
// Prerequisites:
// - Set DATABASE_URL environment variable
// - Run migrations first (the 009 migration creates the outbox table):
//   cargo run
//
// All tests here share the outbox table and `flush_pending` sweeps the whole
// table, so they hold the `crate::integration::outbox_table_guard()` lock (and
// clear the table) to stay isolated from each other and from the Redis E2E tests
// even though the harness runs ignored tests in parallel by default.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use app_home_services::infrastructure::access_token_blacklist::durable::DurableRevocationBlacklist;
use shared::ports::{AccessTokenBlacklist, BlacklistError};
use sqlx::PgPool;
use uuid::Uuid;

async fn outbox_count(pool: &PgPool, jti: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM access_token_revocation_outbox WHERE jti = $1")
        .bind(jti)
        .fetch_one(pool)
        .await
        .expect("outbox count query should execute successfully")
}

/// Mock revocation backend that can be told to fail `revoke`, recording every jti
/// it was asked to revoke (so tests can assert whether the flush worker actually
/// delegated) and every jti it accepted (so tests can assert the revocation
/// landed). Shared behind `Arc` so a test can hold its own handle after handing
/// the backend to `DurableRevocationBlacklist`.
struct MockBackend {
    fail_revoke: AtomicBool,
    revoked: tokio::sync::Mutex<HashSet<Uuid>>,
    revoke_calls: tokio::sync::Mutex<Vec<Uuid>>,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self {
            fail_revoke: AtomicBool::new(false),
            revoked: tokio::sync::Mutex::new(HashSet::new()),
            revoke_calls: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

impl MockBackend {
    fn set_failing(&self, failing: bool) {
        self.fail_revoke.store(failing, Ordering::Relaxed);
    }

    async fn was_revoked(&self, jti: Uuid) -> bool {
        self.revoked.lock().await.contains(&jti)
    }

    async fn revoke_call_count(&self, jti: Uuid) -> usize {
        self.revoke_calls
            .lock()
            .await
            .iter()
            .filter(|&&j| j == jti)
            .count()
    }
}

#[async_trait::async_trait]
impl AccessTokenBlacklist for MockBackend {
    async fn revoke(&self, jti: Uuid, _ttl_secs: u64) -> Result<(), BlacklistError> {
        self.revoke_calls.lock().await.push(jti);
        if self.fail_revoke.load(Ordering::Relaxed) {
            return Err(BlacklistError);
        }
        self.revoked.lock().await.insert(jti);
        Ok(())
    }

    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
        Ok(self.revoked.lock().await.contains(&jti))
    }
}

#[test]
#[ignore]
fn revoke_journals_when_inner_fails() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let mock = Arc::new(MockBackend::default());
        mock.set_failing(true);
        let durable = DurableRevocationBlacklist::new(mock, pool.clone());
        let jti = Uuid::now_v7();

        durable
            .revoke(jti, 900)
            .await
            .expect("journaled revocation must be reported as successful");

        assert_eq!(
            outbox_count(pool, jti).await,
            1,
            "a revocation Redis rejected must be journaled in the outbox"
        );
        assert_eq!(
            durable.pending_count().await,
            1,
            "the journaled jti must be tracked in the in-memory pending set"
        );
    })
}

#[test]
#[ignore]
fn is_revoked_rejects_journaled_jti_immediately() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let mock = Arc::new(MockBackend::default());
        mock.set_failing(true);
        let durable = DurableRevocationBlacklist::new(mock, pool.clone());
        let jti = Uuid::now_v7();

        durable.revoke(jti, 900).await.unwrap();

        assert!(
            durable.is_revoked(jti).await.unwrap(),
            "a journaled-but-unflushed token must be rejected by this instance immediately"
        );
    })
}

#[test]
#[ignore]
fn flush_clears_row_once_inner_recovers() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let mock = Arc::new(MockBackend::default());
        mock.set_failing(true);
        let inner: Arc<dyn AccessTokenBlacklist> = mock.clone();
        let durable = DurableRevocationBlacklist::new(inner, pool.clone());
        let jti = Uuid::now_v7();

        durable.revoke(jti, 900).await.unwrap();

        // Redis is "back": the next revoke succeeds.
        mock.set_failing(false);

        let remaining = durable.flush_pending().await.expect("flush should succeed");

        assert_eq!(
            remaining, 0,
            "the journaled revocation must flush once Redis recovers"
        );
        assert_eq!(
            outbox_count(pool, jti).await,
            0,
            "the flushed row must be deleted from the outbox"
        );
        assert!(
            mock.was_revoked(jti).await,
            "the flush worker must have delegated the revoke to the (recovered) backend"
        );
    })
}

#[test]
#[ignore]
fn flush_keeps_row_while_inner_still_down() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let mock = Arc::new(MockBackend::default());
        mock.set_failing(true);
        let inner: Arc<dyn AccessTokenBlacklist> = mock.clone();
        let durable = DurableRevocationBlacklist::new(inner, pool.clone());
        let jti = Uuid::now_v7();

        durable.revoke(jti, 900).await.unwrap();

        let remaining = durable.flush_pending().await.expect("flush should succeed");

        assert_eq!(
            remaining, 1,
            "a revocation Redis still rejects must stay in the backlog"
        );
        assert_eq!(
            outbox_count(pool, jti).await,
            1,
            "the failed-to-flush row must remain journaled for the next sweep"
        );
        assert!(
            durable.is_revoked(jti).await.unwrap(),
            "a token still in the backlog must keep being rejected by this instance"
        );
    })
}

#[test]
#[ignore]
fn flush_drops_expired_row_without_revoking_inner() {
    crate::integration::test_runtime().block_on(async {
        let pool = crate::integration::test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let mock = Arc::new(MockBackend::default());
        mock.set_failing(true);
        let inner: Arc<dyn AccessTokenBlacklist> = mock.clone();
        let durable = DurableRevocationBlacklist::new(inner, pool.clone());
        let jti = Uuid::now_v7();

        // ttl 0 means the token was already expired at revocation time: there is
        // nothing left to revoke, so the flush worker must drop the row without
        // ever calling the backend.
        durable.revoke(jti, 0).await.unwrap();

        let remaining = durable.flush_pending().await.expect("flush should succeed");

        assert_eq!(remaining, 0);
        assert_eq!(
            outbox_count(pool, jti).await,
            0,
            "the expired row must be deleted"
        );
        assert_eq!(
            mock.revoke_call_count(jti).await,
            1,
            "an expired revocation must not trigger an extra backend round-trip during flush (the only call is the initial revoke)"
        );
    })
}
