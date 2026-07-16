use std::net::{IpAddr, Ipv4Addr};

use app_home_services::adapters::inbound::login_routes::resolve_client_ip;
use axum::http::HeaderMap;

fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

#[test]
fn untrusted_peer_forwarded_headers_are_ignored() {
    // Attacker connects directly and tries to spoof their IP via X-Forwarded-For.
    let attacker_peer = ip(203, 0, 113, 50);
    let mut headers = HeaderMap::new();
    headers.insert("X-Forwarded-For", "8.8.8.8".parse().unwrap());

    // No trusted proxies configured at all.
    let resolved = resolve_client_ip(attacker_peer, &headers, &[]);

    assert_eq!(
        resolved, attacker_peer,
        "forwarded header must be ignored when the peer isn't a trusted proxy"
    );
}

#[test]
fn untrusted_peer_cannot_spoof_ip_even_with_other_proxies_configured() {
    let trusted = vec![ip(10, 0, 0, 1)];
    let attacker_peer = ip(203, 0, 113, 50);
    let mut headers = HeaderMap::new();
    headers.insert("X-Forwarded-For", "1.2.3.4".parse().unwrap());

    let resolved = resolve_client_ip(attacker_peer, &headers, &trusted);

    assert_eq!(
        resolved, attacker_peer,
        "an untrusted peer's spoofed header must never be honored, even if other proxies are trusted"
    );
}

#[test]
fn trusted_proxy_forwarded_for_is_used() {
    let trusted = vec![ip(10, 0, 0, 1)];
    let proxy_peer = ip(10, 0, 0, 1);
    let mut headers = HeaderMap::new();
    headers.insert("X-Forwarded-For", "203.0.113.7, 10.0.0.1".parse().unwrap());

    let resolved = resolve_client_ip(proxy_peer, &headers, &trusted);

    assert_eq!(
        resolved,
        ip(203, 0, 113, 7),
        "the first entry in X-Forwarded-For is the original client when the peer is trusted"
    );
}

#[test]
fn trusted_proxy_falls_back_to_x_real_ip() {
    let trusted = vec![ip(10, 0, 0, 1)];
    let proxy_peer = ip(10, 0, 0, 1);
    let mut headers = HeaderMap::new();
    headers.insert("X-Real-IP", "203.0.113.9".parse().unwrap());

    let resolved = resolve_client_ip(proxy_peer, &headers, &trusted);

    assert_eq!(resolved, ip(203, 0, 113, 9));
}

#[test]
fn trusted_proxy_with_no_forwarded_headers_uses_peer_ip() {
    let trusted = vec![ip(10, 0, 0, 1)];
    let proxy_peer = ip(10, 0, 0, 1);
    let headers = HeaderMap::new();

    let resolved = resolve_client_ip(proxy_peer, &headers, &trusted);

    assert_eq!(resolved, proxy_peer);
}

#[test]
fn trusted_proxy_with_malformed_forwarded_for_falls_back_to_peer_ip() {
    let trusted = vec![ip(10, 0, 0, 1)];
    let proxy_peer = ip(10, 0, 0, 1);
    let mut headers = HeaderMap::new();
    headers.insert("X-Forwarded-For", "not-an-ip".parse().unwrap());

    let resolved = resolve_client_ip(proxy_peer, &headers, &trusted);

    assert_eq!(
        resolved, proxy_peer,
        "a malformed header should fall back to the peer ip, not panic or resolve incorrectly"
    );
}
