use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use crate::application::ports::rate_limiter::RateLimiter;

#[derive(Debug, Clone)]
struct RateLimitEntry {
    attempts: u32,
    window_start: Instant,
}

#[derive(Debug, Clone)]
pub struct MemoryRateLimiter {
    max_attempts: u32,
    window_duration: Duration,
    entries: HashMap<IpAddr, RateLimitEntry>,
}

impl MemoryRateLimiter {
    pub fn new(max_attempts: u32, window_seconds: u64) -> Self {
        Self {
            max_attempts,
            window_duration: Duration::from_secs(window_seconds),
            entries: HashMap::new(),
        }
    }

    fn clean_expired(&mut self) {
        let now = Instant::now();
        self.entries
            .retain(|_, entry| now.duration_since(entry.window_start) < self.window_duration);
    }
}

impl RateLimiter for MemoryRateLimiter {
    fn check(&mut self, ip: IpAddr) -> bool {
        self.clean_expired();

        match self.entries.get(&ip) {
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

    fn record_attempt(&mut self, ip: IpAddr) {
        let now = Instant::now();

        let entry = self.entries.entry(ip).or_insert(RateLimitEntry {
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

    fn remaining_attempts(&self, ip: IpAddr) -> u32 {
        match self.entries.get(&ip) {
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

    fn reset(&mut self, ip: IpAddr) {
        self.entries.remove(&ip);
    }
}
