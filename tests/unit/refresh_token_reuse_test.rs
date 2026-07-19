use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use auth::domain::aggregate::UserAggregate;
use chrono::{Duration, Utc};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::auth_provider::AuthProvider;
use shared::domain::value_objects::email::Email;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use app_home_services::application::ports::jwt_service::{
    AccessTokenClaims, JwtService, RefreshTokenClaims, TokenPair,
};
use app_home_services::application::ports::session_repository::SessionRepository;
use app_home_services::application::ports::user_repository::UserRepository;
use app_home_services::application::use_cases::refresh_token::refresh_token;
use app_home_services::domain::entities::session::{NewSession, Session};
use app_home_services::domain::entities::user::{NewUser, User};
use app_home_services::domain::entities::user_action::{NewUserAction, UserAction};
use app_home_services::domain::errors::AuthError;
use auth::config::auth_settings::AuthSettings;

type SharedSessions = std::sync::Arc<Mutex<HashMap<Uuid, Session>>>;

fn shared_sessions() -> SharedSessions {
    std::sync::Arc::new(Mutex::new(HashMap::new()))
}

struct MockSessionRepository {
    sessions: SharedSessions,
}

impl MockSessionRepository {
    fn new(sessions: SharedSessions) -> Self {
        Self { sessions }
    }
}

#[async_trait]
impl SessionRepository for MockSessionRepository {
    async fn create(&self, session: NewSession) -> Result<Session, AuthError> {
        let session = Session::new(
            session.id,
            session.user_id,
            session.refresh_token_hash,
            session.expires_at,
            true,
            Utc::now(),
            session.auth_method,
        );
        self.sessions
            .lock()
            .unwrap()
            .insert(session.id(), session.clone());
        Ok(session)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Session>, AuthError> {
        Ok(self.sessions.lock().unwrap().get(&id).cloned())
    }

    async fn find_active_by_user_id(&self, user_id: Uuid) -> Result<Vec<Session>, AuthError> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.user_id() == user_id && s.is_active())
            .cloned()
            .collect())
    }

    async fn invalidate(&self, id: Uuid) -> Result<(), AuthError> {
        if let Some(session) = self.sessions.lock().unwrap().get_mut(&id) {
            session.invalidate();
        }
        Ok(())
    }

    async fn invalidate_all_for_user(&self, user_id: Uuid) -> Result<(), AuthError> {
        for session in self.sessions.lock().unwrap().values_mut() {
            if session.user_id() == user_id {
                session.invalidate();
            }
        }
        Ok(())
    }

    async fn sessions_for_user(&self, user_id: Uuid) -> Result<Vec<Session>, AuthError> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.user_id() == user_id)
            .cloned()
            .collect())
    }
}

struct MockUserRepository {
    sessions: SharedSessions,
}

impl MockUserRepository {
    fn new(sessions: SharedSessions) -> Self {
        Self { sessions }
    }

    fn insert(&self, session: Session) {
        self.sessions.lock().unwrap().insert(session.id(), session);
    }
}

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_username(&self, _username: &str) -> Result<Option<User>, AuthError> {
        unimplemented!("not used by refresh_token")
    }
    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, AuthError> {
        unimplemented!("not used by refresh_token")
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<User>, AuthError> {
        unimplemented!("not used by refresh_token")
    }
    async fn create(&self, _user: NewUser) -> Result<User, AuthError> {
        unimplemented!("not used by refresh_token")
    }
    async fn create_user_action(&self, _action: NewUserAction) -> Result<UserAction, AuthError> {
        unimplemented!("not used by refresh_token")
    }

    async fn find_aggregate_by_id(&self, id: Uuid) -> Result<Option<UserAggregate>, AuthError> {
        let sessions: Vec<Session> = self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.user_id() == id)
            .cloned()
            .collect();

        let user = User::new(
            id,
            Some("testuser".to_string()),
            Email::new("test@example.com").unwrap(),
            "Test User".to_string(),
            None,
            AuthProvider::Google,
            Utc::now(),
            Utc::now(),
        );

        Ok(Some(UserAggregate::new(user, sessions)))
    }

    async fn find_aggregate_by_username(
        &self,
        _username: &str,
    ) -> Result<Option<UserAggregate>, AuthError> {
        unimplemented!("not used by refresh_token")
    }

    async fn find_aggregate_by_email(
        &self,
        _email: &str,
    ) -> Result<Option<UserAggregate>, AuthError> {
        unimplemented!("not used by refresh_token")
    }

    async fn save_aggregate(
        &self,
        aggregate: &mut UserAggregate,
        new_sessions: &[NewSession],
    ) -> Result<(), AuthError> {
        let mut stored = self.sessions.lock().unwrap();
        for session in &aggregate.sessions {
            stored.insert(session.id(), session.clone());
        }
        for ns in new_sessions {
            let session = Session::new(
                ns.id,
                ns.user_id,
                ns.refresh_token_hash.clone(),
                ns.expires_at,
                true,
                Utc::now(),
                ns.auth_method,
            );
            stored.insert(session.id(), session);
        }
        Ok(())
    }
}

struct MockJwtService {
    claims: RefreshTokenClaims,
}

impl JwtService for MockJwtService {
    fn generate_token_pair(
        &self,
        _user_id: Uuid,
        _session_id: Uuid,
    ) -> Result<TokenPair, AuthError> {
        Ok(TokenPair {
            access_token: "mock-access-token".to_string(),
            refresh_token: "mock-refresh-token".to_string(),
        })
    }

    fn validate_access_token(&self, _token: &str) -> Result<AccessTokenClaims, AuthError> {
        unimplemented!("not used by refresh_token")
    }

    fn validate_refresh_token(&self, _token: &str) -> Result<RefreshTokenClaims, AuthError> {
        Ok(self.claims.clone())
    }
}

fn test_settings() -> AuthSettings {
    AuthSettings {
        default_user_username: "admin".to_string(),
        default_user_password: "irrelevant".to_string(),
        default_user_email: "admin@example.com".to_string(),
        google_client_id: String::new(),
        jwt_secret: "irrelevant".to_string(),
        access_token_expiry_minutes: 15,
        refresh_token_expiry_days: 7,
    }
}

const TEST_BCRYPT_COST: u32 = 4;

#[tokio::test]
async fn reusing_invalidated_refresh_token_revokes_all_sessions_for_user() {
    let user_id = Uuid::now_v7();
    let old_session_id = Uuid::now_v7();
    let other_session_id = Uuid::now_v7();

    let sessions = shared_sessions();

    let old_hash =
        HashedPassword::new(bcrypt::hash("old-refresh-token", TEST_BCRYPT_COST).unwrap()).unwrap();
    let old_session = Session::new(
        old_session_id,
        user_id,
        old_hash,
        Utc::now() + Duration::days(7),
        false,
        Utc::now(),
        AuthMethod::Password,
    );

    let other_hash =
        HashedPassword::new(bcrypt::hash("other-refresh-token", TEST_BCRYPT_COST).unwrap())
            .unwrap();
    let other_session = Session::new(
        other_session_id,
        user_id,
        other_hash,
        Utc::now() + Duration::days(7),
        true,
        Utc::now(),
        AuthMethod::Password,
    );

    let session_repo = MockSessionRepository::new(sessions.clone());
    let user_repo = MockUserRepository::new(sessions);
    user_repo.insert(old_session);
    user_repo.insert(other_session);

    let jwt_service = MockJwtService {
        claims: RefreshTokenClaims {
            sub: user_id,
            session_id: old_session_id,
            exp: 9_999_999_999,
            iat: 1,
        },
    };
    let settings = test_settings();

    let result = refresh_token(
        &user_repo,
        &jwt_service,
        "stolen-old-refresh-token",
        &settings,
    )
    .await;

    assert!(
        matches!(result, Err(AuthError::SessionInvalidated)),
        "expected SessionInvalidated, got {result:?}"
    );

    let remaining_active = session_repo.find_active_by_user_id(user_id).await.unwrap();

    assert!(
        remaining_active.is_empty(),
        "all sessions for the user should be revoked after reuse is detected, found: {remaining_active:?}"
    );
}

#[tokio::test]
async fn legitimate_refresh_of_an_active_session_does_not_touch_other_sessions() {
    let user_id = Uuid::now_v7();
    let active_session_id = Uuid::now_v7();
    let other_session_id = Uuid::now_v7();

    let sessions = shared_sessions();
    let real_refresh_token = "real-refresh-token";

    let active_hash =
        HashedPassword::new(bcrypt::hash(real_refresh_token, TEST_BCRYPT_COST).unwrap()).unwrap();
    let active_session = Session::new(
        active_session_id,
        user_id,
        active_hash,
        Utc::now() + Duration::days(7),
        true,
        Utc::now(),
        AuthMethod::Password,
    );

    let other_hash =
        HashedPassword::new(bcrypt::hash("unrelated-token", TEST_BCRYPT_COST).unwrap()).unwrap();
    let other_session = Session::new(
        other_session_id,
        user_id,
        other_hash,
        Utc::now() + Duration::days(7),
        true,
        Utc::now(),
        AuthMethod::Password,
    );

    let session_repo = MockSessionRepository::new(sessions.clone());
    let user_repo = MockUserRepository::new(sessions);
    user_repo.insert(active_session);
    user_repo.insert(other_session);

    let jwt_service = MockJwtService {
        claims: RefreshTokenClaims {
            sub: user_id,
            session_id: active_session_id,
            exp: 9_999_999_999,
            iat: 1,
        },
    };
    let settings = test_settings();

    let result = refresh_token(&user_repo, &jwt_service, real_refresh_token, &settings).await;

    assert!(
        result.is_ok(),
        "expected a successful refresh, got {result:?}"
    );

    let remaining_active = session_repo.find_active_by_user_id(user_id).await.unwrap();

    assert_eq!(remaining_active.len(), 2);
    assert!(remaining_active.iter().any(|s| s.id() == other_session_id));
    assert!(!remaining_active.iter().any(|s| s.id() == active_session_id));
}
