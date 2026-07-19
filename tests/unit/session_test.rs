use app_home_services::domain::entities::session::{NewSession, Session};
use chrono::{Duration, Utc};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

#[test]
fn test_new_session_creation() {
    let user_id = Uuid::now_v7();
    let expires_at = Utc::now() + Duration::hours(1);
    let hash = HashedPassword::new("hashed_refresh_token").unwrap();
    let session = NewSession::new(
        Uuid::now_v7(),
        user_id,
        hash,
        expires_at,
        AuthMethod::Password,
    );

    assert_eq!(session.user_id, user_id);
    assert_eq!(session.refresh_token_hash.as_ref(), "hashed_refresh_token");
    assert_eq!(session.auth_method, AuthMethod::Password);
    assert!(session.validate().is_ok());
}

#[test]
fn test_hashed_password_rejects_empty_string() {
    assert!(HashedPassword::new("").is_err());
}

#[test]
fn test_new_session_expired_fails_validation() {
    let hash = HashedPassword::new("hash").unwrap();
    let session = NewSession::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        hash,
        Utc::now() - Duration::hours(1),
        AuthMethod::Password,
    );

    assert!(session.validate().is_err());
}

#[test]
fn test_auth_method_enum_has_valid_variants() {
    assert_ne!(AuthMethod::Password.as_str(), "");
    assert_ne!(AuthMethod::GoogleOAuth.as_str(), "");
    assert!(AuthMethod::Password.as_str() == "password");
    assert!(AuthMethod::GoogleOAuth.as_str() == "google_oauth");
}

#[test]
fn test_session_is_expired() {
    let hash = HashedPassword::new("hash").unwrap();
    let session = Session::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        hash,
        Utc::now() - Duration::hours(1),
        true,
        Utc::now() - Duration::hours(2),
        AuthMethod::Password,
    );

    assert!(session.is_expired());
}

#[test]
fn test_session_is_not_expired() {
    let hash = HashedPassword::new("hash").unwrap();
    let session = Session::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        hash,
        Utc::now() + Duration::hours(1),
        true,
        Utc::now(),
        AuthMethod::GoogleOAuth,
    );

    assert!(!session.is_expired());
}

#[test]
fn test_session_invalidate() {
    let hash = HashedPassword::new("hash").unwrap();
    let mut session = Session::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        hash,
        Utc::now() + Duration::hours(1),
        true,
        Utc::now(),
        AuthMethod::Password,
    );

    assert!(session.is_active());
    session.invalidate();
    assert!(!session.is_active());
}

#[test]
fn test_session_invalidate_is_one_way() {
    let hash = HashedPassword::new("hash").unwrap();
    let mut session = Session::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        hash,
        Utc::now() + Duration::hours(1),
        true,
        Utc::now(),
        AuthMethod::Password,
    );

    session.invalidate();
    assert!(!session.is_active());
}
