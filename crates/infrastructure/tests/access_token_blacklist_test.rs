use uuid::Uuid;

use infrastructure::access_token_blacklist::memory::MemoryAccessTokenBlacklist;
use shared::ports::AccessTokenBlacklist;

#[tokio::test]
async fn test_blacklist_not_revoked_by_default() {
    let blacklist = MemoryAccessTokenBlacklist::new();
    let jti = Uuid::now_v7();

    assert!(!blacklist.is_revoked(jti).await.unwrap());
}

#[tokio::test]
async fn test_blacklist_revokes_token() {
    let blacklist = MemoryAccessTokenBlacklist::new();
    let jti = Uuid::now_v7();

    blacklist.revoke(jti, 900).await.unwrap();
    assert!(blacklist.is_revoked(jti).await.unwrap());
}

#[tokio::test]
async fn test_blacklist_revocation_does_not_leak_between_tokens() {
    let blacklist = MemoryAccessTokenBlacklist::new();
    let revoked = Uuid::now_v7();
    let other = Uuid::now_v7();

    blacklist.revoke(revoked, 900).await.unwrap();
    assert!(blacklist.is_revoked(revoked).await.unwrap());
    assert!(!blacklist.is_revoked(other).await.unwrap());
}

#[tokio::test]
async fn test_blacklist_zero_ttl_entry_is_never_revoked() {
    // ttl_secs = 0 means the entry is already expired the instant it's
    // inserted, so is_revoked must report it as not revoked immediately --
    // this is the zero-TTL boundary, not expiry over elapsed time (which
    // Instant-based ttl arithmetic makes impractical to test directly, since it
    // can't be mocked/advanced from an integration test).
    let blacklist = MemoryAccessTokenBlacklist::new();
    let jti = Uuid::now_v7();

    blacklist.revoke(jti, 0).await.unwrap();
    assert!(!blacklist.is_revoked(jti).await.unwrap());
}

#[tokio::test]
async fn test_blacklist_revoke_prunes_expired_entries() {
    // Covers the opportunistic sweep added to revoke() for #135: an expired
    // entry (ttl=0, so it's expired on insertion) must be pruned by the next
    // revoke() call instead of lingering forever.
    let blacklist = MemoryAccessTokenBlacklist::new();

    let expired = Uuid::now_v7();
    blacklist.revoke(expired, 0).await.unwrap();
    assert_eq!(blacklist.entry_count().await, 1);

    let active = Uuid::now_v7();
    blacklist.revoke(active, 900).await.unwrap();
    assert_eq!(
        blacklist.entry_count().await,
        1,
        "revoke() should prune the already-expired entry before inserting the new one, see #135"
    );
    assert!(blacklist.is_revoked(active).await.unwrap());
}
