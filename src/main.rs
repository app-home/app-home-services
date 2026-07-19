use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::routing::{get, post};
use utoipa::OpenApi;

use app_home_services::AppState;
use app_home_services::adapters::inbound::api_doc::ApiDoc;
use app_home_services::adapters::inbound::health_routes::health_check;
use app_home_services::adapters::inbound::login_routes::login_password_handler;
use app_home_services::adapters::inbound::logout_routes::logout_handler;
use app_home_services::adapters::inbound::oauth_callback::login_google_handler;
use app_home_services::adapters::inbound::refresh_routes::refresh_token_handler;
use app_home_services::adapters::outbound::audit_event_handler::AuditEventHandler;
use app_home_services::adapters::outbound::google_auth_provider::GoogleAuthProvider;
use app_home_services::adapters::outbound::jwt_service::JwtServiceImpl;
use app_home_services::adapters::outbound::postgres_session_repo::PostgresSessionRepo;
use app_home_services::adapters::outbound::postgres_user_repo::PostgresUserRepo;
use app_home_services::infrastructure::config::settings::Settings;
use shared::event_bus::EventBus;
use app_home_services::infrastructure::rate_limiter_setup::{
    RateLimiterErrorCounters, build_rate_limiters,
};
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

    let pool = app_home_services::infrastructure::database::db::create_pool(&settings.database_url)
        .await
        .expect("Failed to create database pool");

    app_home_services::infrastructure::database::db::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    if let Err(e) = seed_default_user(&pool, &settings).await {
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

    let user_repo = PostgresUserRepo::new(pool.clone());
    let session_repo = PostgresSessionRepo::new(pool.clone());

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
    let auth_provider = GoogleAuthProvider::new(settings.google_client_id.clone());
    let jwt_service = JwtServiceImpl::new(
        &settings.jwt_secret,
        settings.access_token_expiry_minutes,
        settings.refresh_token_expiry_days,
    );

    // See build_rate_limiters' docs for why REDIS_URL selects the backend, and why
    // this is a fatal startup error (rather than a silent fallback) when REDIS_URL is
    // set but Redis is unreachable.
    let (rate_limiter, refresh_rate_limiter, rate_limiter_error_counters) =
        build_rate_limiters(&settings)
            .await
            .expect("Failed to set up rate limiters");

    spawn_rate_limiter_metrics_poller(rate_limiter_error_counters);

    if settings.server_host == "0.0.0.0" {
        tracing::warn!(
            "Binding to 0.0.0.0 exposes the /metrics endpoint (no auth) and all API routes on every network interface; set SERVER_HOST=127.0.0.1 if this is unintended"
        );
    }

    let addr = format!("{}:{}", settings.server_host, settings.server_port);

    let state = AppState::new(
        user_repo,
        session_repo,
        auth_provider,
        jwt_service,
        rate_limiter,
        refresh_rate_limiter,
        event_bus,
        settings,
    );

    let cors = {
        let origins_str = &state.settings.cors_allowed_origins;
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

    let app = axum::Router::new()
        .route("/api/auth/login/password", post(login_password_handler))
        .route("/api/auth/login/google", post(login_google_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/refresh", post(refresh_token_handler))
        .route("/api/health", get(health_check))
        // Not gated behind auth: Prometheus scrape endpoints are conventionally
        // reached only from inside a private network / the cluster's monitoring
        // namespace, never exposed publicly. If this service is ever reachable from
        // the public internet without a network boundary in front of it, this route
        // should not be exposed as-is (see .env.example's CORS/proxy notes for the
        // service's general public-exposure assumptions).
        .route(
            "/metrics",
            get(move || std::future::ready(metrics_handle.render())),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors)
        .with_state(state);

    tracing::info!(address = %addr, "Listening");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    // `into_make_service_with_connect_info` exposes the real TCP peer address to
    // extractors (`ConnectInfo<SocketAddr>`), which the login and refresh handlers use
    // to safely resolve the client IP for rate limiting (see `resolve_client_ip`).
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

async fn seed_default_user(pool: &sqlx::PgPool, settings: &Settings) -> Result<bool, String> {
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
