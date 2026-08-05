use auth::domain::entities::user::{NewUser, User};
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

// Verify behavior is cost-independent, so tests hash at the minimum cost for
// speed (mirrors the TEST_BCRYPT_COST pattern in the token-reuse tests).
const TEST_BCRYPT_COST: u32 = 4;

fn create_test_user(password_hash: Option<HashedPassword>) -> User {
    let email = Email::new("test@example.com").unwrap();
    let auth_provider = if password_hash.is_some() {
        AuthProvider::Local
    } else {
        AuthProvider::Google
    };
    User::new(
        Uuid::now_v7(),
        Some("testuser".to_string()),
        email,
        "Test User".to_string(),
        password_hash,
        auth_provider,
        chrono::Utc::now(),
        chrono::Utc::now(),
    )
}

#[test]
fn test_verify_password_correct() {
    let hash = bcrypt::hash("correct_password", TEST_BCRYPT_COST).unwrap();
    let user = create_test_user(Some(HashedPassword::new(hash).unwrap()));
    assert!(user.verify_password("correct_password"));
}

#[test]
fn test_verify_password_incorrect() {
    let hash = bcrypt::hash("correct_password", TEST_BCRYPT_COST).unwrap();
    let user = create_test_user(Some(HashedPassword::new(hash).unwrap()));
    assert!(!user.verify_password("wrong_password"));
}

#[test]
fn test_verify_password_no_hash() {
    let user = create_test_user(None);
    assert!(!user.verify_password("any_password"));
}

#[test]
fn test_verify_password_empty_string() {
    let hash = bcrypt::hash("", TEST_BCRYPT_COST).unwrap();
    let user = create_test_user(Some(HashedPassword::new(hash).unwrap()));
    assert!(user.verify_password(""));
    assert!(!user.verify_password("not_empty"));
}

#[test]
fn test_new_user_creation() {
    let email = Email::new("new@example.com").unwrap();
    let hash = HashedPassword::new("hash").unwrap();
    let new_user = NewUser::new_local(
        Some("newuser".to_string()),
        email,
        "New User".to_string(),
        hash,
    );
    assert_eq!(new_user.username, Some("newuser".to_string()));
    assert_eq!(new_user.email.as_ref(), "new@example.com");
    assert_eq!(new_user.auth_provider, AuthProvider::Local);
}
