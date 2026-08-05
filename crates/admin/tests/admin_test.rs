use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use admin::adapters::inbound::responses::{UpdateRoleRequest, UserResponse};
use admin::application::ports::admin_repository::{AdminRepository, ListUsersResult};
use admin::application::use_cases::{get_user, list_users, update_user_role};
use admin::domain::entities::admin_user::AdminUser;
use admin::domain::errors::AdminError;
use admin::domain::value_objects::role::Role;

// ---------------------------------------------------------------------------
// Mock repository
// ---------------------------------------------------------------------------

struct MockAdminRepo {
    store: Mutex<HashMap<Uuid, AdminUser>>,
}

impl MockAdminRepo {
    fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    fn seed(&self, user: AdminUser) {
        self.store.lock().unwrap().insert(user.id(), user);
    }
}

#[async_trait]
impl AdminRepository for MockAdminRepo {
    async fn list_users(&self, page: u32, per_page: u32) -> Result<ListUsersResult, AdminError> {
        let mut users: Vec<AdminUser> = self.store.lock().unwrap().values().cloned().collect();
        users.sort_by(|left, right| {
            right
                .created_at()
                .cmp(&left.created_at())
                .then_with(|| right.id().cmp(&left.id()))
        });
        let total = users.len() as u64;
        let offset = u64::from(page.saturating_sub(1)) * u64::from(per_page);
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let users = users
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect();
        Ok(ListUsersResult { users, total })
    }

    async fn get_user(&self, user_id: Uuid) -> Result<AdminUser, AdminError> {
        self.store
            .lock()
            .unwrap()
            .get(&user_id)
            .cloned()
            .ok_or(AdminError::NotFound(user_id))
    }

    async fn is_admin(&self, user_id: Uuid) -> Result<bool, AdminError> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .get(&user_id)
            .map(|u| u.is_admin())
            .unwrap_or(false))
    }

    async fn update_role(&self, user_id: Uuid, role: &Role) -> Result<AdminUser, AdminError> {
        let mut store = self.store.lock().unwrap();
        let user = store
            .get_mut(&user_id)
            .ok_or(AdminError::NotFound(user_id))?;
        let updated = AdminUser::new(
            user.id(),
            user.username().map(|s| s.to_string()),
            user.email().to_string(),
            user.display_name().to_string(),
            role.clone(),
            user.auth_provider().to_string(),
            user.created_at(),
            Utc::now(),
        );
        store.insert(user_id, updated.clone());
        Ok(updated)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_admin_user(id: Uuid, username: Option<&str>, role: Role) -> AdminUser {
    AdminUser::new(
        id,
        username.map(|s| s.to_string()),
        format!("{}@example.com", username.unwrap_or("user")),
        "Display Name".to_string(),
        role,
        "local".to_string(),
        Utc::now(),
        Utc::now(),
    )
}

// ---------------------------------------------------------------------------
// AdminUser entity
// ---------------------------------------------------------------------------

#[test]
fn admin_user_creation_with_all_fields() {
    let id = Uuid::now_v7();
    let now = Utc::now();
    let user = AdminUser::new(
        id,
        Some("admin".to_string()),
        "admin@example.com".to_string(),
        "Administrator".to_string(),
        Role::Admin,
        "local".to_string(),
        now,
        now,
    );

    assert_eq!(user.id(), id);
    assert_eq!(user.username(), Some("admin"));
    assert_eq!(user.email(), "admin@example.com");
    assert_eq!(user.display_name(), "Administrator");
    assert_eq!(user.role(), &Role::Admin);
    assert_eq!(user.auth_provider(), "local");
    assert_eq!(user.created_at(), now);
    assert_eq!(user.updated_at(), now);
}

#[test]
fn admin_user_creation_without_username() {
    let user = AdminUser::new(
        Uuid::now_v7(),
        None,
        "google@example.com".to_string(),
        "Google User".to_string(),
        Role::User,
        "google".to_string(),
        Utc::now(),
        Utc::now(),
    );

    assert!(user.username().is_none());
    assert_eq!(user.auth_provider(), "google");
}

#[test]
fn admin_user_is_admin_for_admin_role() {
    let user = make_admin_user(Uuid::now_v7(), Some("admin"), Role::Admin);
    assert!(user.is_admin());
    assert_eq!(user.role(), &Role::Admin);
}

#[test]
fn admin_user_is_not_admin_for_user_role() {
    let user = make_admin_user(Uuid::now_v7(), Some("user"), Role::User);
    assert!(!user.is_admin());
    assert_eq!(user.role(), &Role::User);
}

// ---------------------------------------------------------------------------
// AdminError
// ---------------------------------------------------------------------------

#[test]
fn admin_error_not_found_message() {
    let user_id = Uuid::now_v7();
    let err = AdminError::NotFound(user_id);
    assert!(err.to_string().contains(&user_id.to_string()));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn admin_error_unauthorized_message() {
    let err = AdminError::Unauthorized;
    assert_eq!(err.to_string(), "Not authorized");
}

#[test]
fn admin_error_invalid_value_message() {
    let err = AdminError::InvalidValue("bad role".into());
    assert_eq!(err.to_string(), "Invalid value: bad role");
}

#[test]
fn admin_error_internal_error_message() {
    let err = AdminError::InternalError("db failure".into());
    assert_eq!(err.to_string(), "Internal error: db failure");
}

// ---------------------------------------------------------------------------
// list_users use case
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_users_returns_all_users_on_first_page() {
    let repo = MockAdminRepo::new();
    let user1 = make_admin_user(Uuid::now_v7(), Some("alice"), Role::User);
    let user2 = make_admin_user(Uuid::now_v7(), Some("bob"), Role::Admin);
    repo.seed(user1);
    repo.seed(user2);

    let result = list_users::list_users(&repo, 1, 100).await.unwrap();
    assert_eq!(result.users.len(), 2);
    assert_eq!(result.total, 2);
}

#[tokio::test]
async fn list_users_returns_empty_when_no_users() {
    let repo = MockAdminRepo::new();
    let result = list_users::list_users(&repo, 1, 100).await.unwrap();
    assert!(result.users.is_empty());
    assert_eq!(result.total, 0);
}

#[tokio::test]
async fn list_users_paginates_by_created_at_desc() {
    let repo = MockAdminRepo::new();
    let base = Utc::now();
    for i in 0..5u32 {
        let created_at = base + chrono::Duration::seconds(i64::from(i));
        let user = AdminUser::new(
            Uuid::now_v7(),
            Some(format!("user-{i}")),
            format!("user-{i}@example.com"),
            "Display Name".to_string(),
            Role::User,
            "local".to_string(),
            created_at,
            created_at,
        );
        repo.seed(user);
    }

    let page1 = list_users::list_users(&repo, 1, 2).await.unwrap();
    assert_eq!(page1.users.len(), 2);
    assert_eq!(page1.total, 5);
    assert_eq!(page1.users[0].username(), Some("user-4"));
    assert_eq!(page1.users[1].username(), Some("user-3"));

    let page3 = list_users::list_users(&repo, 3, 2).await.unwrap();
    assert_eq!(page3.users.len(), 1);
    assert_eq!(page3.users[0].username(), Some("user-0"));

    let beyond = list_users::list_users(&repo, 4, 2).await.unwrap();
    assert!(beyond.users.is_empty());
    assert_eq!(beyond.total, 5);
}

#[tokio::test]
async fn list_users_uses_id_to_break_created_at_ties() {
    let repo = MockAdminRepo::new();
    let created_at = Utc::now();
    for (id, username) in [
        (Uuid::from_u128(1), "user-1"),
        (Uuid::from_u128(3), "user-3"),
        (Uuid::from_u128(2), "user-2"),
    ] {
        repo.seed(AdminUser::new(
            id,
            Some(username.to_string()),
            format!("{username}@example.com"),
            "Display Name".to_string(),
            Role::User,
            "local".to_string(),
            created_at,
            created_at,
        ));
    }

    let page1 = list_users::list_users(&repo, 1, 2).await.unwrap();
    let page2 = list_users::list_users(&repo, 2, 2).await.unwrap();

    assert_eq!(
        page1.users.iter().map(|user| user.id()).collect::<Vec<_>>(),
        [Uuid::from_u128(3), Uuid::from_u128(2)]
    );
    assert_eq!(
        page2.users.iter().map(|user| user.id()).collect::<Vec<_>>(),
        [Uuid::from_u128(1)]
    );
}

#[tokio::test]
async fn list_users_preserves_offsets_larger_than_u32() {
    let repo = MockAdminRepo::new();
    repo.seed(make_admin_user(Uuid::now_v7(), Some("alice"), Role::User));

    let page = u32::MAX / 500 + 3;
    let offset = u64::from(page - 1) * 500;
    assert!(offset > u64::from(u32::MAX));

    let result = list_users::list_users(&repo, page, 500).await.unwrap();
    assert!(result.users.is_empty());
    assert_eq!(result.total, 1);
}

// ---------------------------------------------------------------------------
// get_user use case
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_user_found() {
    let repo = MockAdminRepo::new();
    let id = Uuid::now_v7();
    let user = make_admin_user(id, Some("alice"), Role::Admin);
    repo.seed(user);

    let result = get_user::get_user(&repo, id).await.unwrap();
    assert_eq!(result.id(), id);
    assert_eq!(result.username(), Some("alice"));
    assert!(result.is_admin());
}

#[tokio::test]
async fn get_user_not_found() {
    let repo = MockAdminRepo::new();
    let missing_id = Uuid::now_v7();

    let err = get_user::get_user(&repo, missing_id).await.unwrap_err();
    assert!(matches!(err, AdminError::NotFound(id) if id == missing_id));
}

// ---------------------------------------------------------------------------
// Serialization / response types
// ---------------------------------------------------------------------------

#[test]
fn user_response_round_trip() {
    let resp = UserResponse {
        id: Uuid::now_v7().to_string(),
        username: Some("admin".into()),
        email: "admin@example.com".into(),
        display_name: "Administrator".into(),
        role: "admin".into(),
        auth_provider: "local".into(),
        created_at: "2026-07-19T12:00:00Z".into(),
        updated_at: "2026-07-19T12:00:00Z".into(),
    };
    let json = serde_json::to_value(&resp).unwrap();
    let deserialized: UserResponse = serde_json::from_value(json).unwrap();
    assert_eq!(resp.id, deserialized.id);
    assert_eq!(resp.username, deserialized.username);
    assert_eq!(resp.email, deserialized.email);
    assert_eq!(resp.role, deserialized.role);
}

#[test]
fn user_response_null_username() {
    let resp = UserResponse {
        id: Uuid::now_v7().to_string(),
        username: None,
        email: "google@example.com".into(),
        display_name: "Google User".into(),
        role: "user".into(),
        auth_provider: "google".into(),
        created_at: "2026-07-19T12:00:00Z".into(),
        updated_at: "2026-07-19T12:00:00Z".into(),
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json["username"].is_null());
    let deserialized: UserResponse = serde_json::from_value(json).unwrap();
    assert!(deserialized.username.is_none());
}

// ---------------------------------------------------------------------------
// update_user_role use case
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_user_role_promotes_to_admin() {
    let repo = MockAdminRepo::new();
    let actor_id = Uuid::now_v7();
    let id = Uuid::now_v7();
    let user = make_admin_user(id, Some("alice"), Role::User);
    repo.seed(make_admin_user(actor_id, Some("root"), Role::Admin));
    repo.seed(user);

    let result = update_user_role::update_user_role(&repo, actor_id, id, "admin")
        .await
        .unwrap();
    assert!(result.is_admin());
    assert_eq!(result.role(), &Role::Admin);
}

#[tokio::test]
async fn update_user_role_demotes_to_user() {
    let repo = MockAdminRepo::new();
    let actor_id = Uuid::now_v7();
    let id = Uuid::now_v7();
    let user = make_admin_user(id, Some("bob"), Role::Admin);
    repo.seed(make_admin_user(actor_id, Some("root"), Role::Admin));
    repo.seed(user);

    let result = update_user_role::update_user_role(&repo, actor_id, id, "user")
        .await
        .unwrap();
    assert!(!result.is_admin());
    assert_eq!(result.role(), &Role::User);
}

#[tokio::test]
async fn update_user_role_not_found() {
    let repo = MockAdminRepo::new();
    let actor_id = Uuid::now_v7();
    let missing_id = Uuid::now_v7();

    let err = update_user_role::update_user_role(&repo, actor_id, missing_id, "admin")
        .await
        .unwrap_err();
    assert!(matches!(err, AdminError::NotFound(id) if id == missing_id));
}

#[tokio::test]
async fn update_user_role_rejects_invalid_role() {
    let repo = MockAdminRepo::new();
    let actor_id = Uuid::now_v7();
    let id = Uuid::now_v7();
    let user = make_admin_user(id, Some("alice"), Role::User);
    repo.seed(make_admin_user(actor_id, Some("root"), Role::Admin));
    repo.seed(user);

    let err = update_user_role::update_user_role(&repo, actor_id, id, "superadmin")
        .await
        .unwrap_err();
    assert!(matches!(err, AdminError::InvalidValue(_)));
    assert!(err.to_string().contains("role"));
}

#[tokio::test]
async fn update_user_role_rejects_self_demotion() {
    let repo = MockAdminRepo::new();
    let id = Uuid::now_v7();
    repo.seed(make_admin_user(id, Some("root"), Role::Admin));

    let err = update_user_role::update_user_role(&repo, id, id, "user")
        .await
        .unwrap_err();
    assert!(matches!(err, AdminError::CannotChangeOwnRole));
}

#[tokio::test]
async fn update_user_role_rejects_self_promotion() {
    let repo = MockAdminRepo::new();
    let id = Uuid::now_v7();
    repo.seed(make_admin_user(id, Some("root"), Role::User));

    let err = update_user_role::update_user_role(&repo, id, id, "admin")
        .await
        .unwrap_err();
    assert!(matches!(err, AdminError::CannotChangeOwnRole));
}

// ---------------------------------------------------------------------------
// UpdateRoleRequest serialization
// ---------------------------------------------------------------------------

#[test]
fn update_role_request_round_trip() {
    let req = UpdateRoleRequest {
        role: "admin".into(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["role"], "admin");
    let deserialized: UpdateRoleRequest = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.role, "admin");
}
