use app_home_services::domain::entities::user::User;
use app_home_services::domain::entities::user_action::UserAction;

#[test]
fn test_user_struct_creation() {
    let now = chrono::Utc::now();
    let user = User {
        id: uuid::Uuid::now_v7(),
        username: Some("admin".to_string()),
        email: "admin@example.com".to_string(),
        display_name: "Administrator".to_string(),
        password_hash: Some("$2b$12$hash".to_string()),
        auth_provider: "local".to_string(),
        created_at: now,
        updated_at: now,
    };

    assert_eq!(user.username, Some("admin".to_string()));
    assert_eq!(user.auth_provider, "local");
    assert!(user.password_hash.is_some());
}

#[test]
fn test_google_user_no_password_hash() {
    let user = User {
        id: uuid::Uuid::now_v7(),
        username: None,
        email: "googleuser@gmail.com".to_string(),
        display_name: "Google User".to_string(),
        password_hash: None,
        auth_provider: "google".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    assert_eq!(user.username, None);
    assert!(user.password_hash.is_none());
    assert_eq!(user.auth_provider, "google");
    assert!(!user.verify_password("anything"));
}

#[test]
fn test_user_action_creation() {
    let now = chrono::Utc::now();
    let user_id = uuid::Uuid::now_v7();
    let action = UserAction {
        id: uuid::Uuid::now_v7(),
        user_id,
        auth_method: "password".to_string(),
        created_at: now,
    };

    assert_eq!(action.user_id, user_id);
    assert_eq!(action.auth_method, "password");
}

#[test]
fn test_user_action_google_oauth_method() {
    let action = UserAction {
        id: uuid::Uuid::now_v7(),
        user_id: uuid::Uuid::now_v7(),
        auth_method: "google_oauth".to_string(),
        created_at: chrono::Utc::now(),
    };

    assert_eq!(action.auth_method, "google_oauth");
}
