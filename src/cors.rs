use axum::http::HeaderValue;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Builds the CORS layer from the raw `CORS_ALLOWED_ORIGINS` value (a
/// comma-separated origin list; empty means none configured).
///
/// Takes the string rather than the whole `Settings` so the layer's policy
/// depends on exactly one input and can be exercised directly -- the same
/// "config in, layer out" shape as `crate::security_headers`.
///
/// An empty list is not "allow anything": it produces an explicitly empty
/// allow-list, so no `Access-Control-Allow-Origin` is ever echoed back and the
/// service is same-origin only. That is the default posture, and the deliberate
/// one -- an unconfigured deployment should not be reachable cross-origin.
///
/// Origins that fail to parse as a header value are dropped. See #191's
/// follow-up note: this is silent today, which makes a typo in the env var look
/// like a CORS bug at runtime.
pub fn build_cors_layer(allowed_origins: &str) -> CorsLayer {
    if allowed_origins.is_empty() {
        tracing::info!("CORS: same-origin only (no origins configured)");
        return CorsLayer::new().allow_origin(AllowOrigin::list(Vec::<HeaderValue>::new()));
    }

    let origins: Vec<HeaderValue> = allowed_origins
        .split(',')
        .filter_map(|o| o.trim().parse::<HeaderValue>().ok())
        .collect();
    tracing::info!(?origins, "CORS: configured origins");

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
}
