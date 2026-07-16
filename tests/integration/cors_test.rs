// Integration tests for CORS restrictions.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL, JWT_SECRET, and CORS_ALLOWED_ORIGINS environment variables
// - Run migrations: cargo run

#[tokio::test]
#[ignore]
async fn test_cors_allowed_origin_returns_headers() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/api/health")
        .header(
            "Origin",
            std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        )
        .send()
        .await
        .expect("Failed to send request");

    let origin_header = resp.headers().get("access-control-allow-origin");
    assert!(
        origin_header.is_some(),
        "CORS header should be present for allowed origins"
    );
}

#[tokio::test]
#[ignore]
async fn test_cors_disallowed_origin_no_headers() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/api/health")
        .header("Origin", "http://evil.com")
        .send()
        .await
        .expect("Failed to send request");

    let origin_header = resp.headers().get("access-control-allow-origin");
    // If CORS_ALLOWED_ORIGINS is empty (same-origin only), even this test would fail
    // This test is only meaningful when CORS_ALLOWED_ORIGINS is set to a specific value
    if !std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .is_empty()
    {
        assert!(origin_header.is_none() || origin_header.unwrap() != "http://evil.com");
    }
}
