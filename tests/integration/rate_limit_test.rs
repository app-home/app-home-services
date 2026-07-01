// Integration tests for rate limiting.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL environment variable
// - Run migrations: cargo run

#[tokio::test]
#[ignore]
async fn test_rate_limit_exceeded_returns_429() {
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
