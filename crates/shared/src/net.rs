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
pub fn resolve_client_ip(
    peer_ip: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[IpAddr],
) -> IpAddr {
    if !trusted_proxies.contains(&peer_ip) {
        return peer_ip;
    }

    if let Some(value) = headers.get("X-Forwarded-For").and_then(|v| v.to_str().ok()) {
        // Each proxy in a legitimate chain *appends* its view of the previous hop to the
        // end of the header, so entries accumulate left-to-right as the request travels:
        // "client, proxy1, proxy2, ...". The rightmost entries are the most recently
        // added -- closest to us -- while the leftmost entry is furthest away and, in
        // particular, can be set to *anything* by the original client before the header
        // ever reaches the first real proxy.
        //
        // Walking right-to-left and returning the first entry that is NOT itself a known
        // trusted proxy finds the exact boundary between "hops we've verified are our own
        // trusted infrastructure" and "unverified input". Scanning from the left instead
        // (the previous behavior) returned whatever the original, potentially malicious
        // client put first, completely bypassing the trust boundary -- see #84.
        for candidate in value.split(',').rev() {
            if let Ok(ip) = candidate.trim().parse::<IpAddr>()
                && !trusted_proxies.contains(&ip)
            {
                return ip;
            }
        }
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
