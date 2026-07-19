use auth::adapters::inbound::responses::{
    AuthTokensResponse, ErrorResponse, GoogleAuthResponse, HealthResponse, RefreshResponse,
    StatusResponse,
};

#[test]
fn auth_tokens_response_round_trip() {
    let original = AuthTokensResponse {
        status: "authenticated".into(),
        user_id: "018f9a8b-7c3d-4e5f-8a1b-2c3d4e5f6a7b".into(),
        access_token: "<jwt>".into(),
        refresh_token: "<jwt>".into(),
    };
    let json = serde_json::to_value(&original).unwrap();
    let obj = json.as_object().unwrap();
    assert!(obj.contains_key("status"));
    assert!(obj.contains_key("user_id"));
    assert!(obj.contains_key("access_token"));
    assert!(obj.contains_key("refresh_token"));
    let deserialized: AuthTokensResponse = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.status, original.status);
    assert_eq!(deserialized.user_id, original.user_id);
}

#[test]
fn google_auth_response_round_trip() {
    let original = GoogleAuthResponse {
        status: "authenticated".into(),
        user_id: "018f9a8b-7c3d-4e5f-8a1b-2c3d4e5f6a7b".into(),
        access_token: "<jwt>".into(),
        refresh_token: "<jwt>".into(),
        is_new_user: false,
    };
    let json = serde_json::to_value(&original).unwrap();
    let obj = json.as_object().unwrap();
    assert!(obj.contains_key("is_new_user"));
    let deserialized: GoogleAuthResponse = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.is_new_user, original.is_new_user);
}

#[test]
fn refresh_response_round_trip() {
    let original = RefreshResponse {
        access_token: "<jwt>".into(),
        refresh_token: "<jwt>".into(),
    };
    let json = serde_json::to_value(&original).unwrap();
    let obj = json.as_object().unwrap();
    assert!(obj.contains_key("access_token"));
    assert!(obj.contains_key("refresh_token"));
}

#[test]
fn status_response_round_trip() {
    let original = StatusResponse {
        status: "logged_out".into(),
    };
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(json["status"], "logged_out");
}

#[test]
fn health_response_round_trip() {
    let original = HealthResponse {
        status: "ok".into(),
    };
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(json["status"], "ok");
}

#[test]
fn error_response_round_trip() {
    let original = ErrorResponse {
        error: "Invalid username or password".into(),
    };
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(json["error"], "Invalid username or password");
}
