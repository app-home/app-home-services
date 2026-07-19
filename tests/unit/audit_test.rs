use app_home_services::domain::entities::user_action::{NewUserAction, UserAction};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::event_type::EventType;
use uuid::Uuid;

#[test]
fn test_new_user_action_login() {
    let user_id = Uuid::now_v7();
    let action = NewUserAction::new_login(user_id, AuthMethod::Password);

    assert_eq!(action.event_type, EventType::Login);
    assert_eq!(action.session_id, None);
    assert!(action.validate().is_ok());
}

#[test]
fn test_new_user_action_logout() {
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let action = NewUserAction::new_logout(user_id, session_id, AuthMethod::Password);

    assert_eq!(action.event_type, EventType::Logout);
    assert_eq!(action.session_id, Some(session_id));
    assert!(action.validate().is_ok());
}

#[test]
fn test_new_user_action_refresh() {
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let action = NewUserAction::new_refresh(user_id, session_id, AuthMethod::GoogleOAuth);

    assert_eq!(action.event_type, EventType::Refresh);
    assert_eq!(action.session_id, Some(session_id));
    assert_eq!(action.auth_method, AuthMethod::GoogleOAuth);
    assert!(action.validate().is_ok());
}

#[test]
fn test_user_action_immutable_after_creation() {
    let now = chrono::Utc::now();
    let action = UserAction::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        Some(Uuid::now_v7()),
        EventType::Login,
        AuthMethod::Password,
        now,
    );

    let _ = action.id();
    let _ = action.user_id();
    let _ = action.session_id();
    let _ = action.event_type();
    let _ = action.auth_method();
    let _ = action.created_at();
}
