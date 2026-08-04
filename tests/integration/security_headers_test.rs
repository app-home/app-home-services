// Integration tests for HTTP security headers (see #90).
// These tests require the server to be started.
//
// To run: cargo test --test integration -- --ignored security_headers
//
// Prerequisites:
// - Set DATABASE_URL and JWT_SECRET environment variables
// - Run the server: cargo run

#[tokio::test]
#[ignore]
async fn test_security_headers_present_on_api_responses() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/api/health")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let headers = resp.headers();
    assert_eq!(
        headers.get("strict-transport-security"),
        Some(&reqwest::header::HeaderValue::from_static(
            "max-age=31536000; includeSubDomains"
        )),
        "HSTS header should be present with the configured value"
    );
    assert_eq!(
        headers.get("x-content-type-options"),
        Some(&reqwest::header::HeaderValue::from_static("nosniff")),
        "X-Content-Type-Options header should be present"
    );
    assert_eq!(
        headers.get("x-frame-options"),
        Some(&reqwest::header::HeaderValue::from_static("DENY")),
        "X-Frame-Options header should be present"
    );
    assert_eq!(
        headers.get("referrer-policy"),
        Some(&reqwest::header::HeaderValue::from_static(
            "strict-origin-when-cross-origin"
        )),
        "Referrer-Policy header should be present"
    );
}
