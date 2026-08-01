use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

use shared::ports::RateLimiter;

/// Hard cap on the number of distinct IPs tracked at once.
///
/// `clean_expired` only removes entries whose window has already elapsed, which
/// does not help during an active flood: an attacker spraying requests across
/// many distinct IPs within a single window keeps every entry "current" (not
/// expired), so the map can grow unbounded for the duration of the attack even
/// with cleanup running on every call (see #96). This cap bounds worst-case
/// memory (~40 bytes/entry, so 100_000 entries is a few MB) by evicting the
/// single oldest entry whenever a new IP would exceed it.
const MAX_ENTRIES: usize = 100_000;

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

    /// Evicts the single oldest entry (smallest `window_start`) when the map is
    /// at capacity, so tracking a brand-new IP never grows it past
    /// `MAX_ENTRIES`. Called right before inserting a not-yet-tracked IP; a
    /// no-op once cleanup (or normal traffic) has room again. O(n) worst case,
    /// but only runs at capacity -- not on every request.
    fn evict_oldest_if_full(&self, entries: &mut HashMap<IpAddr, RateLimitEntry>) {
        if entries.len() < MAX_ENTRIES {
            return;
        }
        if let Some(&oldest_ip) = entries
            .iter()
            .min_by_key(|(_, entry)| entry.window_start)
            .map(|(ip, _)| ip)
        {
            entries.remove(&oldest_ip);
        }
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
        self.clean_expired(&mut entries);
        let now = Instant::now();

        if !entries.contains_key(&ip) {
            self.evict_oldest_if_full(&mut entries);
        }

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

    async fn try_check_and_record(&self, ip: IpAddr) -> bool {
        let mut entries = self.entries.lock().await;
        self.clean_expired(&mut entries);
        let now = Instant::now();

        if !entries.contains_key(&ip) {
            self.evict_oldest_if_full(&mut entries);
        }

        let entry = entries.entry(ip).or_insert(RateLimitEntry {
            attempts: 0,
            window_start: now,
        });

        let elapsed = now.duration_since(entry.window_start);
        if elapsed >= self.window_duration {
            entry.attempts = 1;
            entry.window_start = now;
            return true;
        }

        if entry.attempts < self.max_attempts {
            entry.attempts += 1;
            true
        } else {
            false
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
