use std::net::IpAddr;

pub trait RateLimiter: Send + Sync {
    fn check(&mut self, ip: IpAddr) -> bool;
    fn record_attempt(&mut self, ip: IpAddr);
    fn remaining_attempts(&self, ip: IpAddr) -> u32;
    fn reset(&mut self, ip: IpAddr);
}
