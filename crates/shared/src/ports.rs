use std::net::IpAddr;

use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check(&self, ip: IpAddr) -> bool;
    async fn record_attempt(&self, ip: IpAddr);
    async fn try_check_and_record(&self, ip: IpAddr) -> bool;
    async fn remaining_attempts(&self, ip: IpAddr) -> u32;
    async fn reset(&self, ip: IpAddr);
}

/// Marker for a failed call to the access-token revocation list (e.g. the Redis
/// backend is unreachable). Callers are expected to fail open: treat an `Err` as
/// "not revoked" so a backend outage degrades availability rather than locking
/// out every authenticated user -- the same posture as `RateLimiter`.
#[derive(Debug, Clone, Copy)]
pub struct BlacklistError;

#[async_trait]
pub trait AccessTokenBlacklist: Send + Sync {
    /// Marks `jti` as revoked for `ttl_secs` seconds (callers pass the token's
    /// remaining lifetime). Returns `Err` if the backend failed; the token is
    /// then simply not recorded.
    async fn revoke(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError>;

    /// Returns `true` if `jti` is revoked, `Err` if the backend failed (callers
    /// fail open and treat the token as not revoked).
    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError>;
}
