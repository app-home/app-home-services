use uuid::Uuid;

use auth::adapters::jwt_service::JwtServiceImpl;
use auth::application::ports::jwt_service::{AccessTokenClaims, JwtService};
use jsonwebtoken::{EncodingKey, Header};
use shared::auth::JwtVerification;

fn create_service() -> JwtServiceImpl {
    JwtServiceImpl::new(
        "test-secret-key-that-is-long-enough-for-hmac",
        15,
        7,
        "app-home-services",
        "app-home-services",
    )
}

#[test]
fn test_generate_token_pair_returns_ok() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let result = service.generate_token_pair(user_id, session_id);
    assert!(result.is_ok());

    let pair = result.unwrap();
    assert!(!pair.access_token.is_empty());
    assert!(!pair.refresh_token.is_empty());
    assert_ne!(pair.access_token, pair.refresh_token);
}

#[test]
fn test_validate_access_token_returns_claims() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let pair = service.generate_token_pair(user_id, session_id).unwrap();
    let claims = service.validate_access_token(&pair.access_token).unwrap();

    assert_eq!(claims.sub, user_id);
    assert!(claims.exp > 0);
    assert!(claims.iat > 0);
    assert_ne!(claims.jti, Uuid::nil(), "jti must be present");
}

#[test]
fn test_access_tokens_have_unique_jti() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let pair1 = service.generate_token_pair(user_id, session_id).unwrap();
    let pair2 = service.generate_token_pair(user_id, session_id).unwrap();

    let claims1 = service.validate_access_token(&pair1.access_token).unwrap();
    let claims2 = service.validate_access_token(&pair2.access_token).unwrap();

    assert_ne!(claims1.jti, Uuid::nil(), "jti must be present");
    assert_ne!(
        claims1.jti, claims2.jti,
        "each access token must have its own jti so it can be revoked individually (see #88)"
    );
}

#[test]
fn test_validate_refresh_token_returns_claims() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let pair = service.generate_token_pair(user_id, session_id).unwrap();
    let claims = service.validate_refresh_token(&pair.refresh_token).unwrap();

    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.session_id, session_id);
    assert!(claims.exp > 0);
}

#[test]
fn test_validate_invalid_token_fails() {
    let service = create_service();
    let result = service.validate_access_token("invalid-token");
    assert!(result.is_err());
}

#[test]
fn test_validate_tampered_token_fails() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let pair = service.generate_token_pair(user_id, session_id).unwrap();
    let mut tampered = pair.access_token.clone();
    tampered.push('x');

    let result = service.validate_access_token(&tampered);
    assert!(result.is_err());
}

#[test]
fn test_access_and_refresh_tokens_are_different() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let pair = service.generate_token_pair(user_id, session_id).unwrap();
    assert_ne!(pair.access_token, pair.refresh_token);
}

#[test]
fn test_valid_token_with_iss_aud_validates() {
    let service = create_service();
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let pair = service.generate_token_pair(user_id, session_id).unwrap();

    let access_claims = service.validate_access_token(&pair.access_token).unwrap();
    assert_eq!(access_claims.iss, "app-home-services");
    assert_eq!(access_claims.aud, "app-home-services");

    let refresh_claims = service.validate_refresh_token(&pair.refresh_token).unwrap();
    assert_eq!(refresh_claims.iss, "app-home-services");
    assert_eq!(refresh_claims.aud, "app-home-services");
}

/// Builds a raw access-token JWT with an explicit `iss`/`aud` pair (rather than
/// going through `JwtServiceImpl`, which always sets both to the same value),
/// so `iss` and `aud` enforcement can be tested independently of each other.
/// Always includes a valid `jti` so decoding never fails for a reason unrelated
/// to the claim under test (see #87 / CodeRabbit review on PR #142).
fn encode_access_token_with(secret: &str, iss: &str, aud: &str) -> String {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = serde_json::json!({
        "sub": Uuid::now_v7(),
        "jti": Uuid::now_v7(),
        "iss": iss,
        "aud": aud,
        "exp": now + 900,
        "iat": now,
    });
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

#[test]
fn test_access_token_with_foreign_iss_is_rejected() {
    // Correct aud, wrong iss -- isolates iss enforcement. A verifier that
    // checks only aud would incorrectly accept this token.
    let secret = "test-secret-key-that-is-long-enough-for-hmac";
    let token = encode_access_token_with(secret, "staging", "app-home-services");

    let verification = JwtVerification::new(
        secret,
        "app-home-services".to_string(),
        "app-home-services".to_string(),
    );

    let result: Option<AccessTokenClaims> = verification.decode(&token);
    assert!(
        result.is_none(),
        "a token with a foreign iss must be rejected even when aud matches"
    );
}

#[test]
fn test_access_token_with_foreign_aud_is_rejected() {
    // Correct iss, wrong aud -- isolates aud enforcement. A verifier that
    // checks only iss would incorrectly accept this token.
    let secret = "test-secret-key-that-is-long-enough-for-hmac";
    let token = encode_access_token_with(secret, "app-home-services", "staging");

    let verification = JwtVerification::new(
        secret,
        "app-home-services".to_string(),
        "app-home-services".to_string(),
    );

    let result: Option<AccessTokenClaims> = verification.decode(&token);
    assert!(
        result.is_none(),
        "a token with a foreign aud must be rejected even when iss matches"
    );
}

#[test]
fn test_token_without_iss_aud_is_rejected() {
    // A legacy token signed with the correct secret but missing iss/aud (the
    // pre-#87 format) must no longer validate. `jti` is included (even though
    // this predates #88 too) so a missing-jti decode failure can't be mistaken
    // for the iss/aud rejection this test is actually checking.
    let secret = "test-secret-key-that-is-long-enough-for-hmac";
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = serde_json::json!({
        "sub": Uuid::now_v7(),
        "jti": Uuid::now_v7(),
        "exp": now + 900,
        "iat": now,
    });
    let token = jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();

    let verification = JwtVerification::new(
        secret,
        "app-home-services".to_string(),
        "app-home-services".to_string(),
    );

    let result: Option<AccessTokenClaims> = verification.decode(&token);
    assert!(result.is_none(), "a token missing iss/aud must be rejected");
}

#[test]
fn test_multiple_pairs_with_different_users_are_unique() {
    let service = create_service();
    let session_id = Uuid::now_v7();

    let pair1 = service
        .generate_token_pair(Uuid::now_v7(), session_id)
        .unwrap();
    let pair2 = service
        .generate_token_pair(Uuid::now_v7(), session_id)
        .unwrap();

    assert_ne!(pair1.access_token, pair2.access_token);
    assert_ne!(pair1.refresh_token, pair2.refresh_token);
}
