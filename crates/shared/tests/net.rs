use std::net::{IpAddr, Ipv4Addr};

use axum::http::HeaderMap;
use shared::net::resolve_client_ip;

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
fn uses_the_rightmost_untrusted_entry_in_x_forwarded_for_when_peer_is_trusted() {
    // "9.9.9.9, 8.8.8.8": 8.8.8.8 was appended most recently (by our trusted peer), so
    // it is the correct answer -- not 9.9.9.9, which is whatever the original client
    // (or an earlier, unverified hop) put first. See #84.
    let headers = {
        let mut h = HeaderMap::new();
        h.insert("X-Forwarded-For", "9.9.9.9, 8.8.8.8".parse().unwrap());
        h
    };
    let peer = ip("1.2.3.4");
    let trusted = [peer];
    assert_eq!(resolve_client_ip(peer, &headers, &trusted), ip("8.8.8.8"));
}

#[test]
fn returns_the_real_client_ip_not_an_attacker_spoofed_leftmost_entry() {
    // Regression test for #84: a client connecting through a trusted proxy can prepend
    // an arbitrary fake IP to X-Forwarded-For before the proxy appends the real one.
    // Scanning left-to-right (the old, vulnerable behavior) would return the attacker's
    // fake "6.6.6.6" instead of the real client at "203.0.113.9".
    let headers = {
        let mut h = HeaderMap::new();
        h.insert("X-Forwarded-For", "6.6.6.6, 203.0.113.9".parse().unwrap());
        h
    };
    let peer = ip("1.2.3.4"); // the trusted proxy that appended 203.0.113.9
    let trusted = [peer];

    let resolved = resolve_client_ip(peer, &headers, &trusted);

    assert_eq!(
        resolved,
        ip("203.0.113.9"),
        "must return the real client IP appended by the trusted proxy, not the attacker-controlled leftmost entry"
    );
    assert_ne!(
        resolved,
        ip("6.6.6.6"),
        "must never trust an entry the client could have prepended itself"
    );
}

#[test]
fn walks_past_multiple_trusted_proxies_in_the_chain() {
    // A chain of multiple trusted proxies (e.g. an internal load balancer in front of a
    // reverse proxy) should be walked through entirely, skipping every trusted hop,
    // until the first untrusted (i.e. real client, or first unverified) entry is found.
    let proxy1 = ip("10.0.0.1");
    let proxy2 = ip("10.0.0.2"); // the one directly connecting to us
    let headers = {
        let mut h = HeaderMap::new();
        h.insert(
            "X-Forwarded-For",
            "6.6.6.6, 203.0.113.9, 10.0.0.1".parse().unwrap(),
        );
        h
    };
    let trusted = [proxy1, proxy2];

    let resolved = resolve_client_ip(proxy2, &headers, &trusted);

    assert_eq!(resolved, ip("203.0.113.9"));
}

#[test]
fn falls_back_to_x_real_ip_when_every_forwarded_for_entry_is_itself_trusted() {
    // An edge case where the whole X-Forwarded-For chain is made up of known proxies
    // (e.g. a misconfiguration, or a health-checker hopping through internal infra) --
    // there is no untrusted entry to return, so fall back to X-Real-IP (and ultimately
    // peer_ip) rather than returning nothing.
    let peer = ip("10.0.0.2");
    let other_trusted = ip("10.0.0.1");
    let headers = {
        let mut h = HeaderMap::new();
        h.insert("X-Forwarded-For", "10.0.0.1".parse().unwrap());
        h.insert("X-Real-IP", "9.9.9.9".parse().unwrap());
        h
    };
    let trusted = [peer, other_trusted];

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
    assert_eq!(resolve_client_ip(peer, &HeaderMap::new(), &trusted), peer);
}

#[test]
fn ipv6_loopback_parses_and_resolves_like_any_other_address() {
    let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert_eq!(resolve_client_ip(peer, &HeaderMap::new(), &[]), peer);
}
