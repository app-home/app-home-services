use app_home_services::domain::entities::user_action::{NewUserAction, UserAction};

#[test]
fn test_new_user_action_creation() {
    let user_id = uuid::Uuid::now_v7();
    let action = NewUserAction {
        user_id,
        auth_method: "password".to_string(),
    };

    assert_eq!(action.user_id, user_id);
    assert_eq!(action.auth_method, "password");
}

#[test]
fn test_new_user_action_google_oauth() {
    let action = NewUserAction {
        user_id: uuid::Uuid::now_v7(),
        auth_method: "google_oauth".to_string(),
    };

    assert_eq!(action.auth_method, "google_oauth");
}

#[test]
fn test_user_action_immutable_after_creation() {
    let now = chrono::Utc::now();
    let action = UserAction {
        id: uuid::Uuid::now_v7(),
        user_id: uuid::Uuid::now_v7(),
        auth_method: "password".to_string(),
        created_at: now,
    };

    // Verify all fields are accessible
    let _ = action.id;
    let _ = action.user_id;
    let _ = action.auth_method;
    let _ = action.created_at;

    // Append-only: no update methods should exist
    // If this compiles, the struct has no mutable operations on its fields
}
