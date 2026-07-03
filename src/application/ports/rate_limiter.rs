use std::net::IpAddr;

use async_trait::async_trait;

/// Tracks failed-attempt counters to protect endpoints (e.g. login) from brute force.
///
/// Implementations may be backed by process-local memory (single instance only, see
/// `MemoryRateLimiter`) or by a shared store such as Redis (`RedisRateLimiter`), which
/// keeps counters consistent across multiple service instances. Methods take `&self`
/// so implementations can be shared behind an `Arc<dyn RateLimiter>` and handle their
/// own internal synchronization.
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Returns `true` if the given IP is still within its attempt budget.
    async fn check(&self, ip: IpAddr) -> bool;
    /// Records a new attempt for the given IP.
    async fn record_attempt(&self, ip: IpAddr);
    /// Returns how many attempts the given IP has left in the current window.
    async fn remaining_attempts(&self, ip: IpAddr) -> u32;
    /// Clears the counter for the given IP (e.g. after a successful login).
    async fn reset(&self, ip: IpAddr);
}
