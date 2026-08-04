use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;

use std::sync::Arc;

use axum::{
    Extension,
    routing::{get, post, put},
};
use shared::auth::JwtVerification;
use utoipa::OpenApi;

use admin::adapters::inbound::admin_routes::{
    get_user_handler, list_users_handler, update_user_role_handler,
};
use admin::adapters::outbound::postgres_admin_repo::PostgresAdminRepo;
use admin::application::ports::admin_repository::AdminRepository;
use app_home_services::api_doc::ApiDoc;
use app_home_services::health::health_check;
use app_home_services::infrastructure::access_token_blacklist::durable::DurableRevocationBlacklist;
use app_home_services::infrastructure::access_token_blacklist_setup::{
    AccessTokenBlacklistErrorCounter, build_access_token_blacklist,
};
use app_home_services::infrastructure::config::Settings;
use app_home_services::infrastructure::metrics_guard::{MetricsGuardConfig, metrics_ip_allowlist};
use app_home_services::infrastructure::rate_limiter_setup::{
    RateLimiterErrorCounters, build_rate_limiters,
};
use auth::adapters::audit_event_handler::AuditEventHandler;
use auth::adapters::google_auth_provider::GoogleAuthProvider;
use auth::adapters::inbound::login_routes::login_password_handler;
use auth::adapters::inbound::logout_routes::logout_handler;
use auth::adapters::inbound::oauth_callback::login_google_handler;
use auth::adapters::inbound::refresh_routes::refresh_token_handler;
use auth::adapters::jwt_service::JwtServiceImpl;
use auth::adapters::postgres_session_repo::PostgresSessionRepo;
use auth::adapters::postgres_user_directory::PostgresUserDirectory;
use auth::adapters::postgres_user_repo::PostgresUserRepo;
use auth::config::auth_settings::AuthSettings;
use profiles::adapters::inbound::profile_routes::{get_profile_handler, update_profile_handler};
use profiles::adapters::outbound::postgres_profile_repo::PostgresProfileRepo;
use profiles::application::ports::profile_repository::ProfileRepository;
use shared::event_bus::EventBus;
use shared::user_directory::UserDirectory;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    app_home_services::infrastructure::telemetry::logging::init_logging();

    tracing::info!("Starting App Home Services");

    // Installed once, up front, before anything below records a metric -- the
    // metrics::counter!/gauge! macros are no-ops until a recorder is installed.
    let metrics_handle =
        app_home_services::infrastructure::telemetry::metrics::install_prometheus_recorder();

    let settings = Settings::from_env().expect("Failed to load settings");
    let auth_settings = AuthSettings::from_env().expect("Failed to load auth settings");

    let pool = app_home_services::infrastructure::database::create_pool(&settings)
        .await
        .expect("Failed to create database pool");

    run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    if let Err(e) = seed_default_user(&pool, &auth_settings).await {
        tracing::error!(error = %e, "Default user check failed");
        std::process::exit(1);
    }

    if settings.trusted_proxy_ips.is_empty() {
        tracing::info!(
            "TRUSTED_PROXY_IPS not configured: X-Forwarded-For/X-Real-IP will be ignored, rate limiting uses the direct peer address"
        );
    } else {
        tracing::info!(
            trusted_proxies = ?settings.trusted_proxy_ips,
            "Trusted reverse proxies configured"
        );
    }

    if settings.metrics_allowed_ips.is_empty() {
        tracing::info!(
            "METRICS_ALLOWED_IPS not configured: /metrics is reachable by anything that can reach this process's port"
        );
    } else {
        tracing::info!(
            allowed = ?settings.metrics_allowed_ips,
            "/metrics restricted to an IP allowlist (plus loopback, always allowed)"
        );
    }

    if settings.enable_swagger {
        tracing::info!(
            "ENABLE_SWAGGER=true: serving Swagger UI at /swagger-ui and the OpenAPI spec at /api-docs/openapi.json"
        );
    } else {
        tracing::info!(
            "ENABLE_SWAGGER unset/false: /swagger-ui and /api-docs/openapi.json are disabled (no API surface exposure)"
        );
    }

    let user_repo = PostgresUserRepo::new(pool.clone());
    let session_repo = PostgresSessionRepo::new(pool.clone());
    // Coerced to `Arc<dyn ...>` so the Extension key matches what the profile and
    // admin handlers extract -- `Extension<Arc<ConcreteRepo>>` would be a
    // different key than `Extension<Arc<dyn Repo>>` and the routes would 500 with
    // "Missing request extension".
    let profile_repo: Arc<dyn ProfileRepository> = Arc::new(PostgresProfileRepo::new(pool.clone()));

    // `admin` depends only on the `UserDirectory` port (defined in `shared`) for user
    // identity, not on the `auth` crate or its `users` table directly -- this is the
    // composition root wiring the concrete `auth`-owned implementation in. See
    // docs/adr/0001-modular-monolith.md for why this replaced admin's previous direct
    // SQL access to `users`.
    let user_directory: Arc<dyn UserDirectory> = Arc::new(PostgresUserDirectory::new(pool.clone()));
    let admin_repo: Arc<dyn AdminRepository> =
        Arc::new(PostgresAdminRepo::new(pool.clone(), user_directory));

    let (event_bus, mut event_rx) = EventBus::new(256);
    let audit_handler = AuditEventHandler::new(pool.clone());

    tokio::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match event_rx.recv().await {
                Ok(event) => audit_handler.handle(event).await,
                Err(RecvError::Closed) => {
                    tracing::warn!("Event bus closed");
                    break;
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = %n, "Event bus receiver lagged");
                }
            }
        }
    });
    let auth_provider = GoogleAuthProvider::new(auth_settings.google_client_id.clone());
    let jwt_service = JwtServiceImpl::new(
        &auth_settings.jwt_secret,
        auth_settings.access_token_expiry_minutes,
        auth_settings.refresh_token_expiry_days,
        &auth_settings.jwt_issuer,
        &auth_settings.jwt_audience,
    );

    // See build_rate_limiters' docs for why REDIS_URL selects the backend, and why
    // this is a fatal startup error (rather than a silent fallback) when REDIS_URL is
    // set but Redis is unreachable.
    let (rate_limiter, refresh_rate_limiter, rate_limiter_error_counters) =
        build_rate_limiters(&settings)
            .await
            .expect("Failed to set up rate limiters");

    spawn_rate_limiter_metrics_poller(rate_limiter_error_counters);

    // See build_access_token_blacklist's docs for why REDIS_URL selects the
    // backend, and why (unlike the rate limiters) an unreachable Redis falls back
    // to in-memory at startup rather than aborting -- the blacklist check fails
    // open anyway (#88). On the Redis backend, revocations that Redis rejects are
    // journaled in Postgres and flushed by a background worker (see #140).
    let (access_token_blacklist, blacklist_error_counter, revocation_flusher) =
        build_access_token_blacklist(&settings, &pool).await;

    spawn_access_token_blacklist_metrics_poller(blacklist_error_counter);

    if let Some(flusher) = revocation_flusher {
        spawn_access_token_revocation_flusher(flusher, settings.revocation_flush_interval_seconds);
    }

    // Single JWT verification config (secret + iss/aud) shared by every
    // protected route's `AuthenticatedUser` extractor. Enforcing a non-default
    // issuer/audience rejects tokens minted in another environment that shares
    // the same JWT_SECRET -- see #87.
    let verification = Arc::new(JwtVerification::new(
        &auth_settings.jwt_secret,
        auth_settings.jwt_issuer.clone(),
        auth_settings.jwt_audience.clone(),
    ));

    if settings.server_host == "0.0.0.0" {
        tracing::warn!(
            "Binding to 0.0.0.0 exposes all API routes on every network interface; set SERVER_HOST=127.0.0.1 if this is unintended. /metrics specifically can additionally be restricted via METRICS_ALLOWED_IPS."
        );
    }

    let addr = format!("{}:{}", settings.server_host, settings.server_port);

    let health_check_pool = pool.clone();

    let metrics_guard_config = MetricsGuardConfig {
        allowed_ips: settings.metrics_allowed_ips.clone(),
        trusted_proxy_ips: settings.trusted_proxy_ips.clone(),
    };

    let state = auth::AppState::new(
        user_repo,
        session_repo,
        auth_provider,
        jwt_service,
        rate_limiter,
        refresh_rate_limiter,
        event_bus,
        auth_settings,
        settings.trusted_proxy_ips.clone(),
    );

    let cors = {
        let origins_str = &settings.cors_allowed_origins;
        if origins_str.is_empty() {
            tracing::info!("CORS: same-origin only (no origins configured)");
            tower_http::cors::CorsLayer::new().allow_origin(tower_http::cors::AllowOrigin::list(
                Vec::<axum::http::HeaderValue>::new(),
            ))
        } else {
            let origins: Vec<axum::http::HeaderValue> = origins_str
                .split(',')
                .filter_map(|o| o.trim().parse::<axum::http::HeaderValue>().ok())
                .collect();
            tracing::info!(?origins, "CORS: configured origins");
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::list(origins))
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                ])
        }
    };

    // Kept as its own sub-router (merged below) rather than a plain `.route()` on the
    // main router, so the IP allowlist middleware/Extension only ever apply to
    // `/metrics` -- not to every other route on the service.
    let metrics_router = axum::Router::new()
        .route(
            "/metrics",
            get(move || std::future::ready(metrics_handle.render())),
        )
        .layer(axum::middleware::from_fn(metrics_ip_allowlist))
        .layer(Extension(metrics_guard_config));

    let mut app = axum::Router::new()
        .route("/api/auth/login/password", post(login_password_handler))
        .route("/api/auth/login/google", post(login_google_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/refresh", post(refresh_token_handler))
        .route("/api/health", get(health_check))
        .route(
            "/api/profile",
            get(get_profile_handler).put(update_profile_handler),
        )
        .route("/api/admin/users", get(list_users_handler))
        .route("/api/admin/users/{id}", get(get_user_handler))
        .route("/api/admin/users/{id}/role", put(update_user_role_handler))
        .layer(Extension(profile_repo))
        .layer(Extension(admin_repo))
        .layer(Extension(verification))
        // Shared access token revocation list: every protected route's
        // `AuthenticatedUser` extractor rejects tokens whose `jti` was revoked
        // (e.g. at logout, see #88), and the logout handler itself uses it to
        // revoke the presented token.
        .layer(Extension(access_token_blacklist))
        // /api/health runs a real `SELECT 1` against the pool (see src/health.rs),
        // so it needs its own handle to it -- this clone is cheap (PgPool wraps an
        // Arc internally), not a second pool.
        .layer(Extension(health_check_pool))
        // Prometheus scrape endpoints are conventionally reached only from inside a
        // private network / the cluster's monitoring namespace, never exposed
        // publicly. `/metrics` is still unauthenticated (no credentials required),
        // but is now additionally gated by an IP allowlist when METRICS_ALLOWED_IPS
        // is configured -- see crates/infrastructure/src/metrics_guard.rs and #83.
        .merge(metrics_router);

    // Swagger UI and the OpenAPI spec are only registered when explicitly
    // enabled (ENABLE_SWAGGER=true) -- see #86. Without the flag both routes
    // return 404, so a publicly reachable instance exposes no API surface via
    // docs. `ApiDoc::openapi()` is a generated static spec, so this conditional
    // has no runtime cost beyond an already-generated constant.
    if settings.enable_swagger {
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }

    // HTTP security headers (see #90). Emitted unconditionally on every response:
    // HSTS is inert over plain HTTP (browsers only process it on HTTPS responses),
    // so it is safe to send even when TLS is terminated by a reverse proxy ahead
    // of this service. CSP is deliberately NOT set: this service renders no HTML
    // except the Swagger UI when ENABLE_SWAGGER=true (which relies on inline
    // scripts/CDN assets), so a strict CSP would break it without adding value.
    let app = app
        .layer(cors)
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .with_state(state);

    tracing::info!(address = %addr, "Listening");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    // `into_make_service_with_connect_info` exposes the real TCP peer address to
    // extractors (`ConnectInfo<SocketAddr>`), which the login and refresh handlers use
    // to safely resolve the client IP for rate limiting (see `resolve_client_ip`), and
    // which the `/metrics` IP allowlist guard uses the same way.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}

/// Spawns a background task that, every 15 seconds, reads each rate limiter's Redis
/// error counter (if it has one -- see `RateLimiterErrorCounters`) and publishes it
/// as `rate_limiter_redis_errors_total{scope="login"|"refresh"}` to the installed
/// Prometheus recorder.
///
/// Uses `Counter::absolute` (not `increment`) since `counter` is already the
/// cumulative total maintained independently inside `RedisRateLimiter` -- this task
/// just mirrors that value into the metrics recorder on an interval, rather than
/// tracking its own delta.
///
/// A no-op for a scope currently on the in-memory backend (`counters.login`/`refresh`
/// is `None`), since there's nothing to poll there.
fn spawn_rate_limiter_metrics_poller(counters: RateLimiterErrorCounters) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;

            if let Some(counter) = &counters.login {
                let value = counter.load(Ordering::Relaxed);
                metrics::counter!("rate_limiter_redis_errors_total", "scope" => "login")
                    .absolute(value);
            }
            if let Some(counter) = &counters.refresh {
                let value = counter.load(Ordering::Relaxed);
                metrics::counter!("rate_limiter_redis_errors_total", "scope" => "refresh")
                    .absolute(value);
            }
        }
    });
}

/// Spawns a background task that, every 15 seconds, reads the access token
/// blacklist's Redis error counter (if it has one -- see
/// `AccessTokenBlacklistErrorCounter`) and publishes it as
/// `access_token_blacklist_redis_errors_total` to the installed Prometheus
/// recorder.
///
/// Mirrors `spawn_rate_limiter_metrics_poller` (same `absolute`, not `increment`,
/// since the counter is already the cumulative total held inside
/// `RedisAccessTokenBlacklist`). A no-op on the in-memory backend.
fn spawn_access_token_blacklist_metrics_poller(counter: AccessTokenBlacklistErrorCounter) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;

            if let Some(counter) = &counter.redis {
                let value = counter.load(Ordering::Relaxed);
                metrics::counter!("access_token_blacklist_redis_errors_total").absolute(value);
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
fn spawn_access_token_revocation_flusher(
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

async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

async fn seed_default_user(
    pool: &sqlx::PgPool,
    settings: &auth::config::auth_settings::AuthSettings,
) -> Result<bool, String> {
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE auth_provider = 'local'")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("database query failed: {e}"))?;

    if count > 0 {
        tracing::info!(username = %settings.default_user_username, "Default user already exists");
        return Ok(true);
    }

    let password_hash = bcrypt::hash(&settings.default_user_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("password hashing failed: {e}"))?;

    let id = uuid::Uuid::now_v7();
    let now = chrono::Utc::now();

    sqlx::query(
        r#"INSERT INTO users (id, username, email, display_name, password_hash, auth_provider, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, 'local', $6, $7)
        ON CONFLICT (username) DO NOTHING"#,
    )
    .bind(id)
    .bind(&settings.default_user_username)
    .bind(&settings.default_user_email)
    .bind("Administrator")
    .bind(&password_hash)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| format!("default user insert failed: {e}"))?;

    tracing::info!(username = %settings.default_user_username, "Default user created successfully");
    Ok(false)
}
