use std::net::IpAddr;

use axum::http::HeaderMap;

/// Resolves the "real" client IP for a request, honoring `X-Forwarded-For` /
/// `X-Real-IP` only when the direct TCP peer is in `trusted_proxies`.
///
/// This is cross-cutting (used by rate limiting in `auth`'s login/refresh handlers,
/// and by the `/metrics` IP allowlist guard in `infrastructure`), so it lives here
/// in `shared` rather than in any one bounded context -- see
/// `docs/adr/0001-modular-monolith.md` for why cross-context-usable logic belongs in
/// `shared`, not duplicated per consumer or borrowed from whichever context defined
/// it first.
///
/// If `peer_ip` is not in `trusted_proxies`, the forwarded headers are ignored
/// entirely and `peer_ip` itself is returned -- otherwise any client could spoof
/// these headers to impersonate a different IP (bypassing rate limiting, or an IP
/// allowlist) unless a trusted proxy is known to be the one actually setting them.
pub fn resolve_client_ip(peer_ip: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> IpAddr {
    if !trusted_proxies.contains(&peer_ip) {
        return peer_ip;
    }

    if let Some(value) = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse::<IpAddr>().ok())
    {
        return value;
    }

    if let Some(value) = headers
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<IpAddr>().ok())
    {
        return value;
    }

    peer_ip
}

