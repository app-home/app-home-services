use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use shared::domain::value_objects::auth_method::AuthMethod;
use shared::domain::value_objects::hashed_password::HashedPassword;
use uuid::Uuid;

use app_home_services::application::ports::jwt_service::{
    AccessTokenClaims, JwtService, RefreshTokenClaims, TokenPair,
};
use app_home_services::application::ports::session_repository::SessionRepository;
use app_home_services::application::ports::user_repository::UserRepository;
use app_home_services::application::use_cases::logout::logout;
use app_home_services::application::use_cases::refresh_token::refresh_token;
use app_home_services::domain::entities::session::{NewSession, Session};
use app_home_services::domain::entities::user::{NewUser, User};
use app_home_services::domain::entities::user_action::{NewUserAction, UserAction};
use app_home_services::domain::errors::AuthError;
use app_home_services::infrastructure::config::settings::Settings;

struct MockSessionRepository {
    sessions: Mutex<HashMap<Uuid, Session>>,
}

impl MockSessionRepository {
    fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn insert(&self, session: Session) {
        self.sessions.lock().unwrap().insert(session.id(), session);
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
}

struct MockUserRepository;

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_username(&self, _username: &str) -> Result<Option<User>, AuthError> {
        unimplemented!("not used by logout/refresh_token")
    }
    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, AuthError> {
        unimplemented!("not used by logout/refresh_token")
    }
    async fn find_by_id(&self, _id: Uuid) -> Result<Option<User>, AuthError> {
        unimplemented!("not used by logout/refresh_token")
    }
    async fn create(&self, _user: NewUser) -> Result<User, AuthError> {
        unimplemented!("not used by logout/refresh_token")
    }
    async fn create_user_action(&self, _action: NewUserAction) -> Result<UserAction, AuthError> {
        unimplemented!("not used by logout/refresh_token")
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

fn test_settings() -> Settings {
    Settings {
        database_url: String::new(),
        server_host: "0.0.0.0".to_string(),
        server_port: 3000,
        default_user_username: "admin".to_string(),
        default_user_password: "irrelevant".to_string(),
        default_user_email: "admin@example.com".to_string(),
        google_client_id: String::new(),
        jwt_secret: "irrelevant".to_string(),
        rate_limit_max_attempts: 10,
        rate_limit_window_seconds: 300,
        access_token_expiry_minutes: 15,
        refresh_token_expiry_days: 7,
        cors_allowed_origins: String::new(),
        trusted_proxy_ips: vec![],
        redis_url: None,
    }
}

const TEST_BCRYPT_COST: u32 = 4;

#[tokio::test]
async fn logout_reports_google_oauth_auth_method_not_password() {
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let session_repo = MockSessionRepository::new();
    let hash = HashedPassword::new(bcrypt::hash("some-refresh-token", TEST_BCRYPT_COST).unwrap())
        .unwrap();
    session_repo.insert(Session::new(
        session_id,
        user_id,
        hash,
        Utc::now() + Duration::days(7),
        true,
        Utc::now(),
        AuthMethod::GoogleOAuth,
    ));
    let user_repo = MockUserRepository;

    let (auth_method, _event) =
        logout(&session_repo, &user_repo, user_id, session_id)
            .await
            .expect("logout should succeed");

    assert_eq!(
        auth_method,
        AuthMethod::GoogleOAuth,
        "the audit entry's auth_method must reflect how this session was actually \
         created, not be hardcoded to \"password\""
    );
}

#[tokio::test]
async fn logout_reports_password_auth_method_for_a_password_session() {
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();

    let session_repo = MockSessionRepository::new();
    let hash = HashedPassword::new(bcrypt::hash("some-refresh-token", TEST_BCRYPT_COST).unwrap())
        .unwrap();
    session_repo.insert(Session::new(
        session_id,
        user_id,
        hash,
        Utc::now() + Duration::days(7),
        true,
        Utc::now(),
        AuthMethod::Password,
    ));
    let user_repo = MockUserRepository;

    let (auth_method, _event) =
        logout(&session_repo, &user_repo, user_id, session_id)
            .await
            .expect("logout should succeed");

    assert_eq!(auth_method, AuthMethod::Password);
}

#[tokio::test]
async fn refresh_carries_google_oauth_auth_method_forward_after_rotation() {
    let user_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let refresh_token_value = "google-session-refresh-token";

    let session_repo = MockSessionRepository::new();
    let hash =
        HashedPassword::new(bcrypt::hash(refresh_token_value, TEST_BCRYPT_COST).unwrap()).unwrap();
    session_repo.insert(Session::new(
        session_id,
        user_id,
        hash,
        Utc::now() + Duration::days(7),
        true,
        Utc::now(),
        AuthMethod::GoogleOAuth,
    ));
    let user_repo = MockUserRepository;
    let jwt_service = MockJwtService {
        claims: RefreshTokenClaims {
            sub: user_id,
            session_id,
            exp: 9_999_999_999,
            iat: 1,
        },
    };
    let settings = test_settings();

    let result = refresh_token(
        &session_repo,
        &user_repo,
        &jwt_service,
        refresh_token_value,
        &settings,
    )
    .await
    .expect("refresh should succeed");

    assert_eq!(
        result.auth_method,
        AuthMethod::GoogleOAuth,
        "the audit entry's auth_method must reflect the original session's method, \
         not be reset to \"password\" on rotation"
    );

    let new_session = session_repo
        .find_by_id(result.session_id)
        .await
        .unwrap()
        .expect("rotated session should exist");
    assert_eq!(new_session.auth_method(), &AuthMethod::GoogleOAuth);
}
