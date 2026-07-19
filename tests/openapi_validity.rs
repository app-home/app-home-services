use utoipa::OpenApi;

use app_home_services::api_doc::ApiDoc;

fn spec_json() -> serde_json::Value {
    let spec = ApiDoc::openapi();
    serde_json::to_value(&spec).expect("Failed to serialize spec to JSON")
}

#[test]
fn openapi_passes_structural_validation() {
    let json = spec_json();

    assert!(
        json.get("openapi").and_then(|v| v.as_str()).is_some(),
        "Missing or invalid openapi version"
    );
    assert!(
        json.get("info").and_then(|v| v.as_object()).is_some(),
        "Missing info section"
    );
    let paths = json["paths"].as_object().expect("Missing paths section");
    assert!(!paths.is_empty(), "No paths in spec");

    let components = json["components"]
        .as_object()
        .expect("Missing components section");
    assert!(
        components
            .get("schemas")
            .and_then(|v| v.as_object())
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "No schemas in components"
    );
}

#[test]
fn openapi_has_bearer_jwt_security_scheme() {
    let json = spec_json();
    let components = json["components"].as_object().expect("Missing components");
    let security_schemes = components["securitySchemes"]
        .as_object()
        .expect("Missing securitySchemes");
    let scheme = security_schemes
        .get("bearer_jwt")
        .expect("Missing bearer_jwt security scheme");
    let scheme_obj = scheme.as_object().expect("Security scheme not an object");
    assert_eq!(scheme_obj["type"], "http");
    assert_eq!(scheme_obj["scheme"], "bearer");
    assert_eq!(scheme_obj["bearerFormat"], "JWT");
}

#[test]
fn openapi_includes_all_expected_dtos() {
    let json = spec_json();
    let schemas = json["components"]["schemas"]
        .as_object()
        .expect("Missing schemas");

    let expected = [
        "PasswordLoginRequest",
        "GoogleLoginRequest",
        "LogoutRequest",
        "RefreshTokenRequest",
        "AuthTokensResponse",
        "GoogleAuthResponse",
        "RefreshResponse",
        "StatusResponse",
        "HealthResponse",
        "ErrorResponse",
        "ProfileResponse",
        "UpdateProfileRequest",
        "UserResponse",
        "UpdateRoleRequest",
    ];
    for name in &expected {
        assert!(schemas.contains_key(*name), "Missing schema: {name}");
    }
}
