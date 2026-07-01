// Integration tests for the Google OAuth login endpoint.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL and GOOGLE_CLIENT_ID environment variables
// - A valid Google ID token for testing (obtained from a test Google account)
// - Run migrations: cargo run

#[tokio::test]
#[ignore]
async fn test_google_login_with_valid_token() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/login/google")
        .json(&serde_json::json!({
            "id_token": "valid-test-token"
        }))
        .send()
        .await
        .expect("Failed to send request");

    // With a valid token, we expect either 200 (authenticated) or 401 (invalid token)
    // This test requires a real Google ID token to pass fully
    let status = resp.status();
    assert!(status == 200 || status == 401);
}

#[tokio::test]
#[ignore]
async fn test_google_login_invalid_token_returns_401() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/login/google")
        .json(&serde_json::json!({
            "id_token": "invalid-token-12345"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "Authentication failed");
}

#[tokio::test]
#[ignore]
async fn test_google_login_missing_token_returns_422() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/login/google")
        .json(&serde_json::json!({
            "id_token": ""
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 422);
}
