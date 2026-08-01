use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use shared::ports::AccessTokenBlacklist;

use crate::access_token_blacklist::memory::MemoryAccessTokenBlacklist;
use crate::access_token_blacklist::redis::RedisAccessTokenBlacklist;
use crate::config::Settings;

/// Shareable handle to the Redis-backed blacklist's fail-open error counter, for
/// polling into a metrics exporter (see `infrastructure::telemetry::metrics` and
/// the `/metrics` route wired up in `main`).
///
/// `None` when running on the in-memory backend (`REDIS_URL` unset, or Redis
/// unreachable at startup), since `MemoryAccessTokenBlacklist` has no network
/// errors to observe.
#[derive(Clone, Default)]
pub struct AccessTokenBlacklistErrorCounter {
    pub redis: Option<Arc<AtomicU64>>,
}

/// Chooses and constructs the access token revocation list backend based on
/// `settings.redis_url`: Redis-backed when set (required for revocation to be
/// shared across instances), otherwise in-memory (single instance only -- see
/// `MemoryAccessTokenBlacklist`'s docs).
///
/// Unlike `build_rate_limiters`, a *set* but unreachable `REDIS_URL` is NOT a
/// fatal startup error here: the blacklist check fails open by design, so the
/// service logs a warning and falls back to in-memory rather than refusing to
/// start. See the `AccessTokenBlacklist` trait docs and #88.
///
/// Returns `AccessTokenBlacklistErrorCounter` so a handle to the Redis error
/// counter can be captured before the concrete type is erased into
/// `Arc<dyn AccessTokenBlacklist>`.
pub async fn build_access_token_blacklist(
    settings: &Settings,
) -> (
    Arc<dyn AccessTokenBlacklist>,
    AccessTokenBlacklistErrorCounter,
) {
    match &settings.redis_url {
        Some(redis_url) => match RedisAccessTokenBlacklist::connect(redis_url).await {
            Ok(blacklist) => {
                let error_counter = AccessTokenBlacklistErrorCounter {
                    redis: Some(blacklist.error_counter_handle()),
                };
                tracing::info!("Access token revocation backend: Redis (shared across instances)");
                (Arc::new(blacklist), error_counter)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Redis unavailable for access token blacklist, falling back to in-memory (single instance only)"
                );
                (
                    Arc::new(MemoryAccessTokenBlacklist::new()),
                    AccessTokenBlacklistErrorCounter::default(),
                )
            }
        },
        None => {
            tracing::info!(
                "Access token revocation backend: in-memory (REDIS_URL not set -- only safe for a single instance)"
            );
            (
                Arc::new(MemoryAccessTokenBlacklist::new()),
                AccessTokenBlacklistErrorCounter::default(),
            )
        }
    }
}
