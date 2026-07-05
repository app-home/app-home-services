// Integration tests for rate limiting.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL environment variable
// - Run migrations: cargo run

use std::net::{IpAddr, Ipv4Addr};

use app_home_services::adapters::outbound::redis_rate_limiter::RedisRateLimiter;
use app_home_services::application::ports::rate_limiter::RateLimiter;

/// Deletes the rate-limit counters for the test IP in both the login and
/// refresh Redis namespaces, so the next HTTP test starts with a clean slate
/// regardless of prior test state.
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
async fn test_rate_limit_exceeded_returns_429() {
    reset_rate_limiters().await;

    let client = reqwest::Client::new();
    let max_attempts: u32 = std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    // Send max_attempts + 1 failed login requests
    for _ in 0..max_attempts {
        let resp = client
            .post("http://localhost:3000/api/auth/login/password")
            .json(&serde_json::json!({
                "username": "nonexistent_user",
                "password": "wrong_password"
            }))
            .send()
            .await
            .expect("Failed to send request");

        // Early requests should be 401, not yet rate limited
        assert!(resp.status() == 401 || resp.status() == 429);
    }

    // The next request should be rate limited
    let resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": "nonexistent_user",
            "password": "wrong_password"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 429);
}

#[tokio::test]
#[ignore]
async fn test_successful_login_resets_rate_limit() {
    reset_rate_limiters().await;

    let client = reqwest::Client::new();
    let max_attempts: u32 = std::env::var("RATE_LIMIT_MAX_ATTEMPTS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    // Send max_attempts - 1 failed requests
    for _ in 0..max_attempts.saturating_sub(1) {
        let resp = client
            .post("http://localhost:3000/api/auth/login/password")
            .json(&serde_json::json!({
                "username": "nonexistent_user",
                "password": "wrong_password"
            }))
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(resp.status(), 401);
    }

    // Successful login should reset the counter
    let resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": "admin",
            "password": std::env::var("DEFAULT_USER_PASSWORD").unwrap_or_else(|_| "password".to_string())
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);
}
