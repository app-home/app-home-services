use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use shared::ports::{AccessTokenBlacklist, BlacklistError};

#[derive(Debug, Clone, Copy)]
struct RevokedEntry {
    revoked_at: Instant,
    ttl: Duration,
}

/// In-memory, single-instance access token revocation list.
///
/// Entries live only in this process's memory: they are lost on restart and are
/// not shared with any other instance of the service. This is fine for a
/// single-instance deployment, but with more than one replica a token revoked on
/// one instance keeps validating on the others until it expires. For
/// multi-instance deployments use `RedisAccessTokenBlacklist` instead (selected
/// automatically in `main.rs` when `REDIS_URL` is configured).
///
/// This backend never fails: `revoke` and `is_revoked` always return `Ok`.
#[derive(Debug, Default)]
pub struct MemoryAccessTokenBlacklist {
    entries: Mutex<HashMap<Uuid, RevokedEntry>>,
}

impl MemoryAccessTokenBlacklist {
    /// Creates an empty blacklist with no revoked entries.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AccessTokenBlacklist for MemoryAccessTokenBlacklist {
    async fn revoke(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError> {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();

        // Opportunistic sweep: `is_revoked` only removes an entry when that exact
        // `jti` is looked up again after it expires, but a client that logs out
        // never presents the revoked token again, so most entries would
        // otherwise never be queried -- and never removed. `revoke` is called on
        // every logout, so pruning here (while the lock is already held) keeps
        // `entries` bounded by the number of currently-valid revocations instead
        // of growing by one permanent entry per logout for the life of the
        // process (see #135).
        entries.retain(|_, entry| now.duration_since(entry.revoked_at) < entry.ttl);

        entries.insert(
            jti,
            RevokedEntry {
                revoked_at: now,
                ttl: Duration::from_secs(ttl_secs),
            },
        );
        Ok(())
    }

    async fn is_revoked(&self, jti: Uuid) -> Result<bool, BlacklistError> {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();
        match entries.get(&jti) {
            Some(entry) if now.duration_since(entry.revoked_at) < entry.ttl => Ok(true),
            _ => {
                entries.remove(&jti);
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
impl MemoryAccessTokenBlacklist {
    /// Test-only: number of entries currently stored, including any not yet
    /// pruned. Used to assert that the opportunistic sweep in `revoke` actually
    /// bounds growth instead of just trusting the implementation (see #135 and
    /// the CodeRabbit review on PR #142).
    pub async fn entry_count(&self) -> usize {
        self.entries.lock().await.len()
    }
}
