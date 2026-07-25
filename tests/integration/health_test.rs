// Integration test for the DB-backed /api/health endpoint (see src/health.rs).
//
// To run: cargo test --test integration -- --ignored health
//
// Only the happy path is covered here (a healthy DB -> 200). Testing the unhealthy
// path (DB unreachable -> 503) meaningfully would require killing the shared
// Postgres this test suite's server is already connected to mid-run, which would
// break every other integration test sharing that same live server -- unlike
// redis_connection_failure_test.rs, which can safely spin up and kill its own
// disposable Redis container without affecting anything else. If that coverage is
// wanted later, it would need its own dedicated server instance (its own DB
// connection, started and torn down just for that test), similar in spirit to
// tests/integration/redis_startup_test.rs's subprocess approach but for a live
// HTTP call instead of a startup failure.

#[tokio::test]
#[ignore]
async fn test_health_returns_200_when_database_is_reachable() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/api/health")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.expect("Response was not valid JSON");
    assert_eq!(body["status"], "ok");
}
