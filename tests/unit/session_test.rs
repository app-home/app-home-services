use app_home_services::domain::entities::session::{NewSession, Session};
use chrono::{Duration, Utc};
use uuid::Uuid;

#[test]
fn test_new_session_creation() {
    let user_id = Uuid::now_v7();
    let expires_at = Utc::now() + Duration::hours(1);
    let session = NewSession::new(
        user_id,
        "hashed_refresh_token".to_string(),
        expires_at,
    );

    assert_eq!(session.user_id, user_id);
    assert_eq!(session.refresh_token_hash, "hashed_refresh_token");
    assert!(session.validate().is_ok());
}

#[test]
fn test_new_session_empty_hash_fails_validation() {
    let session = NewSession {
        user_id: Uuid::now_v7(),
        refresh_token_hash: String::new(),
        expires_at: Utc::now() + Duration::hours(1),
    };

    assert!(session.validate().is_err());
}

#[test]
fn test_new_session_expired_fails_validation() {
    let session = NewSession {
        user_id: Uuid::now_v7(),
        refresh_token_hash: "hash".to_string(),
        expires_at: Utc::now() - Duration::hours(1),
    };

    assert!(session.validate().is_err());
}

#[test]
fn test_session_is_expired() {
    let session = Session {
        id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        refresh_token_hash: "hash".to_string(),
        expires_at: Utc::now() - Duration::hours(1),
        is_active: true,
        created_at: Utc::now() - Duration::hours(2),
    };

    assert!(session.is_expired());
}

#[test]
fn test_session_is_not_expired() {
    let session = Session {
        id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        refresh_token_hash: "hash".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        is_active: true,
        created_at: Utc::now(),
    };

    assert!(!session.is_expired());
}

#[test]
fn test_session_invalidate() {
    let mut session = Session {
        id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        refresh_token_hash: "hash".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        is_active: true,
        created_at: Utc::now(),
    };

    assert!(session.is_active);
    session.invalidate();
    assert!(!session.is_active);
}

#[test]
fn test_session_invalidate_is_one_way() {
    let mut session = Session {
        id: Uuid::now_v7(),
        user_id: Uuid::now_v7(),
        refresh_token_hash: "hash".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        is_active: true,
        created_at: Utc::now(),
    };

    session.invalidate();
    assert!(!session.is_active);
    // There is no re-activate method — one-way transition
}
