// Integration tests for the token refresh endpoint.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL and JWT_SECRET environment variables
// - Run migrations: cargo run

#[tokio::test]
#[ignore]
async fn test_refresh_with_valid_token_returns_new_tokens() {
    let client = reqwest::Client::new();

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

#[tokio::test]
#[ignore]
async fn test_refresh_empty_token_returns_422() {
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
