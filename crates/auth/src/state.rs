use std::net::IpAddr;
use std::sync::Arc;

use shared::event_bus::EventBus;

use crate::adapters::google_auth_provider::GoogleAuthProvider;
use crate::adapters::jwt_service::JwtServiceImpl;
use crate::adapters::postgres_session_repo::PostgresSessionRepo;
use crate::adapters::postgres_user_repo::PostgresUserRepo;
use shared::ports::RateLimiter;
use crate::config::auth_settings::AuthSettings;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: PostgresUserRepo,
    pub session_repo: PostgresSessionRepo,
    pub auth_provider: GoogleAuthProvider,
    pub jwt_service: JwtServiceImpl,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub refresh_rate_limiter: Arc<dyn RateLimiter>,
    pub event_bus: EventBus,
    pub auth_settings: AuthSettings,
    pub trusted_proxy_ips: Vec<IpAddr>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_repo: PostgresUserRepo,
        session_repo: PostgresSessionRepo,
        auth_provider: GoogleAuthProvider,
        jwt_service: JwtServiceImpl,
        rate_limiter: Arc<dyn RateLimiter>,
        refresh_rate_limiter: Arc<dyn RateLimiter>,
        event_bus: EventBus,
        auth_settings: AuthSettings,
        trusted_proxy_ips: Vec<IpAddr>,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            auth_provider,
            jwt_service,
            rate_limiter,
            refresh_rate_limiter,
            event_bus,
            auth_settings,
            trusted_proxy_ips,
        }
    }
}
