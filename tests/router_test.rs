//! In-process tests for the application router (see #191).
//!
//! These call `build_router` directly and drive it with
//! `tower::ServiceExt::oneshot`: no server is started, no port is bound and no
//! query ever reaches Postgres. That is the whole point of extracting the router
//! out of `main` -- the equivalent assertions in `tests/integration/` are
//! `#[ignore]`d and require a manually-started server on `localhost:3000`, so
//! they never run in CI. These do.
//!
//! Scope is deliberately router-level only: which routes exist, and what the
//! layers wrapped around them do. Handler behaviour that needs a real database
//! stays in the integration suite.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use metrics_exporter_prometheus::PrometheusBuilder;
use tower::ServiceExt;

use admin::adapters::outbound::postgres_admin_repo::PostgresAdminRepo;
use admin::application::ports::admin_repository::AdminRepository;
use app_home_services::router::{RouterDeps, build_router};
use auth::adapters::google_auth_provider::GoogleAuthProvider;
use auth::adapters::jwt_service::JwtServiceImpl;
use auth::adapters::postgres_session_repo::PostgresSessionRepo;
use auth::adapters::postgres_user_directory::PostgresUserDirectory;
use auth::adapters::postgres_user_repo::PostgresUserRepo;
use auth::config::auth_settings::AuthSettings;
use auth::domain::services::bcrypt_task::BcryptLimiter;
use infrastructure::access_token_blacklist::memory::MemoryAccessTokenBlacklist;
use infrastructure::metrics_guard::MetricsGuardConfig;
use infrastructure::rate_limiter::memory::MemoryRateLimiter;
use profiles::adapters::outbound::postgres_profile_repo::PostgresProfileRepo;
use profiles::application::ports::profile_repository::ProfileRepository;
use shared::auth::JwtVerification;
use shared::event_bus::EventBus;
use shared::ports::{AccessTokenBlacklist, RateLimiter};
use shared::user_directory::UserDirectory;

/// Lowest cost bcrypt accepts. These tests never hash anything -- `AuthSettings`
/// is only here because `AppState` requires one -- so the cost is irrelevant
/// beyond being valid.
const TEST_BCRYPT_COST: u32 = 4;
const TEST_BCRYPT_MAX_CONCURRENT: usize = 2;

/// Knobs the individual tests vary. Everything else about the router is fixed.
///
/// The derived default is the service's own default posture: Swagger off, no
/// CORS origins, no `/metrics` allowlist.
#[derive(Default)]
struct RouterOptions {
    enable_swagger: bool,
    cors_allowed_origins: Vec<&'static str>,
    metrics_allowed_ips: Vec<IpAddr>,
}

/// Builds the real production router with in-memory adapters where a backend
/// would otherwise be needed.
///
/// The `PgPool` comes from `connect_lazy`, which parses the URL but opens no
/// connection until a query actually runs. Nothing these tests request reaches a
/// query, so the unreachable host in the URL is never contacted -- the repos
/// exist purely to satisfy `AppState`'s and the `Extension`s' types.
fn router(options: RouterOptions) -> Router {
    let pool = sqlx::PgPool::connect_lazy("postgres://router-test:router-test@127.0.0.1:1/none")
        .expect("connect_lazy should accept a well-formed URL without connecting");

    let auth_settings = AuthSettings {
        default_user_username: "admin".to_string(),
        default_user_password: "irrelevant".to_string(),
        default_user_email: "admin@example.com".to_string(),
        google_client_id: String::new(),
        jwt_secret: "irrelevant-but-long-enough-for-the-tests".to_string(),
        jwt_issuer: "app-home-services".to_string(),
        jwt_audience: "app-home-services".to_string(),
        access_token_expiry_minutes: 15,
        refresh_token_expiry_days: 7,
        bcrypt_cost: TEST_BCRYPT_COST,
        bcrypt_max_concurrent: TEST_BCRYPT_MAX_CONCURRENT,
        bcrypt_limiter: BcryptLimiter::new(TEST_BCRYPT_MAX_CONCURRENT),
    };

    let rate_limiter: Arc<dyn RateLimiter> = Arc::new(MemoryRateLimiter::new(10, 300));
    let refresh_rate_limiter: Arc<dyn RateLimiter> = Arc::new(MemoryRateLimiter::new(10, 300));
    let (event_bus, _event_rx) = EventBus::new(256);

    let state = auth::AppState::new(
        PostgresUserRepo::new(pool.clone()),
        PostgresSessionRepo::new(pool.clone()),
        GoogleAuthProvider::new(String::new()),
        JwtServiceImpl::new(
            &auth_settings.jwt_secret,
            auth_settings.access_token_expiry_minutes,
            auth_settings.refresh_token_expiry_days,
            &auth_settings.jwt_issuer,
            &auth_settings.jwt_audience,
        ),
        rate_limiter,
        refresh_rate_limiter,
        event_bus,
        auth_settings.clone(),
        Vec::new(),
    );

    let profile_repo: Arc<dyn ProfileRepository> = Arc::new(PostgresProfileRepo::new(pool.clone()));
    let user_directory: Arc<dyn UserDirectory> = Arc::new(PostgresUserDirectory::new(pool.clone()));
    let admin_repo: Arc<dyn AdminRepository> =
        Arc::new(PostgresAdminRepo::new(pool.clone(), user_directory));
    let access_token_blacklist: Arc<dyn AccessTokenBlacklist> =
        Arc::new(MemoryAccessTokenBlacklist::new());

    let verification = Arc::new(JwtVerification::new(
        &auth_settings.jwt_secret,
        auth_settings.jwt_issuer.clone(),
        auth_settings.jwt_audience.clone(),
    ));

    // `build_recorder`, not `install_recorder`: installing would register a
    // process-wide recorder, which can only happen once per test binary and
    // would make these tests order-dependent. The handle renders fine either way.
    let metrics_handle = PrometheusBuilder::new().build_recorder().handle();

    let cors = if options.cors_allowed_origins.is_empty() {
        tower_http::cors::CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::list(Vec::<
                axum::http::HeaderValue,
            >::new()))
    } else {
        let origins: Vec<axum::http::HeaderValue> = options
            .cors_allowed_origins
            .iter()
            .map(|o| o.parse().expect("test origin should be a valid header"))
            .collect();
        tower_http::cors::CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::list(origins))
            .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
            ])
    };

    build_router(RouterDeps {
        state,
        profile_repo,
        admin_repo,
        verification,
        access_token_blacklist,
        health_check_pool: pool,
        metrics_handle,
        metrics_guard_config: MetricsGuardConfig {
            allowed_ips: options.metrics_allowed_ips,
            trusted_proxy_ips: Vec::new(),
        },
        cors,
        enable_swagger: options.enable_swagger,
    })
}

/// Sends `request` through the router, attaching the `ConnectInfo` that
/// `axum::serve` would normally supply from the real TCP peer -- the `/metrics`
/// guard extracts it, and without it that route would fail to even reach the
/// middleware.
async fn send(
    router: Router,
    mut request: Request<Body>,
    peer: SocketAddr,
) -> axum::response::Response {
    request.extensions_mut().insert(ConnectInfo(peer));
    router
        .oneshot(request)
        .await
        .expect("routing a request is infallible")
}

fn get(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("test request should build")
}

fn loopback() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 54321))
}

#[tokio::test]
async fn security_headers_are_applied_to_every_response_including_404s() {
    // Deliberately an unrouted path: the headers come from a layer wrapping the
    // whole service, so a 404 proves they can't be missed by any route -- a
    // stronger claim than checking one known-good endpoint.
    let response = send(
        router(RouterOptions::default()),
        get("/no-such-route"),
        loopback(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let headers = response.headers();
    for (name, expected) in [
        (
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        ),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
    ] {
        assert_eq!(
            headers.get(name).and_then(|v| v.to_str().ok()),
            Some(expected),
            "{name} should be present on every response"
        );
    }
}

#[tokio::test]
async fn swagger_routes_are_absent_unless_enabled() {
    let router = router(RouterOptions::default());

    let ui = send(router.clone(), get("/swagger-ui"), loopback()).await;
    let spec = send(router, get("/api-docs/openapi.json"), loopback()).await;

    assert_eq!(
        ui.status(),
        StatusCode::NOT_FOUND,
        "/swagger-ui must not exist when ENABLE_SWAGGER is unset (see #86)"
    );
    assert_eq!(
        spec.status(),
        StatusCode::NOT_FOUND,
        "the OpenAPI spec must not be served when ENABLE_SWAGGER is unset (see #86)"
    );
}

#[tokio::test]
async fn swagger_spec_is_served_when_enabled() {
    let response = send(
        router(RouterOptions {
            enable_swagger: true,
            ..RouterOptions::default()
        }),
        get("/api-docs/openapi.json"),
        loopback(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn cors_does_not_allow_a_foreign_origin_when_none_are_configured() {
    let mut request = get("/no-such-route");
    request
        .headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());

    let response = send(router(RouterOptions::default()), request, loopback()).await;

    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "an empty CORS origin list must not echo back any origin"
    );
}

#[tokio::test]
async fn cors_allows_a_configured_origin() {
    let mut request = get("/no-such-route");
    request
        .headers_mut()
        .insert("origin", "https://app.example".parse().unwrap());

    let response = send(
        router(RouterOptions {
            cors_allowed_origins: vec!["https://app.example"],
            ..RouterOptions::default()
        }),
        request,
        loopback(),
    )
    .await;

    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://app.example")
    );
}

#[tokio::test]
async fn cors_rejects_a_foreign_origin_even_when_an_allowlist_is_configured() {
    // The dangerous regression is not "no origins configured" but "origins
    // configured, and the list is honoured rather than reflecting whatever
    // arrives".
    let mut request = get("/no-such-route");
    request
        .headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());

    let response = send(
        router(RouterOptions {
            cors_allowed_origins: vec!["https://app.example"],
            ..RouterOptions::default()
        }),
        request,
        loopback(),
    )
    .await;

    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "an origin outside the configured list must not be echoed back"
    );
}

#[tokio::test]
async fn metrics_is_unrestricted_when_no_allowlist_is_configured() {
    let response = send(
        router(RouterOptions::default()),
        get("/metrics"),
        SocketAddr::from(([203, 0, 113, 7], 40000)),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_rejects_an_ip_outside_the_configured_allowlist() {
    let response = send(
        router(RouterOptions {
            metrics_allowed_ips: vec![IpAddr::V4(Ipv4Addr::new(198, 51, 100, 4))],
            ..RouterOptions::default()
        }),
        get("/metrics"),
        SocketAddr::from(([203, 0, 113, 7], 40000)),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_always_allows_loopback_even_with_an_allowlist() {
    let response = send(
        router(RouterOptions {
            metrics_allowed_ips: vec![IpAddr::V4(Ipv4Addr::new(198, 51, 100, 4))],
            ..RouterOptions::default()
        }),
        get("/metrics"),
        loopback(),
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "loopback is always allowed so local scraping can't be locked out"
    );
}

#[tokio::test]
async fn the_metrics_allowlist_does_not_leak_onto_other_routes() {
    // The guard lives on a dedicated sub-router precisely so it can't gate the
    // rest of the service; an allowlist that rejects this peer for /metrics must
    // leave every other route untouched.
    let response = send(
        router(RouterOptions {
            metrics_allowed_ips: vec![IpAddr::V4(Ipv4Addr::new(198, 51, 100, 4))],
            ..RouterOptions::default()
        }),
        get("/no-such-route"),
        SocketAddr::from(([203, 0, 113, 7], 40000)),
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a non-allowlisted peer should get normal routing everywhere except /metrics"
    );
}
