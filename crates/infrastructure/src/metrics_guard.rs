use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use shared::api::ErrorResponse;
use shared::net::resolve_client_ip;

/// Configuration the `/metrics` IP allowlist guard needs, bundled so it can be
/// injected as a single `Extension` rather than several.
#[derive(Clone)]
pub struct MetricsGuardConfig {
    pub allowed_ips: Vec<IpAddr>,
    pub trusted_proxy_ips: Vec<IpAddr>,
}

/// Whether `ip` is allowed to reach `/metrics`, given the configured allowlist.
///
/// Pulled out as a standalone, synchronous function (no `axum` types involved) so
/// the actual access-control decision can be unit-tested directly against IP
/// addresses, without needing to build a fake `Request` or middleware stack -- see
/// the tests below.
///
/// - An empty `allowed_ips` means no restriction: this is the backward-compatible
///   default (see `Settings::metrics_allowed_ips`'s docs for why).
/// - Loopback addresses (`127.0.0.0/8`, `::1`) are always allowed regardless of the
///   configured list, so local scraping/testing/health-checking never gets locked
///   out by a misconfigured or forgotten allowlist.
pub fn is_metrics_access_allowed(ip: IpAddr, allowed_ips: &[IpAddr]) -> bool {
    if allowed_ips.is_empty() {
        return true;
    }

    if ip.is_loopback() {
        return true;
    }

    allowed_ips.contains(&ip)
}

/// Axum middleware enforcing `is_metrics_access_allowed` for whichever route(s) it's
/// layered onto. Meant to be scoped to just the `/metrics` route (via a dedicated
/// sub-router merged into the main one in `main.rs`), not applied service-wide.
///
/// Resolves the caller's IP the same way login/refresh rate limiting does --
/// honoring `X-Forwarded-For`/`X-Real-IP` only when the request's direct peer is a
/// trusted proxy -- so this works correctly whether the service is reached directly
/// or through a reverse proxy.
pub async fn metrics_ip_allowlist(
    Extension(config): Extension<MetricsGuardConfig>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let ip = resolve_client_ip(peer.ip(), request.headers(), &config.trusted_proxy_ips);

    if is_metrics_access_allowed(ip, &config.allowed_ips) {
        return next.run(request).await;
    }

    tracing::warn!(%ip, "Rejected /metrics request: IP not in METRICS_ALLOWED_IPS");
    (
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "Not allowed".into(),
        }),
    )
        .into_response()
}

