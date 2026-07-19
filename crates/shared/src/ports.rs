use std::net::IpAddr;

use async_trait::async_trait;

#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check(&self, ip: IpAddr) -> bool;
    async fn record_attempt(&self, ip: IpAddr);
    async fn try_check_and_record(&self, ip: IpAddr) -> bool;
    async fn remaining_attempts(&self, ip: IpAddr) -> u32;
    async fn reset(&self, ip: IpAddr);
}
