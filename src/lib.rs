pub mod adapters;
pub mod application;
pub mod domain;
pub mod infrastructure;

use adapters::outbound::google_auth_provider::GoogleAuthProvider;
use adapters::outbound::postgres_user_repo::PostgresUserRepo;
use infrastructure::config::settings::Settings;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: PostgresUserRepo,
    pub auth_provider: GoogleAuthProvider,
    pub settings: Settings,
}

impl AppState {
    pub fn new(
        user_repo: PostgresUserRepo,
        auth_provider: GoogleAuthProvider,
        settings: Settings,
    ) -> Self {
        Self {
            user_repo,
            auth_provider,
            settings,
        }
    }
}
