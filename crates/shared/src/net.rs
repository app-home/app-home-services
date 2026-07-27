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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn returns_peer_ip_when_peer_is_not_a_trusted_proxy() {
        let headers = {
            let mut h = HeaderMap::new();
            h.insert("X-Forwarded-For", "9.9.9.9".parse().unwrap());
            h
        };
        let peer = ip("1.2.3.4");
        assert_eq!(resolve_client_ip(peer, &headers, &[]), peer);
    }

    #[test]
    fn uses_x_forwarded_for_when_peer_is_a_trusted_proxy() {
        let headers = {
            let mut h = HeaderMap::new();
            h.insert("X-Forwarded-For", "9.9.9.9, 8.8.8.8".parse().unwrap());
            h
        };
        let peer = ip("1.2.3.4");
        let trusted = [peer];
        assert_eq!(resolve_client_ip(peer, &headers, &trusted), ip("9.9.9.9"));
    }

    #[test]
    fn falls_back_to_x_real_ip_when_forwarded_for_is_absent() {
        let headers = {
            let mut h = HeaderMap::new();
            h.insert("X-Real-IP", "9.9.9.9".parse().unwrap());
            h
        };
        let peer = ip("1.2.3.4");
        let trusted = [peer];
        assert_eq!(resolve_client_ip(peer, &headers, &trusted), ip("9.9.9.9"));
    }

    #[test]
    fn falls_back_to_peer_ip_when_trusted_but_no_forwarding_headers_present() {
        let peer = ip("1.2.3.4");
        let trusted = [peer];
        assert_eq!(
            resolve_client_ip(peer, &HeaderMap::new(), &trusted),
            peer
        );
    }

    #[test]
    fn ipv6_loopback_parses_and_resolves_like_any_other_address() {
        let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert_eq!(resolve_client_ip(peer, &HeaderMap::new(), &[]), peer);
    }
}
