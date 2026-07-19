use std::sync::Arc;

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infrastructure;

use adapters::outbound::google_auth_provider::GoogleAuthProvider;
use adapters::outbound::jwt_service::JwtServiceImpl;
use adapters::outbound::postgres_session_repo::PostgresSessionRepo;
use adapters::outbound::postgres_user_repo::PostgresUserRepo;
use application::ports::rate_limiter::RateLimiter;
use infrastructure::config::settings::Settings;
use shared::event_bus::EventBus;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: PostgresUserRepo,
    pub session_repo: PostgresSessionRepo,
    pub auth_provider: GoogleAuthProvider,
    pub jwt_service: JwtServiceImpl,
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub refresh_rate_limiter: Arc<dyn RateLimiter>,
    pub event_bus: EventBus,
    pub settings: Settings,
}

impl AppState {
    pub fn new(
        user_repo: PostgresUserRepo,
        session_repo: PostgresSessionRepo,
        auth_provider: GoogleAuthProvider,
        jwt_service: JwtServiceImpl,
        rate_limiter: Arc<dyn RateLimiter>,
        refresh_rate_limiter: Arc<dyn RateLimiter>,
        event_bus: EventBus,
        settings: Settings,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            auth_provider,
            jwt_service,
            rate_limiter,
            refresh_rate_limiter,
            event_bus,
            settings,
        }
    }
}
