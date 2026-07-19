// Integration tests for rate limiting on /api/auth/refresh.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored

use std::net::{IpAddr, Ipv4Addr};

use app_home_services::application::ports::rate_limiter::RateLimiter;
use auth::adapters::redis_rate_limiter::RedisRateLimiter;

/// Deletes the rate-limit counters for 127.0.0.1 in both the login and refresh
/// Redis namespaces so tests are not blocked by state from a previous test.
async fn reset_rate_limiters() {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    for prefix in ["login", "refresh"] {
        if let Ok(limiter) = RedisRateLimiter::connect(&redis_url, 10, 300, prefix).await {
            limiter.reset(ip).await;
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_refresh_rate_limit_exceeded_returns_429() {
    reset_rate_limiters().await;
    let client = reqwest::Client::new();
    let max_attempts: u32 = std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    // Send max_attempts invalid refresh requests.
    for _ in 0..max_attempts {
        let resp = client
            .post("http://localhost:3000/api/auth/refresh")
            .json(&serde_json::json!({"refresh_token": "not-a-real-refresh-token"}))
            .send()
            .await
            .expect("Failed to send request");

        // Early requests should be rejected as unauthorized, not yet rate limited.
        assert!(resp.status() == 401 || resp.status() == 429);
    }

    // The next request should be rate limited.
    let resp = client
        .post("http://localhost:3000/api/auth/refresh")
        .json(&serde_json::json!({"refresh_token": "not-a-real-refresh-token"}))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 429);
}

#[tokio::test]
#[ignore]
async fn test_refresh_rate_limit_is_independent_from_login_rate_limit() {
    reset_rate_limiters().await;
    // Exhausting the refresh endpoint's rate limit must not affect the login
    // endpoint's rate limit for the same IP, and vice versa -- they're tracked with
    // separate counters (see AppState::rate_limiter vs AppState::refresh_rate_limiter).
    let client = reqwest::Client::new();
    let max_attempts: u32 = std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    for _ in 0..=max_attempts {
        let _ = client
            .post("http://localhost:3000/api/auth/refresh")
            .json(&serde_json::json!({"refresh_token": "not-a-real-refresh-token"}))
            .send()
            .await
            .expect("Failed to send request");
    }

    // The refresh endpoint should now be rate limited...
    let refresh_resp = client
        .post("http://localhost:3000/api/auth/refresh")
        .json(&serde_json::json!({"refresh_token": "not-a-real-refresh-token"}))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(refresh_resp.status(), 429);

    // ...but a login attempt from the same client/IP should still be evaluated
    // normally (401 for bad credentials, not 429).
    let login_resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": "nonexistent_user",
            "password": "wrong_password"
        }))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(login_resp.status(), 401);
}
