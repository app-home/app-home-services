// Integration tests for the Redis-backed access token blacklist (issue #88):
// confirms revoked tokens are rejected while the entry lives, that revocation
// doesn't leak between tokens, and that the entry expires with its TTL (the
// token's remaining lifetime).
//
// The last two tests exercise the durable-retry wrapper (issue #140) against a
// real Redis: that a revocation Redis accepts is never journaled, and that the
// flush worker lands a journaled revocation in Redis and clears the outbox.
// Those two additionally require Postgres (DATABASE_URL + migrations, including
// the 009 outbox table).
//
// To run: cargo test --test integration -- --ignored redis_access_token_blacklist
//
// Prerequisites:
// - `podman` (or an aliased `docker`) available on PATH and able to pull/run
//   images without sudo.
//
// Fully self-contained: each test starts its own disposable `redis:7-alpine`
// container on an OS-assigned free port and removes it via Drop.
//
// The two durable-retry tests share the outbox table with the tests in
// `access_token_revocation_outbox_test.rs` and `flush_pending` sweeps the whole
// table, so they hold `crate::integration::outbox_table_guard()` (and clear the
// table) to stay isolated even though the harness runs ignored tests in parallel.

use std::net::TcpListener;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use app_home_services::infrastructure::access_token_blacklist::durable::DurableRevocationBlacklist;
use app_home_services::infrastructure::access_token_blacklist::redis::RedisAccessTokenBlacklist;
use shared::ports::AccessTokenBlacklist;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::sync::OnceCell;
use uuid::Uuid;

static NEXT_CONTAINER_ID: AtomicU32 = AtomicU32::new(0);

// Shared Postgres pool for the durable-revocation tests below -- same rationale
// as `database_test.rs` (opening a PgPool per test is slow and flaky over a
// Podman VM hop).
static POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn get_test_pool() -> &'static PgPool {
    POOL.get_or_init(|| async {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        // `test_before_acquire` + a short `idle_timeout`: connections that the
        // Podman VM quietly drops while idle would otherwise be handed to the
        // next test already-dead, hanging its acquire until the 30s timeout.
        PgPoolOptions::new()
            .test_before_acquire(true)
            .idle_timeout(Duration::from_secs(30))
            .connect(&database_url)
            .await
            .expect("Failed to connect to database")
    })
    .await
}

fn find_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("failed to bind to an ephemeral port to find a free one")
        .local_addr()
        .expect("failed to read the bound ephemeral port")
        .port()
}

struct RedisTestContainer {
    name: String,
    port: u16,
}

impl RedisTestContainer {
    fn start() -> Self {
        let name = format!(
            "apphome-redis-blacklist-test-{}-{}",
            std::process::id(),
            NEXT_CONTAINER_ID.fetch_add(1, Ordering::Relaxed)
        );
        let port = find_free_port();

        let _ = Command::new("podman").args(["rm", "-f", &name]).output();

        let status = Command::new("podman")
            .args([
                "run",
                "-d",
                "--name",
                &name,
                "-p",
                &format!("{port}:6379"),
                "docker.io/redis:7-alpine",
            ])
            .status()
            .expect("failed to run `podman run` -- is podman installed and on PATH?");

        assert!(
            status.success(),
            "`podman run` failed to start the test Redis container"
        );

        let container = Self { name, port };
        container.wait_until_ready();
        container
    }

    fn url(&self) -> String {
        // `localhost`, not `127.0.0.1`: podman's port forwarding on Windows/WSL
        // relays to the IPv6 loopback (::1), and `127.0.0.1` is unreachable there.
        format!("redis://localhost:{}", self.port)
    }

    fn wait_until_ready(&self) {
        let addr = self.url();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);

        loop {
            let ready = std::thread::spawn({
                let addr = addr.clone();
                move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        redis::Client::open(addr.as_str())
                            .ok()?
                            .get_connection_manager()
                            .await
                            .ok()
                    })
                }
            })
            .join()
            .expect("readiness check thread panicked")
            .is_some();

            if ready {
                return;
            }

            assert!(
                std::time::Instant::now() < deadline,
                "test Redis container never became ready within 15s"
            );
            std::thread::sleep(Duration::from_millis(300));
        }
    }
}

impl Drop for RedisTestContainer {
    fn drop(&mut self) {
        let _ = Command::new("podman")
            .args(["rm", "-f", &self.name])
            .output();
    }
}

#[tokio::test]
#[ignore]
async fn not_revoked_by_default() {
    let container = RedisTestContainer::start();
    let blacklist = RedisAccessTokenBlacklist::connect(&container.url())
        .await
        .unwrap();

    assert!(!blacklist.is_revoked(Uuid::now_v7()).await.unwrap());
}

#[tokio::test]
#[ignore]
async fn revoked_token_is_rejected_and_expires_with_ttl() {
    let container = RedisTestContainer::start();
    let blacklist = RedisAccessTokenBlacklist::connect(&container.url())
        .await
        .unwrap();
    let jti = Uuid::now_v7();

    blacklist.revoke(jti, 1).await.unwrap();
    assert!(blacklist.is_revoked(jti).await.unwrap());

    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(!blacklist.is_revoked(jti).await.unwrap());
}

#[tokio::test]
#[ignore]
async fn revocation_does_not_leak_between_tokens() {
    let container = RedisTestContainer::start();
    let blacklist = RedisAccessTokenBlacklist::connect(&container.url())
        .await
        .unwrap();
    let revoked = Uuid::now_v7();
    let other = Uuid::now_v7();

    blacklist.revoke(revoked, 900).await.unwrap();
    assert!(blacklist.is_revoked(revoked).await.unwrap());
    assert!(!blacklist.is_revoked(other).await.unwrap());
}

#[test]
#[ignore]
fn durable_revocation_fast_path_does_not_journal() {
    crate::integration::test_runtime().block_on(async {
        let container = RedisTestContainer::start();
        let pool = get_test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let redis = RedisAccessTokenBlacklist::connect(&container.url())
            .await
            .unwrap();
        let durable = DurableRevocationBlacklist::new(Arc::new(redis), pool.clone());
        let jti = Uuid::now_v7();

        durable.revoke(jti, 900).await.unwrap();
        assert!(durable.is_revoked(jti).await.unwrap());

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM access_token_revocation_outbox WHERE jti = $1",
        )
        .bind(jti)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            count, 0,
            "a revocation Redis accepted must not be journaled (durable retry is only for failures)"
        );
    })
}

#[test]
#[ignore]
fn flush_pending_lands_journaled_revocation_in_redis() {
    crate::integration::test_runtime().block_on(async {
        let container = RedisTestContainer::start();
        let pool = get_test_pool().await;
        let _table_guard = crate::integration::outbox_table_guard().await;
        crate::integration::clean_outbox(pool).await;
        let redis = RedisAccessTokenBlacklist::connect(&container.url())
            .await
            .unwrap();
        let durable = DurableRevocationBlacklist::new(Arc::new(redis.clone()), pool.clone());
        let jti = Uuid::now_v7();

        // Simulate a revocation that was journaled while Redis was down (e.g. by
        // another instance, or before this process restarted), without needing to
        // take this test's own Redis down: write the outbox row directly.
        sqlx::query("INSERT INTO access_token_revocation_outbox (jti, ttl_secs) VALUES ($1, 900)")
            .bind(jti)
            .execute(pool)
            .await
            .unwrap();

        let remaining = durable.flush_pending().await.unwrap();
        assert_eq!(
            remaining, 0,
            "the journaled revocation must flush on the first sweep"
        );

        assert!(
            redis.is_revoked(jti).await.unwrap(),
            "the flush worker must have landed the revocation in Redis"
        );

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM access_token_revocation_outbox WHERE jti = $1",
        )
        .bind(jti)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "the flushed row must be deleted from the outbox");
    })
}
