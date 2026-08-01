use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use infrastructure::metrics_guard::is_metrics_access_allowed;

fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

#[test]
fn empty_allowlist_permits_any_ip() {
    assert!(is_metrics_access_allowed(ip(203, 0, 113, 5), &[]));
}

#[test]
fn allows_an_ip_present_in_the_allowlist() {
    let allowed = [ip(10, 0, 0, 5)];
    assert!(is_metrics_access_allowed(ip(10, 0, 0, 5), &allowed));
}

#[test]
fn rejects_an_ip_not_present_in_a_non_empty_allowlist() {
    let allowed = [ip(10, 0, 0, 5)];
    assert!(!is_metrics_access_allowed(ip(203, 0, 113, 5), &allowed));
}

#[test]
fn always_allows_ipv4_loopback_even_with_a_non_empty_allowlist_that_excludes_it() {
    let allowed = [ip(10, 0, 0, 5)];
    assert!(is_metrics_access_allowed(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        &allowed
    ));
}

#[test]
fn always_allows_ipv6_loopback_even_with_a_non_empty_allowlist() {
    let allowed = [ip(10, 0, 0, 5)];
    assert!(is_metrics_access_allowed(
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        &allowed
    ));
}
