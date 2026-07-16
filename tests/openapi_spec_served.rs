// Integration test for the OpenAPI specification endpoint.
// Requires a running server (cargo run) on localhost:3000.
// Run with: cargo test --test openapi_spec_served -- --ignored

#[tokio::test]
#[ignore]
async fn spec_returns_200_with_valid_structure() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/api-docs/openapi.json")
        .send()
        .await
        .expect("Failed to fetch spec");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("openapi").is_some(), "Missing openapi version");
    assert!(body.get("info").is_some(), "Missing info section");
    assert!(body.get("paths").is_some(), "Missing paths section");
    assert!(
        body.get("components").is_some(),
        "Missing components section"
    );
}

#[tokio::test]
#[ignore]
async fn spec_contains_expected_paths_and_security() {
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:3000/api-docs/openapi.json")
        .send()
        .await
        .expect("Failed to fetch spec");

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let paths = body["paths"].as_object().unwrap();

    assert!(
        paths.contains_key("/api/auth/login/password"),
        "Missing login-password path"
    );
    assert!(
        paths.contains_key("/api/auth/login/google"),
        "Missing login-google path"
    );
    assert!(
        paths.contains_key("/api/auth/logout"),
        "Missing logout path"
    );
    assert!(
        paths.contains_key("/api/auth/refresh"),
        "Missing refresh path"
    );
    assert!(paths.contains_key("/api/health"), "Missing health path");
    assert!(
        !paths.contains_key("/metrics"),
        "/metrics should be excluded"
    );

    let components = body["components"].as_object().unwrap();
    let security_schemes = components["securitySchemes"]
        .as_object()
        .expect("Missing securitySchemes");
    assert!(
        security_schemes.contains_key("bearer_jwt"),
        "Missing bearer_jwt security scheme"
    );

    let scheme = &security_schemes["bearer_jwt"];
    assert_eq!(scheme["type"], "http");
    assert_eq!(scheme["scheme"], "bearer");
    assert_eq!(scheme["bearerFormat"], "JWT");
}
