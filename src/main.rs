use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::post;

use app_home_services::AppState;
use app_home_services::adapters::inbound::login_routes::login_password_handler;
use app_home_services::adapters::inbound::logout_routes::logout_handler;
use app_home_services::adapters::inbound::oauth_callback::login_google_handler;
use app_home_services::adapters::inbound::refresh_routes::refresh_token_handler;
use app_home_services::adapters::outbound::google_auth_provider::GoogleAuthProvider;
use app_home_services::adapters::outbound::jwt_service::JwtServiceImpl;
use app_home_services::adapters::outbound::memory_rate_limiter::MemoryRateLimiter;
use app_home_services::adapters::outbound::postgres_session_repo::PostgresSessionRepo;
use app_home_services::adapters::outbound::postgres_user_repo::PostgresUserRepo;
use app_home_services::adapters::outbound::redis_rate_limiter::RedisRateLimiter;
use app_home_services::application::ports::rate_limiter::RateLimiter;
use app_home_services::infrastructure::config::settings::Settings;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    app_home_services::infrastructure::telemetry::logging::init_logging();

    tracing::info!("Starting App Home Services");

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
    let session_repo = PostgresSessionRepo::new(pool);
    let auth_provider = GoogleAuthProvider::new(settings.google_client_id.clone());
    let jwt_service = JwtServiceImpl::new(
        &settings.jwt_secret,
        settings.access_token_expiry_minutes,
        settings.refresh_token_expiry_days,
    );

    // Rate limiter backends: Redis when REDIS_URL is configured (required for correct
    // rate limiting when running more than one instance of this service), otherwise
    // in-memory limiters (single instance only -- see MemoryRateLimiter's docs).
    //
    // Login and refresh each get their own limiter instance (and, on Redis, their own
    // key namespace) so attempts against one endpoint never consume the other's
    // attempt budget for the same IP.
    let (rate_limiter, refresh_rate_limiter): (Arc<dyn RateLimiter>, Arc<dyn RateLimiter>) =
        match &settings.redis_url {
            Some(redis_url) => {
                let login_limiter = RedisRateLimiter::connect(
                    redis_url,
                    settings.rate_limit_max_attempts,
                    settings.rate_limit_window_seconds,
                    "login",
                )
                .await
                .expect("Failed to connect to Redis for login rate limiting");
                let refresh_limiter = RedisRateLimiter::connect(
                    redis_url,
                    settings.rate_limit_max_attempts,
                    settings.rate_limit_window_seconds,
                    "refresh",
                )
                .await
                .expect("Failed to connect to Redis for refresh rate limiting");
                tracing::info!("Rate limiting backend: Redis (shared across instances)");
                (Arc::new(login_limiter), Arc::new(refresh_limiter))
            }
            None => {
                tracing::info!(
                    "Rate limiting backend: in-memory (REDIS_URL not set -- only safe for a single instance)"
                );
                (
                    Arc::new(MemoryRateLimiter::new(
                        settings.rate_limit_max_attempts,
                        settings.rate_limit_window_seconds,
                    )),
                    Arc::new(MemoryRateLimiter::new(
                        settings.rate_limit_max_attempts,
                        settings.rate_limit_window_seconds,
                    )),
                )
            }
        };

    let addr = format!("{}:{}", settings.server_host, settings.server_port);

    let state = AppState::new(
        user_repo,
        session_repo,
        auth_provider,
        jwt_service,
        rate_limiter,
        refresh_rate_limiter,
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
        .route("/api/health", axum::routing::get(health_check))
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

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"status": "ok"}))
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
