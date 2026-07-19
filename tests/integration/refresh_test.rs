// Integration tests for the token refresh endpoint.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL and JWT_SECRET environment variables
// - Run migrations: cargo run

use std::net::{IpAddr, Ipv4Addr};

use app_home_services::shared::ports::RateLimiter;
use app_home_services::infrastructure::rate_limiter::redis::RedisRateLimiter;

/// Deletes the rate-limit counters for 127.0.0.1 in both the login and refresh
/// Redis namespaces so this test's calls are not blocked by state from a previous test.
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
async fn test_refresh_with_valid_token_returns_new_tokens() {
    let client = reqwest::Client::new();
    reset_rate_limiters().await;

    // First login to get tokens
    let login_resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": "admin",
            "password": std::env::var("DEFAULT_USER_PASSWORD").unwrap_or_else(|_| "password".to_string())
        }))
        .send()
        .await
        .expect("Failed to login");
    assert_eq!(login_resp.status(), 200);

    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    let refresh_token = login_body["refresh_token"].as_str().unwrap();

    // Refresh the token
    let refresh_resp = client
        .post("http://localhost:3000/api/auth/refresh")
        .json(&serde_json::json!({
            "refresh_token": refresh_token
        }))
        .send()
        .await
        .expect("Failed to refresh token");

    assert_eq!(refresh_resp.status(), 200);
    let refresh_body: serde_json::Value = refresh_resp.json().await.unwrap();
    assert!(refresh_body["access_token"].is_string());
    assert!(refresh_body["refresh_token"].is_string());
}

#[tokio::test]
#[ignore]
async fn test_refresh_with_invalid_token_returns_401() {
    reset_rate_limiters().await;
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/refresh")
        .json(&serde_json::json!({
            "refresh_token": "invalid-refresh-token"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 401);
}

/// Regression test for issue #15: a refresh token that is well-formed and validly
/// signed, but whose session_id doesn't exist in the database (AuthError::SessionNotFound
/// deep in the use case), must be reported to the client as 401 Unauthorized -- not
/// fall through to the generic 500 Internal Server Error catch-all.
///
/// This crafts a token directly with JwtServiceImpl (using the same JWT_SECRET the
/// running server is configured with) for a random session_id that was never
/// created, rather than reusing test_refresh_with_invalid_token_returns_401's
/// garbage string, since that one already gets rejected earlier as a signature/parse
/// failure (AuthError::TokenVerificationFailed) and doesn't exercise this code path.
#[tokio::test]
#[ignore]
async fn test_refresh_with_nonexistent_session_returns_401_not_500() {
    reset_rate_limiters().await;

    use app_home_services::application::ports::jwt_service::JwtService;
    use auth::adapters::jwt_service::JwtServiceImpl;
    use uuid::Uuid;

    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set to the same value the running server uses");
    let jwt_service = JwtServiceImpl::new(&jwt_secret, 15, 7);

    // Neither of these IDs was ever persisted, so the session lookup inside
    // refresh_token() will come back None -> AuthError::SessionNotFound.
    let never_created_user_id = Uuid::now_v7();
    let never_created_session_id = Uuid::now_v7();
    let token_pair = jwt_service
        .generate_token_pair(never_created_user_id, never_created_session_id)
        .expect("failed to generate a token pair for the test");

    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/refresh")
        .json(&serde_json::json!({
            "refresh_token": token_pair.refresh_token
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(
        resp.status(),
        401,
        "a refresh token for a nonexistent session must be reported as 401, not 500"
    );
}

#[tokio::test]
#[ignore]
async fn test_refresh_empty_token_returns_422() {
    reset_rate_limiters().await;
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/refresh")
        .json(&serde_json::json!({
            "refresh_token": ""
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 422);
}
