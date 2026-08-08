use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use metrics::Counter;

use crate::access_token_blacklist::durable::DurableRevocationBlacklist;
use crate::access_token_blacklist_setup::AccessTokenBlacklistErrorCounter;
use crate::rate_limiter_setup::RateLimiterErrorCounters;

/// How often every metrics poller in this module samples its source. Shared so
/// the scrape-side resolution of these metrics is uniform, rather than each
/// poller picking its own interval.
const POLL_INTERVAL: Duration = Duration::from_secs(15);

/// Spawns a background task that, every 15 seconds, reads the shared Postgres
/// pool's current size/idle-connection counts and publishes them as
/// `db_pool_size` and `db_pool_idle` gauges to the installed Prometheus recorder
/// (see #100).
///
/// `PgPool::size`/`num_idle` are cheap, synchronous, in-memory reads (no query
/// against the database), so polling them costs nothing beyond the interval
/// tick itself. `db_pool_size - db_pool_idle` is the number of connections
/// currently checked out; that approaching `DB_MAX_CONNECTIONS` is the signal
/// for pool exhaustion this metric exists to make visible (previously there was
/// none -- a caller could only infer trouble indirectly, e.g. via
/// `DB_ACQUIRE_TIMEOUT_SECONDS` errors after the fact).
pub fn spawn_db_pool_metrics_poller(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        loop {
            interval.tick().await;

            metrics::gauge!("db_pool_size").set(pool.size() as f64);
            metrics::gauge!("db_pool_idle").set(pool.num_idle() as f64);
        }
    });
}

/// Pairs each backend's own cumulative error counter with the metric that
/// mirrors it.
///
/// The `Counter` handles are registered once here rather than re-resolved on
/// every tick -- that is exactly what holding a handle is for (see the
/// `metrics::counter!` docs).
///
/// A backend running in-memory contributes nothing: `MemoryRateLimiter` and
/// `MemoryAccessTokenBlacklist` have no network errors to observe, so their
/// counter is `None`.
fn error_counter_mirrors(
    rate_limiters: RateLimiterErrorCounters,
    blacklist: AccessTokenBlacklistErrorCounter,
) -> Vec<(Counter, Arc<AtomicU64>)> {
    let mut mirrors = Vec::new();

    if let Some(source) = rate_limiters.login {
        mirrors.push((
            metrics::counter!("rate_limiter_redis_errors_total", "scope" => "login"),
            source,
        ));
    }
    if let Some(source) = rate_limiters.refresh {
        mirrors.push((
            metrics::counter!("rate_limiter_redis_errors_total", "scope" => "refresh"),
            source,
        ));
    }
    if let Some(source) = blacklist.redis {
        mirrors.push((
            metrics::counter!("access_token_blacklist_redis_errors_total"),
            source,
        ));
    }

    mirrors
}

/// Spawns the single background task that mirrors every Redis-backed component's
/// fail-open error counter into the installed Prometheus recorder every 15
/// seconds: `rate_limiter_redis_errors_total{scope="login"|"refresh"}` and
/// `access_token_blacklist_redis_errors_total`.
///
/// Uses `Counter::absolute` (not `increment`) because each source is already the
/// cumulative total maintained independently inside `RedisRateLimiter` /
/// `RedisAccessTokenBlacklist` -- this task mirrors that value on an interval
/// rather than tracking its own delta.
///
/// One task for all of them, rather than one per component: the bodies were
/// identical apart from the metric name and labels, and a task that ticks every
/// 15 seconds to publish nothing is not worth spawning per backend. When every
/// component is on its in-memory backend there is nothing to mirror at all, so
/// no task is spawned.
pub fn spawn_backend_error_counter_poller(
    rate_limiters: RateLimiterErrorCounters,
    blacklist: AccessTokenBlacklistErrorCounter,
) {
    let mirrors = error_counter_mirrors(rate_limiters, blacklist);
    if mirrors.is_empty() {
        return;
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        loop {
            interval.tick().await;

            for (counter, source) in &mirrors {
                counter.absolute(source.load(Ordering::Relaxed));
            }
        }
    });
}

/// Spawns the durable-revocation flush worker: retries every journaled access
/// token revocation (`access_token_revocation_outbox`, see #140 and
/// `DurableRevocationBlacklist`) against Redis on an interval, publishing the
/// current backlog as `access_token_revocation_outbox_pending` after each sweep.
///
/// The first `tokio::time::interval` tick fires immediately, so any backlog that
/// accumulated while the process was down is retried right at startup, not after
/// the first full interval. `interval_secs` comes from
/// `REVOCATION_FLUSH_INTERVAL_SECONDS` and is clamped to a minimum of 1 second
/// (`interval` panics on a zero duration; a misconfigured 0 would otherwise kill
/// the task -- and this worker, not the request path, is the right thing to
/// protect here).
///
/// Unlike the pollers above this one does real work per tick, which is why it
/// keeps its own interval and its own missed-tick policy.
pub fn spawn_access_token_revocation_flusher(
    flusher: Arc<DurableRevocationBlacklist>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs.max(1)));
        // Default (Burst) replays every missed tick back-to-back with no delay
        // between them if a sweep ever runs longer than the interval. A large
        // outbox backlog is exactly the condition that makes a long sweep
        // likely, so Burst would pile consecutive sweeps against Postgres/Redis
        // right when they're already under the most load. Delay instead waits a
        // full interval after each sweep before the next one, regardless of how
        // long that sweep took.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;

            match flusher.flush_pending().await {
                Ok(remaining) => {
                    metrics::gauge!("access_token_revocation_outbox_pending").set(remaining as f64);
                }
                Err(e) => {
                    // Postgres was unreachable for the sweep itself. The gauge is
                    // left at its last known value rather than reset to 0, so a
                    // genuine backlog isn't hidden by a failed sweep.
                    tracing::error!(
                        error = %e,
                        "Access token revocation outbox flush failed (will retry on the next sweep)"
                    );
                }
            }
        }
    });
}
