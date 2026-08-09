use std::sync::Arc;

use axum::{
    Extension, Router,
    routing::{get, post, put},
};
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use admin::adapters::inbound::admin_routes::{
    get_user_handler, list_users_handler, update_user_role_handler,
};
use admin::application::ports::admin_repository::AdminRepository;
use auth::adapters::inbound::login_routes::login_password_handler;
use auth::adapters::inbound::logout_routes::logout_handler;
use auth::adapters::inbound::oauth_callback::login_google_handler;
use auth::adapters::inbound::refresh_routes::refresh_token_handler;
use infrastructure::metrics_guard::{MetricsGuardConfig, metrics_ip_allowlist};
use profiles::adapters::inbound::profile_routes::{get_profile_handler, update_profile_handler};
use profiles::application::ports::profile_repository::ProfileRepository;
use shared::auth::JwtVerification;
use shared::ports::AccessTokenBlacklist;

use crate::api_doc::ApiDoc;
use crate::health::health_check;
use crate::security_headers::apply_security_headers;

/// Everything `build_router` needs to wire the route table together.
///
/// A struct rather than a ten-parameter function: these are all independently
/// constructed collaborators with no meaningful ordering between them, so named
/// fields at the call site beat positional arguments that are easy to transpose.
pub struct RouterDeps {
    /// The auth `AppState` backing every auth route's handler (repos, JWT,
    /// rate limiters, event bus). Owned here and moved into the router.
    pub state: auth::AppState,
    /// Profile repo injected into the profile routes' `Extension`. Coerced to
    /// `Arc<dyn ProfileRepository>` at the call site so the `Extension` key
    /// matches what the profiles handlers extract.
    pub profile_repo: Arc<dyn ProfileRepository>,
    /// Admin repo injected into the admin routes' `Extension`, same `Arc<dyn>`
    /// coercion rationale as `profile_repo`.
    pub admin_repo: Arc<dyn AdminRepository>,
    /// Shared JWT verification (secret + iss/aud) for the `AuthenticatedUser`
    /// extractor on every protected route. See #87.
    pub verification: Arc<JwtVerification>,
    /// Shared access-token revocation list: every protected route's
    /// `AuthenticatedUser` extractor consults it and the logout handler writes
    /// to it (see #88, #140).
    pub access_token_blacklist: Arc<dyn AccessTokenBlacklist>,
    /// `/api/health` runs a real `SELECT 1` against the pool (see
    /// `crate::health`), so it needs its own handle to it. Cloning a `PgPool` is
    /// cheap (it wraps an `Arc` internally), not a second pool.
    pub health_check_pool: sqlx::PgPool,
    /// Render handle for the installed Prometheus recorder; serves `/metrics`.
    pub metrics_handle: PrometheusHandle,
    /// IP-allowlist configuration gating `/metrics` via `metrics_ip_allowlist`.
    pub metrics_guard_config: MetricsGuardConfig,
    /// CORS policy layer built from `CORS_ALLOWED_ORIGINS` (see `crate::cors`).
    pub cors: CorsLayer,
    /// `ENABLE_SWAGGER`; see the conditional merge below for why the docs routes
    /// are opt-in.
    pub enable_swagger: bool,
}

/// Builds the fully-layered application router: routes, shared `Extension`s, the
/// guarded `/metrics` sub-router, the optional Swagger UI, security headers and
/// CORS.
///
/// Lives here rather than inline in `main` so the application can be constructed
/// without starting the process (see #191). Router-level behaviour -- security
/// headers, CORS, `/swagger-ui` being absent unless enabled, the `/metrics` IP
/// allowlist -- is then testable in-process via `tower::ServiceExt::oneshot`,
/// with no bound port and no live server.
pub fn build_router(deps: RouterDeps) -> Router {
    let RouterDeps {
        state,
        profile_repo,
        admin_repo,
        verification,
        access_token_blacklist,
        health_check_pool,
        metrics_handle,
        metrics_guard_config,
        cors,
        enable_swagger,
    } = deps;

    // Kept as its own sub-router (merged below) rather than a plain `.route()` on the
    // main router, so the IP allowlist middleware/Extension only ever apply to
    // `/metrics` -- not to every other route on the service.
    //
    // `route_layer`, not `layer`: `layer` also wraps the sub-router's fallback,
    // and merging then carries that wrapped fallback into the main router -- so
    // the allowlist ended up gating every unrouted path too, answering 403
    // instead of 404 to any non-allowlisted caller. `route_layer` applies only
    // to matched routes, which is what the paragraph above always claimed.
    // Covered by `the_metrics_allowlist_does_not_leak_onto_other_routes` in
    // tests/router_test.rs.
    let metrics_router = Router::new()
        .route(
            "/metrics",
            get(move || std::future::ready(metrics_handle.render())),
        )
        .route_layer(axum::middleware::from_fn(metrics_ip_allowlist))
        .route_layer(Extension(metrics_guard_config));

    let mut app = Router::new()
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
    if enable_swagger {
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }

    // HTTP security headers (see #90) -- see
    // `security_headers::apply_security_headers` for why each is set and why
    // HSTS is emitted unconditionally.
    //
    // `layer(cors)` is applied before `apply_security_headers`, so the header
    // layers sit *outside* CORS. tower-http's `CorsLayer` short-circuits valid
    // `OPTIONS` preflight requests and answers them directly without ever
    // calling the inner service; with the opposite ordering those preflight
    // responses would bypass the security headers entirely. Wrapping CORS with
    // the header layers keeps the latter on every response, preflight included.
    apply_security_headers(app.layer(cors)).with_state(state)
}
