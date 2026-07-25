use std::time::Duration;

use shared::config::settings::Settings;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Creates the shared Postgres connection pool for the whole service, using the
/// pool-tuning fields on `Settings` (all configurable via env vars -- see
/// `.env.example` for defaults and guidance).
///
/// This pool is meant to be created exactly once, at startup, and shared (via
/// `PgPool::clone`, which is cheap -- it clones an internal `Arc`) across every
/// bounded context's repositories. See `docs/adr/0001-modular-monolith.md` and
/// `docs/modules/*.md` for how each context uses it.
pub async fn create_pool(settings: &Settings) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(settings.db_max_connections)
        .min_connections(settings.db_min_connections)
        // Bounds how long a caller waits for a connection when the pool is fully
        // checked out, turning pool exhaustion into a fast, explicit error instead
        // of a request hanging indefinitely (see the PoolTimedOut incident in PR
        // #26, which motivated making this tunable rather than relying on sqlx's
        // implicit default).
        .acquire_timeout(Duration::from_secs(settings.db_acquire_timeout_seconds))
        .idle_timeout(non_zero_duration(settings.db_idle_timeout_seconds))
        .max_lifetime(non_zero_duration(settings.db_max_lifetime_seconds))
        .connect(&settings.database_url)
        .await
}

/// Converts a "0 means disabled" seconds value into the `Option<Duration>` sqlx's
/// pool options expect (`None` = no idle/lifetime-based recycling).
fn non_zero_duration(seconds: u64) -> Option<Duration> {
    if seconds == 0 {
        None
    } else {
        Some(Duration::from_secs(seconds))
    }
}
