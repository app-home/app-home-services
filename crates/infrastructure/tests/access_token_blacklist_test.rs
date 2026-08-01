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
async fn test_blacklist_entry_expires_after_ttl() {
    let blacklist = MemoryAccessTokenBlacklist::new();
    let jti = Uuid::now_v7();

    blacklist.revoke(jti, 0).await.unwrap();
    assert!(!blacklist.is_revoked(jti).await.unwrap());
}
