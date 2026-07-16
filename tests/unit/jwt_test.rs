use uuid::Uuid;

use app_home_services::adapters::outbound::jwt_service::JwtServiceImpl;
use app_home_services::application::ports::jwt_service::JwtService;

fn create_service() -> JwtServiceImpl {
    JwtServiceImpl::new("test-secret-key-that-is-long-enough-for-hmac", 15, 7)
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
