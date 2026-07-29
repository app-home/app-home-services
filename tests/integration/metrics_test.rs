// Integration test for the /metrics endpoint after adding the IP allowlist guard
// (#83). Only covers that /metrics stays reachable by default -- a regression guard
// against the new middleware accidentally locking out the default (no
// METRICS_ALLOWED_IPS configured) case.
//
// The "blocked" path (a non-allowlisted, non-loopback IP getting 403) is NOT
// covered here: this test suite's server is always reached via 127.0.0.1, and
// loopback is deliberately always allowed regardless of METRICS_ALLOWED_IPS (see
// crates/infrastructure/src/metrics_guard.rs's docs for why). That decision logic
// is covered directly by unit tests in that same file instead
// (is_metrics_access_allowed), which don't need a real HTTP server or a non-loopback
// peer address to exercise.
//
// To run: cargo test --test integration -- --ignored metrics

#[tokio::test]
#[ignore]
async fn test_metrics_is_reachable_with_default_configuration() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/metrics")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(
        resp.status(),
        200,
        "/metrics should be reachable from loopback with no METRICS_ALLOWED_IPS configured"
    );
}
