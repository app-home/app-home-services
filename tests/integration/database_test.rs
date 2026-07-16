// Database integration tests.
// These tests require a running PostgreSQL database.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL environment variable
// - Run migrations first: cargo run

use sqlx::PgPool;
use tokio::sync::OnceCell;

// A single, shared pool for every test in this file, instead of opening a brand new
// PgPool::connect() per test. Each test previously paid the full cost (and risk of
// PoolTimedOut under any transient network/Postgres latency, e.g. over the extra hop
// a Podman VM on Windows adds) of establishing a new pool; sharing one avoids that
// connection churn entirely. tokio::sync::OnceCell makes this safe to initialize
// exactly once even if tests run concurrently.
static POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn get_test_pool() -> &'static PgPool {
    POOL.get_or_init(|| async {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to database")
    })
    .await
}

#[tokio::test]
#[ignore]
async fn test_users_table_exists() {
    let pool = get_test_pool().await;

    let result: Result<(i64,), sqlx::Error> = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await;

    assert!(result.is_ok(), "users table should exist");
}

#[tokio::test]
#[ignore]
async fn test_user_actions_table_exists() {
    let pool = get_test_pool().await;

    let result: Result<(i64,), sqlx::Error> = sqlx::query_as("SELECT COUNT(*) FROM user_actions")
        .fetch_one(pool)
        .await;

    assert!(result.is_ok(), "user_actions table should exist");
}

#[tokio::test]
#[ignore]
async fn test_seed_user_exists() {
    let pool = get_test_pool().await;

    let user: Result<(String, String), sqlx::Error> =
        sqlx::query_as("SELECT username, email FROM users WHERE auth_provider = 'local' LIMIT 1")
            .fetch_one(pool)
            .await;

    assert!(user.is_ok(), "A seeded local user should exist");
    let (username, email) = user.unwrap();
    assert!(!username.is_empty(), "Username should not be empty");
    assert!(!email.is_empty(), "Email should not be empty");
}

#[tokio::test]
#[ignore]
async fn test_user_actions_foreign_key() {
    let pool = get_test_pool().await;

    // Verify the foreign key constraint exists. Note this asserts on the actual
    // count, not just that the query executed successfully: a COUNT(*) query always
    // returns Ok with a single row even when the count is 0, so checking only
    // `result.is_ok()` (as a previous version of this test did) could never actually
    // detect a missing foreign key -- only a connection-level failure.
    let result: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.table_constraints
         WHERE constraint_type = 'FOREIGN KEY'
         AND table_name = 'user_actions'
         AND constraint_name LIKE '%user_id%'",
    )
    .fetch_one(pool)
    .await
    .expect("query should execute successfully");

    assert!(
        result.0 > 0,
        "user_actions should have a FOREIGN KEY constraint referencing user_id, found {}",
        result.0
    );
}
