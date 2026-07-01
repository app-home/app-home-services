use std::net::{IpAddr, Ipv4Addr};

use app_home_services::adapters::outbound::memory_rate_limiter::MemoryRateLimiter;
use app_home_services::application::ports::rate_limiter::RateLimiter;

#[test]
fn test_rate_limiter_allows_first_attempt() {
    let mut limiter = MemoryRateLimiter::new(3, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    assert!(limiter.check(ip));
    assert_eq!(limiter.remaining_attempts(ip), 3);
}

#[test]
fn test_rate_limiter_blocks_after_max_attempts() {
    let mut limiter = MemoryRateLimiter::new(3, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    for _ in 0..3 {
        limiter.record_attempt(ip);
    }

    assert!(!limiter.check(ip));
    assert_eq!(limiter.remaining_attempts(ip), 0);
}

#[test]
fn test_rate_limiter_allows_after_reset() {
    let mut limiter = MemoryRateLimiter::new(3, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    for _ in 0..3 {
        limiter.record_attempt(ip);
    }

    assert!(!limiter.check(ip));

    limiter.reset(ip);
    assert!(limiter.check(ip));
    assert_eq!(limiter.remaining_attempts(ip), 3);
}

#[test]
fn test_rate_limiter_tracks_attempts_correctly() {
    let mut limiter = MemoryRateLimiter::new(5, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    assert_eq!(limiter.remaining_attempts(ip), 5);

    limiter.record_attempt(ip);
    assert_eq!(limiter.remaining_attempts(ip), 4);

    limiter.record_attempt(ip);
    assert_eq!(limiter.remaining_attempts(ip), 3);
}

#[test]
fn test_rate_limiter_independent_per_ip() {
    let mut limiter = MemoryRateLimiter::new(2, 60);
    let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    limiter.record_attempt(ip1);
    limiter.record_attempt(ip1);

    assert!(!limiter.check(ip1));
    assert!(limiter.check(ip2));
}

#[test]
fn test_rate_limiter_remaining_attempts_for_unknown_ip() {
    let limiter = MemoryRateLimiter::new(10, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));

    assert_eq!(limiter.remaining_attempts(ip), 10);
}

#[test]
fn test_rate_limiter_record_attempt_creates_entry() {
    let mut limiter = MemoryRateLimiter::new(5, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

    limiter.record_attempt(ip);
    assert_eq!(limiter.remaining_attempts(ip), 4);
}
