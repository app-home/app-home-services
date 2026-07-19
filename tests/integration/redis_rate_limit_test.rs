// Integration tests for the Redis-backed rate limiter.
// These require a running Redis instance.
//
// To run: REDIS_URL=redis://127.0.0.1:6379 cargo test --test integration -- --ignored redis_rate_limit

use std::net::{IpAddr, Ipv4Addr};

use app_home_services::application::ports::rate_limiter::RateLimiter;
use auth::adapters::redis_rate_limiter::RedisRateLimiter;

fn test_ip(last_octet: u8) -> IpAddr {
    // Distinct per-test IP (in the documentation/test range) so tests running against
    // a shared Redis instance don't interfere with each other's counters.
    IpAddr::V4(Ipv4Addr::new(198, 51, 100, last_octet))
}

async fn connect() -> RedisRateLimiter {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    RedisRateLimiter::connect(&redis_url, 3, 60, "test")
        .await
        .expect("Failed to connect to Redis -- is it running and REDIS_URL correct?")
}

#[tokio::test]
#[ignore]
async fn test_redis_rate_limiter_allows_first_attempt() {
    let limiter = connect().await;
    let ip = test_ip(1);
    limiter.reset(ip).await;

    assert!(limiter.check(ip).await);
    assert_eq!(limiter.remaining_attempts(ip).await, 3);
}

#[tokio::test]
#[ignore]
async fn test_redis_rate_limiter_blocks_after_max_attempts() {
    let limiter = connect().await;
    let ip = test_ip(2);
    limiter.reset(ip).await;

    for _ in 0..3 {
        limiter.record_attempt(ip).await;
    }

    assert!(!limiter.check(ip).await);
    assert_eq!(limiter.remaining_attempts(ip).await, 0);
}

#[tokio::test]
#[ignore]
async fn test_redis_rate_limiter_allows_after_reset() {
    let limiter = connect().await;
    let ip = test_ip(3);
    limiter.reset(ip).await;

    for _ in 0..3 {
        limiter.record_attempt(ip).await;
    }
    assert!(!limiter.check(ip).await);

    limiter.reset(ip).await;
    assert!(limiter.check(ip).await);
    assert_eq!(limiter.remaining_attempts(ip).await, 3);
}

#[tokio::test]
#[ignore]
async fn test_redis_rate_limiter_counters_shared_across_instances() {
    // Two independent RedisRateLimiter instances (as if two service replicas) must
    // observe the same counter, since that's the entire point of this adapter.
    let limiter_a = connect().await;
    let limiter_b = connect().await;
    let ip = test_ip(4);
    limiter_a.reset(ip).await;

    limiter_a.record_attempt(ip).await;
    limiter_b.record_attempt(ip).await;

    assert_eq!(limiter_a.remaining_attempts(ip).await, 1);
    assert_eq!(limiter_b.remaining_attempts(ip).await, 1);
}

#[tokio::test]
#[ignore]
async fn test_redis_rate_limiter_different_prefixes_are_isolated() {
    // Login and refresh use separate key_prefix values in production (see main.rs).
    // Two limiters with different prefixes over the same IP must not share a counter.
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let login_limiter = RedisRateLimiter::connect(&redis_url, 3, 60, "test-login")
        .await
        .expect("Failed to connect to Redis");
    let refresh_limiter = RedisRateLimiter::connect(&redis_url, 3, 60, "test-refresh")
        .await
        .expect("Failed to connect to Redis");
    let ip = test_ip(5);
    login_limiter.reset(ip).await;
    refresh_limiter.reset(ip).await;

    for _ in 0..3 {
        login_limiter.record_attempt(ip).await;
    }

    assert!(!login_limiter.check(ip).await);
    assert!(
        refresh_limiter.check(ip).await,
        "a different key_prefix must not share the exhausted counter"
    );
}
