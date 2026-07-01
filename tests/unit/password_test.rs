use app_home_services::domain::entities::user::{NewUser, User};

fn create_test_user(password_hash: Option<String>) -> User {
    User {
        id: uuid::Uuid::now_v7(),
        username: Some("testuser".to_string()),
        email: "test@example.com".to_string(),
        display_name: "Test User".to_string(),
        password_hash,
        auth_provider: "local".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[test]
fn test_verify_password_correct() {
    let hash = bcrypt::hash("correct_password", bcrypt::DEFAULT_COST).unwrap();
    let user = create_test_user(Some(hash));
    assert!(user.verify_password("correct_password"));
}

#[test]
fn test_verify_password_incorrect() {
    let hash = bcrypt::hash("correct_password", bcrypt::DEFAULT_COST).unwrap();
    let user = create_test_user(Some(hash));
    assert!(!user.verify_password("wrong_password"));
}

#[test]
fn test_verify_password_no_hash() {
    let user = create_test_user(None);
    assert!(!user.verify_password("any_password"));
}

#[test]
fn test_verify_password_empty_string() {
    let hash = bcrypt::hash("", bcrypt::DEFAULT_COST).unwrap();
    let user = create_test_user(Some(hash));
    assert!(user.verify_password(""));
    assert!(!user.verify_password("not_empty"));
}

#[test]
fn test_new_user_creation() {
    let new_user = NewUser {
        username: Some("newuser".to_string()),
        email: "new@example.com".to_string(),
        display_name: "New User".to_string(),
        password_hash: Some("hash".to_string()),
        auth_provider: "local".to_string(),
    };
    assert_eq!(new_user.username, Some("newuser".to_string()));
    assert_eq!(new_user.email, "new@example.com");
    assert_eq!(new_user.auth_provider, "local");
}
