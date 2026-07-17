use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::application::ports::rate_limiter::RateLimiter;

#[derive(Debug, Clone)]
struct RateLimitEntry {
    attempts: u32,
    window_start: Instant,
}

/// In-memory, single-instance rate limiter.
///
/// Counters live only in this process's memory: they are lost on restart and are not
/// shared with any other instance of the service. This is fine for a single-instance
/// deployment, but if the service ever runs with more than one replica behind a load
/// balancer, an attacker can bypass the limit by spreading requests across replicas.
/// For multi-instance deployments, use `RedisRateLimiter` instead (selected
/// automatically in `main.rs` when `REDIS_URL` is configured).
#[derive(Debug)]
pub struct MemoryRateLimiter {
    max_attempts: u32,
    window_duration: Duration,
    entries: Mutex<HashMap<IpAddr, RateLimitEntry>>,
}

impl MemoryRateLimiter {
    pub fn new(max_attempts: u32, window_seconds: u64) -> Self {
        Self {
            max_attempts,
            window_duration: Duration::from_secs(window_seconds),
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn clean_expired(&self, entries: &mut HashMap<IpAddr, RateLimitEntry>) {
        let now = Instant::now();
        let window = self.window_duration;
        entries.retain(|_, entry| now.duration_since(entry.window_start) < window);
    }
}

#[async_trait]
impl RateLimiter for MemoryRateLimiter {
    async fn check(&self, ip: IpAddr) -> bool {
        let mut entries = self.entries.lock().await;
        self.clean_expired(&mut entries);

        match entries.get(&ip) {
            Some(entry) => {
                let elapsed = Instant::now().duration_since(entry.window_start);
                if elapsed >= self.window_duration {
                    true
                } else {
                    entry.attempts < self.max_attempts
                }
            }
            None => true,
        }
    }

    async fn record_attempt(&self, ip: IpAddr) {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();

        let entry = entries.entry(ip).or_insert(RateLimitEntry {
            attempts: 0,
            window_start: now,
        });

        let elapsed = now.duration_since(entry.window_start);
        if elapsed >= self.window_duration {
            entry.attempts = 1;
            entry.window_start = now;
        } else {
            entry.attempts += 1;
        }
    }

    async fn remaining_attempts(&self, ip: IpAddr) -> u32 {
        let entries = self.entries.lock().await;
        match entries.get(&ip) {
            Some(entry) => {
                let elapsed = Instant::now().duration_since(entry.window_start);
                if elapsed >= self.window_duration {
                    self.max_attempts
                } else {
                    self.max_attempts.saturating_sub(entry.attempts)
                }
            }
            None => self.max_attempts,
        }
    }

    async fn reset(&self, ip: IpAddr) {
        let mut entries = self.entries.lock().await;
        entries.remove(&ip);
    }
}
