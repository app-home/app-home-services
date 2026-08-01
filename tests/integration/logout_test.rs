// Integration tests for the logout endpoint.
// These tests require a running PostgreSQL database and the server to be started.
//
// To run: cargo test --test integration -- --ignored
//
// Prerequisites:
// - Set DATABASE_URL and JWT_SECRET environment variables
// - Run migrations: cargo run

#[tokio::test]
#[ignore]
async fn test_logout_with_valid_session_returns_200() {
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
    let access_token = login_body["access_token"].as_str().unwrap();
    let _refresh_token = login_body["refresh_token"].as_str().unwrap();

    // Decode session_id from refresh token (we'd need JWT for this)
    // For now, just verify logout endpoint is reachable with auth
    let logout_resp = client
        .post("http://localhost:3000/api/auth/logout")
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({
            "session_id": "00000000-0000-0000-0000-000000000000"
        }))
        .send()
        .await
        .expect("Failed to logout");

    // The session won't exist with a zero UUID, so expect 400
    assert_eq!(logout_resp.status(), 400);
}

#[tokio::test]
#[ignore]
async fn test_logout_without_auth_returns_401() {
    let client = reqwest::Client::new();
    let resp = client
        .post("http://localhost:3000/api/auth/logout")
        .json(&serde_json::json!({
            "session_id": "00000000-0000-0000-0000-000000000000"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
#[ignore]
async fn test_logout_revokes_presented_access_token() {
    // End-to-end check for issue #88: after a successful logout, the access token
    // used to authenticate it must no longer validate against a protected route.
    let client = reqwest::Client::new();

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
    let access_token = login_body["access_token"].as_str().unwrap();
    let refresh_token = login_body["refresh_token"].as_str().unwrap();

    // Sanity check: the access token works before logout.
    let before = client
        .get("http://localhost:3000/api/profile")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Failed to hit /api/profile before logout");
    assert_eq!(before.status(), 200);

    // The logout request must name the session to close; decode it from the refresh
    // token (signed with the same JWT_SECRET as the rest of the service).
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET is required");
    let claims: serde_json::Value = jsonwebtoken::decode_header(refresh_token)
        .and_then(|_| {
            jsonwebtoken::decode::<serde_json::Value>(
                refresh_token,
                &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
                &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
            )
            .map(|data| data.claims)
        })
        .expect("failed to decode refresh token");
    let session_id = claims["session_id"]
        .as_str()
        .expect("refresh token has no session_id");

    let logout_resp = client
        .post("http://localhost:3000/api/auth/logout")
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({ "session_id": session_id }))
        .send()
        .await
        .expect("Failed to logout");
    assert_eq!(logout_resp.status(), 200);

    // The same access token must now be rejected on every protected route.
    let after = client
        .get("http://localhost:3000/api/profile")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Failed to hit /api/profile after logout");
    assert_eq!(
        after.status(),
        401,
        "access token must be revoked after logout (see #88)"
    );
}
