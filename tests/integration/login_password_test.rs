// Integration tests for the password login endpoint.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL environment variable
// - Run migrations: cargo run

#[tokio::test]
#[ignore]
async fn test_valid_login_returns_tokens() {
    let client = reqwest::Client::new();
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
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "authenticated");
    assert!(body["user_id"].is_string());
    assert!(body["access_token"].is_string());
    assert!(body["refresh_token"].is_string());
}

#[tokio::test]
#[ignore]
async fn test_wrong_password_returns_401() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": "admin",
            "password": "wrongpassword"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "Invalid username or password");
}

#[tokio::test]
#[ignore]
async fn test_nonexistent_username_returns_401() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": "nonexistent",
            "password": "anything"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "Invalid username or password");
}

#[tokio::test]
#[ignore]
async fn test_missing_fields_returns_422() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/login/password")
        .json(&serde_json::json!({
            "username": ""
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 422);
}
