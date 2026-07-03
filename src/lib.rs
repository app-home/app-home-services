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

#[derive(Clone)]
pub struct AppState {
    pub user_repo: PostgresUserRepo,
    pub session_repo: PostgresSessionRepo,
    pub auth_provider: GoogleAuthProvider,
    pub jwt_service: JwtServiceImpl,
    /// Rate limiter backend, chosen at startup based on configuration: `RedisRateLimiter`
    /// when `REDIS_URL` is set (safe for multi-instance deployments), otherwise
    /// `MemoryRateLimiter` (single-instance only). See `main.rs`.
    pub rate_limiter: Arc<dyn RateLimiter>,
    pub settings: Settings,
}

impl AppState {
    pub fn new(
        user_repo: PostgresUserRepo,
        session_repo: PostgresSessionRepo,
        auth_provider: GoogleAuthProvider,
        jwt_service: JwtServiceImpl,
        rate_limiter: Arc<dyn RateLimiter>,
        settings: Settings,
    ) -> Self {
        Self {
            user_repo,
            session_repo,
            auth_provider,
            jwt_service,
            rate_limiter,
            settings,
        }
    }
}
