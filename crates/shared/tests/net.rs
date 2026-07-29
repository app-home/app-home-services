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
