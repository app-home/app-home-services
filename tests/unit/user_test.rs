use app_home_services::domain::entities::user::User;
use app_home_services::domain::entities::user_action::UserAction;
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::event_type::EventType;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

#[test]
fn test_user_struct_creation() {
    let now = chrono::Utc::now();
    let email = Email::new("admin@example.com").unwrap();
    let password_hash = HashedPassword::new("$2b$12$hash").unwrap();
    let user = User::new(
        Uuid::now_v7(),
        Some("admin".to_string()),
        email,
        "Administrator".to_string(),
        Some(password_hash),
        AuthProvider::Local,
        now,
        now,
    );

    assert_eq!(user.username(), Some("admin"));
    assert_eq!(user.auth_provider(), &AuthProvider::Local);
    assert!(user.password_hash().is_some());
}

#[test]
fn test_google_user_no_password_hash() {
    let email = Email::new("googleuser@gmail.com").unwrap();
    let user = User::new(
        Uuid::now_v7(),
        None,
        email,
        "Google User".to_string(),
        None,
        AuthProvider::Google,
        chrono::Utc::now(),
        chrono::Utc::now(),
    );

    assert_eq!(user.username(), None);
    assert!(user.password_hash().is_none());
    assert_eq!(user.auth_provider(), &AuthProvider::Google);
    assert!(!user.verify_password("anything"));
}

#[test]
fn test_user_action_creation() {
    let now = chrono::Utc::now();
    let user_id = Uuid::now_v7();
    let action = UserAction::new(
        Uuid::now_v7(),
        user_id,
        None,
        EventType::Login,
        AuthMethod::Password,
        now,
    );

    assert_eq!(action.user_id(), user_id);
    assert_eq!(action.auth_method(), &AuthMethod::Password);
    assert_eq!(action.event_type(), &EventType::Login);
    assert!(action.session_id().is_none());
}

#[test]
fn test_user_action_google_oauth_method() {
    let action = UserAction::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        Some(Uuid::now_v7()),
        EventType::Login,
        AuthMethod::GoogleOAuth,
        chrono::Utc::now(),
    );

    assert_eq!(action.auth_method(), &AuthMethod::GoogleOAuth);
}
