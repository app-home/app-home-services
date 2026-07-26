# Module: Infrastructure (`crates/infrastructure/`)

## Purpose

Cross-cutting infrastructure services consumed by all bounded contexts. Provides database pool creation, telemetry (logging + metrics), and rate limiter setup. Re-exports `shared::config::settings::Settings`. Has no domain logic or HTTP endpoints of its own.

## Dependencies

| Crate | Role |
|-------|------|
| `shared` | `Settings` config, `RateLimiter` trait |

## Modules

### `database`

```rust
pub async fn create_pool(settings: &Settings) -> Result<PgPool, sqlx::Error>
```

PostgreSQL connection pool, tuned from `Settings`' `db_*` fields (all configurable via env vars, all optional with defaults -- see Configuration below): `db_max_connections` (default 10), `db_min_connections` (default 0), `db_acquire_timeout_seconds` (default 30), `db_idle_timeout_seconds` (default 600, `0` = disabled), `db_max_lifetime_seconds` (default 1800, `0` = disabled). Called once at startup in `main.rs`; the resulting `PgPool` is cloned (cheap -- it wraps an internal `Arc`) into every bounded context's repositories, and separately into the router as an `Extension` for `/api/health`'s connectivity check.

### `config`

Re-exports `shared::config::settings::Settings` — no additional logic.

### `telemetry`

**Logging** (`telemetry::logging`):
- `init_logging()` — `tracing-subscriber` with `EnvFilter` from `RUST_LOG` (default: `info`)

**Metrics** (`telemetry::metrics`):
- `install_prometheus_recorder()` — Installs Prometheus recorder, returns `PrometheusHandle` used by `GET /metrics`
- Custom metric: `rate_limiter_redis_errors_total{scope="login", scope="refresh"}` — polled every 15 seconds

### `rate_limiter`

| Adapter | Use Case |
|---------|----------|
| `MemoryRateLimiter` | In-memory, single-instance only (no shared state between instances) |
| `RedisRateLimiter` | Redis-backed, multi-instance, fail-open on network errors |

**Redis fail-open behaviour**: if Redis is reachable at startup but fails later, `check()` returns `true` (allows request) — blocking all users due to a transient Redis error would be worse than temporarily disabling rate limiting.

### `rate_limiter_setup`

```rust
pub async fn build_rate_limiters(settings: &Settings)
    -> Result<(Arc<dyn RateLimiter>, Arc<dyn RateLimiter>, RateLimiterErrorCounters)>
```

- Login and refresh each get their own limiter (separate key namespace)
- `REDIS_URL` unset → `MemoryRateLimiter` (single-instance safe)
- `REDIS_URL` set → `RedisRateLimiter` (startup error if Redis unreachable)
- Returns `RateLimiterErrorCounters` — shared `AtomicU64` handles for Redis error polling

## External Docs

- [Redis Security & TLS](../redis-security.md) — Redis password auth, TLS decision
- [Alerting](../alerting.md) — Prometheus metrics for `rate_limiter_redis_errors_total`

## Configuration

`Settings` (owned by `shared`, re-exported here):

| Env Var | Default | Description |
|---------|---------|-------------|
| `DATABASE_URL` | **required** | PostgreSQL connection string |
| `DB_MAX_CONNECTIONS` | `10` | Max pool connections per instance. `N * DB_MAX_CONNECTIONS` across N instances must stay under Postgres's own `max_connections`. |
| `DB_MIN_CONNECTIONS` | `0` | Idle connections the pool tries to keep pre-warmed |
| `DB_ACQUIRE_TIMEOUT_SECONDS` | `30` | Max wait for a pool connection before failing fast |
| `DB_IDLE_TIMEOUT_SECONDS` | `600` | Idle connection recycling; `0` disables it |
| `DB_MAX_LIFETIME_SECONDS` | `1800` | Forced connection recycling regardless of activity; `0` disables it |
| `SERVER_HOST` | `127.0.0.1` | Bind address |
| `SERVER_PORT` | `3000` | Bind port |
| `RATE_LIMIT_MAX_ATTEMPTS` | `10` | Max attempts per window |
| `RATE_LIMIT_WINDOW_SECONDS` | `300` | Window duration |
| `CORS_ALLOWED_ORIGINS` | (empty) | Comma-separated; empty = same-origin only |
| `TRUSTED_PROXY_IPS` | (empty) | Comma-separated IPs for X-Forwarded-For trust |
| `REDIS_URL` | (optional) | If set, uses `RedisRateLimiter`; unset = `MemoryRateLimiter` |

## Integration

In `main.rs`:

```rust
// Logging
app_home_services::infrastructure::telemetry::logging::init_logging();

// Metrics
let metrics_handle = infrastructure::telemetry::metrics::install_prometheus_recorder();

// Database pool (tuned from settings' db_* fields)
let pool = infrastructure::database::create_pool(&settings).await?;

// Rate limiters
let (rate_limiter, refresh_rate_limiter, counters) =
    infrastructure::rate_limiter_setup::build_rate_limiters(&settings).await?;

// /metrics route (Prometheus scrape endpoint)
app.route("/metrics", get(move || std::future::ready(metrics_handle.render())))

// /api/health needs its own handle to the pool to run its connectivity check
app.layer(Extension(pool.clone()))
```
