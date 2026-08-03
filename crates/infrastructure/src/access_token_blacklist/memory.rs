use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;
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
///
/// `RwLock`, not `Mutex`: `is_revoked` runs on every authenticated request (the
/// hottest path touching this struct) and, in its common cases -- token found
/// and still valid, or token never seen at all -- only needs to *read* the map,
/// with no `.await` inside that read. A plain `Mutex` would serialize every
/// concurrent request behind whichever one currently holds the lock even though
/// none of them are actually mutating anything in those cases; `RwLock` lets
/// them proceed concurrently. `revoke` still needs exclusive (write) access, and
/// so does the rare "found but expired" branch of `is_revoked` that removes the
/// stale entry.
#[derive(Debug, Default)]
pub struct MemoryAccessTokenBlacklist {
    entries: RwLock<HashMap<Uuid, RevokedEntry>>,
}

impl MemoryAccessTokenBlacklist {
    /// Creates an empty blacklist with no revoked entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries currently stored, including any not yet pruned by the
    /// opportunistic sweep in `revoke`. Not part of the `AccessTokenBlacklist`
    /// port -- exists so integration tests can assert that sweep actually
    /// bounds growth instead of just trusting the implementation (see #135 and
    /// the CodeRabbit review on PR #142). `entries` itself stays private;
    /// nothing outside this module can inspect *which* tokens are revoked
    /// through this method, only how many.
    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }
}

#[async_trait]
impl AccessTokenBlacklist for MemoryAccessTokenBlacklist {
    async fn revoke(&self, jti: Uuid, ttl_secs: u64) -> Result<(), BlacklistError> {
        let mut entries = self.entries.write().await;
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
        // Fast path: a shared read lock covers both common outcomes (revoked and
        // still valid, or never revoked at all) without blocking any other
        // concurrent reader.
        {
            let now = Instant::now();
            let entries = self.entries.read().await;
            match entries.get(&jti) {
                Some(entry) if now.duration_since(entry.revoked_at) < entry.ttl => {
                    return Ok(true);
                }
                None => return Ok(false),
                Some(_) => {} // present but expired -- fall through to remove it
            }
        }

        // Slow path: the entry was expired under the read lock above. Re-check
        // under the write lock rather than assuming it's still the same stale
        // entry: another task may have called `revoke()` for this exact `jti`
        // (refreshing it with a new TTL) in the window between us releasing the
        // read lock and acquiring the write lock. Blindly removing here would
        // silently un-revoke a token someone just re-revoked. See #142 review
        // (CodeRabbit).
        let now = Instant::now();
        let mut entries = self.entries.write().await;
        match entries.get(&jti) {
            Some(entry) if now.duration_since(entry.revoked_at) < entry.ttl => Ok(true),
            Some(_) => {
                entries.remove(&jti);
                Ok(false)
            }
            None => Ok(false),
        }
    }
}
