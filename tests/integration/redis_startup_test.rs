// Integration test for issue #19 (item 5): startup must fail fast when REDIS_URL is
// set but Redis is unreachable, mirroring startup_test.rs's existing pattern for an
// unreachable database.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - A reachable PostgreSQL database at DATABASE_URL (unlike startup_test.rs, this
//   test needs the DB step to succeed so startup actually reaches the Redis
//   connection step it's meant to exercise)
// - Migrations already applied (cargo run once against that database is enough)

use std::process::Command;

#[tokio::test]
#[ignore]
async fn test_startup_fails_on_unreachable_redis() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must point to a reachable, migrated Postgres for this test");

    let output = Command::new("cargo")
        .args(["run", "--quiet"])
        .env("DATABASE_URL", database_url)
        .env("JWT_SECRET", "test-secret-that-is-at-least-32-bytes-long")
        .env("DEFAULT_USER_PASSWORD", "irrelevant-for-this-test")
        // Port 1 is a privileged port very unlikely to have anything listening on it,
        // so the connection attempt fails fast rather than needing a timeout to elapse.
        .env("REDIS_URL", "redis://127.0.0.1:1")
        .output()
        .expect("Failed to run cargo");

    assert!(
        !output.status.success(),
        "Process should exit with error when REDIS_URL is set but unreachable"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to set up rate limiters"),
        "Should log a rate limiter setup failure message, got: {stderr}"
    );
}
