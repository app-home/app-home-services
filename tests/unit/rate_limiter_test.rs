use std::net::{IpAddr, Ipv4Addr};

use app_home_services::application::ports::rate_limiter::RateLimiter;
use auth::adapters::memory_rate_limiter::MemoryRateLimiter;

#[tokio::test]
async fn test_rate_limiter_allows_first_attempt() {
    let limiter = MemoryRateLimiter::new(3, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    assert!(limiter.check(ip).await);
    assert_eq!(limiter.remaining_attempts(ip).await, 3);
}

#[tokio::test]
async fn test_rate_limiter_blocks_after_max_attempts() {
    let limiter = MemoryRateLimiter::new(3, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    for _ in 0..3 {
        limiter.record_attempt(ip).await;
    }

    assert!(!limiter.check(ip).await);
    assert_eq!(limiter.remaining_attempts(ip).await, 0);
}

#[tokio::test]
async fn test_rate_limiter_allows_after_reset() {
    let limiter = MemoryRateLimiter::new(3, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    for _ in 0..3 {
        limiter.record_attempt(ip).await;
    }

    assert!(!limiter.check(ip).await);

    limiter.reset(ip).await;
    assert!(limiter.check(ip).await);
    assert_eq!(limiter.remaining_attempts(ip).await, 3);
}

#[tokio::test]
async fn test_rate_limiter_tracks_attempts_correctly() {
    let limiter = MemoryRateLimiter::new(5, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    assert_eq!(limiter.remaining_attempts(ip).await, 5);

    limiter.record_attempt(ip).await;
    assert_eq!(limiter.remaining_attempts(ip).await, 4);

    limiter.record_attempt(ip).await;
    assert_eq!(limiter.remaining_attempts(ip).await, 3);
}

#[tokio::test]
async fn test_rate_limiter_independent_per_ip() {
    let limiter = MemoryRateLimiter::new(2, 60);
    let ip1 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    limiter.record_attempt(ip1).await;
    limiter.record_attempt(ip1).await;

    assert!(!limiter.check(ip1).await);
    assert!(limiter.check(ip2).await);
}

#[tokio::test]
async fn test_rate_limiter_remaining_attempts_for_unknown_ip() {
    let limiter = MemoryRateLimiter::new(10, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1));

    assert_eq!(limiter.remaining_attempts(ip).await, 10);
}

#[tokio::test]
async fn test_rate_limiter_record_attempt_creates_entry() {
    let limiter = MemoryRateLimiter::new(5, 60);
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

    limiter.record_attempt(ip).await;
    assert_eq!(limiter.remaining_attempts(ip).await, 4);
}
