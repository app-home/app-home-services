use app_home_services::domain::entities::user_action::{NewUserAction, UserAction};

#[test]
fn test_new_user_action_login() {
    let action = NewUserAction::new_login(uuid::Uuid::now_v7(), "password".to_string());

    assert_eq!(action.event_type, "login");
    assert_eq!(action.session_id, None);
    assert!(action.validate().is_ok());
}

#[test]
fn test_new_user_action_logout() {
    let session_id = uuid::Uuid::now_v7();
    let action = NewUserAction::new_logout(
        uuid::Uuid::now_v7(),
        session_id,
        "password".to_string(),
    );

    assert_eq!(action.event_type, "logout");
    assert_eq!(action.session_id, Some(session_id));
    assert!(action.validate().is_ok());
}

#[test]
fn test_new_user_action_refresh() {
    let session_id = uuid::Uuid::now_v7();
    let action = NewUserAction::new_refresh(
        uuid::Uuid::now_v7(),
        session_id,
        "google_oauth".to_string(),
    );

    assert_eq!(action.event_type, "refresh");
    assert_eq!(action.session_id, Some(session_id));
    assert_eq!(action.auth_method, "google_oauth");
    assert!(action.validate().is_ok());
}

#[test]
fn test_new_user_action_invalid_event_type() {
    let action = NewUserAction {
        user_id: uuid::Uuid::now_v7(),
        session_id: None,
        event_type: "invalid".to_string(),
        auth_method: "password".to_string(),
    };

    assert!(action.validate().is_err());
}

#[test]
fn test_user_action_immutable_after_creation() {
    let now = chrono::Utc::now();
    let action = UserAction {
        id: uuid::Uuid::now_v7(),
        user_id: uuid::Uuid::now_v7(),
        session_id: Some(uuid::Uuid::now_v7()),
        event_type: "login".to_string(),
        auth_method: "password".to_string(),
        created_at: now,
    };

    let _ = action.id;
    let _ = action.user_id;
    let _ = action.session_id;
    let _ = action.event_type;
    let _ = action.auth_method;
    let _ = action.created_at;
}
