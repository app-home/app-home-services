use std::sync::OnceLock;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, OnceCell};

// Tests that read or write the shared `access_token_revocation_outbox` table are
// serialized through this lock: `DurableRevocationBlacklist::flush_pending` sweeps
// the WHOLE table, so a parallel test would flush (or get caught in) another
// test's rows -- and the test harness runs ignored tests in parallel by default.
// Every outbox test in this binary holds it, regardless of which file it lives in.
static OUTBOX_TABLE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

static TEST_RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// A single tokio runtime shared by every DB test in this binary.
///
/// `#[tokio::test]` creates a fresh runtime per test and drops it when the test
/// ends; a sqlx `PgPool` created inside one of those short-lived runtimes can
/// stop handing out connections once that runtime is gone (its background reaper
/// task dies), which shows up as intermittent `PoolTimedOut`. Running the tests
/// on one long-lived runtime keeps the shared `OnceCell<PgPool>` pools (see
/// `database_test.rs` and the outbox/redis blacklist test files) on the same
/// runtime that created them.
pub(crate) fn test_runtime() -> &'static Runtime {
    TEST_RUNTIME.get_or_init(|| Runtime::new().expect("failed to build the shared test runtime"))
}

/// Acquires the outbox-table serialization lock. Returns a guard that must live
/// for the rest of the test (see `OUTBOX_TABLE_LOCK` for why).
pub(crate) async fn outbox_table_guard() -> tokio::sync::MutexGuard<'static, ()> {
    OUTBOX_TABLE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await
}

/// Empties the outbox table so a test starts against a clean slate even when a
/// previous (possibly failing) test left rows behind.
pub(crate) async fn clean_outbox(pool: &sqlx::PgPool) {
    sqlx::query("DELETE FROM access_token_revocation_outbox")
        .execute(pool)
        .await
        .expect("clearing the outbox table for the test should succeed");
}

static POOL: OnceCell<PgPool> = OnceCell::const_new();

/// Shared Postgres pool for every DB-backed integration test in this binary
/// (durable-revocation outbox tests, Redis blacklist durable-retry tests, etc.)
/// -- opening a brand new `PgPool` per test is slow and flaky over the extra hop
/// a Podman VM on Windows adds, so every test file borrows this one instead of
/// keeping its own copy of the same `OnceCell`/`PgPoolOptions` setup.
pub(crate) async fn test_pool() -> &'static PgPool {
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

mod access_token_revocation_outbox_test;
mod cors_test;
mod database_test;
mod health_test;
mod login_google_test;
mod login_password_test;
mod logout_test;
mod metrics_test;
mod migration_recovery_test;
mod rate_limit_test;
mod redis_access_token_blacklist_test;
mod redis_auth_test;
mod redis_connection_failure_test;
mod redis_rate_limit_test;
mod redis_startup_test;
mod refresh_rate_limit_test;
mod refresh_test;
mod security_headers_test;
mod startup_test;
